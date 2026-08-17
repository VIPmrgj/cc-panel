use std::time::Duration;

use futures_util::StreamExt;
use reqwest::{redirect::Policy, Client};
use serde::{Deserialize, Serialize};
use url::{Host, Url};

use crate::dto::{ApiError, ApiResult, EnhancePromptResponse, OllamaModel, OllamaStatus};

const TAGS_RESPONSE_LIMIT: usize = 2 * 1024 * 1024;
const GENERATE_RESPONSE_LIMIT: usize = 256 * 1024;
const PROMPT_LIMIT: usize = 128 * 1024;

#[derive(Clone)]
pub struct OllamaClient {
    client: Client,
}

impl OllamaClient {
    pub fn new() -> ApiResult<Self> {
        let client = Client::builder()
            .connect_timeout(Duration::from_secs(3))
            .redirect(Policy::none())
            .no_proxy()
            .build()
            .map_err(|_| {
                ApiError::new("HTTP_CLIENT_FAILED", "无法初始化本地 HTTP 客户端。", false)
            })?;
        Ok(Self { client })
    }

    pub async fn status(
        &self,
        base_url: &str,
        preferred_model: Option<&str>,
    ) -> ApiResult<OllamaStatus> {
        let base = validate_loopback_origin(base_url)?;
        let url = base.join("api/tags").map_err(|_| invalid_base_url())?;
        let bytes = tokio::time::timeout(Duration::from_secs(5), async {
            let response =
                self.client.get(url).send().await.map_err(|_| {
                    ApiError::new("OLLAMA_UNAVAILABLE", "无法连接本地 Ollama。", true)
                })?;
            if !response.status().is_success() {
                return Err(ApiError::new(
                    "OLLAMA_UNAVAILABLE",
                    "Ollama 返回了非成功状态。",
                    true,
                ));
            }
            let bytes = read_limited(response, TAGS_RESPONSE_LIMIT).await?;
            Ok::<_, ApiError>(bytes)
        })
        .await
        .map_err(|_| ApiError::new("OLLAMA_TIMEOUT", "Ollama 状态检查超时。", true))??;
        let mut payload: TagsResponse = serde_json::from_slice(&bytes).map_err(|_| {
            ApiError::new("OLLAMA_RESPONSE_INVALID", "Ollama 模型清单格式无效。", true)
        })?;
        payload.models.sort_by(|a, b| a.name.cmp(&b.name));
        let models = payload
            .models
            .into_iter()
            .map(|model| OllamaModel {
                name: model.name,
                size: model.size,
                modified_at: model.modified_at,
            })
            .collect::<Vec<_>>();
        let selected_model = preferred_model
            .filter(|preferred| models.iter().any(|model| model.name == *preferred))
            .map(ToOwned::to_owned)
            .or_else(|| models.first().map(|model| model.name.clone()));
        let auto_selected = preferred_model.is_none() && selected_model.is_some()
            || preferred_model.is_some() && selected_model.as_deref() != preferred_model;
        let message = if models.is_empty() {
            "Ollama 在线，但未安装可用模型。".into()
        } else if auto_selected {
            "Ollama 在线，已自动选择本地模型。".into()
        } else {
            "Ollama 在线。".into()
        };
        Ok(OllamaStatus {
            online: true,
            base_url: normalized_origin(&base),
            selected_model,
            models,
            auto_selected,
            message,
        })
    }

