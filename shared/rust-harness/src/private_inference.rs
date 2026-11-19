//! Opt-in private transport. The gateway, never this client, owns spending authority.
use serde_json::{json, Value as Json};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub fn configured() -> bool {
    std::env::var_os("PRIVATE_INFERENCE_URL").is_some()
}

/// Specialist routing is explicit. An absent specialist route keeps the established chat route;
/// media targets never silently cross modalities.
pub fn target_for_role(role: &str) -> &'static str {
    if role == "cyber-lab" && std::env::var_os("SYNDICATE_CYBER_ROUTE").is_some() {
        "cyber"
    } else {
        "chat"
    }
}

/// Creative enhancement is a separate authenticated inference call. It is on by default only
/// when an explicit creative route exists, and can be bypassed per process.
pub struct PromptEnhancement {
    pub positive: String,
    pub negative: String,
    pub receipt: Json,
}

fn parse_enhancement(value: &str, receipt: Json) -> Result<PromptEnhancement, String> {
    let trimmed = value.trim();
    let without_prefix = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed);
    let json_text = without_prefix.strip_suffix("```").unwrap_or(without_prefix).trim();
    let parsed: Json = match serde_json::from_str(json_text) {
        Ok(value) => value,
        Err(_) if !trimmed.is_empty() && trimmed.len() <= 32_768 => return Ok(PromptEnhancement {
            positive: trimmed.to_owned(),
            negative: "low quality, blurry, malformed anatomy, malformed geometry, unintended text, watermark, compression artifacts, duplication".into(),
            receipt,
        }),
        Err(_) => return Err("private_invalid_creative_prompt".into()),
    };
    let field = |name: &str, max: usize| {
        parsed.get(name).and_then(Json::as_str).map(str::trim)
            .filter(|v| !v.is_empty() && v.len() <= max).map(str::to_owned)
            .ok_or("private_invalid_creative_prompt")
    };
    Ok(PromptEnhancement { positive: field("positive_prompt", 32_768)?, negative: field("negative_prompt", 8_192)?, receipt })
}

pub fn enhance_prompt(
    kind: &str,
    prompt: &str,
    deadline: Instant,
) -> Result<Option<PromptEnhancement>, String> {
    if std::env::var_os("SYNDICATE_CREATIVE_ROUTE").is_none()
        || std::env::var("PRIVATE_INFERENCE_PROMPT_ENHANCEMENT").as_deref() == Ok("0")
    {
        return Ok(None);
    }
    let payload = json!({
        "messages":[
            {"role":"system","content":format!("You are an elite production creative director for {kind}. Convert the user's intent into model-ready prompting that maximizes composition, subject fidelity, spatial relationships, anatomy, materials, environment, camera and lens choices, lighting, color, depth, texture, continuity, and finish. Preserve the requested content and do not replace or omit requested details. Build a concise negative prompt that removes only unintended artifacts, malformed geometry or anatomy, blur, accidental text or watermarks, compression defects, duplication, and conflicts with the requested style; never negate requested subjects or actions. Return strict JSON only with exactly two non-empty string fields: positive_prompt and negative_prompt. No markdown, commentary, headings, warnings, or policy language.")},
            {"role":"user","content":prompt}
        ],
        "temperature":0.45,"max_tokens":1200,"reasoning_enabled":false,"response_format":{"type":"json_object"}
    });
    let response = match request("creative", &payload, deadline) {
        Ok(r) => r,
        Err(_) => return Ok(None),
    };
    let Some(rewritten) = response
        .pointer("/result/choices/0/message/content")
        .and_then(Json::as_str)
    else {
        return Ok(None);
    };
    Ok(parse_enhancement(rewritten, response["receipt"].clone()).ok())
}

