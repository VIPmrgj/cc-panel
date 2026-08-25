use std::time::Duration;

use reqwest::{redirect::Policy, Client, StatusCode};
use serde_json::json;
use url::Url;

use crate::dto::{ApiError, ApiResult, ModelConnectionTestResult};

use super::ResolvedModelSecret;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

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
    let result = classify_status(
        response.status(),
        &secret.profile.provider_name,
        &secret.profile.model_id,
    );
    if result.ok {
        Ok(result)
    } else {
        Err(ApiError::new(&result.code, &result.message, true))
    }
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
            "API Key 无效或没有使用该模型的权限。".to_owned(),
        ),
        StatusCode::PAYMENT_REQUIRED | StatusCode::TOO_MANY_REQUESTS => (
            false,
            "MODEL_TEST_PROVIDER_LIMIT",
            "服务商拒绝了请求，可能是余额不足、限流或账户额度已用完。".to_owned(),
        ),
        StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND => (
            false,
            "MODEL_TEST_CONFIGURATION",
            "模型或 API 地址不正确，请检查配置。".to_owned(),
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
        let result = classify_status(StatusCode::UNAUTHORIZED, "DeepSeek", "deepseek-v4-pro");
        assert!(!result.ok);
        assert_eq!(result.code, "MODEL_TEST_AUTH_FAILED");
        assert!(!result.message.contains("sk-"));
    }
}
