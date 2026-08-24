use std::{fs, path::Path, process::Command};

use crate::dto::{ApiError, ApiResult, DemoRunResult};

const MAX_USER_ID_CHARS: usize = 48;
const MAX_DEMO_BYTES: usize = 32 * 1024;

#[tauri::command]
pub fn run_demo_sandbox(user_id: String, _app: tauri::AppHandle) -> ApiResult<DemoRunResult> {
    let safe_user_id = sanitize_demo_user_id(&user_id)?;
    let desktop = dirs::desktop_dir().ok_or_else(|| {
        ApiError::new(
            "DESKTOP_DIRECTORY_UNAVAILABLE",
            "无法确定当前用户的桌面目录。",
            true,
        )
    })?;
    ensure_regular_directory(&desktop)?;

    let file_name = format!("hello_{safe_user_id}.html");
    let file_path = desktop.join(&file_name);
    if file_path.exists() {
        let metadata =
            fs::symlink_metadata(&file_path).map_err(|_| ApiError::io("inspect-demo-file"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ApiError::new(
                "UNSAFE_DEMO_FILE",
                "演示文件路径不是普通文件，已停止写入。",
                false,
            ));
        }
    }

    let content = build_demo_html(&safe_user_id);
    if content.len() > MAX_DEMO_BYTES {
        return Err(ApiError::new(
            "DEMO_CONTENT_TOO_LARGE",
            "演示内容超过安全大小。",
            false,
        ));
    }
    fs::write(&file_path, content.as_bytes()).map_err(|_| ApiError::io("write-demo-file"))?;

    let display_path = format!("桌面/hello_{safe_user_id}.html");
    Ok(DemoRunResult {
        user_id: safe_user_id,
        file_name,
        display_path,
        content,
        created_at_ms: now_ms(),
    })
}

#[tauri::command]
pub fn open_demo_file(file_name: String) -> ApiResult<()> {
    if !is_demo_file_name(&file_name) {
        return Err(ApiError::new(
            "INVALID_DEMO_FILE",
            "只能打开当前演示流程生成的文件。",
            false,
        ));
    }
    let desktop = dirs::desktop_dir().ok_or_else(|| {
        ApiError::new(
            "DESKTOP_DIRECTORY_UNAVAILABLE",
            "无法确定当前用户的桌面目录。",
            true,
        )
    })?;
    ensure_regular_directory(&desktop)?;
    let file_path = desktop.join(&file_name);
    let metadata = fs::symlink_metadata(&file_path).map_err(|_| {
        ApiError::new(
            "DEMO_FILE_NOT_FOUND",
            "没有找到演示文件，请先完成演示。",
            true,
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ApiError::new(
            "UNSAFE_DEMO_FILE",
            "演示文件不是普通文件，已停止打开。",
            false,
        ));
    }

    #[cfg(windows)]
    {
        let target = format!("/select,{}", file_path.display());
        Command::new("explorer.exe")
            .arg(target)
            .spawn()
            .map_err(|_| ApiError::new("DESKTOP_OPEN_FAILED", "无法打开桌面文件夹。", true))?;
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&file_path)
            .spawn()
            .map_err(|_| ApiError::new("DESKTOP_OPEN_FAILED", "无法打开演示文件。", true))?;
        return Ok(());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(&file_path)
            .spawn()
            .map_err(|_| ApiError::new("DESKTOP_OPEN_FAILED", "无法打开演示文件。", true))?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Ok(())
}

fn is_demo_file_name(file_name: &str) -> bool {
    let path = Path::new(file_name);
    path.file_name().and_then(|name| name.to_str()) == Some(file_name)
        && file_name.starts_with("hello_")
        && file_name.ends_with(".html")
}
fn ensure_regular_directory(path: &Path) -> ApiResult<()> {
    if path.exists() {
        let metadata =
            fs::symlink_metadata(path).map_err(|_| ApiError::io("inspect-demo-directory"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ApiError::new(
                "UNSAFE_DEMO_DIRECTORY",
                "桌面目录不是普通目录，已停止写入。",
                false,
            ));
        }
        return Ok(());
    }
    fs::create_dir_all(path).map_err(|_| ApiError::io("create-demo-directory"))?;
    let metadata =
        fs::symlink_metadata(path).map_err(|_| ApiError::io("inspect-demo-directory"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(ApiError::new(
            "UNSAFE_DEMO_DIRECTORY",
            "桌面目录检查失败，已停止写入。",
            false,
        ));
    }
    Ok(())
}

fn sanitize_demo_user_id(value: &str) -> ApiResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(ApiError::new(
            "DEMO_USER_ID_REQUIRED",
            "请输入一个名字或用户 ID。",
            false,
        ));
    }
    if trimmed.chars().count() > MAX_USER_ID_CHARS {
        return Err(ApiError::new(
            "DEMO_USER_ID_TOO_LONG",
            format!("名字或用户 ID 最多 {MAX_USER_ID_CHARS} 个字符。"),
            false,
        ));
    }
    if trimmed.chars().any(char::is_control) {
        return Err(ApiError::new(
            "DEMO_USER_ID_INVALID",
            "名字或用户 ID 不能包含控制字符。",
            false,
        ));
    }
    let safe = sanitize_filename_component(trimmed);
    if safe.is_empty() {
        return Err(ApiError::new(
            "DEMO_USER_ID_INVALID",
            "请输入至少一个有效字符。",
            false,
        ));
    }
    Ok(safe)
}

