//! Image and video workers (ports of image_worker.py, video_worker.py and the
//! MediaGuard in media_policy.py). Rust owns everything around the model:
//!
//!   validate request ─► heavy lock ─► reclaim RAM (quit ChatGPT, then largest app)
//!     ─► admission check ─► spawn MLX kernel (media_kernel.py, own process group)
//!     ─► watchdog every 2 s: reclaim more on warning, SIGTERM the kernel on a breach
//!     ─► validate the file ─► evidence + resources JSON ─► reopen quit apps ─► unlock
//!
//! Only media_kernel.py (mflux / the pinned Wan pipeline on MLX) stays Python.
use crate::jobs::{prerequisites, root};
use crate::reclaim::Policy;
use crate::runtime::{
    admission_reason, save_resources, spawn_watchdog, terminate_group, HeavyLock, RestoreGuard,
    Sample,
};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::fs;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How the kernel is launched and guarded; tests swap the program and drop the guard.
#[derive(Clone)]
pub struct Runner {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub lock_path: PathBuf,
    pub reclaim: Option<Policy>,
    pub guard: bool,
    pub time_limit: Duration,
    pub cancelled: fn() -> bool,
}

impl Runner {
    pub fn standard() -> Runner {
        let r = root();
        Runner {
            program: r.join("shared/runtime/venv/bin/python"),
            args: vec![r
                .join("shared/harness/media_kernel.py")
                .to_string_lossy()
                .into_owned()],
            lock_path: HeavyLock::default_path(),
            reclaim: Policy::load(&r)
                .ok()
                .flatten()
                .filter(|p| p.target_available_bytes > 0 && (p.auto_largest || !p.apps.is_empty())),
            guard: true,
            time_limit: Duration::from_secs(1800),
            cancelled: crate::agent::cancelled,
        }
    }

    /// Run one kernel request under the lock and guard. Returns the kernel's facts.
    pub fn run(&self, request: &Json, output: &Path) -> Result<Json, String> {
        let _lock = HeavyLock::acquire(&self.lock_path, Duration::ZERO)?;
        if let Some(d) = output.parent() {
            fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let resources = output.with_extension("resources.json");
        let restore = RestoreGuard::new(
            self.reclaim.as_ref(),
            output.with_extension("memory-reclaim.json"),
        );
        if let Some(p) = &self.reclaim {
            restore.reclaim_before(p);
        }
        let samples = Arc::new(Mutex::new(Vec::new()));
        let initial = if self.guard {
            let s = Sample::take(None)?;
            samples.lock().unwrap().push(s.to_json());
            if let Some(reason) = admission_reason(&s) {
                save_resources(
                    &resources,
                    "admission_blocked",
                    Some(reason),
                    &samples.lock().unwrap(),
                );
                return Err(reason.into());
            }
            save_resources(&resources, "running", None, &samples.lock().unwrap());
            Some(s)
        } else {
            None
        };
        let request_path = output.with_extension("request.json");
        fs::write(
            &request_path,
            serde_json::to_string_pretty(request).unwrap_or_default(),
        )
        .map_err(|e| e.to_string())?;
        let log_path = output.with_extension("kernel.log");
        let log = fs::File::create(&log_path).map_err(|e| e.to_string())?;
        let mut cmd = Command::new(&self.program);
        cmd.args(&self.args)
            .arg(&request_path)
            .current_dir(root())
            .env("HF_HUB_OFFLINE", "1")
            .env("HF_HUB_DISABLE_TELEMETRY", "1")
            .stdin(Stdio::null())
            .stdout(log.try_clone().map_err(|e| e.to_string())?)
            .stderr(log);
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("could not start media kernel: {e}"))?;
        let in_run = self.reclaim.clone().map(|p| (p, restore.clone()));
        let mut watchdog = initial.map(|s| {
            spawn_watchdog(
                child.id() as i32,
                s,
                in_run,
                samples.clone(),
                resources.clone(),
            )
        });
        let deadline = Instant::now() + self.time_limit;
        let outcome: Result<(), String> = loop {
            if let Ok(Some(status)) = child.try_wait() {
                break if status.success() {
                    Ok(())
                } else {
                    Err(format!(
                        "Media kernel failed ({status}); inspect {}",
                        log_path.display()
                    ))
                };
            }
            if let Some(r) = watchdog.as_ref().and_then(|w| w.reason()) {
                break Err(format!("ResourceStop: {r}"));
            }
            if (self.cancelled)() {
                break Err("KeyboardInterrupt: Media job cancelled".into());
            }
            if Instant::now() >= deadline {
                break Err(format!(
                    "TimeoutError: media kernel exceeded {} seconds",
                    self.time_limit.as_secs()
                ));
            }
            std::thread::sleep(Duration::from_millis(200));
        };
        let reason = watchdog.as_ref().and_then(|w| w.reason());
        if let Some(w) = watchdog.as_mut() {
            w.finish();
        }
        terminate_group(&mut child);
        if self.guard {
            if let Ok(s) = Sample::take(None) {
                samples.lock().unwrap().push(s.to_json());
            }
            let status = if reason.is_some() {
                "resource_stopped"
            } else if outcome.is_ok() {
                "completed"
            } else {
                "failed"
            };
            save_resources(
                &resources,
                status,
                reason.as_deref(),
                &samples.lock().unwrap(),
            );
        }
        drop(restore); // reopen apps reclaim quit, then release the lock
        outcome?;
        let text = fs::read_to_string(&log_path).unwrap_or_default();
        text.lines()
            .rev()
            .find_map(|l| {
                serde_json::from_str::<Json>(l)
                    .ok()
                    .and_then(|v| v.get("kernel").cloned())
            })
            .ok_or_else(|| {
                format!(
                    "Media kernel printed no result; inspect {}",
                    log_path.display()
                )
            })
    }
}

