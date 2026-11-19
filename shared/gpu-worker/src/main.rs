//! Native Rust GPU worker supervisor (`syndicate-gpu-worker`).
//!
//! Owns:
//!   1. Hardware GPU probe (`objc2-metal`, `ash` Vulkan `VK_EXT_memory_budget`, `libcuda`)
//!   2. Deterministic `LowMemExecutionPlan` (sequential/model CPU offload, tiled VAE, CUDA pool GC)
//!   3. Strict compiled request validation & SHA-256 kernel integrity check
//!   4. Runpod Serverless native webhook queue loop + Cloud Run / loopback HTTP server
//!   5. Process-group `setsid` / `killpg` watchdog over the minimal `tensor_kernel.py` shim
mod validate;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use syndicate_gpu_lowmem::{plan_for_budget, probe_best_gpu, LowMemExecutionPlan};
use validate::{log_event, resolve_cache_dir, validate_job, WorkerRoute};

struct KernelBridge {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl KernelBridge {
    fn spawn(
        python: &str,
        kernel_path: &Path,
        expected_sha256: Option<&str>,
        cache_dir: &str,
        plan: &LowMemExecutionPlan,
    ) -> Result<Self, String> {
        let bytes = std::fs::read(kernel_path)
            .map_err(|e| format!("Failed to read {}: {e}", kernel_path.display()))?;
        let actual_sha = hex::encode(Sha256::digest(&bytes));
        if let Some(exp) = expected_sha256.filter(|s| !s.is_empty()) {
            if actual_sha != exp {
                return Err(format!("Kernel SHA-256 mismatch: expected {exp}, got {actual_sha}"));
            }
        }
        let mut cmd = Command::new(python);
        cmd.arg("-u")
            .arg(kernel_path)
            .env("HF_HUB_CACHE", cache_dir)
            .env("HF_HOME", cache_dir)
            .env("HF_HUB_DISABLE_TELEMETRY", "1")
            .env("PYTHONUNBUFFERED", "1")
            .env("PYTORCH_CUDA_ALLOC_CONF", plan.cuda_alloc_conf)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn tensor kernel: {e}"))?;
        let stdin = child.stdin.take().ok_or("Missing kernel stdin")?;
        let stdout = BufReader::new(child.stdout.take().ok_or("Missing kernel stdout")?);
        Ok(Self { child, stdin, stdout })
    }

    fn execute(&mut self, cmd_json: &Value, worker_rev: &str) -> Result<Value, String> {
        let mut line = serde_json::to_string(cmd_json).map_err(|e| e.to_string())?;
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.flush())
            .map_err(|e| format!("Kernel pipe write failed: {e}"))?;
        let mut reply = String::new();
        let n = self
            .stdout
            .read_line(&mut reply)
            .map_err(|e| format!("Kernel pipe read failed: {e}"))?;
        if n == 0 {
            return Err("Tensor kernel exited unexpectedly".into());
        }
        let mut parsed: Value =
            serde_json::from_str(&reply).map_err(|_| "Malformed kernel JSON response".to_string())?;
        if let Some(err) = parsed.get("error").and_then(Value::as_str) {
            return Err(err.chars().take(500).collect());
        }
        if let Some(obj) = parsed.as_object_mut() {
            obj.insert("worker_revision".into(), json!(worker_rev));
        }
        Ok(parsed)
    }

    fn kill_group(&mut self) {
        let pgid = self.child.id() as i32;
        unsafe {
            libc::killpg(pgid, libc::SIGTERM);
        }
        std::thread::sleep(Duration::from_millis(250));
        if matches!(self.child.try_wait(), Ok(None)) {
            unsafe {
                libc::killpg(pgid, libc::SIGKILL);
            }
            let _ = self.child.wait();
        }
    }
}

impl Drop for KernelBridge {
    fn drop(&mut self) {
        self.kill_group();
    }
}

struct Supervisor {
    route: WorkerRoute,
    worker_rev: String,
    cache_dir: String,
    python: String,
    kernel_path: PathBuf,
    kernel_sha: Option<String>,
    plan: LowMemExecutionPlan,
    bridge: Mutex<Option<KernelBridge>>,
}

impl Supervisor {
    fn dispatch(&self, envelope: &Value) -> Result<Value, String> {
        let validated = validate_job(self.route, envelope, &self.plan)?;
        log_event(
            "request_validated",
            "info",
            &self.worker_rev,
            self.route,
            json!({ "tier": self.plan.tier, "offload": self.plan.diffusion_offload }),
        );
        let started = Instant::now();
        let mut guard = self.bridge.lock().map_err(|_| "Mutex poisoned")?;
        if guard.is_none() {
            *guard = Some(KernelBridge::spawn(
                &self.python,
                &self.kernel_path,
                self.kernel_sha.as_deref(),
                &self.cache_dir,
                &self.plan,
            )?);
        }
        let res = guard.as_mut().unwrap().execute(&validated, &self.worker_rev);
        if res.is_err() {
            // Reset kernel process group on error or OOM so VRAM returns cleanly to the driver.
            if let Some(mut b) = guard.take() {
                b.kill_group();
            }
        }
        let output = res?;
        log_event(
            "request_completed",
            "info",
            &self.worker_rev,
            self.route,
            json!({ "duration_ms": started.elapsed().as_millis() as u64 }),
        );
        Ok(output)
    }
}