fn sanitize_filename_component(value: &str) -> String {
    let mut safe = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_alphanumeric() || matches!(character, '-' | '_') {
            safe.push(character);
        } else if character.is_whitespace() {
            safe.push('-');
        } else {
            safe.push('_');
        }
    }
    safe.trim_matches(['-', '_']).to_owned()
}

fn build_demo_html(user_id: &str) -> String {
    let escaped = escape_html(user_id);
    format!(
        r#"<!doctype html>
<html lang="zh-CN">
<head>
  <meta charset="utf-8">
  <title>CC Panel 演示模式</title>
  <style>
    :root {{ color-scheme: light; font-family: "Segoe UI", system-ui, sans-serif; }}
    body {{ margin: 0; padding: 32px; color: #30343b; background: #f5f6f8; }}
    main {{ max-width: 560px; margin: 0 auto; padding: 28px; border: 1px solid #dfe3ea; border-radius: 16px; background: white; box-shadow: 0 12px 32px rgba(25, 34, 52, .08); }}
    .tag {{ display: inline-block; padding: 4px 8px; border-radius: 999px; color: #3155a6; background: #edf2ff; font-size: 12px; }}
    h1 {{ margin: 18px 0 8px; font-size: 28px; }}
    p {{ line-height: 1.6; }}
    small {{ color: #6b7280; }}
  </style>
</head>
<body>
  <main>
    <span class="tag">CC Panel · 演示模式</span>
    <h1>你好，{escaped}！</h1>
    <p>这是一个由 CC Panel 沙盒预设流程创建的安全示例文件。</p>
    <p>本次体验没有调用真实模型，没有读取真实项目，也没有执行外部命令。</p>
    <small>你已经看到了 Agent 工作流中的“接收任务 → 按步骤执行 → 产出文件 → 展示结果”。</small>
  </main>
</body>
</html>
"#,
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace(char::from(39), "&#39;")
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_demo_user_ids_without_path_separators() {
        assert_eq!(sanitize_demo_user_id("小明 / test").unwrap(), "小明-_-test");
    }

    #[test]
    fn rejects_empty_or_long_demo_user_ids() {
        assert!(sanitize_demo_user_id("   ").is_err());
        assert!(sanitize_demo_user_id(&"a".repeat(MAX_USER_ID_CHARS + 1)).is_err());
    }

    #[test]
    fn escapes_user_content_in_demo_html() {
        let html = build_demo_html("safe");
        assert!(html.contains("你好，safe"));
        assert!(!html.contains("<script>"));
    }
}
