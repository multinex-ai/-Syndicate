//! Strict compiled schema validation and redaction-safe structured telemetry for all GPU routes.
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use syndicate_gpu_lowmem::LowMemExecutionPlan;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerRoute {
    Chroma,
    Kromoss,
    Speech,
    Music,
    Effects,
    VideoGenerate,
    VideoEdit,
}

impl WorkerRoute {
    pub fn from_env(raw: &str) -> Result<Self, String> {
        match raw.trim() {
            "chroma" | "image-quality" => Ok(Self::Chroma),
            "kromoss" | "image-local" => Ok(Self::Kromoss),
            "speech" | "speech-quality" => Ok(Self::Speech),
            "music" | "music-generation" => Ok(Self::Music),
            "effects" | "sound-effects" => Ok(Self::Effects),
            "video-generate" | "video-generation" | "generate" => Ok(Self::VideoGenerate),
            "video-edit" | "edit" => Ok(Self::VideoEdit),
            other => Err(format!("Unsupported WORKER_ROUTE: {other}")),
        }
    }

    pub fn pinned_model(self) -> &'static str {
        match self {
            Self::Chroma => "syndicate/chroma-hd",
            Self::Kromoss => "syndicate/diffusion-pro",
            Self::Speech => "syndicate/tts-mini",
            Self::Music => "syndicate/music-gen",
            Self::Effects => "syndicate/sfx-gen",
            Self::VideoGenerate => "syndicate/video-i2v",
            Self::VideoEdit => "syndicate/video-edit",
        }
    }

    pub fn estimated_weight_bytes(self) -> u64 {
        const GIB: u64 = 1 << 30;
        match self {
            Self::Chroma => 26 * GIB,
            Self::Kromoss => 24 * GIB,
            Self::Speech => 4 * GIB,
            Self::Music => 7 * GIB,
            Self::Effects => 6 * GIB,
            Self::VideoGenerate => 28 * GIB,
            Self::VideoEdit => 12 * GIB,
        }
    }
}

/// Emit one redaction-safe JSON log record to stdout. Prompt and lyrics text can never be passed here.
pub fn log_event(event: &str, level: &str, worker_revision: &str, route: WorkerRoute, extra: Value) {
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut obj = Map::new();
    obj.insert("timestamp_ms".into(), json!(ts_ms));
    obj.insert("level".into(), json!(level));
    obj.insert("event".into(), json!(event));
    obj.insert("worker_revision".into(), json!(worker_revision));
    obj.insert("route".into(), json!(route));
    obj.insert("model".into(), json!(route.pinned_model()));
    if let Value::Object(fields) = extra {
        for (k, v) in fields {
            if !matches!(k.as_str(), "prompt" | "negative_prompt" | "text" | "lyrics") {
                obj.insert(k, v);
            }
        }
    }
    println!("{}", Value::Object(obj));
}

pub fn resolve_cache_dir() -> (String, bool) {
    let configured = std::env::var("HF_HUB_CACHE")
        .or_else(|_| std::env::var("HF_HOME"))
        .unwrap_or_else(|_| "/runpod-volume/huggingface-cache/hub".into());
    let vol = Path::new("/runpod-volume");
    let persistent = vol.is_dir() && is_mount_point(vol);
    let candidate = if persistent || !configured.starts_with("/runpod-volume") {
        Path::new(&configured).to_path_buf()
    } else {
        Path::new("/tmp/huggingface-cache/hub").to_path_buf()
    };
    if std::fs::create_dir_all(&candidate).is_ok() {
        return (candidate.to_string_lossy().into_owned(), persistent);
    }
    let fallback = Path::new("/tmp/huggingface-cache/hub");
    let _ = std::fs::create_dir_all(fallback);
    (fallback.to_string_lossy().into_owned(), false)
}

fn is_mount_point(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    let Ok(m1) = std::fs::metadata(path) else {
        return false;
    };
    let Some(parent) = path.parent() else {
        return false;
    };
    let Ok(m2) = std::fs::metadata(parent) else {
        return false;
    };
    m1.dev() != m2.dev()
}

fn req_u32(map: &Map<String, Value>, key: &str, default: Option<u32>, min: u32, max: u32) -> Result<u32, String> {
    let val = match map.get(key) {
        Some(v) => v
            .as_u64()
            .filter(|_| !v.is_boolean())
            .ok_or_else(|| format!("Invalid integer field: {key}"))? as u32,
        None => default.ok_or_else(|| format!("Missing integer field: {key}"))?,
    };
    if val < min || val > max {
        return Err(format!("Field {key} out of bounds [{min}, {max}]"));
    }
    Ok(val)
}

fn req_f64(map: &Map<String, Value>, key: &str, default: f64, min: f64, max: f64) -> Result<f64, String> {
    let val = match map.get(key) {
        Some(v) => v
            .as_f64()
            .filter(|x| x.is_finite() && !v.is_boolean())
            .ok_or_else(|| format!("Invalid float field: {key}"))?,
        None => default,
    };
    if val < min || val > max {
        return Err(format!("Field {key} out of bounds"));
    }
    Ok(val)
}

