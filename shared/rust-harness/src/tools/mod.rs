//! The Armory: native port of shared/harness/tools.py.
//!
//!   model tool call ──► execute(name, args)
//!        │  parse JSON, validate against the tool's schema (strict, no extras)
//!        ▼
//!   file / shell / messages / skills / workers / MemQ / browser (CDP) / desktop (Quartz)
//!        │  every slow step polls `cancelled()`, so SIGTERM or the run deadline
//!        ▼  stops a tool mid-call (child process groups are killed, Chrome is closed)
//!   {"ok": true, "result": ...} | {"ok": false, "error": "Type: message"}  + event log
//!
//! Same tool names, schemas and result shapes as the Python class. Workspace path
//! checks are not an OS sandbox.
pub mod browser;
pub mod desktop;
pub mod schema;

use crate::jobs::{now, prerequisites, root};
use serde_json::{json, Map, Value as Json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::io::Read;
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// A tool failure, carrying the Python exception name the model has always seen.
#[derive(Debug, Clone)]
pub struct ToolError(pub String);

pub type R<T> = Result<T, ToolError>;

pub fn err<T>(kind: &str, msg: impl std::fmt::Display) -> R<T> {
    Err(ToolError(format!("{kind}: {msg}")))
}

/// Raised when the run is cancelled or out of time; the agent loop turns it into the run status.
pub const CANCELLED: &str = "KeyboardInterrupt: Agent termination requested";

fn spec(name: &str, description: &str, properties: Json, required: Option<Vec<&str>>) -> Json {
    let req: Vec<String> = match required {
        Some(r) => r.into_iter().map(String::from).collect(),
        None => properties
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default(),
    };
    json!({"type": "function", "function": {"name": name, "description": description,
        "parameters": {"type": "object", "properties": properties, "required": req, "additionalProperties": false}}})
}

const CAPABILITIES: [(&str, &str, &str); 6] = [
    ("coordinator", "Qwen3.5 9B Q4", "Plan tasks, use memory and skills, route subtasks and combine receipts."),
    ("cyber-lab", "Qwen3.5 9B Q4", "Authorized local security analysis, code inspection and command-line tasks."),
    ("browser", "Qwen3.5 9B Q4", "Browser navigation, forms and page inspection on explicitly allowed hosts."),
    ("desktop", "Qwen3.5 9B Q4 + vision projector", "Observe screenshots and operate native macOS controls; OS permissions required."),
    ("image-studio", "FLUX.2 Klein 4B Q4 + ponpoke local encoder", "Generate or reference-edit a still image. draft=512px, quality=768px; one local reference up to 768px. Returns PNG and resource/validation evidence."),
    ("video-studio", "Wan 1.3B e14 + UMT5 Q8", "Generate a short text-to-video clip. draft=320x192/33 frames; quality=384x224/33; detail-short=512x288/17. No image-to-video. Returns MP4 and resource/validation evidence."),
];

/// worker_catalog.prompt()
pub fn worker_prompt() -> String {
    format!(
        "Worker routes: {}",
        CAPABILITIES
            .iter()
            .map(|(r, _, c)| format!("{r}: {c}"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

pub struct Tools {
    pub role: String,
    pub workspace: PathBuf,
    pub run_dir: PathBuf,
    pub task: String,
    pub root: PathBuf,
    pub allowed_hosts: BTreeSet<String>,
    pub schemas: Vec<Json>,
    pub events: Vec<Json>,
    pub desktop_window_title: Option<String>,
    pub observed_desktop_pids: HashSet<i64>,
    pub desktop: Box<dyn desktop::Desktop>,
    pub cancelled: Box<dyn Fn() -> bool>,
    /// Python interpreter used only for the MemQ OAuth client (memq_call).
    pub memq_python: PathBuf,
    browser: Option<browser::Browser>,
    by_name: Map<String, Json>,
}

impl Tools {
    pub fn new(
        role: &str,
        workspace: &Path,
        run_dir: &Path,
        allowed_hosts: &[String],
        task: &str,
    ) -> Result<Tools, String> {
        Tools::with_root(root(), role, workspace, run_dir, allowed_hosts, task)
    }

    /// Like `new`, with the harness root given explicitly (tests use a temporary root).
    pub fn with_root(
        r: PathBuf,
        role: &str,
        workspace: &Path,
        run_dir: &Path,
        allowed_hosts: &[String],
        task: &str,
    ) -> Result<Tools, String> {
        fs::create_dir_all(workspace).map_err(|e| format!("workspace: {e}"))?;
        let workspace = workspace.canonicalize().map_err(|e| e.to_string())?;
        let s = json!({"type": "string"});
        // Syndicate members get the tools their config grants (see syndicate::grants); base roles are chosen by role, as before.
        let member = crate::syndicate::membership(&r, role);
        let grants = member.as_ref().map(|m| m.grants());
        let named = |sp: &Json| {
            sp["function"]["name"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        };
        let ws_specs = vec![
            spec(
                "read_file",
                "Read a UTF-8 file relative to the workspace.",
                json!({"path": s}),
                None,
            ),
            spec(
                "write_file",
                "Write a UTF-8 file relative to the workspace.",
                json!({"path": s, "content": s}),
                Some(vec!["path", "content"]),
            ),
            spec("list_files", "List workspace files.", json!({}), None),
        ];
        let mut schemas: Vec<Json> = match &grants {
            Some(g) => ws_specs
                .into_iter()
                .filter(|sp| g.workspace.contains(&named(sp)))
                .collect(),
            None => ws_specs,
        };
        if role == "cyber-lab" || grants.as_ref().map(|g| g.exec).unwrap_or(false) {
            schemas.push(spec(
                "run_command",
                "Execute an argv command in the workspace (no shell). This is not an OS sandbox.",
                json!({"argv": {"type": "array", "items": s, "minItems": 1, "maxItems": 30}}),
                None,
            ));
        }
        let browser_specs = vec![
            spec(
                "browser_open",
                "Open an authorized URL in a fresh, isolated Chrome context.",
                json!({"url": s}),
                None,
            ),
            spec(
                "browser_observe",
                "Read current URL, visible text and interactive element attributes.",
                json!({}),
                None,
            ),
            spec(
                "browser_click",
                "Click one element using a CSS selector observed in browser_observe.",
                json!({"selector": s}),
                None,
            ),
            spec(
                "browser_fill",
                "Fill a form field identified by an observed CSS selector.",
                json!({"selector": s, "text": s}),
                Some(vec!["selector", "text"]),
            ),
            spec(
                "browser_screenshot",
                "Observe a screenshot of the current page.",
                json!({}),
                None,
            ),
        ];
        if role == "browser" {
            schemas.extend(browser_specs);
        } else if let Some(g) = &grants {
            schemas.extend(
                browser_specs
                    .into_iter()
                    .filter(|sp| g.browser.contains(&named(sp))),
            );
        }
        if role == "desktop" {
            schemas.extend([
                spec("desktop_observe", "List up to 12 native windows and permissions. Optional query filters observed app names or window titles, including other Spaces. Narrow the query if results are truncated.",
                     json!({"query": {"type": "string", "maxLength": 200}}), Some(vec![])),
                spec("desktop_screenshot", "Capture the main display. Requires macOS screen recording permission.", json!({}), None),
                spec("desktop_click", "Click an observed point. Use normalized_1000 for Qwen visual coordinates (0–1000 across the full screenshot), or explicit screen_points.",
                     json!({"x": {"type": "number", "minimum": 0}, "y": {"type": "number", "minimum": 0}, "coordinate_space": {"type": "string", "enum": ["screen_points", "normalized_1000"]}}), Some(vec!["x", "y"])),
                spec("desktop_type", "Type text into the currently focused native control.", json!({"text": s}), None),
                spec("desktop_key", "Press an allowed native shortcut/key.", json!({"key": {"type": "string", "enum": ["Return", "Tab", "Escape", "cmd+s", "cmd+a", "cmd+n"]}}), None),
            ]);
        }
        let mut send = spec("send_message", "Queue a local task. For image-studio/video-studio, message is the generation prompt. Set needs_followup only when further reasoning depends on the output. End this planning turn to release GPU memory for dispatch; execution is not immediate. Media presets: draft or quality; video also has detail-short. Image reference editing accepts one absolute local reference_image path (at most 768 pixels per side).",
            json!({"recipient": {"type": "string", "enum": ["coordinator", "cyber-lab", "browser", "desktop", "image-studio", "video-studio"]}, "message": s, "needs_followup": {"type": "boolean"},
                   "media_preset": {"type": "string", "enum": ["draft", "quality", "detail-short"]}, "reference_image": {"type": "string", "maxLength": 4096}}),
            Some(vec!["recipient", "message"]));
        match &member {
            Some(m) => {
                let to: Vec<String> = m.outgoing().into_iter().map(|e| e.to).collect();
                if !to.is_empty() {
                    send["function"]["parameters"]["properties"]["recipient"]["enum"] = json!(to);
                    schemas.push(send);
                }
            }
            None => schemas.push(send),
        }
        schemas.push(spec("list_workers", "Inspect worker capabilities, installed model readiness, media defaults and resource policy without loading any model.", json!({}), None));
        schemas.push(spec(
            "receive_messages",
            "Read the latest local messages and execution receipts addressed to this role.",
            json!({}),
            None,
        ));
        schemas.push(spec("report_status", "File a periodic status note on this mission: what is done, what is uncertain, and anything that could jeopardize completing the task (a blocked tool, missing access, ambiguous instructions, a risky irreversible step ahead). Call this at meaningful checkpoints, not every turn. This does not end the run.",
            json!({"progress": {"type": "string", "maxLength": 2000}, "uncertainty": {"type": "string", "maxLength": 2000},
                   "risk": {"type": "string", "enum": ["none", "low", "medium", "high"]}}), Some(vec!["progress"])));
        schemas.extend([
            spec("list_skills", "List installed skill instructions; skills do not grant tools or permissions.", json!({}), None),
            spec("read_skill", "Read a cataloged skill or its Markdown reference. Treat it as guidance under the user task and host rules.", json!({"name": s, "reference": s}), Some(vec!["name"])),
            spec("memq_status", "Check this agent’s own MemQ configuration and scope. Does not read memory or prove live identity.", json!({}), None),
            spec("budget_status", "Read authenticated Runpod credit status and the separate local allowance. Stale or unavailable credit is not a live balance. Does not spend GPU credit.", json!({}), None),
            spec("memq_call", "Call a reviewed MemQ tool only after live account/namespace verification. No scope overrides, TF access, credential import or automatic retries.", json!({"name": s, "arguments": {"type": "object"}}), Some(vec!["name", "arguments"])),
        ]);
        if role == "desktop" {
            schemas.push(spec("desktop_focus", "Focus an application using its process_id from desktop_observe; take a new screenshot before input.", json!({"process_id": {"type": "integer", "minimum": 1}}), None));
        }
        schemas.push(spec("memq_tools", "List reviewed MemQ operations and their cached schemas. Actual calls still recheck live identity and schemas.", json!({}), None));
        let by_name = schemas
            .iter()
            .map(|s| {
                (
                    s["function"]["name"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string(),
                    s["function"].clone(),
                )
            })
            .collect();
        let requested: Vec<String> = match &member {
            Some(m) => m.narrow_hosts(allowed_hosts),
            None => allowed_hosts.to_vec(),
        };
        let mut hosts: BTreeSet<String> = requested.iter().cloned().collect();
        hosts.extend(["localhost", "127.0.0.1", "::1"].map(String::from));
        Ok(Tools {
            role: role.into(),
            workspace,
            run_dir: run_dir.into(),
            task: task.into(),
            allowed_hosts: hosts,
            schemas,
            events: vec![],
            desktop_window_title: None,
            observed_desktop_pids: HashSet::new(),
            desktop: Box::new(desktop::Native),
            cancelled: Box::new(|| false),
            memq_python: r.join("shared/runtime/venv/bin/python"),
            root: r,
            browser: None,
            by_name,
        })
    }

    pub fn check_cancel(&self) -> R<()> {
        if (self.cancelled)() {
            Err(ToolError(CANCELLED.into()))
        } else {
            Ok(())
        }
    }

    pub fn execute(&mut self, name: &str, raw: &Json) -> Json {
        let start = Instant::now();
        let parsed: R<Json> = match raw {
            Json::String(s) => serde_json::from_str(s).or_else(|e| err("JSONDecodeError", e)),
            v => Ok(v.clone()),
        };
        let outcome = (|| -> R<Json> {
            let spec = self
                .by_name
                .get(name)
                .cloned()
                .ok_or_else(|| ToolError(format!("ValueError: Unknown tool: {name}")))?;
            let args = parsed.clone()?;
            schema::validate(&spec["parameters"], &args)
                .map_err(|e| ToolError(format!("ValidationError: {e}")))?;
            self.check_cancel()?;
            self.dispatch(name, args.as_object().cloned().unwrap_or_default())
        })();
        let result = match outcome {
            Ok(v) => json!({"ok": true, "result": v}),
            Err(ToolError(e)) => json!({"ok": false, "error": e}),
        };
        let recorded = parsed.unwrap_or_else(|_| raw.clone());
        self.events.push(json!({"tool": name, "arguments": recorded, "response": result,
                                "seconds": (start.elapsed().as_secs_f64() * 1000.0).round() / 1000.0}));
        result
    }

    fn dispatch(&mut self, name: &str, a: Map<String, Json>) -> R<Json> {
        let s = |k: &str| {
            a.get(k)
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_string()
        };
        match name {
            "read_file" => self.read_file(&s("path")),
            "write_file" => self.write_file(&s("path"), &s("content")),
            "list_files" => Ok(self.list_files()),
            "run_command" => self.run_command(
                &a["argv"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap_or_default().to_string())
                    .collect::<Vec<_>>(),
            ),
            "send_message" => self.send_message(
                &s("recipient"),
                &s("message"),
                a.get("needs_followup")
                    .and_then(Json::as_bool)
                    .unwrap_or(false),
                a.get("media_preset")
                    .and_then(Json::as_str)
                    .unwrap_or("draft"),
                a.get("reference_image").and_then(Json::as_str),
            ),
            "receive_messages" => self.receive_messages(),
            "list_workers" => Ok(self.list_workers()),
            "list_skills" => self.list_skills(),
            "read_skill" => self.read_skill(
                &s("name"),
                a.get("reference")
                    .and_then(Json::as_str)
                    .unwrap_or("SKILL.md"),
            ),
            "memq_status" => self.memq_status(),
            "memq_tools" => self.memq_tools(),
            "budget_status" => self.budget_status(),
            "memq_call" => self.memq_call(&s("name"), &a["arguments"]),
            "report_status" => self.report_status(
                &s("progress"),
                a.get("uncertainty").and_then(Json::as_str).unwrap_or(""),
                a.get("risk").and_then(Json::as_str).unwrap_or("none"),
            ),
            n if n.starts_with("browser_") => self.browser_tool(n, &a),
            n if n.starts_with("desktop_") => self.desktop_tool(n, &a),
            _ => err("ValueError", format!("Unknown tool: {name}")),
        }
    }

    // ---------- files ----------

    /// Python Path.resolve(): resolve symlinks of existing parts, `..` lexically for the rest.
    pub fn path(&self, rel: &str) -> R<PathBuf> {
        let p = resolve(&self.workspace.join(rel));
        if !p.starts_with(&self.workspace) {
            return err("ValueError", "Path is outside the selected workspace");
        }
        Ok(p)
    }

    fn read_file(&self, rel: &str) -> R<Json> {
        let p = self.path(rel)?;
        let bytes = fs::read(&p).or_else(|e| io_err(e, &p))?;
        let text = String::from_utf8(bytes).or_else(|e| err("UnicodeDecodeError", e))?;
        Ok(json!(text.chars().take(16000).collect::<String>()))
    }

    fn write_file(&self, rel: &str, content: &str) -> R<Json> {
        let p = self.path(rel)?;
        if let Some(d) = p.parent() {
            fs::create_dir_all(d).or_else(|e| io_err(e, d))?;
        }
        fs::write(&p, content).or_else(|e| io_err(e, &p))?;
        Ok(
            json!({"path": p.to_string_lossy(), "bytes": fs::metadata(&p).map(|m| m.len()).unwrap_or(0)}),
        )
    }

    fn list_files(&self) -> Json {
        let mut out = vec![];
        let mut stack = vec![self.workspace.clone()];
        while let Some(dir) = stack.pop() {
            if out.len() >= 200 || crate::agent::cancelled() {
                break;
            }
            let mut entries: Vec<PathBuf> = fs::read_dir(&dir)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .collect();
            entries.sort();
            for p in entries.into_iter().rev() {
                if out.len() >= 200 || crate::agent::cancelled() {
                    break;
                }
                let link = fs::symlink_metadata(&p)
                    .map(|m| m.file_type().is_symlink())
                    .unwrap_or(true);
                if p.is_dir() {
                    if !link {
                        stack.push(p);
                    }
                } else if p.is_file() {
                    out.push(
                        p.strip_prefix(&self.workspace)
                            .unwrap_or(&p)
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
        out.sort();
        out.truncate(200);
        json!(out)
    }

    // ---------- shell ----------

    fn run_command(&self, argv: &[String]) -> R<Json> {
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..])
            .current_dir(&self.workspace)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let child = cmd.spawn().or_else(|e| io_err(e, Path::new(&argv[0])))?;
        let (code, timed_out, out, errs) = self.wait_cancellable(child, Duration::from_secs(30))?;
        let mut r =
            json!({"exit_code": code, "stdout": tail(&out, 12000), "stderr": tail(&errs, 4000)});
        if timed_out {
            r["timeout"] = json!(true);
        }
        Ok(r)
    }

    /// Wait for a child in its own process group; kill the group on timeout or cancel.
    /// Returns (exit code, timed out, stdout, stderr).
    pub fn wait_cancellable(
        &self,
        mut child: Child,
        limit: Duration,
    ) -> R<(Option<i32>, bool, String, String)> {
        let reader = |r: Option<Box<dyn Read + Send>>| {
            std::thread::spawn(move || {
            let mut b = vec![];
            if let Some(mut r) = r {
                let _ = r.read_to_end(&mut b);
            }
            String::from_utf8_lossy(&b).into_owned()
            })
        };
        let out = reader(
            child
                .stdout
                .take()
                .map(|s| Box::new(s) as Box<dyn Read + Send>),
        );
        let errs = reader(
            child
                .stderr
                .take()
                .map(|s| Box::new(s) as Box<dyn Read + Send>),
        );
        let deadline = Instant::now() + limit;
        let pgid = child.id() as i32;
        let mut timed_out = false;
        let status = loop {
            if let Ok(Some(s)) = child.try_wait() {
                break s;
            }
            let cancel = (self.cancelled)();
            if cancel || Instant::now() >= deadline {
                unsafe { libc::killpg(pgid, libc::SIGKILL) };
                let s = child
                    .wait()
                    .map_err(|e| ToolError(format!("OSError: {e}")))?;
                if cancel {
                    return Err(ToolError(CANCELLED.into()));
                }
                timed_out = true;
                break s;
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        // Grandchildren that kept the pipes open would block the readers forever.
        unsafe { libc::killpg(pgid, libc::SIGKILL) };
        use std::os::unix::process::ExitStatusExt;
        let code = status.code().or_else(|| status.signal().map(|s| -s));
        Ok((
            code,
            timed_out,
            out.join().unwrap_or_default(),
            errs.join().unwrap_or_default(),
        ))
    }

    // ---------- local messages ----------

    fn send_message(
        &self,
        recipient: &str,
        message: &str,
        needs_followup: bool,
        preset: &str,
        reference: Option<&str>,
    ) -> R<Json> {
        if recipient == self.role {
            return err(
                "ValueError",
                "Execute this role’s assignment directly; self-delegation is not supported.",
            );
        }
        if let Some(m) = crate::syndicate::membership(&self.root, &self.role) {
            if m.can_message(recipient).is_none() {
                return err(
                    "PermissionError",
                    &format!(
                        "No relationship allows {} to message {recipient}.",
                        self.role
                    ),
                );
            }
        }
        if recipient == "image-studio" || recipient == "video-studio" {
            crate::jobs::media_arguments(recipient, preset, reference)
                .map_err(|e| ToolError(format!("ValueError: {e}")))?;
        } else if preset != "draft" || reference.is_some() {
            return err("ValueError", "Media options require a media recipient");
        }
        let dir = crate::agent::inbox_dir(&self.root, recipient);
        fs::create_dir_all(&dir).or_else(|e| io_err(e, &dir))?;
        let id = crate::jobs::new_job_id().map_err(|e| ToolError(format!("OSError: {e}")))?;
        let envelope = json!({"id": id, "sender": self.role, "recipient": recipient, "message": message, "needs_followup": needs_followup,
            "status": "queued", "created_unix": now(), "allowed_hosts": self.allowed_hosts, "origin_task": self.task,
            "media_preset": preset, "reference_image": reference});
        let target = dir.join(format!("{id}.json"));
        fs::write(
            &target,
            serde_json::to_string_pretty(&envelope).unwrap_or_default(),
        )
        .or_else(|e| io_err(e, &target))?;
        Ok(
            json!({"message_id": id, "status": "queued", "path": target.to_string_lossy(), "note": "Recipient execution is separate; delivery is a local file only."}),
        )
    }

    fn receive_messages(&self) -> R<Json> {
        let dir = crate::agent::inbox_dir(&self.root, &self.role);
        fs::create_dir_all(&dir).or_else(|e| io_err(e, &dir))?;
        let mut files: Vec<(std::time::SystemTime, PathBuf)> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter_map(|p| {
                fs::metadata(&p)
                    .and_then(|m| m.modified())
                    .ok()
                    .map(|t| (t, p))
            })
            .collect();
        files.sort_by(|a, b| b.0.cmp(&a.0));
        files
            .into_iter()
            .take(20)
            .map(|(_, p)| {
            let t = fs::read_to_string(&p).or_else(|e| io_err(e, &p))?;
            serde_json::from_str(&t).or_else(|e| err("JSONDecodeError", e))
            })
            .collect::<R<Vec<Json>>>()
            .map(Json::Array)
    }

    /// Append a status note to <run_dir>/status-reports.jsonl. Durable, structured, and surfaced
    /// by `telemetry`/`report` as an uncertainty when risk is medium/high or uncertainty is non-empty.
    fn report_status(&self, progress: &str, uncertainty: &str, risk: &str) -> R<Json> {
        if !["none", "low", "medium", "high"].contains(&risk) {
            return err("ValueError", "risk must be none, low, medium or high");
        }
        let entry = json!({"at_unix": now(), "progress": progress, "uncertainty": uncertainty, "risk": risk});
        fs::create_dir_all(&self.run_dir).or_else(|e| io_err(e, &self.run_dir))?;
        let path = self.run_dir.join("status-reports.jsonl");
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .or_else(|e| io_err(e, &path))?;
        use std::io::Write;
        writeln!(f, "{entry}").map_err(|e| ToolError(e.to_string()))?;
        Ok(json!({"recorded": true, "path": path}))
    }

    // ---------- workers and skills ----------

    fn list_workers(&self) -> Json {
        let workers: Vec<Json> = CAPABILITIES.iter().map(|(role, model, capability)| {
            let cfg: Json = fs::read_to_string(self.root.join("agents").join(role).join("config.json")).ok()
                .and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(json!({}));
            let missing = prerequisites(&self.root, role);
            let tests: &[&str] = match *role {
                "coordinator" => &["communication", "lifecycle", "memory-agent", "media-routing"],
                "cyber-lab" => &["cyber"], "browser" => &["browser"], "desktop" => &["desktop", "vision"], _ => &[],
            };
            let read = |p: PathBuf| fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<Json>(&t).ok()).unwrap_or(json!({}));
            let mut qualification: Vec<Json> = tests.iter().map(|t| {
                let d = read(self.root.join("evidence/setup-20261102").join(format!("{t}-acceptance.json")));
                json!({"test": t, "passed": d.get("passed") == Some(&json!(true))})
            }).collect();
            if role.ends_with("-studio") {
                let d = read(self.root.join("agents").join(role).join("outputs/local-smoke.evidence.json"));
                qualification.push(json!({"test": "generation_and_visual_review", "passed": d.get("visual_inspection") == Some(&json!("passed"))}));
            }
            let defaults: Map<String, Json> = ["width", "height", "frames", "fps", "steps", "vision"].iter()
                .filter_map(|k| cfg.get(*k).map(|v| (k.to_string(), v.clone()))).collect();
            json!({"role": role, "model": model, "capability": capability, "installation_ready": missing.is_empty(), "missing": missing,
                   "last_recorded_qualification": qualification, "defaults": defaults})
        }).collect();
        let gib = |b: u64| ((b as f64 / (1u64 << 30) as f64) * 100.0).round() / 100.0;
        let resources = crate::runtime::Sample::take(None).map(|s| json!({"physical_memory_gib": gib(s.physical_bytes),
            "available_memory_gib": gib(s.available_bytes), "swap_used_gib": gib(s.swap_used_bytes)})).unwrap_or(json!({}));
        json!({"workers": workers, "resources": resources,
               "resource_policy": "One package-managed heavy model process at a time. Release the parent before dispatch. No idle model residency. Other applications and model servers are not controlled.",
               "readiness_is_not_task_acceptance": true})
    }

    fn skill_catalog(&self) -> R<Vec<Json>> {
        let p = self.root.join("shared/config/skills.json");
        let t = fs::read_to_string(&p).or_else(|e| io_err(e, &p))?;
        let v: Json = serde_json::from_str(&t).or_else(|e| err("JSONDecodeError", e))?;
        Ok(v["skills"].as_array().cloned().unwrap_or_default())
    }

    fn list_skills(&self) -> R<Json> {
        Ok(Json::Array(self.skill_catalog()?.iter().map(|s| json!({"name": s["name"], "description": s["description"], "capability_note": s["capability_note"]})).collect()))
    }

    fn read_skill(&self, name: &str, reference: &str) -> R<Json> {
        let entry = self
            .skill_catalog()?
            .into_iter()
            .find(|s| s["name"] == name)
            .ok_or_else(|| ToolError("ValueError: Skill is not in the installed catalog".into()))?;
        let main = self.root.join(entry["path"].as_str().unwrap_or_default());
        let dir = main.parent().map(resolve).unwrap_or_default();
        let target = resolve(&dir.join(reference));
        if !target.starts_with(&dir) || target.extension().map(|e| e != "md").unwrap_or(true) {
            return err(
                "ValueError",
                "Skill reference must be a Markdown file inside this skill",
            );
        }
        let main_bytes = fs::read(dir.join("SKILL.md")).or_else(|e| io_err(e, &main))?;
        if hex::encode(Sha256::digest(&main_bytes)) != entry["sha256"].as_str().unwrap_or_default()
        {
            return err(
                "ValueError",
                "Installed skill changed; refresh its reviewed manifest",
            );
        }
        let text = fs::read_to_string(&target).or_else(|e| io_err(e, &target))?;
        if text.chars().count() > 24000 {
            return err("ValueError", "Skill reference exceeds 24,000 characters");
        }
        let mut refs = vec![];
        let mut stack = vec![dir.clone()];
        while let Some(d) = stack.pop() {
            for p in fs::read_dir(&d)
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
            {
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|e| e == "md") {
                    refs.push(
                        p.strip_prefix(&dir)
                            .unwrap_or(&p)
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
        refs.sort();
        refs.truncate(80);
        Ok(
            json!({"name": name, "reference": reference, "text": text, "guidance_only": true, "available_references": refs}),
        )
    }

    // ---------- MemQ ----------
    // Status and cached schemas are config reads, done natively. Live calls need the
    // MCP OAuth (PKCE + streamable HTTP over TLS) client in memq_client.py, which stays
    // Python; it runs as a killable child so a call can still be cancelled.

    fn memq_config(&self) -> R<Json> {
        let p = self.root.join("shared/config/memq.json");
        let mut cfg: Json =
            serde_json::from_str(&fs::read_to_string(&p).or_else(|e| io_err(e, &p))?)
                .or_else(|e| err("JSONDecodeError", e))?;
        if cfg["binding"] == json!({"source": "own-client-private-binding"}) {
            let b = memq_private_dir().join("binding.json");
            cfg["binding"] = match fs::symlink_metadata(&b) {
                Err(_) => Json::Null,
                Ok(m) => {
                    use std::os::unix::fs::PermissionsExt;
                    if m.file_type().is_symlink() || m.permissions().mode() & 0o077 != 0 {
                        return err("PermissionError", "Binding file must be owner-only");
                    }
                    serde_json::from_str(&fs::read_to_string(&b).or_else(|e| io_err(e, &b))?)
                        .or_else(|e| err("JSONDecodeError", e))?
                }
            };
        }
        Ok(cfg)
    }

    /// memq_client.check_scope: Galileo/TF projects and tasks never use this connection.
    pub fn memq_check_scope(&self, cfg: &Json) -> R<()> {
        let project = resolve(&self.workspace);
        let mut paths = vec![project.clone()];
        if let Ok(o) = Command::new("git")
            .args([
                "-C",
                &project.to_string_lossy(),
                "rev-parse",
                "--path-format=absolute",
                "--git-common-dir",
            ])
            .stderr(Stdio::null())
            .output()
        {
            if o.status.success() {
                paths.push(resolve(Path::new(
                    String::from_utf8_lossy(&o.stdout).trim(),
                )));
            }
        }
        let blocked: Vec<PathBuf> = cfg["excluded_project_roots"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Json::as_str)
            .map(|p| resolve(Path::new(p)))
            .collect();
        if paths
            .iter()
            .any(|p| blocked.iter().any(|b| p.starts_with(b)))
        {
            return err("PermissionError", "Galileo/TF must use its separate verified Umbrage connection; Multinex access is blocked.");
        }
        if mentions_tf(&self.task) {
            return err("PermissionError", "Task mentions Galileo/TF; resolve its separate Umbrage scope before any memory access.");
        }
        Ok(())
    }

    fn budget_status(&self) -> R<Json> {
        let mut cmd = Command::new("node");
        cmd.arg(self.root.join("shared/inference-gateway/client.ts"))
            .arg("budget")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let child = cmd.spawn().map_err(|e| ToolError(e.to_string()))?;
        let (code, timed_out, out, _) = self.wait_cancellable(child, Duration::from_secs(15))?;
        if timed_out || code != Some(0) {
            return err(
                "RuntimeError",
                "Budget status unavailable; no GPU request submitted",
            );
        }
        serde_json::from_str(&out).map_err(|_| ToolError("Invalid budget status".into()))
    }

    fn memq_status(&self) -> R<Json> {
        let cfg = self.memq_config()?;
        self.memq_check_scope(&cfg)?;
        Ok(
            json!({"configured": cfg["enabled"], "expected_account": cfg["expected_account"], "namespace_organization": cfg["expected_namespace_organization"],
            "own_oauth_material_present": memq_private_dir().join("oauth-tokens.json").is_file(), "binding_configured": truthy(&cfg["binding"]),
            "live_identity_verified": false,
            "identity_check_scope": "Configuration-only status; each memq_call performs fresh identity and namespace verification before private access and after the response.",
            "allowed_tools": cfg["allowed_tools"], "other_client_credentials_used": false}),
        )
    }

    fn memq_tools(&self) -> R<Json> {
        let cfg = self.memq_config()?;
        self.memq_check_scope(&cfg)?;
        if !truthy(&cfg["binding"]) {
            return Ok(json!({"tools": [], "reason": "Verified binding not yet configured"}));
        }
        let cached = self.root.join("shared/runtime/memq/discovered-tools.json");
        let Ok(text) = fs::read_to_string(&cached) else {
            return Ok(json!({"tools": [], "reason": "Run client discovery first"}));
        };
        let allowed = cfg["allowed_tools"].as_array().cloned().unwrap_or_default();
        let v: Json = serde_json::from_str(&text).or_else(|e| err("JSONDecodeError", e))?;
        let tools: Vec<Json> = v["tools"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|t| allowed.contains(&t["name"]))
            .map(|t| {
            let mut schema = t["inputSchema"].clone();
            if let Some(p) = schema.get_mut("properties").and_then(Json::as_object_mut) {
                p.remove("namespace");
            }
            json!({"name": t["name"], "description": t["description"], "inputSchema": schema})
            })
            .collect();
        Ok(
            json!({"tools": tools, "cached_metadata_only": true, "live_identity_verified": false,
                  "identity_check_scope": "Schema discovery only; memq_call validates live identity and the current schema before executing."}),
        )
    }

    fn memq_call(&self, name: &str, arguments: &Json) -> R<Json> {
        let cfg = self.memq_config()?;
        self.memq_check_scope(&cfg)?;
        let script = self.root.join("shared/harness/memq_client.py");
        let mut cmd = Command::new(&self.memq_python);
        cmd.arg(&script)
            .args(["call", "--name", name, "--project"])
            .arg(&self.workspace)
            .args(["--task", &self.task, "--arguments", &arguments.to_string()])
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        let child = cmd.spawn().or_else(|e| io_err(e, &self.memq_python))?;
        let (code, timed_out, out, _) = self.wait_cancellable(child, Duration::from_secs(120))?;
        if timed_out {
            return err(
                "TimeoutError",
                "MemQ call exceeded 120 seconds; reconcile this operation before retrying",
            );
        }
        let reply: Json = out
            .lines()
            .rev()
            .find_map(|l| serde_json::from_str(l).ok())
            .ok_or_else(|| {
                ToolError(format!(
                    "RuntimeError: MemQ client exited ({code:?}) without a reply"
                ))
            })?;
        if reply["ok"] == json!(true) {
            Ok(reply["result"].clone())
        } else {
            Err(ToolError(
                reply["error"]
                    .as_str()
                    .unwrap_or("RuntimeError: MemQ call failed")
                    .into(),
            ))
        }
    }

    // ---------- browser ----------

    pub fn url_allowed(&self, url: &str) -> bool {
        browser::url_allowed(url, &self.allowed_hosts)
    }

    fn browser_tool(&mut self, name: &str, a: &Map<String, Json>) -> R<Json> {
        let s = |k: &str| {
            a.get(k)
                .and_then(Json::as_str)
                .unwrap_or_default()
                .to_string()
        };
        if name == "browser_open" {
            let url = s("url");
            if !self.url_allowed(&url) {
                return err("ValueError", "Host is not in the run allowlist");
            }
            if self.browser.is_none() {
                self.browser = Some(browser::Browser::launch(
                    &self.run_dir,
                    self.allowed_hosts.clone(),
                    &*self.cancelled,
                )?);
            }
        }
        let shot = self
            .run_dir
            .join(format!("browser-{}.png", self.events.len()));
        let cancelled = &*self.cancelled;
        let b = self
            .browser
            .as_mut()
            .ok_or_else(|| ToolError("ValueError: Open a page first".into()))?;
        match name {
            "browser_open" => {
                b.goto(&s("url"), cancelled)?;
                b.observe(cancelled)
            }
            "browser_observe" => b.observe(cancelled),
            "browser_click" => {
                b.click(&s("selector"), cancelled)?;
                b.observe(cancelled)
            }
            "browser_fill" => {
                b.fill(&s("selector"), &s("text"), cancelled)?;
                b.observe(cancelled)
            }
            "browser_screenshot" => {
                b.screenshot(&shot, cancelled)?;
                Ok(json!({"image_path": shot.to_string_lossy()}))
            }
            _ => err("ValueError", format!("Unknown tool: {name}")),
        }
    }

    pub fn close(&mut self) {
        if let Some(b) = self.browser.take() {
            b.close();
        }
    }
}

impl Drop for Tools {
    fn drop(&mut self) {
        self.close();
    }
}

pub fn memq_private_dir() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config/local-agents/memq")
}

/// re.search(r'\b(galileo|thermo\s*fisher|TF)\b', task, re.I)
pub fn mentions_tf(task: &str) -> bool {
    let words: Vec<String> = task
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect();
    if words
        .iter()
        .any(|w| w == "galileo" || w == "tf" || w == "thermofisher")
    {
        return true;
    }
    words
        .windows(2)
        .any(|w| w[0] == "thermo" && w[1] == "fisher")
}

fn truthy(v: &Json) -> bool {
    match v {
        Json::Null => false,
        Json::Bool(b) => *b,
        Json::Object(m) => !m.is_empty(),
        Json::Array(a) => !a.is_empty(),
        Json::String(s) => !s.is_empty(),
        Json::Number(n) => n.as_f64() != Some(0.0),
    }
}

fn tail(s: &str, n: usize) -> String {
    let count = s.chars().count();
    s.chars().skip(count.saturating_sub(n)).collect()
}

pub fn io_err<T>(e: std::io::Error, p: &Path) -> R<T> {
    let kind = match e.kind() {
        std::io::ErrorKind::NotFound => "FileNotFoundError",
        std::io::ErrorKind::PermissionDenied => "PermissionError",
        std::io::ErrorKind::IsADirectory => "IsADirectoryError",
        _ => "OSError",
    };
    err(kind, format!("{e}: '{}'", p.display()))
}

/// Non-strict resolve: follows symlinks for parts that exist, handles `..` lexically after.
pub fn resolve(p: &Path) -> PathBuf {
    let mut out = PathBuf::from("/");
    let mut hops = 0;
    let mut parts: Vec<std::ffi::OsString> = p
        .components()
        .filter_map(|c| match c {
        Component::Normal(s) => Some(s.to_os_string()),
        Component::ParentDir => Some("..".into()),
        _ => None,
        })
        .collect();
    parts.reverse();
    while let Some(part) = parts.pop() {
        if part == ".." {
            out.pop();
            continue;
        }
        out.push(&part);
        if let Ok(target) = fs::read_link(&out) {
            hops += 1;
            if hops > 40 {
                break;
            }
            out.pop();
            let mut rest: Vec<std::ffi::OsString> = target
                .components()
                .filter_map(|c| match c {
                Component::Normal(s) => Some(s.to_os_string()),
                Component::ParentDir => Some("..".into()),
                _ => None,
                })
                .collect();
            if target.is_absolute() {
                out = PathBuf::from("/");
            }
            rest.reverse();
            parts.extend(rest);
        }
    }
    out
}

#[cfg(test)]
mod tests;