pub fn request(mode: &str, payload: &Json, deadline: Instant) -> Result<Json, String> {
    if !configured() {
        return Err("private_not_configured".into());
    }
    let mut child = Command::new("node")
        .arg(crate::jobs::root().join("shared/inference-gateway/client.ts"))
        .arg(mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(
            if std::env::var("PRIVATE_INFERENCE_EVENTS").as_deref() == Ok("1") {
                Stdio::inherit()
            } else {
                Stdio::null()
            },
        )
        .spawn()
        .map_err(|_| "private_transport_unavailable")?;
    let stdout = child.stdout.take().ok_or("private_transport_unavailable")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.take(24_000_001).read_to_end(&mut bytes);
        (result, bytes)
    });
    let bytes = serde_json::to_vec(payload).map_err(|_| "invalid_request")?;
    if bytes.len() > 2_000_000 {
        let _ = child.kill();
        let _ = child.wait();
        return Err("request_too_large".into());
    }
    let write = child
        .stdin
        .take()
        .ok_or("private_transport_unavailable")?
        .write_all(&bytes);
    if write.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return Err("private_transport_failed".into());
    }
    let timeout_ms = std::env::var("PRIVATE_INFERENCE_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| (1000..=900000).contains(v))
        .unwrap_or(900000);
    let end = deadline.min(Instant::now() + Duration::from_millis(timeout_ms + 5000));
    loop {
        if crate::agent::cancelled() || Instant::now() >= end {
            let _ = child.kill();
            let _ = child.wait();
            return Err("private_cancelled_or_timeout".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let (read, bytes) = reader.join().map_err(|_| "private_transport_failed")?;
                if read.is_err() || bytes.len() > 24_000_000 {
                    return Err("private_unavailable_or_denied".into());
                }
                if !status.success() {
                    let code = serde_json::from_slice::<Json>(&bytes).ok().and_then(|v| {
                        v.pointer("/error/code")
                            .and_then(Json::as_str)
                            .map(str::to_owned)
                    });
                    return Err(match code.as_deref() {
                        Some("private_budget_exhausted") => "private_budget_exhausted",
                        Some("private_access_denied") => "private_access_denied",
                        _ => "private_unavailable_or_denied",
                    }
                    .into());
                }
                let response: Json =
                    serde_json::from_slice(&bytes).map_err(|_| "private_invalid_response")?;
                if !response["result"].is_object() || !response["receipt"].is_object() {
                    return Err("private_invalid_response".into());
                }
                return Ok(response);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("private_transport_failed".into());
            }
        }
    }
}

pub fn receipt(dir: &Path, value: &Json) {
    use std::fs::OpenOptions;
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("inference-routing.jsonl"))
    {
        let _ = writeln!(file, "{}", value);
    }
}

/// Single-turn JSON transport for downstream apps. Local admission/lock remain authoritative.
pub fn chat_cli() -> Result<(), String> {
    let mut text = String::new();
    std::io::stdin()
        .take(2_000_001)
        .read_to_string(&mut text)
        .map_err(|e| e.to_string())?;
    if text.len() > 2_000_000 {
        return Err("request_too_large".into());
    }
    let body: Json = serde_json::from_str(&text).map_err(|_| "invalid_json")?;
    let deadline = Instant::now() + Duration::from_secs(900);
    if configured() {
        match request("chat", &body, deadline) {
            Ok(result) => {
                println!("{result}");
                return Ok(());
            }
            Err(reason) if !fallback_allowed() => {
                return Err(format!("{reason}; local fallback is OFF"))
            }
            Err(_) => {}
        }
    }
    if crate::agent::cancelled() {
        return Err("cancelled".into());
    }
    let dir = crate::jobs::data_root()
        .join("runs")
        .join(crate::jobs::new_job_id()?);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let server =
        crate::runtime::ModelServer::start(&crate::runtime::ServerConfig::qwen(false), &dir)?;
    let mut local = body;
    local["model"] = json!("local-agent");
    let result = crate::runtime::request_json(
        server.port,
        "/v1/chat/completions",
        Some(&local),
        Duration::from_secs(180),
    )?;
    println!(
        "{}",
        json!({"result":result,"receipt":{"source":"local","model":"configured-local-qwen","reason":"private_unavailable_or_unconfigured"}})
    );
    Ok(())
}

/// Public progress contains selected model identity, never provider credentials.
pub fn local_route(model: &Path) {
    if std::env::var("PRIVATE_INFERENCE_EVENTS").as_deref() == Ok("1") {
        eprintln!(
            "INFERENCE_EVENT {}",
            json!({"type":"route","text":format!("local · {}",model.display())})
        );
    }
}

pub fn fallback_allowed() -> bool {
    if let Ok(file) = std::env::var("PRIVATE_INFERENCE_CONTROL_FILE") {
        if let Ok(text) = std::fs::read_to_string(file) {
            if let Ok(value) = serde_json::from_str::<Json>(&text) {
                if let Some(enabled) = value.get("localFallbackEnabled").and_then(Json::as_bool) {
                    return enabled;
                }
            }
        }
    }
    std::env::var("LOCAL_AGENTS_ALLOW_LOCAL_FALLBACK").as_deref() == Ok("1")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn creative_prompt_parsing_keeps_positive_and_negative_controls() {
        let parsed=parse_enhancement(r#"{"positive_prompt":"sharp portrait","negative_prompt":"blur, watermark"}"#,json!({"id":"r"})).unwrap();
        assert_eq!(parsed.positive,"sharp portrait");assert_eq!(parsed.negative,"blur, watermark");
        let fenced=parse_enhancement("```json\n{\"positive_prompt\":\"city\",\"negative_prompt\":\"fog\"}\n```",json!({})).unwrap();
        assert_eq!(fenced.positive,"city");
        let fallback=parse_enhancement("detailed landscape",json!({})).unwrap();
        assert_eq!(fallback.positive,"detailed landscape");assert!(fallback.negative.contains("watermark"));
    }
}
