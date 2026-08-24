use std::{
    env,
    path::{Path, PathBuf},
};

/// Resolves the installed official Claude Code executable without invoking a
/// shell. On Windows, npm exposes a `.cmd` shim that `std::process::Command`
/// cannot execute directly; the resolver uses the native executable shipped by
/// the same official package instead of parsing or running the shim.
pub fn resolve_claude_executable() -> Option<PathBuf> {
    let mut directories = env::var_os("PATH")
        .map(|path| env::split_paths(&path).collect::<Vec<_>>())
        .unwrap_or_default();
    #[cfg(windows)]
    if let Some(app_data) = env::var_os("APPDATA") {
        let npm = PathBuf::from(app_data).join("npm");
        if !directories.iter().any(|directory| directory == &npm) {
            directories.push(npm);
        }
    }
    #[cfg(windows)]
    if let Some(user_profile) = env::var_os("USERPROFILE") {
        let local_bin = PathBuf::from(user_profile).join(".local").join("bin");
        if !directories.iter().any(|directory| directory == &local_bin) {
            directories.push(local_bin);
        }
    }
    resolve_in_directories(directories)
}

fn resolve_in_directories(directories: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    for directory in directories {
        #[cfg(windows)]
        {
            if let Some(executable) = safe_executable(&directory.join("claude.exe")) {
                return Some(executable);
            }
            let npm_shim = directory.join("claude.cmd");
            if safe_file(&npm_shim).is_some() {
                let native = directory
                    .join("node_modules")
                    .join("@anthropic-ai")
                    .join("claude-code")
                    .join("bin")
                    .join("claude.exe");
                if let Some(executable) = safe_executable(&native) {
                    return Some(executable);
                }
            }
        }
        #[cfg(not(windows))]
        if let Some(executable) = safe_executable(&directory.join("claude")) {
            return Some(executable);
        }
    }
    None
}

fn safe_executable(path: &Path) -> Option<PathBuf> {
    let path = safe_file(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = path.metadata().ok()?;
        if metadata.permissions().mode() & 0o111 == 0 {
            return None;
        }
    }
    Some(path)
}

fn safe_file(path: &Path) -> Option<PathBuf> {
    let metadata = path.symlink_metadata().ok()?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || is_reparse_point(&metadata) {
        return None;
    }
    path.canonicalize().ok()
}

#[cfg(windows)]
fn is_reparse_point(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn is_reparse_point(_: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_a_direct_executable_from_the_supplied_path() {
        let temp = tempfile::tempdir().unwrap();
        #[cfg(windows)]
        let executable = temp.path().join("claude.exe");
        #[cfg(not(windows))]
        let executable = temp.path().join("claude");
        std::fs::write(&executable, b"test").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = executable.metadata().unwrap().permissions();
            permissions.set_mode(0o700);
            std::fs::set_permissions(&executable, permissions).unwrap();
        }

        assert_eq!(
            resolve_in_directories([temp.path().to_path_buf()]),
            Some(executable.canonicalize().unwrap())
        );
    }

    #[cfg(windows)]
    #[test]
    fn resolves_the_native_binary_beside_an_npm_shim() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("claude.cmd"), b"@echo off").unwrap();
        let executable = temp
            .path()
            .join("node_modules")
            .join("@anthropic-ai")
            .join("claude-code")
            .join("bin")
            .join("claude.exe");
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        std::fs::write(&executable, b"test").unwrap();

        assert_eq!(
            resolve_in_directories([temp.path().to_path_buf()]),
            Some(executable.canonicalize().unwrap())
        );
    }
}
