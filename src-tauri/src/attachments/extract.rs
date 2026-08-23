use std::{
    fs::File,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use image::{ImageFormat, ImageReader};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{
    dto::{
        ApiError, AttachmentImportResult, AttachmentKind, AttachmentRecord,
        PendingSensitiveAttachment, RejectedAttachment,
    },
    state::AttachmentSnapshot,
};

use super::sensitive_reason;

const MAX_ATTACHMENTS: usize = 10;
const MAX_AGGREGATE_RAW: usize = 40 * 1024 * 1024;
const MAX_TEXT_FILE: usize = 2 * 1024 * 1024;
const MAX_PDF_FILE: usize = 15 * 1024 * 1024;
const MAX_IMAGE_FILE: usize = 15 * 1024 * 1024;
const MAX_EXTRACTED_PER_PDF: usize = 1024 * 1024;
const MAX_AGGREGATE_EXTRACTED: usize = 2 * 1024 * 1024;
const MAX_IMAGE_SIDE: u32 = 12_000;
const MAX_IMAGE_PIXELS: u64 = 25_000_000;

pub struct ImportedSnapshot {
    pub snapshot: AttachmentSnapshot,
    pub sensitive_reason: Option<String>,
}

pub fn import_paths(
    paths: Vec<PathBuf>,
    existing: usize,
    existing_raw: usize,
    existing_extracted: usize,
) -> (AttachmentImportResult, Vec<ImportedSnapshot>) {
    let mut result = AttachmentImportResult {
        imported: Vec::new(),
        pending_confirmation: Vec::new(),
        rejected: Vec::new(),
    };
    let mut snapshots = Vec::new();
    let mut raw_total = existing_raw;
    let mut extracted_total = existing_extracted;

    for path in paths {
        if existing + snapshots.len() >= MAX_ATTACHMENTS {
            result.rejected.push(rejection(
                &path,
                "ATTACHMENT_COUNT_EXCEEDED",
                "最多只能导入 10 个附件。",
            ));
            continue;
        }
        match import_one(&path) {
            Ok(imported) => {
                let record = &imported.snapshot.record;
                if raw_total + record.raw_bytes > MAX_AGGREGATE_RAW {
                    result.rejected.push(rejection(
                        &path,
                        "ATTACHMENT_TOTAL_TOO_LARGE",
                        "附件原始总量将超过 40 MiB。",
                    ));
                    continue;
                }
                if extracted_total + record.extracted_bytes > MAX_AGGREGATE_EXTRACTED {
                    result.rejected.push(rejection(
                        &path,
                        "EXTRACTED_TEXT_TOTAL_TOO_LARGE",
                        "附件提取文本总量将超过 2 MiB。",
                    ));
                    continue;
                }
                raw_total += record.raw_bytes;
                extracted_total += record.extracted_bytes;
                if let Some(reason) = imported.sensitive_reason.clone() {
                    result
                        .pending_confirmation
                        .push(PendingSensitiveAttachment {
                            confirmation_token: record.handle.clone(),
                            name: record.name.clone(),
                            reason,
                            raw_bytes: record.raw_bytes,
                        });
                } else {
                    result.imported.push(record.clone());
                }
                snapshots.push(imported);
            }
            Err(error) => result.rejected.push(RejectedAttachment {
                name: safe_name(&path),
                code: error.code,
                message: error.message,
            }),
        }
    }
    (result, snapshots)
}

fn import_one(path: &Path) -> Result<ImportedSnapshot, ApiError> {
    let canonical = dunce::canonicalize(path)
        .map_err(|_| ApiError::new("ATTACHMENT_UNAVAILABLE", "无法访问该附件。", true))?;
    let file = File::open(&canonical).map_err(|_| ApiError::io("open-attachment"))?;
    let metadata = file
        .metadata()
        .map_err(|_| ApiError::io("inspect-attachment"))?;
    if !metadata.is_file() {
        return Err(ApiError::new(
            "ATTACHMENT_NOT_FILE",
            "只支持普通文件附件。",
            false,
        ));
    }
    if metadata.len() > MAX_PDF_FILE.max(MAX_IMAGE_FILE).max(MAX_TEXT_FILE) as u64 {
        return Err(ApiError::new(
            "FILE_TOO_LARGE",
            "附件超过单文件安全上限。",
            false,
        ));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take((MAX_PDF_FILE + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| ApiError::io("read-attachment"))?;
    let raw_size = bytes.len();
    if raw_size > MAX_PDF_FILE.max(MAX_IMAGE_FILE).max(MAX_TEXT_FILE) {
        return Err(ApiError::new(
            "FILE_TOO_LARGE",
            "附件超过单文件安全上限。",
            false,
        ));
    }

    let inferred = infer::get(&bytes);
    let (kind, mime, content, width, height, warnings, preview_bytes) = if inferred
        .as_ref()
        .is_some_and(|kind| kind.mime_type() == "application/pdf")
        || bytes.starts_with(b"%PDF-")
    {
        if raw_size > MAX_PDF_FILE {
            return Err(ApiError::new("FILE_TOO_LARGE", "PDF 超过 15 MiB。", false));
        }
        let text = pdf_extract::extract_text_from_mem(&bytes).map_err(|_| {
            ApiError::new(
                "PDF_TEXT_UNAVAILABLE",
                "无法从 PDF 提取文本。加密、损坏或扫描型 PDF 需要外部 OCR。",
                false,
            )
        })?;
        if text.trim().is_empty() {
            return Err(ApiError::new(
                "PDF_TEXT_UNAVAILABLE",
                "PDF 中没有可用文本；扫描型 PDF 首版不提供 OCR。",
                false,
            ));
        }
        if text.len() > MAX_EXTRACTED_PER_PDF {
            return Err(ApiError::new(
                "PDF_TEXT_TOO_LARGE",
                "PDF 提取文本超过 1 MiB；未进行静默截断。",
                false,
            ));
        }
        (
            AttachmentKind::Pdf,
            "application/pdf".to_owned(),
            text,
            None,
            None,
            Vec::new(),
            None,
        )
    } else if inferred
        .as_ref()
        .is_some_and(|kind| kind.matcher_type() == infer::MatcherType::Image)
    {
        if raw_size > MAX_IMAGE_FILE {
            return Err(ApiError::new("FILE_TOO_LARGE", "图片超过 15 MiB。", false));
        }
        let reader = ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()
            .map_err(|_| ApiError::new("IMAGE_INVALID", "无法识别图片格式。", false))?;
        let (image_width, image_height) = reader
            .into_dimensions()
            .map_err(|_| ApiError::new("IMAGE_INVALID", "无法读取图片尺寸。", false))?;
        if image_width > MAX_IMAGE_SIDE
            || image_height > MAX_IMAGE_SIDE
            || u64::from(image_width) * u64::from(image_height) > MAX_IMAGE_PIXELS
        {
            return Err(ApiError::new(
                "IMAGE_DIMENSIONS_TOO_LARGE",
                "图片尺寸超过 12,000 px 单边或 25MP 上限。",
                false,
            ));
        }
        let mime = inferred
            .as_ref()
            .map(|kind| kind.mime_type().to_owned())
            .unwrap_or_else(|| "image/unknown".into());
        let note = format!(
            "Image attachment metadata only.\nName: {}\nMIME: {}\nDimensions: {}x{}\nNo OCR, image bytes, or native filesystem path is embedded.",
            safe_name(&canonical),
            mime,
            image_width,
            image_height,
        );
        (
            AttachmentKind::Image,
            mime,
            note,
            Some(image_width),
            Some(image_height),
            vec!["图片预览只在用户点击时传输缩略图；不会暴露本地路径。".into()],
            Some(make_image_preview(&bytes)?),
        )
    } else {
        if raw_size > MAX_TEXT_FILE {
            return Err(ApiError::new(
                "FILE_TOO_LARGE",
                "文本/源码文件超过 2 MiB。",
                false,
            ));
        }
        if is_binary(&bytes) {
            return Err(ApiError::new(
                "BINARY_FILE_UNSUPPORTED",
                "该文件看起来是二进制文件，首版不支持。",
                false,
            ));
        }
        let text_bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
        let text = String::from_utf8(text_bytes.to_vec()).map_err(|_| {
            ApiError::new(
                "TEXT_ENCODING_UNSUPPORTED",
                "文本不是有效 UTF-8；首版不猜测编码。",
                false,
            )
        })?;
        let mime = inferred
            .as_ref()
            .map(|kind| kind.mime_type().to_owned())
            .unwrap_or_else(|| "text/plain".into());
        (
            AttachmentKind::Text,
            mime,
            text,
            None,
            None,
            Vec::new(),
            None,
        )
    };

    let sha256 = hash_bytes(&bytes);
    let handle = Uuid::new_v4().to_string();
    let name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("attachment")
        .to_owned();
    let record = AttachmentRecord {
        handle,
        name,
        kind,
        mime,
        raw_bytes: raw_size,
        extracted_bytes: content.len(),
        sha256,
        width,
        height,
        warnings,
    };
    Ok(ImportedSnapshot {
        sensitive_reason: sensitive_reason(&canonical),
        snapshot: AttachmentSnapshot {
            record,
            content,
            preview_bytes,
        },
    })
}

fn make_image_preview(bytes: &[u8]) -> Result<Vec<u8>, ApiError> {
    let image = image::load_from_memory(bytes)
        .map_err(|_| ApiError::new("IMAGE_INVALID", "无法生成图片预览。", false))?;
    let thumbnail = image.thumbnail(1600, 1200);
    let mut output = Cursor::new(Vec::new());
    thumbnail
        .write_to(&mut output, ImageFormat::Png)
        .map_err(|_| ApiError::new("IMAGE_PREVIEW_FAILED", "无法生成图片预览。", false))?;
    Ok(output.into_inner())
}
fn is_binary(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let sample = &bytes[..bytes.len().min(8192)];
    let nul_count = sample.iter().filter(|byte| **byte == 0).count();
    nul_count > 0 || sample.iter().filter(|byte| **byte < 0x09).count() * 100 > sample.len() * 2
}

fn hash_bytes(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(bytes);
    hex::encode(digest.finalize())
}

fn safe_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("attachment")
        .to_owned()
}

fn rejection(path: &Path, code: &str, message: &str) -> RejectedAttachment {
    RejectedAttachment {
        name: safe_name(path),
        code: code.into(),
        message: message.into(),
    }
}