// ---------- image dimension sniffing (PNG / JPEG / WebP) ----------

/// (format, width, height) from the file header.
pub fn image_info(b: &[u8]) -> Option<(&'static str, u32, u32)> {
    let be16 = |i: usize| u16::from_be_bytes([b[i], b[i + 1]]) as u32;
    let le16 = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]) as u32;
    if b.len() >= 24 && b.starts_with(b"\x89PNG\r\n\x1a\n") && &b[12..16] == b"IHDR" {
        return Some((
            "PNG",
            u32::from_be_bytes(b[16..20].try_into().ok()?),
            u32::from_be_bytes(b[20..24].try_into().ok()?),
        ));
    }
    if b.len() >= 30 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        return match &b[12..16] {
            b"VP8 " => Some(("WEBP", le16(26) & 0x3fff, le16(28) & 0x3fff)),
            b"VP8L" => {
                let v = u32::from_le_bytes(b[21..25].try_into().ok()?);
                Some(("WEBP", (v & 0x3fff) + 1, ((v >> 14) & 0x3fff) + 1))
            }
            b"VP8X" => Some((
                "WEBP",
                (b[24] as u32 | (b[25] as u32) << 8 | (b[26] as u32) << 16) + 1,
                (b[27] as u32 | (b[28] as u32) << 8 | (b[29] as u32) << 16) + 1,
            )),
            _ => None,
        };
    }
    if b.len() >= 4 && b[0] == 0xFF && b[1] == 0xD8 {
        let mut i = 2;
        while i + 9 < b.len() {
            if b[i] != 0xFF {
                return None;
            }
            let marker = b[i + 1];
            if marker == 0xFF {
                i += 1;
                continue;
            }
            if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
                return Some(("JPEG", be16(i + 7), be16(i + 5)));
            }
            i += 2 + be16(i + 2) as usize;
        }
    }
    None
}

/// image_worker.reference_record: a local PNG/JPEG/WebP of at most 768 px per side and 20 MiB.
pub fn reference_record(path: &Path) -> Result<Json, String> {
    let p = path
        .canonicalize()
        .map_err(|e| format!("Reference image: {e}"))?;
    let bytes = fs::read(&p).map_err(|e| e.to_string())?;
    if bytes.len() > 20 * 1024 * 1024 {
        return Err("Reference exceeds 20 MiB".into());
    }
    match image_info(&bytes) {
        Some((_, w, h)) if w.max(h) <= 768 && w > 0 && h > 0 => {
            Ok(json!({"path": p, "size": [w, h], "sha256": hex::encode(Sha256::digest(&bytes))}))
        }
        _ => Err("Use a PNG/JPEG/WebP reference no larger than 768 pixels per side".into()),
    }
}

