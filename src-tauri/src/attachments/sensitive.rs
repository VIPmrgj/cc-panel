use std::path::Path;

pub fn sensitive_reason(path: &Path) -> Option<String> {
    let lower_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let lower_path = path.to_string_lossy().to_ascii_lowercase();
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if lower_name == ".env" || lower_name.starts_with(".env.") {
        return Some("文件名表明它可能包含环境变量或密钥。".into());
    }
    if lower_path.contains("/.ssh/")
        || lower_path.contains("\\.ssh\\")
        || matches!(lower_name.as_str(), "id_rsa" | "id_ed25519" | "id_ecdsa")
    {
        return Some("路径或文件名表明它可能是 SSH 凭据。".into());
    }
    if matches!(extension.as_str(), "pem" | "key" | "p12" | "pfx") {
        return Some("该扩展名常用于私钥或证书凭据。".into());
    }
    if lower_name.contains("credential")
        || lower_name.contains("secret")
        || lower_name.contains("token")
        || lower_name.contains("keystore")
    {
        return Some("文件名表明它可能包含凭据或令牌。".into());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_common_secret_paths() {
        assert!(sensitive_reason(Path::new("C:/repo/.env.local")).is_some());
        assert!(sensitive_reason(Path::new("C:/Users/u/.ssh/id_ed25519")).is_some());
        assert!(sensitive_reason(Path::new("notes.txt")).is_none());
    }
}
