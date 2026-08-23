use crate::dto::{ApiError, ApiResult};

#[cfg(windows)]
mod platform {
    use std::{ffi::c_void, ptr, slice};

    use super::{ApiError, ApiResult};

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;
    const ENTROPY: &[u8] = b"cc-panel:model-profile-api-key:v1";

    #[repr(C)]
    struct DataBlob {
        size: u32,
        data: *mut u8,
    }

    #[link(name = "Crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            data_in: *mut DataBlob,
            description: *const u16,
            optional_entropy: *mut DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;

        fn CryptUnprotectData(
            data_in: *mut DataBlob,
            description: *mut *mut u16,
            optional_entropy: *mut DataBlob,
            reserved: *mut c_void,
            prompt: *mut c_void,
            flags: u32,
            data_out: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
    }

    pub(super) fn protect(plaintext: &mut [u8]) -> ApiResult<Vec<u8>> {
        let mut input = blob_for(plaintext)?;
        let mut entropy_bytes = ENTROPY.to_vec();
        let mut entropy = blob_for(&mut entropy_bytes)?;
        let mut output = DataBlob {
            size: 0,
            data: ptr::null_mut(),
        };

        let success = unsafe {
            CryptProtectData(
                &mut input,
                ptr::null(),
                &mut entropy,
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        entropy_bytes.fill(0);
        if success == 0 {
            return Err(protection_failed());
        }
        take_output(output)
    }

    pub(super) fn unprotect(ciphertext: &mut [u8]) -> ApiResult<Vec<u8>> {
        let mut input = blob_for(ciphertext)?;
        let mut entropy_bytes = ENTROPY.to_vec();
        let mut entropy = blob_for(&mut entropy_bytes)?;
        let mut output = DataBlob {
            size: 0,
            data: ptr::null_mut(),
        };
        let mut description = ptr::null_mut();

        let success = unsafe {
            CryptUnprotectData(
                &mut input,
                &mut description,
                &mut entropy,
                ptr::null_mut(),
                ptr::null_mut(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        entropy_bytes.fill(0);
        if !description.is_null() {
            unsafe {
                LocalFree(description.cast());
            }
        }
        if success == 0 {
            return Err(unprotection_failed());
        }
        take_output(output)
    }

    fn blob_for(bytes: &mut [u8]) -> ApiResult<DataBlob> {
        let size = u32::try_from(bytes.len()).map_err(|_| protection_failed())?;
        Ok(DataBlob {
            size,
            data: bytes.as_mut_ptr(),
        })
    }

    fn take_output(output: DataBlob) -> ApiResult<Vec<u8>> {
        if output.data.is_null() || output.size == 0 {
            if !output.data.is_null() {
                unsafe {
                    LocalFree(output.data.cast());
                }
            }
            return Err(protection_failed());
        }

        let bytes = unsafe {
            let protected = slice::from_raw_parts_mut(output.data, output.size as usize);
            let copy = protected.to_vec();
            protected.fill(0);
            LocalFree(output.data.cast());
            copy
        };
        Ok(bytes)
    }

    fn protection_failed() -> ApiError {
        ApiError::new(
            "MODEL_SECRET_PROTECTION_FAILED",
            "无法使用 Windows 当前用户凭据保护模型 API Key。",
            false,
        )
    }

    fn unprotection_failed() -> ApiError {
        ApiError::new(
            "MODEL_SECRET_UNAVAILABLE",
            "无法解密模型 API Key；它可能属于其他 Windows 用户或已损坏。",
            false,
        )
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{ApiError, ApiResult};

    pub(super) fn protect(_plaintext: &mut [u8]) -> ApiResult<Vec<u8>> {
        Err(unsupported())
    }

    pub(super) fn unprotect(_ciphertext: &mut [u8]) -> ApiResult<Vec<u8>> {
        Err(unsupported())
    }

    fn unsupported() -> ApiError {
        ApiError::new(
            "MODEL_SECRET_PROTECTION_UNAVAILABLE",
            "当前平台没有可用的系统级 API Key 保护方案；已拒绝存取明文密钥。",
            false,
        )
    }
}

pub(super) fn protect(plaintext: &mut [u8]) -> ApiResult<Vec<u8>> {
    platform::protect(plaintext)
}

pub(super) fn unprotect(ciphertext: &mut [u8]) -> ApiResult<Vec<u8>> {
    platform::unprotect(ciphertext)
}