// ---------- CLI ----------

fn local_stamp() -> String {
    let t = crate::jobs::now() as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    unsafe { libc::localtime_r(&t, &mut tm) };
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

fn role_config(role: &str) -> Result<Json, String> {
    let p = root().join("agents").join(role).join("config.json");
    serde_json::from_str(&fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?)
        .map_err(|e| e.to_string())
}

struct Args {
    prompt: String,
    output: Option<PathBuf>,
    preset: String,
    reference: Option<PathBuf>,
    nums: std::collections::HashMap<String, i64>,
}

fn parse(args: &[String], numeric: &[&str], usage: &str) -> Result<Args, String> {
    let mut a = Args {
        prompt: String::new(),
        output: None,
        preset: "draft".into(),
        reference: None,
        nums: Default::default(),
    };
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut val = || {
            it.next()
                .cloned()
                .ok_or(format!("{x} needs a value\n{usage}"))
        };
        match x.as_str() {
            "--output" => a.output = Some(val()?.into()),
            "--preset" => a.preset = val()?,
            "--reference-image" if numeric.contains(&"width") && !numeric.contains(&"frames") => {
                a.reference = Some(val()?.into())
            }
            f if f.starts_with("--") && numeric.contains(&&f[2..]) => {
                let v = val()?;
                a.nums.insert(
                    f[2..].into(),
                    v.parse().map_err(|_| format!("{f}: not an integer"))?,
                );
            }
            f if f.starts_with("--") => return Err(format!("unknown option {f}\n{usage}")),
            p if a.prompt.is_empty() => a.prompt = p.into(),
            _ => return Err(usage.into()),
        }
    }
    if a.prompt.is_empty() {
        return Err(usage.into());
    }
    Ok(a)
}