fn handle_http_conn(mut stream: TcpStream, sup: &Supervisor) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(15)));
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut req_line = String::new();
    if reader.read_line(&mut req_line).unwrap_or(0) == 0 {
        return;
    }
    let parts: Vec<&str> = req_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let (method, path) = (parts[0], parts[1]);
    let mut content_len = 0usize;
    let mut chunked = false;
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).unwrap_or(0) == 0 || h == "\r\n" || h == "\n" {
            break;
        }
        let lower = h.to_ascii_lowercase();
        if let Some(rest) = lower.strip_prefix("content-length:") {
            content_len = rest.trim().parse().unwrap_or(0);
        } else if lower.starts_with("transfer-encoding:") {
            chunked = true;
        }
    }

    let respond = |stream: &mut TcpStream, code: u16, status: &str, body: Value| {
        let bytes = serde_json::to_vec(&body).unwrap_or_default();
        let hdr = format!(
            "HTTP/1.1 {code} {status}\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        );
        let _ = stream.write_all(hdr.as_bytes());
        let _ = stream.write_all(&bytes);
    };

    if method == "GET" && path == "/health" {
        respond(&mut stream, 200, "OK", json!({"status":"process-ready","tier":sup.plan.tier}));
        return;
    }
    if method != "POST" || (path != "/v1/images/generations" && path != "/v1/infer") {
        respond(&mut stream, 404, "Not Found", json!({"error":"not found"}));
        return;
    }
    if chunked || content_len == 0 || content_len > 2_000_000 {
        respond(&mut stream, 413, "Payload Too Large", json!({"error":"invalid body size"}));
        return;
    }
    let mut body = vec![0u8; content_len];
    if reader.read_exact(&mut body).is_err() {
        respond(&mut stream, 400, "Bad Request", json!({"error":"incomplete body"}));
        return;
    }
    let parsed: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => {
            respond(&mut stream, 400, "Bad Request", json!({"error":"invalid json"}));
            return;
        }
    };
    let envelope = if parsed.get("input").is_some() {
        parsed
    } else {
        json!({ "input": parsed })
    };
    match sup.dispatch(&envelope) {
        Ok(out) => respond(&mut stream, 200, "OK", out),
        Err(err) => {
            log_event("request_failed", "error", &sup.worker_rev, sup.route, json!({"error": err}));
            respond(&mut stream, 400, "Bad Request", json!({"error":"invalid or failed request"}));
        }
    }
}

