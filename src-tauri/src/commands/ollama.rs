use tauri::State;

use crate::{
    dto::{ApiResult, EnhancePromptResponse, OllamaStatus},
    ollama::{validate_loopback_origin, validate_model_name},
    state::AppState,
};

pub async fn build_ollama_status(state: &AppState) -> OllamaStatus {
    let preferences = state.config.preferences();
    match state
        .ollama_client
        .status(
            &preferences.ollama_base_url,
            preferences.ollama_model.as_deref(),
        )
        .await
    {
        Ok(status) => status,
        Err(error) => OllamaStatus {
            online: false,
            base_url: preferences.ollama_base_url,
            selected_model: preferences.ollama_model,
            models: Vec::new(),
            auto_selected: false,
            message: error.message,
        },
    }
}

#[tauri::command]
pub async fn get_ollama_status(state: State<'_, AppState>) -> ApiResult<OllamaStatus> {
    Ok(build_ollama_status(&state).await)
}

#[tauri::command]
pub async fn save_ollama_preferences(
    base_url: String,
    model: Option<String>,
    state: State<'_, AppState>,
) -> ApiResult<OllamaStatus> {
    let normalized = validate_loopback_origin(&base_url)?
        .as_str()
        .trim_end_matches('/')
        .to_owned();
    if let Some(model) = model.as_deref() {
        validate_model_name(model)?;
    }
    state.config.set_ollama(normalized, model)?;
    Ok(build_ollama_status(&state).await)
}

#[tauri::command]
pub async fn enhance_prompt(
    prompt: String,
    model: String,
    state: State<'_, AppState>,
) -> ApiResult<EnhancePromptResponse> {
    let _permit = tokio::time::timeout(
        std::time::Duration::from_secs(1),
        state.ollama_enhancement.acquire(),
    )
    .await
    .map_err(|_| {
        crate::dto::ApiError::new(
            "OLLAMA_BUSY",
            "已有 Ollama 增强任务正在运行，请稍后重试。",
            true,
        )
    })?
    .map_err(|_| {
        crate::dto::ApiError::new("OLLAMA_UNAVAILABLE", "Ollama 增强服务不可用。", true)
    })?;
    let preferences = state.config.preferences();
    state
        .ollama_client
        .enhance(&preferences.ollama_base_url, &model, &prompt)
        .await
}