/// Check the installation evidence, run the kernel, validate the file, write evidence.
fn finish(
    role: &str,
    output: &Path,
    request: Json,
    runner: &Runner,
    validate: &dyn Fn(&Json) -> Result<Json, String>,
) -> Result<Json, String> {
    let missing = prerequisites(&root(), role);
    if !missing.is_empty() {
        return Err(format!(
            "Missing installation evidence: {}",
            missing.join(", ")
        ));
    }
    let facts = runner.run(&request, output)?;
    let mut record = validate(&facts)?;
    record["resources"] = json!(output.with_extension("resources.json"));
    record["controller"] = json!("rust");
    if output.with_extension("memory-reclaim.json").is_file() {
        record["memory_reclaim"] = json!(output.with_extension("memory-reclaim.json"));
    }
    fs::write(
        output.with_extension("evidence.json"),
        serde_json::to_string_pretty(&record).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;
    Ok(record)
}

pub fn image_request(args: &[String]) -> Result<(PathBuf, Json), String> {
    let usage = "usage: local-agents-rs image <prompt> [--output O] [--preset draft|quality] [--reference-image P] [--seed N] [--width W] [--height H]";
    let a = parse(args, &["seed", "width", "height"], usage)?;
    let cfg = role_config("image-studio")?;
    let (pw, ph) = match a.preset.as_str() {
        "draft" => (512, 512),
        "quality" => (768, 768),
        _ => return Err("Unsupported media role/preset".into()),
    };
    let (w, h) = (
        a.nums.get("width").copied().unwrap_or(pw),
        a.nums.get("height").copied().unwrap_or(ph),
    );
    let max_side = if crate::private_inference::configured() { 1536 } else { 768 };
    if !((64..=max_side).contains(&w) && (64..=max_side).contains(&h) && w % 16 == 0 && h % 16 == 0) {
        return Err(format!("Image size must be 64–{max_side} pixels per side, multiples of 16"));
    }
    let r = root();
    let output = a.output.unwrap_or_else(|| {
        r.join("agents/image-studio/outputs").join(format!(
            "{}-{}.png",
            local_stamp(),
            &crate::jobs::new_job_id().unwrap_or_default()[..6]
        ))
    });
    let reference = a.reference.as_deref().map(reference_record).transpose()?;
    Ok((
        output.clone(),
        json!({"mode": "image", "prompt": a.prompt, "output": output, "width": w, "height": h,
        "seed": a.nums.get("seed").copied().unwrap_or(cfg["seed"].as_i64().unwrap_or(729)),
        "steps": cfg["steps"], "guidance": cfg["guidance"], "model_path": r.join(cfg["model_path"].as_str().unwrap_or_default()),
        "reference": reference.as_ref().map(|x| x["path"].clone()), "reference_record": reference, "preset": a.preset}),
    ))
}

pub fn run_image(args: &[String], runner: &Runner) -> Result<Json, String> {
    let (output, req) = image_request(args)?;
    let (w, h) = (
        req["width"].as_u64().unwrap_or(0) as u32,
        req["height"].as_u64().unwrap_or(0) as u32,
    );
    let mut fallback_reason = None;
    if runner.guard && crate::private_inference::configured() {
        let deadline = Instant::now() + runner.time_limit;
        let enhancement = crate::private_inference::enhance_prompt(
            if req["reference"].is_null() { "text-to-image generation" } else { "image-to-image editing" },
            req["prompt"].as_str().unwrap_or_default(),
            deadline,
        )?;
        let prompt = enhancement
            .as_ref()
            .map(|v| v.positive.as_str())
            .unwrap_or_else(|| req["prompt"].as_str().unwrap_or_default());
        let negative = enhancement.as_ref().map(|v| v.negative.as_str())
            .unwrap_or("low quality, blurry, malformed anatomy, unintended text, watermark, compression artifacts");
        let mut payload = json!({"prompt":prompt,"negative_prompt":negative,"width":w,"height":h,"seed":req["seed"]});
        if let Some(reference) = req["reference"].as_str() {
            use base64::Engine;
            let bytes=fs::read(reference).map_err(|e| format!("Reference image: {e}"))?;
            if bytes.len()>1_300_000{return Err("Private reference image exceeds 1.3 MB; resize or recompress it first".into());}
            payload["reference_image_b64"]=json!(base64::engine::general_purpose::STANDARD.encode(bytes));
            payload["strength"]=json!(0.65);
        }
        let target = if req["reference"].is_string() { "image-edit" } else { "image" };
        let attempt = crate::private_inference::request(
            target,
            &payload,
            Instant::now() + runner.time_limit,
        )
        .and_then(|response| save_private_image(&output, &response, w, h));
        match attempt {
            Ok(mut record) => {
                if let Some(enhancement) = enhancement {
                    record["prompt_enhancement"] = json!({"enabled":true,"negative_prompt_applied":true,"receipt":enhancement.receipt});
                    fs::write(
                        output.with_extension("evidence.json"),
                        serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
                }
                return Ok(record);
            }
            Err(reason) => {
                if !crate::private_inference::fallback_allowed() {
                    return Err(format!("{reason}; local fallback is OFF"));
                }
                fallback_reason = Some(reason)
            }
        }
        if (runner.cancelled)() {
            return Err("Media job cancelled".into());
        }
    }
    let mut record = finish("image-studio", &output, req.clone(), runner, &|facts| {
        let bytes = fs::read(&output).map_err(|e| format!("No image was written: {e}"))?;
        if image_info(&bytes).map(|(_, a, b)| (a, b)) != Some((w, h))
            || !bytes.ends_with(b"IEND\xaeB`\x82")
        {
            return Err("Unexpected image dimensions or a truncated PNG".into());
        }
        Ok(
            json!({"status": "generated_and_file_valid", "visual_inspection": "pending", "path": output, "prompt": req["prompt"],
            "seed": req["seed"], "size": [w, h], "steps": req["steps"], "preset": req["preset"],
            "mode": if req["reference"].is_null() { "text_to_image" } else { "reference_edit" }, "reference": req["reference_record"],
            "seconds": facts["seconds"], "mlx_peak_bytes": facts["mlx_peak_bytes"], "model_path": req["model_path"],
            "encoder": "ponpoke local Qwen3 encoder converted to Q4", "low_memory": true, "video_generation": false}),
        )
    })?;
    record["inference_source"] = json!("local");
    record["fallback_reason"] = json!(fallback_reason);
    fs::write(
        output.with_extension("evidence.json"),
        serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(record)
}

pub fn video_request(args: &[String]) -> Result<(PathBuf, Json), String> {
    let usage = "usage: local-agents-rs video <prompt> [--output O] [--preset draft|quality|detail-short] [--width W] [--height H] [--frames F] [--steps S] [--seed N]";
    let a = parse(args, &["seed", "width", "height", "frames", "steps"], usage)?;
    let cfg = role_config("video-studio")?;
    let (pw, ph, pf, ps) = match a.preset.as_str() {
        "draft" => (320, 192, 33, 24),
        "quality" => (384, 224, 33, 32),
        "detail-short" => (512, 288, 17, 32),
        _ => return Err("Unsupported media role/preset".into()),
    };
    let n = |k: &str, d: i64| a.nums.get(k).copied().unwrap_or(d);
    let (w, h, f, s) = (
        n("width", pw),
        n("height", ph),
        n("frames", pf),
        n("steps", ps),
    );
    if !((64..=512).contains(&w)
        && (64..=512).contains(&h)
        && w % 16 == 0
        && h % 16 == 0
        && (5..=81).contains(&f)
        && (f - 1) % 4 == 0
        && (4..=50).contains(&s))
    {
        return Err("Initial memory bounds: 64–512 pixels in multiples of 16; 5–81 frames (4n+1); 4–50 steps".into());
    }
    if w * h > 512 * 288 || f > 33 {
        return Err("This Mac is limited to 512x288 pixels of area and 33 frames".into());
    }
    if w * h > 384 * 224 && f > 17 {
        return Err("Above 384x224, this Mac is limited to 17 frames".into());
    }
    let r = root();
    let output = a.output.unwrap_or_else(|| {
        r.join("agents/video-studio/outputs").join(format!(
            "{}-{}.mp4",
            local_stamp(),
            &crate::jobs::new_job_id().unwrap_or_default()[..6]
        ))
    });
    Ok((
        output.clone(),
        json!({"mode": "video", "prompt": a.prompt, "output": output, "width": w, "height": h, "frames": f, "steps": s,
        "seed": n("seed", cfg["seed"].as_i64().unwrap_or(729)), "fps": cfg["fps"], "guidance": cfg["guidance"], "preset": a.preset,
        "models": r.join(cfg["model_path"].as_str().unwrap_or_default()), "checkpoint": cfg["checkpoint"],
        "vendor": r.join("shared/vendor/mlx-wan")}),
    ))
}

pub fn run_video(args: &[String], runner: &Runner) -> Result<Json, String> {
    if runner.guard
        && crate::private_inference::configured()
        && !crate::private_inference::fallback_allowed()
    {
        return Err("Private video route is not configured; local fallback is OFF".into());
    }
    let (output, req) = video_request(args)?;
    finish("video-studio", &output, req.clone(), runner, &|facts| {
        let probe = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-count_frames",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=width,height,nb_read_frames,r_frame_rate:format=duration",
                "-of",
                "json",
            ])
            .arg(&output)
            .output()
            .map_err(|e| format!("ffprobe: {e}"))?;
        let probe: Json =
            serde_json::from_slice(&probe.stdout).map_err(|e| format!("ffprobe output: {e}"))?;
        let st = &probe["streams"][0];
        let frames: i64 = st["nb_read_frames"]
            .as_str()
            .and_then(|v| v.parse().ok())
            .unwrap_or(-1);
        if (st["width"].as_i64(), st["height"].as_i64(), frames)
            != (
                req["width"].as_i64(),
                req["height"].as_i64(),
                req["frames"].as_i64().unwrap_or(0),
            )
        {
            return Err("Video dimensions/frame count did not match the request".into());
        }
        if facts["finite_pixels"] != json!(true)
            || facts["mean_absolute_frame_change"].as_f64().unwrap_or(0.0) == 0.0
        {
            return Err("Video contains invalid pixels or no temporal variation".into());
        }
        Ok(
            json!({"status": "generated_and_decoded", "visual_inspection": "pending", "path": output, "prompt": req["prompt"], "seed": req["seed"],
            "size": [req["width"], req["height"]], "frames": req["frames"], "steps": req["steps"], "fps": req["fps"], "seconds": facts["seconds"],
            "preset": req["preset"], "mlx_peak_bytes_by_phase": facts["mlx_peak_bytes_by_phase"], "finite_pixels": true,
            "mean_absolute_frame_change": facts["mean_absolute_frame_change"], "ffprobe": probe,
            "model": "blackshirt/NSFW_Wan_1.3b", "revision": "6199ecb457af237b143ca9cc2fccec6dcfbe4c22", "checkpoint": req["checkpoint"],
            "precision": {"dit": "runtime MLX Q8", "text_encoder": "prequantized MLX Q8"},
            "limitations": "A low-resolution smoke test does not establish production quality or universal no-refusal behavior."}),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        b.extend(w.to_be_bytes());
        b.extend(h.to_be_bytes());
        b.extend(b"\x08\x02\0\0\0IEND\xaeB`\x82");
        b
    }

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "la-media-{tag}-{}",
            crate::jobs::new_job_id().unwrap()
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn sniffs_png_jpeg_and_webp_sizes() {
        assert_eq!(image_info(&png(640, 480)), Some(("PNG", 640, 480)));
        // JPEG: SOI, an APP0 segment, then SOF0 with height 300, width 200.
        let jpg = [
            0xFF, 0xD8, 0xFF, 0xE0, 0, 4, 0, 0, 0xFF, 0xC0, 0, 17, 8, 0x01, 0x2C, 0x00, 0xC8, 3, 0,
            0, 0, 0,
        ];
        assert_eq!(image_info(&jpg), Some(("JPEG", 200, 300)));
        let mut webp = b"RIFF\0\0\0\0WEBPVP8X\x0a\0\0\0\0\0\0\0".to_vec();
        webp.extend([0xFF, 0x02, 0x00, 0x7F, 0x01, 0x00]); // 768 x 384 (stored minus one)
        assert_eq!(image_info(&webp), Some(("WEBP", 768, 384)));
        assert_eq!(image_info(b"GIF89a.........................."), None);
    }

    #[test]
    fn requests_enforce_this_macs_bounds() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let (_, r) = image_request(&a(&["a kite", "--preset", "quality"])).unwrap();
        assert_eq!(
            (r["width"].clone(), r["height"].clone(), r["steps"].clone()),
            (json!(768), json!(768), json!(4))
        );
        assert!(image_request(&a(&["x", "--width", "800"])).is_err());
        assert!(
            image_request(&a(&["x", "--width", "500"])).is_err(),
            "multiples of 16"
        );
        assert!(image_request(&a(&["x", "--preset", "detail-short"])).is_err());
        let d = scratch("ref");
        fs::write(d.join("big.png"), png(1024, 512)).unwrap();
        fs::write(d.join("ok.png"), png(512, 512)).unwrap();
        assert!(image_request(&a(&[
            "x",
            "--reference-image",
            d.join("big.png").to_str().unwrap()
        ]))
        .is_err());
        let (_, r) = image_request(&a(&[
            "x",
            "--reference-image",
            d.join("ok.png").to_str().unwrap(),
        ]))
        .unwrap();
        assert_eq!(r["reference_record"]["size"], json!([512, 512]));
        let (_, v) = video_request(&a(&["waves", "--preset", "detail-short"])).unwrap();
        assert_eq!(
            (v["width"].clone(), v["frames"].clone()),
            (json!(512), json!(17))
        );
        assert!(
            video_request(&a(&["x", "--frames", "34"])).is_err(),
            "4n+1 frames"
        );
        assert!(
            video_request(&a(&["x", "--preset", "detail-short", "--frames", "33"])).is_err(),
            "17 frames above 384x224"
        );
        assert!(
            video_request(&a(&["x", "--reference-image", "/tmp/a.png"])).is_err(),
            "no image-to-video"
        );
    }

    static STOP: AtomicBool = AtomicBool::new(false);
    fn never() -> bool {
        false
    }
    fn stop() -> bool {
        STOP.load(Ordering::SeqCst)
    }

    fn fake(d: &Path, script: &str) -> Runner {
        Runner {
            program: "/bin/sh".into(),
            args: vec!["-c".into(), script.into(), "kernel".into()],
            lock_path: d.join("heavy.lock"),
            reclaim: None,
            guard: false,
            time_limit: Duration::from_secs(20),
            cancelled: never,
        }
    }

    #[test]
    fn kernel_runs_under_the_lock_and_reports_facts() {
        let d = scratch("kernel");
        let out = d.join("output.png");
        let runner = fake(
            &d,
            r#"echo progress >&2; cp "$(dirname "$1")/fixture.png" "$(dirname "$1")/output.png"; echo '{"kernel": {"seconds": 1.5, "mlx_peak_bytes": 7}}'"#,
        );
        fs::write(d.join("fixture.png"), png(64, 64)).unwrap();
        let facts = runner.run(&json!({"mode": "image"}), &out).unwrap();
        assert_eq!(facts, json!({"seconds": 1.5, "mlx_peak_bytes": 7}));
        assert!(
            out.with_extension("request.json").is_file()
                && out.with_extension("kernel.log").is_file()
        );
        // A second heavy worker is refused while the lock is held.
        let held = HeavyLock::acquire(&d.join("heavy.lock"), Duration::ZERO).unwrap();
        assert!(runner
            .run(&json!({}), &out)
            .unwrap_err()
            .contains("Another local heavy-model worker"));
        drop(held);
        assert!(fake(&d, "exit 3")
            .run(&json!({}), &out)
            .unwrap_err()
            .contains("Media kernel failed"));
        assert!(fake(&d, "true")
            .run(&json!({}), &out)
            .unwrap_err()
            .contains("printed no result"));
    }

    #[test]
    fn guarded_run_records_resources_and_cancel_kills_the_kernel_group() {
        let d = scratch("guard");
        let out = d.join("output.png");
        let mut runner = fake(&d, r#"echo '{"kernel": {}}'"#);
        runner.guard = true;
        match runner.run(&json!({}), &out) {
            Ok(_) => {
                let r: Json = serde_json::from_str(
                    &fs::read_to_string(out.with_extension("resources.json")).unwrap(),
                )
                .unwrap();
                assert_eq!(r["status"], json!("completed"));
                assert!(r["samples"].as_array().unwrap().len() >= 2);
            }
            Err(e) => assert!(
                e.contains("available") || e.contains("pressure") || e.contains("disk"),
                "only admission may refuse: {e}"
            ),
        }
        let pidfile = d.join("grandchild.pid");
        let mut runner = fake(
            &d,
            &format!("sleep 60 & echo $! > {}; wait", pidfile.display()),
        );
        runner.cancelled = stop;
        std::thread::spawn(|| {
            std::thread::sleep(Duration::from_millis(500));
            STOP.store(true, Ordering::SeqCst);
        });
        let t = Instant::now();
        assert!(runner
            .run(&json!({}), &out)
            .unwrap_err()
            .starts_with("KeyboardInterrupt"));
        assert!(t.elapsed() < Duration::from_secs(10));
        let pid: i32 = fs::read_to_string(&pidfile)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        assert_ne!(
            unsafe { libc::kill(pid, 0) },
            0,
            "kernel's process group is gone"
        );
        // The lock was released for the next job.
        assert!(HeavyLock::acquire(&d.join("heavy.lock"), Duration::ZERO).is_ok());
    }
}

/// Only inline PNGs are accepted; remote artifact URLs never bypass authentication.
fn save_private_image(output: &Path, response: &Json, w: u32, h: u32) -> Result<Json, String> {
    use base64::Engine;
    let encoded = response
        .pointer("/result/data/0/b64_json")
        .and_then(Json::as_str)
        .ok_or("private_invalid_image")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|_| "private_invalid_image")?;
    if image_info(&bytes).map(|(_, a, b)| (a, b)) != Some((w, h))
        || !bytes.ends_with(b"IEND\xaeB\x60\x82")
    {
        return Err("private_invalid_image".into());
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(output, &bytes).map_err(|e| e.to_string())?;
    let record = json!({"status":"generated_and_file_valid","visual_inspection":"pending","path":output,"size":[w,h],"sha256":hex::encode(Sha256::digest(&bytes)),"inference_source":"private","receipt":response["receipt"],"controller":"rust"});
    fs::write(
        output.with_extension("evidence.json"),
        serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(record)
}