fn req_str<'a>(map: &'a Map<String, Value>, key: &str, default: Option<&'a str>, max_len: usize) -> Result<String, String> {
    let raw = match map.get(key) {
        Some(v) => v.as_str().ok_or_else(|| format!("Invalid string field: {key}"))?,
        None => default.ok_or_else(|| format!("Missing string field: {key}"))?,
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > max_len {
        return Err(format!("Field {key} empty or exceeds {max_len} chars"));
    }
    Ok(trimmed.to_owned())
}

fn check_b64(raw: &str, max_b64: usize, max_decoded: usize, name: &str) -> Result<String, String> {
    if raw.is_empty() || raw.len() > max_b64 {
        return Err(format!("Invalid {name} length"));
    }
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(raw)
        .map_err(|_| format!("Invalid {name} base64"))?;
    if decoded.is_empty() || decoded.len() > max_decoded {
        return Err(format!("{name} decoded bytes out of bounds"));
    }
    Ok(raw.to_owned())
}

/// Validate an untrusted Runpod/HTTP job envelope and synthesize the bounded kernel command
/// enriched with the Rust-computed `LowMemExecutionPlan`.
pub fn validate_job(route: WorkerRoute, envelope: &Value, plan: &LowMemExecutionPlan) -> Result<Value, String> {
    let input = envelope
        .get("input")
        .and_then(Value::as_object)
        .ok_or("Missing input object")?;

    if let Some(m) = input.get("model") {
        if m.as_str() != Some(route.pinned_model()) {
            return Err("Untrusted model override rejected".into());
        }
    }

    match route {
        WorkerRoute::Chroma | WorkerRoute::Kromoss => {
            let allowed = [
                "model", "prompt", "negative_prompt", "width", "height",
                "seed", "n", "response_format", "reference_image_b64", "strength",
                "steps", "guidance_scale",
            ];
            if input.keys().any(|k| !allowed.contains(&k.as_str())) {
                return Err("Unsupported image input fields".into());
            }
            let prompt = req_str(input, "prompt", None, 16_384)?;
            let neg_default = if route == WorkerRoute::Kromoss {
                Some("low quality, blurry, malformed anatomy, watermark")
            } else {
                None
            };
            let negative = req_str(input, "negative_prompt", neg_default, 8_192)?;
            let min_dim = if route == WorkerRoute::Kromoss { 512 } else { 64 };
            let width = req_u32(input, "width", Some(1024), min_dim, 1536)?;
            let height = req_u32(input, "height", Some(1024), min_dim, 1536)?;
            if width % 16 != 0 || height % 16 != 0 {
                return Err("Image dimensions must be multiples of 16".into());
            }
            let n = req_u32(input, "n", Some(1), 1, 1)?;
            let _ = n;
            let fmt = input.get("response_format").and_then(Value::as_str).unwrap_or("b64_json");
            if fmt != "b64_json" {
                return Err("Only inline b64_json output is supported".into());
            }
            let seed = req_u32(input, "seed", Some(0), 0, 2_147_483_647)?;
            let steps = req_u32(input, "steps", Some(40), 10, 80)?;
            let guidance = req_f64(input, "guidance_scale", 3.0, 1.0, 10.0)?;
            let ref_b64 = match input.get("reference_image_b64") {
                Some(v) => Some(check_b64(
                    v.as_str().ok_or("Invalid reference_image_b64")?,
                    1_800_000,
                    1_300_000,
                    "reference_image_b64",
                )?),
                None => None,
            };
            if ref_b64.is_none() && input.contains_key("strength") {
                return Err("strength requires reference_image_b64".into());
            }
            let strength = req_f64(input, "strength", 0.65, 0.05, 0.95)?;
            Ok(json!({
                "route": route,
                "model": route.pinned_model(),
                "prompt": prompt,
                "negative_prompt": negative,
                "width": width,
                "height": height,
                "seed": seed,
                "steps": steps,
                "guidance_scale": guidance,
                "reference_image_b64": ref_b64,
                "strength": strength,
                "low_mem": plan,
            }))
        }
        WorkerRoute::Speech | WorkerRoute::Music | WorkerRoute::Effects => {
            let allowed = [
                "model", "prompt", "text", "lyrics", "voice", "language",
                "seed", "seconds", "steps", "cfg_scale", "response_format",
            ];
            if input.keys().any(|k| !allowed.contains(&k.as_str())) {
                return Err("Unsupported audio input fields".into());
            }
            let prompt = input
                .get("prompt")
                .or_else(|| input.get("text"))
                .or_else(|| input.get("lyrics"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|s| !s.is_empty() && s.len() <= 16_000)
                .ok_or("Invalid audio prompt")?;
            let lyrics = input.get("lyrics").and_then(Value::as_str).unwrap_or("");
            if lyrics.len() > 16_000 {
                return Err("Lyrics too long".into());
            }
            let voice = req_str(input, "voice", Some("Savannah"), 128)?;
            let language = req_str(input, "language", Some("en_us"), 32)?;
            let seed = req_u32(input, "seed", Some(0), 0, 2_147_483_647)?;
            let max_sec = if route == WorkerRoute::Effects { 30.0 } else { 600.0 };
            let seconds = req_f64(input, "seconds", 10.0, 1.0, max_sec)?;
            let steps = req_u32(input, "steps", Some(100), 10, 250)?;
            let cfg_scale = req_f64(input, "cfg_scale", 4.0, 0.5, 15.0)?;
            Ok(json!({
                "route": route,
                "model": route.pinned_model(),
                "prompt": prompt,
                "lyrics": lyrics,
                "voice": voice,
                "language": language,
                "seed": seed,
                "seconds": seconds,
                "steps": steps,
                "cfg_scale": cfg_scale,
                "low_mem": plan,
            }))
        }
        WorkerRoute::VideoGenerate | WorkerRoute::VideoEdit => {
            let allowed = [
                "model", "prompt", "negative_prompt", "width", "height",
                "num_frames", "fps", "seed", "response_format",
                "reference_image_b64", "reference_video_b64",
            ];
            if input.keys().any(|k| !allowed.contains(&k.as_str())) {
                return Err("Unsupported video input fields".into());
            }
            let prompt = req_str(input, "prompt", None, 8_192)?;
            let negative = req_str(input, "negative_prompt", Some("blurry, flicker, watermark"), 4_096)?;
            let width = req_u32(input, "width", Some(832), 256, 1280)?;
            let height = req_u32(input, "height", Some(480), 256, 1280)?;
            if width % 16 != 0 || height % 16 != 0 {
                return Err("Video dimensions must be multiples of 16".into());
            }
            let num_frames = req_u32(input, "num_frames", Some(81), 17, 129)?;
            let fps = req_u32(input, "fps", Some(16), 8, 30)?;
            let seed = req_u32(input, "seed", Some(0), 0, 2_147_483_647)?;
            let fmt = input.get("response_format").and_then(Value::as_str).unwrap_or("b64_video");
            if fmt != "b64_video" && fmt != "b64_json" {
                return Err("Only inline video output is supported".into());
            }
            let ref_img = match input.get("reference_image_b64") {
                Some(v) => Some(check_b64(
                    v.as_str().ok_or("Invalid reference_image_b64")?,
                    1_800_000,
                    1_300_000,
                    "reference_image_b64",
                )?),
                None => None,
            };
            let ref_vid = match input.get("reference_video_b64") {
                Some(v) => Some(check_b64(
                    v.as_str().ok_or("Invalid reference_video_b64")?,
                    1_900_000,
                    1_400_000,
                    "reference_video_b64",
                )?),
                None => None,
            };
            if route == WorkerRoute::VideoEdit && ref_vid.is_none() {
                return Err("video-edit requires reference_video_b64".into());
            }
            Ok(json!({
                "route": route,
                "model": route.pinned_model(),
                "prompt": prompt,
                "negative_prompt": negative,
                "width": width,
                "height": height,
                "num_frames": num_frames,
                "fps": fps,
                "seed": seed,
                "reference_image_b64": ref_img,
                "reference_video_b64": ref_vid,
                "low_mem": plan,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syndicate_gpu_lowmem::{plan_for_budget, GIB};

    #[test]
    fn validates_chroma_and_rejects_model_override_or_ssrf() {
        let plan = plan_for_budget(None, Some(12 * GIB), 26 * GIB);
        let ok = validate_job(
            WorkerRoute::Chroma,
            &json!({"input":{"model":"syndicate/chroma-hd","prompt":"vase","negative_prompt":"blur","width":512,"height":512,"seed":1,"n":1,"response_format":"b64_json"}}),
            &plan,
        );
        assert!(ok.is_ok());
        assert_eq!(ok.unwrap()["low_mem"]["diffusion_offload"], "model_cpu");

        for bad in [
            json!({"input":{"model":"evil/model","prompt":"x","negative_prompt":"y","width":512,"height":512}}),
            json!({"input":{"url":"http://169.254.169.254","prompt":"x","negative_prompt":"y","width":512,"height":512}}),
            json!({"input":{"prompt":"x","negative_prompt":"y","width":513,"height":512}}),
            json!({"input":{"prompt":"x","negative_prompt":"y","width":512,"height":512,"strength":0.5}}),
        ] {
            assert!(validate_job(WorkerRoute::Chroma, &bad, &plan).is_err());
        }
    }

    #[test]
    fn video_edit_requires_inline_video_and_audio_clamps_effects_duration() {
        let plan = plan_for_budget(None, Some(24 * GIB), 12 * GIB);
        assert!(validate_job(WorkerRoute::VideoEdit, &json!({"input":{"prompt":"restyle"}}), &plan).is_err());
        let b64 = base64::engine::general_purpose::STANDARD.encode(b"mp4bytes");
        assert!(validate_job(
            WorkerRoute::VideoEdit,
            &json!({"input":{"prompt":"restyle","reference_video_b64":b64}}),
            &plan
        )
        .is_ok());
        assert!(validate_job(
            WorkerRoute::Effects,
            &json!({"input":{"prompt":"thunder","seconds":45}}),
            &plan
        )
        .is_err());
    }
}
