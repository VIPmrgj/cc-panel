use std::{fs, io::Write, path::Path};

use crate::dto::{ApiError, ApiResult};

pub fn replace_file_atomically(target: &Path, bytes: &[u8]) -> ApiResult<()> {
    let parent = target.parent().ok_or_else(|| {
        ApiError::new(
            "UNSAFE_SETTINGS_PATH",
            "Claude Code 设置路径没有有效父目录。",
            false,
        )
    })?;
    if parent.exists() {
        let metadata =
            fs::symlink_metadata(parent).map_err(|_| ApiError::io("inspect-settings-directory"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(ApiError::new(
                "UNSAFE_SETTINGS_PATH",
                "设置目录不是安全的普通目录。",
                false,
            ));
        }
    } else {
        fs::create_dir_all(parent).map_err(|_| ApiError::io("create-settings-directory"))?;
    }
    if target.exists() {
        let metadata =
            fs::symlink_metadata(target).map_err(|_| ApiError::io("inspect-settings-target"))?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(ApiError::new(
                "UNSAFE_SETTINGS_PATH",
                "设置文件不是安全的普通文件。",
                false,
            ));
        }
    }

    let mut temporary = tempfile::Builder::new()
        .prefix(".cc-panel-settings-")
        .tempfile_in(parent)
        .map_err(|_| ApiError::io("create-settings-temporary"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| ApiError::io("secure-settings-temporary"))?;
    }

    temporary
        .write_all(bytes)
        .map_err(|_| ApiError::io("write-settings-temporary"))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| ApiError::io("flush-settings-temporary"))?;

    #[cfg(windows)]
    {
        let temporary_path = temporary.into_temp_path();
        replace_windows(target, temporary_path.as_ref())?;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        temporary
            .persist(target)
            .map_err(|_| ApiError::io("replace-settings"))?;
        if let Ok(directory) = std::fs::OpenOptions::new().read(true).open(parent) {
            let _ = directory.sync_all();
        }
        Ok(())
    }
}

#[cfg(windows)]
fn replace_windows(target: &Path, temporary: &Path) -> ApiResult<()> {
    use std::{os::windows::ffi::OsStrExt, ptr};
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, ReplaceFileW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        REPLACEFILE_WRITE_THROUGH,
    };

    let wide = |path: &Path| {
        path.as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    };
    let target_wide = wide(target);
    let temporary_wide = wide(temporary);

    let success = unsafe {
        if target.exists() {
            ReplaceFileW(
                target_wide.as_ptr(),
                temporary_wide.as_ptr(),
                ptr::null(),
                REPLACEFILE_WRITE_THROUGH,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        } else {
            MoveFileExW(
                temporary_wide.as_ptr(),
                target_wide.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        }
    };
    if success == 0 {
        return Err(ApiError::io("replace-settings"));
    }
    Ok(())
}
