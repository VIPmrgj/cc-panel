#[cfg(windows)]
use std::{ffi::c_void, mem::size_of};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::ERROR_CANCELLED,
    Security::Credentials::{
        CredUIPromptForCredentialsW, CREDUI_FLAGS_ALWAYS_SHOW_UI, CREDUI_FLAGS_DO_NOT_PERSIST,
        CREDUI_FLAGS_GENERIC_CREDENTIALS, CREDUI_FLAGS_KEEP_USERNAME, CREDUI_INFOW,
    },
};

use crate::dto::{ApiError, ApiResult};

const NATIVE_SECRET_LIMIT: usize = 16 * 1024;

pub(crate) fn prompt_api_key(
    provider_name: &str,
    parent_window: Option<usize>,
) -> ApiResult<Option<String>> {
    if provider_name.is_empty()
        || provider_name.len() > 120
        || provider_name.trim() != provider_name
        || provider_name.chars().any(char::is_control)
    {
        return Err(
            ApiError::new("INVALID_MODEL_PROFILE", "模型配置名称无效。", false)
                .field("providerName"),
        );
    }

    prompt_api_key_native(provider_name, parent_window)
}

#[cfg(windows)]
fn prompt_api_key_native(
    provider_name: &str,
    parent_window: Option<usize>,
) -> ApiResult<Option<String>> {
    const USERNAME_CAPACITY: usize = 128;
    const PASSWORD_CAPACITY: usize = NATIVE_SECRET_LIMIT + 1;

    let caption = wide("CC Panel 模型凭据");
    let message = wide(&format!(
        "请输入 {provider_name} 的 API 密钥。密钥将直接交给 Rust 安全存储，不经过网页界面。"
    ));
    let target = wide(&format!("CC Panel:{provider_name}"));
    let mut username = wide("api-key");
    username.resize(USERNAME_CAPACITY, 0);
    let mut password = vec![0_u16; PASSWORD_CAPACITY];
    let info = CREDUI_INFOW {
        cbSize: size_of::<CREDUI_INFOW>() as u32,
        hwndParent: parent_window
            .map(|handle| handle as *mut c_void)
            .unwrap_or(std::ptr::null_mut::<c_void>()),
        pszMessageText: message.as_ptr(),
        pszCaptionText: caption.as_ptr(),
        hbmBanner: std::ptr::null_mut::<c_void>(),
    };
    let flags = CREDUI_FLAGS_ALWAYS_SHOW_UI
        | CREDUI_FLAGS_DO_NOT_PERSIST
        | CREDUI_FLAGS_GENERIC_CREDENTIALS
        | CREDUI_FLAGS_KEEP_USERNAME;

    // SAFETY: all pointers reference initialized buffers that remain alive for
    // the duration of the synchronous native dialog call; sizes match buffers.
    let result = unsafe {
        CredUIPromptForCredentialsW(
            &info,
            target.as_ptr(),
            std::ptr::null(),
            0,
            username.as_mut_ptr(),
            username.len() as u32,
            password.as_mut_ptr(),
            password.len() as u32,
            std::ptr::null_mut(),
            flags,
        )
    };
    if result == ERROR_CANCELLED {
        password.fill(0);
        return Ok(None);
    }
    if result != 0 {
        password.fill(0);
        return Err(ApiError::new(
            "NATIVE_CREDENTIAL_PROMPT_FAILED",
            "无法打开系统凭据输入窗口。",
            true,
        ));
    }
    let length = password
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(password.len());
    let secret = String::from_utf16(&password[..length]).map_err(|_| {
        ApiError::new(
            "NATIVE_CREDENTIAL_INVALID",
            "系统凭据输入包含无效字符。",
            false,
        )
    });
    password.fill(0);
    let secret = secret?;
    if secret.trim().is_empty() || secret.len() > NATIVE_SECRET_LIMIT {
        return Err(ApiError::new(
            "API_KEY_INVALID",
            "API 密钥为空或过长。",
            false,
        ));
    }
    Ok(Some(secret))
}

#[cfg(not(windows))]
fn prompt_api_key_native(_: &str, _: Option<usize>) -> ApiResult<Option<String>> {
    Err(ApiError::new(
        "NATIVE_CREDENTIAL_PROMPT_UNAVAILABLE",
        "当前平台暂不支持系统凭据输入窗口。",
        false,
    ))
}

#[cfg(windows)]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}