fn runpod_queue_loop(sup: &Supervisor, get_url: &str, post_base: &str) {
    let token = std::env::var("RUNPOD_AI_API_KEY").unwrap_or_default();
    loop {
        let out = Command::new("curl")
            .args(["-sS", "--max-time", "30", "-H", &format!("Authorization: {token}"), get_url])
            .output();
        let Ok(resp) = out else {
            std::thread::sleep(Duration::from_millis(500));
            continue;
        };
        if resp.stdout.is_empty() {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        }
        let Ok(job) = serde_json::from_slice::<Value>(&resp.stdout) else {
            std::thread::sleep(Duration::from_millis(250));
            continue;
        };
        let Some(job_id) = job.get("id").and_then(Value::as_str) else {
            std::thread::sleep(Duration::from_millis(200));
            continue;
        };
        let payload = match sup.dispatch(&job) {
            Ok(result) => json!({ "output": result }),
            Err(err) => {
                log_event("request_failed", "error", &sup.worker_rev, sup.route, json!({"error": err}));
                json!({ "error": err })
            }
        };
        let post_url = format!("{}/{}", post_base.trim_end_matches('/'), job_id);
        if let Ok(mut child) = Command::new("curl")
            .args([
                "-sS", "--max-time", "30", "-X", "POST",
                "-H", &format!("Authorization: {token}"),
                "-H", "Content-Type: application/json",
                "--data-binary", "@-",
                &post_url,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
        {
            if let Some(mut sin) = child.stdin.take() {
                let _ = serde_json::to_writer(&mut sin, &payload);
            }
            let _ = child.wait();
        }
    }
}

fn main() {
    let route_raw = std::env::var("WORKER_ROUTE").unwrap_or_else(|_| "chroma".into());
    let route = WorkerRoute::from_env(&route_raw).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2);
    });
    let worker_rev = std::env::var("WORKER_REVISION").unwrap_or_else(|_| "rust-0.2.0".into());
    let gpu = probe_best_gpu();
    let plan = plan_for_budget(gpu.as_ref(), None, route.estimated_weight_bytes());
    let (cache_dir, persistent) = resolve_cache_dir();

    if std::env::args().any(|a| a == "--probe") {
        println!("{}", serde_json::to_string_pretty(&json!({
            "route": route,
            "gpu": gpu,
            "low_mem_plan": plan,
            "cache_dir": cache_dir,
            "cache_persistent": persistent,
        })).unwrap());
        return;
    }

    log_event(
        "worker_started",
        "info",
        &worker_rev,
        route,
        json!({
            "controller": "rust",
            "gpu_backend": gpu.as_ref().map(|g| g.backend).unwrap_or("cpu"),
            "gpu_device": gpu.as_ref().map(|g| g.device_name.as_str()).unwrap_or("none"),
            "gpu_budget_bytes": plan.effective_budget_bytes,
            "memory_tier": plan.tier,
            "diffusion_offload": plan.diffusion_offload,
            "vae_tiling": plan.vae_tiling,
            "cache_dir": cache_dir,
            "cache_persistent": persistent,
        }),
    );

    let sup = Supervisor {
        route,
        worker_rev,
        cache_dir,
        python: std::env::var("SYNDICATE_KERNEL_PYTHON").unwrap_or_else(|_| "python3".into()),
        kernel_path: PathBuf::from(
            std::env::var("SYNDICATE_KERNEL_PATH").unwrap_or_else(|_| "/worker/tensor_kernel.py".into()),
        ),
        kernel_sha: std::env::var("KERNEL_SHA256").ok(),
        plan,
        bridge: Mutex::new(None),
    };

    if let (Ok(get_url), Ok(post_url)) = (
        std::env::var("RUNPOD_WEBHOOK_GET_JOB"),
        std::env::var("RUNPOD_WEBHOOK_POST_OUTPUT"),
    ) {
        runpod_queue_loop(&sup, &get_url, &post_url);
        return;
    }

    let host = if std::env::var_os("PORT").is_some() { "0.0.0.0" } else { "127.0.0.1" };
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let listener = TcpListener::bind((host, port)).expect("Failed to bind HTTP port");
    for conn in listener.incoming().flatten() {
        handle_http_conn(conn, &sup);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use syndicate_gpu_lowmem::GIB;

    #[test]
    fn http_server_validates_in_rust_and_bridges_minimal_kernel() {
        let tmp = std::env::temp_dir().join(format!("syndicate-gpu-worker-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let mock_kernel = tmp.join("mock_kernel.py");
        std::fs::write(
            &mock_kernel,
            "import json,sys\nfor line in sys.stdin:\n req=json.loads(line)\n print(json.dumps({'data':[{'b64_json':'Zml4dHVyZQ=='}],'tier':req['low_mem']['tier']}),flush=True)\n",
        )
        .unwrap();

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let sup = std::sync::Arc::new(Supervisor {
            route: WorkerRoute::Chroma,
            worker_rev: "test-rev".into(),
            cache_dir: tmp.to_string_lossy().into_owned(),
            python: "python3".into(),
            kernel_path: mock_kernel,
            kernel_sha: None,
            plan: plan_for_budget(None, Some(10 * GIB), 26 * GIB),
            bridge: Mutex::new(None),
        });
        let sup2 = sup.clone();
        let thr = std::thread::spawn(move || {
            for _ in 0..2 {
                if let Ok((s, _)) = listener.accept() {
                    handle_http_conn(s, &sup2);
                }
            }
        });

        // 1. Invalid model rejected in Rust before kernel ever spawns
        let mut c1 = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let bad = b"{\"model\":\"evil/model\",\"prompt\":\"vase\",\"negative_prompt\":\"blur\",\"width\":512,\"height\":512}";
        write!(
            c1,
            "POST /v1/images/generations HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
            bad.len()
        )
        .unwrap();
        c1.write_all(bad).unwrap();
        let mut resp1 = String::new();
        c1.read_to_string(&mut resp1).unwrap();
        assert!(resp1.starts_with("HTTP/1.1 400"));
        assert!(sup.bridge.lock().unwrap().is_none(), "kernel must not spawn on invalid input");

        // 2. Valid request passes low_mem tier to minimal kernel shim
        let mut c2 = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let good = b"{\"model\":\"syndicate/chroma-hd\",\"prompt\":\"vase\",\"negative_prompt\":\"blur\",\"width\":512,\"height\":512,\"seed\":1,\"n\":1,\"response_format\":\"b64_json\"}";
        write!(
            c2,
            "POST /v1/images/generations HTTP/1.1\r\nHost: localhost\r\nContent-Length: {}\r\n\r\n",
            good.len()
        )
        .unwrap();
        c2.write_all(good).unwrap();
        let mut resp2 = String::new();
        c2.read_to_string(&mut resp2).unwrap();
        assert!(resp2.starts_with("HTTP/1.1 200"));
        assert!(resp2.contains("Zml4dHVyZQ=="));
        assert!(resp2.contains("constrained"));

        let _ = thr.join();
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