    pub async fn enhance(
        &self,
        base_url: &str,
        model: &str,
        prompt: &str,
    ) -> ApiResult<EnhancePromptResponse> {
        if prompt.len() > PROMPT_LIMIT {
            return Err(ApiError::new(
                "PROMPT_TOO_LARGE",
                "原始 Prompt 超过 128 KiB，无法增强。",
                false,
            ));
        }
        validate_model_name(model)?;
        let base = validate_loopback_origin(base_url)?;
        let url = base.join("api/generate").map_err(|_| invalid_base_url())?;
        let request = GenerateRequest {
            model,
            prompt,
            stream: false,
            system: "Rewrite the user's prompt to be clearer, more specific, and more actionable. Preserve intent, language, factual constraints, and all code or paths. Return only the improved prompt. Do not answer the prompt.",
        };
        let bytes = tokio::time::timeout(Duration::from_secs(120), async {
            let response = self
                .client
                .post(url)
                .json(&request)
                .send()
                .await
                .map_err(|_| {
                    ApiError::new(
                        "OLLAMA_UNAVAILABLE",
                        "无法连接 Ollama；原 Prompt 已保留。",
                        true,
                    )
                })?;
            if !response.status().is_success() {
                return Err(ApiError::new(
                    "OLLAMA_GENERATE_FAILED",
                    "Ollama 未能完成增强；原 Prompt 已保留。",
                    true,
                ));
            }
            read_limited(response, GENERATE_RESPONSE_LIMIT).await
        })
        .await
        .map_err(|_| {
            ApiError::new(
                "OLLAMA_TIMEOUT",
                "Ollama 增强请求超时；原 Prompt 已保留。",
                true,
            )
        })??;
        let payload: GenerateResponse = serde_json::from_slice(&bytes).map_err(|_| {
            ApiError::new(
                "OLLAMA_RESPONSE_INVALID",
                "Ollama 返回了无法识别的内容；原 Prompt 已保留。",
                true,
            )
        })?;
        if payload.response.trim().is_empty() {
            return Err(ApiError::new(
                "OLLAMA_EMPTY_RESPONSE",
                "Ollama 返回空结果；原 Prompt 已保留。",
                true,
            ));
        }
        Ok(EnhancePromptResponse {
            text: payload.response,
            model: model.into(),
        })
    }
}

pub fn validate_model_name(model: &str) -> ApiResult<()> {
    if model.is_empty()
        || model.len() > 512
        || model.trim() != model
        || model.chars().any(char::is_control)
    {
        return Err(ApiError::new(
            "INVALID_OLLAMA_MODEL",
            "Ollama 模型名无效。",
            false,
        ));
    }
    Ok(())
}

pub fn validate_loopback_origin(value: &str) -> ApiResult<Url> {
    let mut url = Url::parse(value).map_err(|_| invalid_base_url())?;
    let is_loopback = match url.host() {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    };
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !is_loopback
        || !(url.path().is_empty() || url.path() == "/")
    {
        return Err(invalid_base_url());
    }
    url.set_path("/");
    Ok(url)
}

fn normalized_origin(url: &Url) -> String {
    let mut value = url.as_str().trim_end_matches('/').to_owned();
    if value.is_empty() {
        value = "http://localhost:11434".into();
    }
    value
}

fn invalid_base_url() -> ApiError {
    ApiError::new(
        "OLLAMA_URL_NOT_LOOPBACK",
        "Ollama 地址必须是无凭据、无路径的本机 HTTP 回环地址。",
        false,
    )
    .field("baseUrl")
}

async fn read_limited(response: reqwest::Response, limit: usize) -> ApiResult<Vec<u8>> {
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(ApiError::new(
            "OLLAMA_RESPONSE_TOO_LARGE",
            "Ollama 响应超过安全上限。",
            false,
        ));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| {
            ApiError::new("OLLAMA_RESPONSE_INVALID", "读取 Ollama 响应失败。", true)
        })?;
        if bytes.len() + chunk.len() > limit {
            return Err(ApiError::new(
                "OLLAMA_RESPONSE_TOO_LARGE",
                "Ollama 响应超过安全上限。",
                false,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TagModel>,
}

#[derive(Debug, Deserialize)]
struct TagModel {
    name: String,
    size: Option<u64>,
    #[serde(rename = "modified_at")]
    modified_at: Option<String>,
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    system: &'a str,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_loopback_only() {
        assert!(validate_loopback_origin("http://localhost:11434").is_ok());
        assert!(validate_loopback_origin("http://127.0.0.1:11434/").is_ok());
        assert!(validate_loopback_origin("http://[::1]:11434").is_ok());
        assert!(validate_loopback_origin("https://localhost:11434").is_err());
        assert!(validate_loopback_origin("http://example.com:11434").is_err());
        assert!(validate_loopback_origin("http://localhost:11434/api").is_err());
        assert!(validate_loopback_origin("http://user@localhost:11434").is_err());
    }
}
