use std::time::Duration;

use futures_util::StreamExt;
use reqwest::{redirect::Policy, Client, StatusCode};
use serde_json::json;
use url::Url;

use crate::dto::{ApiError, ApiResult, ModelConnectionTestResult};

use super::ResolvedModelSecret;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const PROVIDER_ERROR_BODY_LIMIT: usize = 8 * 1024;

pub async fn test_connection(secret: ResolvedModelSecret) -> ApiResult<ModelConnectionTestResult> {
    let endpoint = messages_endpoint(&secret.profile.base_url)?;
    let client = Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(REQUEST_TIMEOUT)
        .redirect(Policy::none())
        .user_agent(format!("cc-panel/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|_| ApiError::new("MODEL_TEST_CLIENT_FAILED", "无法初始化模型连接测试。", true))?;
    let request = json!({
        "model": secret.profile.model_id,
        "max_tokens": 16,
        "messages": [{
            "role": "user",
            "content": "Reply with OK"
        }]
    });
    let response = client
        .post(endpoint)
        .header("x-api-key", secret.api_key())
        .header("anthropic-version", "2023-06-01")
        .json(&request)
        .send()
        .await
        .map_err(|error| {
            if error.is_timeout() {
                ApiError::new(
                    "MODEL_TEST_TIMEOUT",
                    "连接模型超时，请检查网络后重试。",
                    true,
                )
            } else {
                ApiError::new(
                    "MODEL_TEST_NETWORK",
                    "无法连接模型服务，请检查网络和 API 地址。",
                    true,
                )
            }
        })?;
    let status = response.status();
    if status.is_success() {
        return Ok(classify_status(
            status,
            &secret.profile.provider_name,
            &secret.profile.model_id,
            "",
        ));
    }
    let body = response_body_excerpt(response).await;
    let result = classify_status(
        status,
        &secret.profile.provider_name,
        &secret.profile.model_id,
        &body,
    );
    Err(ApiError::new(&result.code, &result.message, true))
}
async fn response_body_excerpt(response: reqwest::Response) -> String {
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let Ok(chunk) = chunk else { break };
        let remaining = PROVIDER_ERROR_BODY_LIMIT.saturating_sub(bytes.len());
        if remaining == 0 {
            break;
        }
        bytes.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

fn messages_endpoint(base_url: &str) -> ApiResult<Url> {
    let mut url = Url::parse(base_url).map_err(|_| invalid_endpoint())?;
    let host = url.host_str().ok_or_else(invalid_endpoint)?;
    let loopback = matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]");
    let secure = url.scheme() == "https" || (url.scheme() == "http" && loopback);
    if !secure
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(invalid_endpoint());
    }
    let path = url.path().trim_end_matches('/');
    let path = if path.is_empty() {
        "/v1/messages".to_owned()
    } else {
        format!("{path}/v1/messages")
    };
    url.set_path(&path);
    Ok(url)
}

fn classify_status(
    status: StatusCode,
    provider_name: &str,
    model_id: &str,
    body: &str,
) -> ModelConnectionTestResult {
    let (ok, code, message) = match status {
        status if status.is_success() => (
            true,
            "MODEL_TEST_OK",
            format!("{provider_name} 已连接，模型 {model_id} 可以使用。"),
        ),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => (
            false,
            "MODEL_TEST_AUTH_FAILED",
            if status == StatusCode::FORBIDDEN {
                "API Key 已被识别，但当前账号没有使用该模型或接口的权限。".to_owned()
            } else {
                "API Key 无效、已过期或未被服务商接受，请重新检查后保存。".to_owned()
            },
        ),
        StatusCode::PAYMENT_REQUIRED => (
            false,
            "MODEL_TEST_BALANCE",
            "账户余额不足或尚未开通计费，请到服务商控制台查看余额后重试。".to_owned(),
        ),
        StatusCode::TOO_MANY_REQUESTS => (
            false,
            "MODEL_TEST_RATE_LIMIT",
            "请求过于频繁或已达到账户额度限制，请稍后重试。".to_owned(),
        ),
        StatusCode::BAD_REQUEST
            if body_mentions(body, &["balance", "insufficient", "quota", "余额", "欠费"]) =>
        {
            (
                false,
                "MODEL_TEST_BALANCE",
                "服务商提示账户余额或额度不足，请到服务商控制台查看后重试。".to_owned(),
            )
        }
        StatusCode::BAD_REQUEST if body_mentions(body, &["model", "模型"]) => (
            false,
            "MODEL_TEST_MODEL",
            "模型 ID 不正确，或当前账号没有使用该模型的权限。".to_owned(),
        ),
        StatusCode::BAD_REQUEST => (
            false,
            "MODEL_TEST_REQUEST",
            "服务商拒绝了测试请求，请检查 API 地址、模型 ID 和接口格式。".to_owned(),
        ),
        StatusCode::NOT_FOUND if body_mentions(body, &["model", "模型"]) => (
            false,
            "MODEL_TEST_MODEL",
            "找不到指定模型，请检查模型 ID 或账号权限。".to_owned(),
        ),
        StatusCode::NOT_FOUND => (
            false,
            "MODEL_TEST_ENDPOINT",
            "找不到模型 API 地址，请检查是否填写了 Anthropic 兼容接口地址。".to_owned(),
        ),
        status if status.is_server_error() => (
            false,
            "MODEL_TEST_PROVIDER_FAILED",
            "模型服务暂时不可用，请稍后重试。".to_owned(),
        ),
        _ => (
            false,
            "MODEL_TEST_FAILED",
            "模型服务返回了无法识别的错误，请检查配置后重试。".to_owned(),
        ),
    };
    ModelConnectionTestResult {
        ok,
        code: code.to_owned(),
        message,
        provider_name: provider_name.to_owned(),
        model_id: model_id.to_owned(),
    }
}
fn body_mentions(body: &str, terms: &[&str]) -> bool {
    let lower = body.to_ascii_lowercase();
    terms
        .iter()
        .any(|term| lower.contains(&term.to_ascii_lowercase()))
}

fn invalid_endpoint() -> ApiError {
    ApiError::new(
        "MODEL_TEST_INVALID_ENDPOINT",
        "模型 API 地址无效，只允许安全的 HTTPS 地址。",
        false,
    )
    .field("baseUrl")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_anthropic_messages_path_to_a_provider_base_url() {
        let endpoint = messages_endpoint("https://api.deepseek.com/anthropic").unwrap();
        assert_eq!(
            endpoint.as_str(),
            "https://api.deepseek.com/anthropic/v1/messages"
        );
    }

    #[test]
    fn rejects_insecure_or_credential_bearing_endpoints() {
        assert!(messages_endpoint("http://example.com/anthropic").is_err());
        assert!(messages_endpoint("https://user:pass@example.com/anthropic").is_err());
        assert!(messages_endpoint("https://example.com/anthropic?key=secret").is_err());
    }

    #[test]
    fn classifies_provider_status_without_returning_response_body() {
        let result = classify_status(StatusCode::UNAUTHORIZED, "DeepSeek", "deepseek-v4-pro", "");
        assert!(!result.ok);
        assert_eq!(result.code, "MODEL_TEST_AUTH_FAILED");
        assert!(!result.message.contains("sk-"));
    }

    #[test]
    fn classifies_balance_rate_limit_and_model_errors() {
        assert_eq!(
            classify_status(StatusCode::PAYMENT_REQUIRED, "DeepSeek", "model", "").code,
            "MODEL_TEST_BALANCE"
        );
        assert_eq!(
            classify_status(StatusCode::TOO_MANY_REQUESTS, "DeepSeek", "model", "").code,
            "MODEL_TEST_RATE_LIMIT"
        );
        assert_eq!(
            classify_status(
                StatusCode::BAD_REQUEST,
                "DeepSeek",
                "model",
                "model not found"
            )
            .code,
            "MODEL_TEST_MODEL"
        );
    }
}
