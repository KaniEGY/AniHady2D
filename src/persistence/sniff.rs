use crate::project::PROJECT_SCHEMA_VERSION;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedFormat {
    EnvelopeZstd { schema_version: u32 },
    EnvelopeJson { schema_version: u32 },
    LegacyV1Json,
    LegacyV1MsgPack,
    BareProjectJson { inferred_schema: u32 },
    BareProjectMsgPack { inferred_schema: u32 },
    NewerUnsupportedSchema { schema_version: u32 },
}

#[derive(Debug, Clone)]
pub struct SniffResult {
    pub format: DetectedFormat,
    pub payload_bytes: Vec<u8>,
    pub was_zstd: bool,
}

const MAX_PROJECT_RAW_BYTES: usize = 250 * 1024 * 1024; // 250 MB limit
const MAX_DECOMPRESSED_BYTES: usize = 512 * 1024 * 1024; // 512 MB limit

pub fn sniff_project_bytes(raw: &[u8]) -> Result<SniffResult, String> {
    if raw.is_empty() {
        return Err("ملف المشروع فارغ تماماً (0 بايت).".to_string());
    }
    if raw.len() > MAX_PROJECT_RAW_BYTES {
        return Err("حجم ملف المشروع يتجاوز الحد الأقصى المسموح به (250MB).".to_string());
    }

    // Zstd magic number: 0xFD2FB528 (Little Endian: 28 B5 2F FD)
    let is_zstd = raw.len() >= 4 && raw[0] == 0x28 && raw[1] == 0xB5 && raw[2] == 0x2F && raw[3] == 0xFD;
    let payload = if is_zstd {
        let dec = zstd::decode_all(raw)
            .map_err(|e| format!("فشل فك ضغط Zstd؛ قد يكون الملف تالفاً أو غير مكتمل: {}", e))?;
        if dec.len() > MAX_DECOMPRESSED_BYTES {
            return Err("حجم البيانات بعد فك الضغط يتجاوز الحد المسموح.".to_string());
        }
        dec
    } else {
        raw.to_vec()
    };

    // 1. محاولة الفحص كـ JSON صريح أولاً
    if let Ok(val) = serde_json::from_slice::<Value>(&payload) {
        let fmt = classify_json_value(&val)?;
        let final_fmt = match fmt {
            DetectedFormat::EnvelopeJson { schema_version } if is_zstd => {
                DetectedFormat::EnvelopeZstd { schema_version }
            }
            other => other,
        };
        return Ok(SniffResult {
            format: final_fmt,
            payload_bytes: payload,
            was_zstd: is_zstd,
        });
    }

    // 2. محاولة الفحص كـ MessagePack تاريخي
    if let Ok(val) = rmp_serde::from_slice::<Value>(&payload) {
        let fmt = classify_msgpack_value(&val)?;
        return Ok(SniffResult {
            format: fmt,
            payload_bytes: payload,
            was_zstd: is_zstd,
        });
    }

    Err("صيغة الملف غير معروفة أو تالفة؛ هذا الملف ليس مشروع Stick صالحاً.".to_string())
}

fn classify_json_value(val: &Value) -> Result<DetectedFormat, String> {
    let obj = val
        .as_object()
        .ok_or_else(|| "بنية المشروع غير صالحة (ليست كائن JSON).".to_string())?;

    // فحص المغلف الرسمي StickProjectEnvelope
    if let Some(magic_val) = obj.get("magic") {
        let magic = magic_val.as_str().unwrap_or("");
        if magic != "STICK_STUDIO_PROJECT" {
            return Err(format!("توقيع الملف (Magic Header) غير صالح: '{}'.", magic));
        }
        let schema = obj
            .get("schema_version")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| "رقم إصدار الصيغة (schema_version) مفقود داخل مغلف المشروع.".to_string())?
            as u32;

        if !obj.contains_key("project") {
            return Err("مغلف المشروع تالف: حقل 'project' مفقود.".to_string());
        }
        if schema > PROJECT_SCHEMA_VERSION {
            return Ok(DetectedFormat::NewerUnsupportedSchema { schema_version: schema });
        }
        return Ok(DetectedFormat::EnvelopeJson { schema_version: schema });
    }

    // فحص البصمة التاريخية Legacy V1 (يحتوي layers[].frames بدلاً من layers[].keyframes)
    if let Some(layers) = obj.get("layers").and_then(|v| v.as_array()) {
        let has_legacy_frames = layers.iter().any(|l| l.get("frames").is_some() && l.get("keyframes").is_none());
        if has_legacy_frames {
            return Ok(DetectedFormat::LegacyV1Json);
        }

        let has_modern_keyframes = layers.iter().any(|l| l.get("keyframes").is_some());
        if has_modern_keyframes || obj.contains_key("fps") || obj.contains_key("total_frames") {
            let schema = obj
                .get("schema_version")
                .and_then(|v| v.as_u64())
                .map(|v| v as u32)
                .unwrap_or(2);
            if schema > PROJECT_SCHEMA_VERSION {
                return Ok(DetectedFormat::NewerUnsupportedSchema { schema_version: schema });
            }
            return Ok(DetectedFormat::BareProjectJson { inferred_schema: schema });
        }
    }

    Err("هذا الملف لا يحتوي على بنية مشروع Stick معروفة (تم رفضه لحماية مشروعك الحالي).".to_string())
}

fn classify_msgpack_value(val: &Value) -> Result<DetectedFormat, String> {
    let obj = val
        .as_object()
        .ok_or_else(|| "بنية MessagePack غير صالحة.".to_string())?;

    if let Some(layers) = obj.get("layers").and_then(|v| v.as_array()) {
        let has_legacy_frames = layers.iter().any(|l| l.get("frames").is_some() && l.get("keyframes").is_none());
        if has_legacy_frames {
            return Ok(DetectedFormat::LegacyV1MsgPack);
        }
        let schema = obj
            .get("schema_version")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .unwrap_or(2);
        if schema > PROJECT_SCHEMA_VERSION {
            return Ok(DetectedFormat::NewerUnsupportedSchema { schema_version: schema });
        }
        return Ok(DetectedFormat::BareProjectMsgPack { inferred_schema: schema });
    }

    Err("ملف MessagePack لا يطابق أي إصدار معروف من مشاريع Stick.".to_string())
}
