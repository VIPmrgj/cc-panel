use serde::Serialize;
use serde_json::{Map, Value};

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Box<Map<String, Value>>>,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable,
            field: None,
            details: None,
        }
    }

    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    pub fn settings_invalid() -> Self {
        Self::new(
            "INVALID_SETTINGS_JSON",
            "Claude Code settings.json 不是有效的 JSON 对象；未做任何修改。",
            false,
        )
    }

    pub fn settings_conflict() -> Self {
        Self::new(
            "SETTINGS_CONFLICT",
            "Claude Code 设置已被其他进程修改。请刷新后重试。",
            true,
        )
    }

    pub fn io(operation: &'static str) -> Self {
        let mut details = Map::new();
        details.insert("operation".into(), Value::String(operation.into()));
        Self {
            code: "LOCAL_IO_ERROR".into(),
            message: "本地文件操作失败。请检查路径与权限后重试。".into(),
            retryable: true,
            field: None,
            details: Some(Box::new(details)),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<std::io::Error> for ApiError {
    fn from(_: std::io::Error) -> Self {
        Self::io("filesystem")
    }
}
