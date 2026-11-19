//! Telemetry for agent operations.
//!
//! Every run, model turn, tool call, delegation and mission becomes one finished span, appended to
//! `shared/runtime/telemetry/spans-YYYYMMDD.jsonl`. Span names and attributes follow the OpenTelemetry
//! GenAI conventions where they exist (`invoke_agent`, `chat`, `execute_tool`; `gen_ai.agent.id`,
//! `gen_ai.usage.input_tokens`, `gen_ai.tool.name` ...) and use a `harness.*` namespace for the rest.
//! The GenAI agent conventions are still marked "Development" upstream, so treat the attribute names as
//! aligned-with, not certified-against, the spec.
//!
//! PRIVACY BY CONSTRUCTION. A span never carries task text, answers, tool arguments or results, file
//! contents, or error messages. Only structure and numbers: ids, names, durations, token counts, status
//! words, an error *class*, and a short hash of the objective so repeats can be correlated.
//!
//! HONESTY. Durations come from the harness's own measurements. Child-span *start offsets* inside a run
//! are reconstructed from those durations, and every such span says so (`harness.timing_source`).
//! An agent with no recorded runs is reported `never_run` — never given invented numbers.
//!
//! Token totals are summed from `chat` spans only; `invoke_agent` spans carry per-run totals for display.
//! (Summing both double counts — the upstream conventions warn about the same trap.)
use crate::jobs::{now, ROLES};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u64 = 1;
/// A trace with more spans than this looks like a runaway loop.
pub const RUNAWAY_SPANS: usize = 60;
/// Tool error rate above this (with enough calls) marks an agent degraded.
pub const TOOL_ERR_RATE: f64 = 0.25;
const MIN_CALLS_FOR_RATE: u64 = 4;

pub fn dir(root: &Path) -> PathBuf {
    root.join("shared/runtime/telemetry")
}

// ---------- time ----------
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}
fn day_name(t: f64) -> String {
    let (y, m, d) = civil((t / 86_400.0).floor() as i64);
    format!("{y:04}{m:02}{d:02}")
}

// ---------- identity ----------
fn h(s: &str) -> String {
    hex::encode(&Sha256::digest(s.as_bytes())[..8])
}

/// The topmost run directory directly under a `runs/` folder; groups a mission's planner, children and follow-ups.
pub fn trace_of(run_dir: &Path) -> String {
    for a in run_dir.ancestors() {
        if a.parent().and_then(|p| p.file_name()).map(|n| n == "runs").unwrap_or(false) {
            return a.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        }
    }
    run_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "run".into())
}

/// `syndicates/<name>/agents/<role>` -> `<name>`; base roles -> `base`.
pub fn syndicate_of(root: &Path, role: &str) -> String {
    if let Some(d) = crate::agent::role_dir(root, role) {
        let comps: Vec<String> = d.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
        if let Some(i) = comps.iter().rposition(|c| c == "syndicates") {
            if let Some(n) = comps.get(i + 1) {
                return n.clone();
            }
        }
    }
    "base".into()
}

fn status_of(run_status: &str) -> &'static str {
    match run_status {
        "model_finished" => "ok",
        s if s.contains("budget") => "budget",
        "cancelled" => "cancelled",
        _ => "error",
    }
}

fn span(trace: &str, id: &str, parent: Option<&str>, name: &str, kind: &str, start: f64, dur_ms: f64, status: &str,
        syndicate: &str, agent: &str, attrs: Json) -> Json {
    json!({"v": SCHEMA_VERSION, "trace_id": trace, "span_id": id, "parent_id": parent, "name": name, "kind": kind,
           "start_unix": (start * 1000.0).round() / 1000.0, "duration_ms": (dur_ms * 100.0).round() / 100.0,
           "status": status, "syndicate": syndicate, "agent": agent, "attrs": attrs})
}

// ---------- storage ----------
pub fn append(root: &Path, spans: &[Json]) {
    if spans.is_empty() {
        return;
    }
    let d = dir(root);
    if fs::create_dir_all(&d).is_err() {
        return;
    }
    let mut by_day: BTreeMap<String, String> = BTreeMap::new();
    for s in spans {
        let e = by_day.entry(day_name(s["start_unix"].as_f64().unwrap_or_else(now))).or_default();
        e.push_str(&s.to_string());
        e.push('\n');
    }
    for (day, text) in by_day {
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(d.join(format!("spans-{day}.jsonl"))) {
            let _ = f.write_all(text.as_bytes());
        }
    }
}

pub fn read_spans(root: &Path, since: f64, until: f64) -> Vec<Json> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir(root)).into_iter().flatten().flatten().map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with("spans-") && n.ends_with(".jsonl")).unwrap_or(false)).collect();
    files.sort();
    let mut out = vec![];
    for f in files {
        for line in fs::read_to_string(&f).unwrap_or_default().lines() {
            if let Ok(s) = serde_json::from_str::<Json>(line) {
                let t = s["start_unix"].as_f64().unwrap_or(0.0);
                if t >= since && t <= until {
                    out.push(s);
                }
            }
        }
    }
    out
}

fn ingested_path(root: &Path) -> PathBuf {
    dir(root).join("ingested.txt")
}
fn is_ingested(root: &Path, p: &Path) -> bool {
    fs::read_to_string(ingested_path(root)).map(|t| t.lines().any(|l| l == p.to_string_lossy())).unwrap_or(false)
}
fn mark_ingested(root: &Path, p: &Path) {
    let _ = fs::create_dir_all(dir(root));
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(ingested_path(root)) {
        let _ = writeln!(f, "{}", p.to_string_lossy());
    }
}

// ---------- resource + reward digests ----------
/// (attributes, estimated model load seconds) from `<run_dir>/model.resources.json` and `memory-reclaim.json`.
fn resource_digest(run_dir: &Path) -> (Json, Option<f64>) {
    let mut a = serde_json::Map::new();
    let mut load = None;
    if let Some(r) = fs::read_to_string(run_dir.join("model.resources.json")).ok().and_then(|t| serde_json::from_str::<Json>(&t).ok()) {
        let samples = r["samples"].as_array().cloned().unwrap_or_default();
        let g = |s: &Json, k: &str| s[k].as_f64().unwrap_or(0.0);
        if !samples.is_empty() {
            a.insert("harness.resource.rss_peak_bytes".into(), json!(samples.iter().map(|s| g(s, "worker_rss_bytes")).fold(0.0, f64::max)));
            a.insert("harness.resource.available_min_bytes".into(), json!(samples.iter().map(|s| g(s, "available_bytes")).fold(f64::MAX, f64::min)));
            a.insert("harness.resource.pressure_max".into(), json!(samples.iter().map(|s| g(s, "pressure_level")).fold(0.0, f64::max)));
            a.insert("harness.resource.pressure_warn_samples".into(), json!(samples.iter().filter(|s| g(s, "pressure_level") != 1.0).count()));
            a.insert("harness.resource.swap_growth_bytes".into(), json!(g(&samples[samples.len() - 1], "swap_used_bytes") - g(&samples[0], "swap_used_bytes")));
            let t0 = g(&samples[0], "time");
            load = samples.iter().find(|s| g(s, "worker_rss_bytes") > 1_073_741_824.0).map(|s| g(s, "time") - t0).filter(|d| *d > 0.0);
        }
        a.insert("harness.resource.guard_status".into(), r["status"].clone());
    }
    if let Some(r) = fs::read_to_string(run_dir.join("memory-reclaim.json")).ok().and_then(|t| serde_json::from_str::<Json>(&t).ok()) {
        a.insert("harness.reclaim.quit_count".into(), json!(r["quit_apps"].as_array().map(|x| x.len()).unwrap_or(0)));
        a.insert("harness.reclaim.target_met".into(), r["target_met"].clone());
    }
    (Json::Object(a), load)
}

fn memq_state(reward: &Json) -> &'static str {
    let m = &reward["memq"];
    if m["written"] == json!(true) {
        "written"
    } else {
        let why = m["reason"].as_str().unwrap_or("");
        if why.contains("memq_write_enabled is off") { "off" } else if why.contains("no verified") { "unavailable" } else if m.is_null() { "off" } else { "failed" }
    }
}

fn reward_attrs(reward: &Json, into: &mut serde_json::Map<String, Json>) {
    if !reward.is_object() {
        return;
    }
    for (k, src) in [("kind", "kind"), ("value", "value"), ("confidence", "confidence"), ("factual", "factual_confidence"),
                     ("alignment", "alignment_confidence"), ("auto", "auto_reward"), ("overclaim", "overclaim_flagged"),
                     ("declared", "criteria_declared")] {
        into.insert(format!("harness.reward.{k}"), reward[src].clone());
    }
    into.insert("harness.reward.non_negotiables_met".into(), reward["non_negotiables"]["all_met"].clone());
    into.insert("harness.memq.state".into(), json!(memq_state(reward)));
}

fn error_class(resp: &Json) -> String {
    let e = resp["error"].as_str().unwrap_or("");
    match e.split_once(':') {
        Some((c, _)) if c.len() <= 48 && c.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_') => c.to_string(),
        _ => "error".into(),
    }
}

// ---------- recording ----------
/// Derive and append the spans for one finished agent run. Returns how many spans were written.
/// `data_root` is where spans are stored; `src_root` is where roles/syndicates are resolved.
pub fn record_run(data_root: &Path, src_root: &Path, run_dir: &Path, record: &Json, model: &str, officiated: Option<bool>, end_unix: f64) -> usize {
    let role = record["role"].as_str().unwrap_or("unknown");
    let syn = syndicate_of(src_root, role);
    let trace = trace_of(run_dir);
    let run_id = format!("run-{}", h(&run_dir.to_string_lossy()));
    let parent = run_dir.file_name().and_then(|n| n.to_str()).and_then(|n| n.strip_prefix("message-")).map(|id| format!("dispatch-{id}"));
    let turns = record["turns"].as_array().cloned().unwrap_or_default();
    let tools = record["tools"].as_array().cloned().unwrap_or_default();
    let total_s = record["seconds"].as_f64().unwrap_or(0.0);
    let start0 = end_unix - total_s;
    let (res, load_s) = resource_digest(run_dir);
    let mut spans = vec![];
    let mut t = start0;
    if let Some(l) = load_s {
        spans.push(span(&trace, &format!("{run_id}-load"), Some(&run_id), "model_load", "model_load", t, l * 1000.0, "ok", &syn, role,
                        json!({"gen_ai.request.model": model, "harness.timing_source": "estimated_from_resource_samples"})));
        t += l;
    }
    let (mut tin, mut tout, mut tool_idx, mut tool_errors) = (0u64, 0u64, 0usize, 0u64);
    let tool_span = |idx: usize, ev: &Json, at: f64| -> (Json, f64, bool) {
        let name = ev["tool"].as_str().unwrap_or("unknown");
        let ok = ev["response"]["ok"] == json!(true);
        let secs = ev["seconds"].as_f64().unwrap_or(0.0);
        let mut attrs = json!({"gen_ai.operation.name": "execute_tool", "gen_ai.tool.name": name, "harness.tool.ok": ok,
                               "harness.timing_source": "derived_offset"});
        if !ok {
            attrs["harness.tool.error_class"] = json!(error_class(&ev["response"]));
        }
        (span(&trace, &format!("{run_id}-x{idx}"), Some(&run_id), &format!("execute_tool {name}"), "execute_tool", at, secs * 1000.0,
              if ok { "ok" } else { "error" }, &syn, role, attrs), secs, ok)
    };
    for (i, turn) in turns.iter().enumerate() {
        let tm = &turn["timings"];
        let dur = tm["prompt_ms"].as_f64().unwrap_or(0.0) + tm["predicted_ms"].as_f64().unwrap_or(0.0);
        let (pi, po) = (turn["usage"]["prompt_tokens"].as_u64().unwrap_or(0), turn["usage"]["completion_tokens"].as_u64().unwrap_or(0));
        tin += pi;
        tout += po;
        spans.push(span(&trace, &format!("{run_id}-t{i}"), Some(&run_id), "chat", "chat", t, dur,
                        if turn["finish_reason"] == "length" { "budget" } else { "ok" }, &syn, role,
                        json!({"gen_ai.operation.name": "chat", "gen_ai.request.model": model, "gen_ai.usage.input_tokens": pi,
                               "gen_ai.usage.output_tokens": po, "harness.turn": i + 1, "harness.decode_tps": tm["predicted_per_second"],
                               "harness.prompt_tps": tm["prompt_per_second"], "harness.finish_reason": turn["finish_reason"],
                               "harness.timing_source": "derived_offset"})));
        t += dur / 1000.0;
        let ncalls = turn["message"]["tool_calls"].as_array().map(|a| a.len()).unwrap_or(0);
        for _ in 0..ncalls {
            if let Some(ev) = tools.get(tool_idx) {
                let (s, secs, ok) = tool_span(tool_idx, ev, t);
                tool_errors += u64::from(!ok);
                spans.push(s);
                t += secs;
                tool_idx += 1;
            }
        }
    }
    while let Some(ev) = tools.get(tool_idx) {
        let (s, secs, ok) = tool_span(tool_idx, ev, t);
        tool_errors += u64::from(!ok);
        spans.push(s);
        t += secs;
        tool_idx += 1;
    }
    let run_status = record["status"].as_str().unwrap_or("unknown");
    let mut attrs = serde_json::Map::new();
    for (k, v) in [("gen_ai.operation.name", json!("invoke_agent")), ("gen_ai.agent.id", json!(role)), ("gen_ai.request.model", json!(model)),
                   ("gen_ai.usage.input_tokens", json!(tin)), ("gen_ai.usage.output_tokens", json!(tout)),
                   ("harness.run.status", json!(run_status)), ("harness.run.turns", json!(turns.len())),
                   ("harness.run.tool_calls", json!(tools.len())), ("harness.run.tool_errors", json!(tool_errors)),
                   ("harness.run.yield", record["yield_reason"].clone()), ("harness.controller", record["controller"].clone()),
                   ("harness.objective_hash", json!(&h(record["objective"].as_str().unwrap_or(""))[..12])),
                   ("harness.officiated", json!(officiated)),
                   ("harness.run_dir", json!(run_dir.strip_prefix(src_root).unwrap_or(run_dir).to_string_lossy()))] {
        attrs.insert(k.into(), v);
    }
    if let Some(o) = res.as_object() {
        for (k, v) in o {
            attrs.insert(k.clone(), v.clone());
        }
    }
    reward_attrs(&record["reward"], &mut attrs);
    // The agent's own status notes become structural facts on the span: counts and the worst risk
    // level, never the free-text progress/uncertainty strings (which may describe task specifics).
    if let Some(reports) = record["status_reports"].as_array() {
        let worst = reports.iter().filter_map(|r| r["risk"].as_str()).map(|r| match r { "high" => 3, "medium" => 2, "low" => 1, _ => 0 }).max().unwrap_or(0);
        attrs.insert("harness.self_report.count".into(), json!(reports.len()));
        attrs.insert("harness.self_report.max_risk".into(), json!(["none", "low", "medium", "high"][worst]));
        attrs.insert("harness.self_report.flagged_uncertainty".into(), json!(reports.iter().any(|r| !r["uncertainty"].as_str().unwrap_or("").is_empty())));
    }
    spans.insert(0, span(&trace, &run_id, parent.as_deref(), &format!("invoke_agent {role}"), "invoke_agent", start0, total_s * 1000.0,
                         status_of(run_status), &syn, role, Json::Object(attrs)));
    let n = spans.len();
    append(data_root, &spans);
    mark_ingested(data_root, &run_dir.join("result.json"));
    n
}

/// One span for a delegation (an envelope run by the dispatcher).
pub fn record_dispatch(data_root: &Path, src_root: &Path, run_dir: &Path, env: &Json, end_unix: f64) {
    let to = env["recipient"].as_str().unwrap_or("unknown");
    let secs = env["result"]["seconds"].as_f64().unwrap_or(0.0);
    let id = env["id"].as_str().unwrap_or("");
    let status = env["status"].as_str().unwrap_or("unknown");
    let s = span(&trace_of(run_dir), &format!("dispatch-{id}"), None, &format!("dispatch {to}"), "dispatch", end_unix - secs, secs * 1000.0,
                 if status == "completed" { "ok" } else { "error" }, &syndicate_of(src_root, to), to,
                 json!({"harness.dispatch.from": env["sender"], "harness.dispatch.to": to, "harness.dispatch.status": status,
                        "harness.dispatch.needs_followup": env["needs_followup"]}));
    append(data_root, &[s]);
}

/// One span for a whole mission (planner + children + follow-ups), carrying the mission-level reward.
pub fn record_mission(data_root: &Path, src_root: &Path, run_dir: &Path, fin: &Json, end_unix: f64) {
    let role = fin["role"].as_str().unwrap_or("coordinator");
    let secs = fin["seconds"].as_f64().unwrap_or(0.0);
    let kids = fin["child_results"].as_array().cloned().unwrap_or_default();
    let mut attrs = serde_json::Map::new();
    for (k, v) in [("gen_ai.agent.id", json!(role)), ("harness.mission.status", fin["status"].clone()), ("harness.mission.children", json!(kids.len())),
                   ("harness.mission.children_failed", json!(kids.iter().filter(|c| c["status"] != "completed").count())),
                   ("harness.objective_hash", json!(&h(fin["objective"].as_str().unwrap_or(""))[..12])),
                   ("harness.run_dir", json!(run_dir.strip_prefix(src_root).unwrap_or(run_dir).to_string_lossy()))] {
        attrs.insert(k.into(), v);
    }
    reward_attrs(&fin["reward"], &mut attrs);
    let trace = trace_of(run_dir);
    append(data_root, &[span(&trace, &format!("mission-{}", h(&run_dir.to_string_lossy())), None, &format!("mission {role}"), "mission",
                             end_unix - secs, secs * 1000.0, status_of(fin["status"].as_str().unwrap_or("")), &syndicate_of(src_root, role), role, Json::Object(attrs))]);
    mark_ingested(data_root, &run_dir.join("result.json"));
}

// ---------- backfill ----------
fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth == 0 {
        return;
    }
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, depth - 1, out);
        } else if p.file_name().map(|n| n == "result.json" || n == "dispatch.json").unwrap_or(false) {
            out.push(p);
        }
    }
}

/// Ingest finished runs that predate telemetry (or were produced by other launchers). Idempotent.
pub fn backfill(data_root: &Path, src_root: &Path, runs_dir: &Path) -> Json {
    let mut files = vec![];
    walk(runs_dir, 6, &mut files);
    files.sort();
    let (mut runs, mut missions, mut dispatches, mut skipped) = (0, 0, 0, 0);
    for f in files {
        if is_ingested(data_root, &f) {
            skipped += 1;
            continue;
        }
        let Ok(rec) = fs::read_to_string(&f).map_err(|e| e.to_string()).and_then(|t| serde_json::from_str::<Json>(&t).map_err(|e| e.to_string())) else { continue };
        let run_dir = f.parent().unwrap_or(runs_dir);
        let end = f.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs_f64()).unwrap_or_else(now);
        if f.file_name().map(|n| n == "dispatch.json").unwrap_or(false) {
            for env in rec["processed"].as_array().into_iter().flatten() {
                record_dispatch(data_root, src_root, run_dir, env, end);
                dispatches += 1;
            }
            mark_ingested(data_root, &f);
        } else if rec["child_results"].is_array() {
            record_mission(data_root, src_root, run_dir, &rec, end);
            missions += 1;
        } else if rec["role"].is_string() && rec["status"].is_string() {
            record_run(data_root, src_root, run_dir, &rec, "qwen3.5-9b-q4_k_m", None, end);
            runs += 1;
        }
    }
    json!({"ingested": {"runs": runs, "missions": missions, "dispatches": dispatches}, "already_ingested": skipped})
}

// ---------- roster (the structure, independent of activity) ----------
pub fn roster(src_root: &Path) -> Json {
    let rd = |p: PathBuf| -> Json { fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Json::Null) };
    let mut out = vec![];
    let mut dirs: Vec<PathBuf> = fs::read_dir(src_root.join("syndicates")).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    for d in dirs {
        let m = rd(d.join("syndicate.json"));
        if m.is_null() {
            continue;
        }
        let name = d.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let members: Vec<Json> = m["agent_ids"].as_array().into_iter().flatten().filter_map(|a| a.as_str()).map(|id| {
            let adir = d.join("agents").join(id);
            let c = rd(adir.join("config.json"));
            json!({"agent_id": id, "role_title": c["role_title"], "persona_name": c["persona"]["name"], "persona_title": c["persona"]["title"],
                   "officiated": crate::syndicate::officiation(&adir).is_some(), "officiation": crate::syndicate::officiation(&adir),
                   "max_turns": c["max_turns"], "timeout_seconds": c["timeout_seconds"],
                   "allowed_tools": c["allowed_tools"], "allowed_hosts": c["allowed_hosts"]})
        }).collect();
        out.push(json!({"id": name, "syndicate_id": m["syndicate_id"], "display_name": m["display_name"], "mission": m["mission"], "purpose": m["purpose"],
                        "coordinator": m["coordinator_agent_id"], "officiation_enabled": m.pointer("/officiation/enabled") != Some(&json!(false)),
                        "state": m["state"], "max_dispatch_rounds": m["max_dispatch_rounds"], "members": members, "edges": m["edges"], "loops": m["loops"]}));
    }
    out.push(json!({"id": "base", "syndicate_id": "base", "display_name": "Base roles", "mission": "The built-in local workers.", "coordinator": "coordinator",
                    "officiation_enabled": false, "members": ROLES.iter().map(|r| json!({"agent_id": r, "officiated": false})).collect::<Vec<_>>(), "edges": [], "loops": []}));
    Json::Array(out)
}

// ---------- rollups ----------
fn pct(v: &mut [f64], p: f64) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let rank = ((p / 100.0) * v.len() as f64).ceil().max(1.0) as usize;
    Some(v[rank.min(v.len()) - 1])
}
fn round(x: f64) -> f64 {
    (x * 100.0).round() / 100.0
}
fn opt(x: Option<f64>) -> Json {
    x.map(|v| json!(round(v))).unwrap_or(Json::Null)
}

#[derive(Default)]
struct Run { t: f64, dur: f64, status: String, turns: u64, tools: u64, tool_errors: u64, reward: String, value: f64, conf: f64, auto: bool, overclaim: bool,
             memq: String, officiated: Option<bool> }
#[derive(Default)]
struct Acc { runs: Vec<Run>, tools: BTreeMap<String, (u64, u64, Vec<f64>)>, tin: u64, tout: u64, tps: Vec<f64>, rss_peak: f64, avail_min: Option<f64>,
             warn_runs: u64, reclaim_runs: u64, missions: u64, missions_ok: u64 }

/// Summarize spans in `[since, until]`. `syndicate` filters to one syndicate id when given.
pub fn summarize(data_root: &Path, src_root: &Path, since: f64, until: f64, syndicate: Option<&str>) -> Json {
    let all: Vec<Json> = read_spans(data_root, since, until).into_iter().filter(|s| syndicate.map(|x| s["syndicate"] == x).unwrap_or(true)).collect();
    let mut accs: BTreeMap<(String, String), Acc> = BTreeMap::new();
    let mut per_trace: BTreeMap<String, usize> = BTreeMap::new();
    let mut edges: BTreeMap<(String, String), (u64, u64, Vec<f64>)> = BTreeMap::new();
    let (mut dispatches, mut dispatches_ok) = (0u64, 0u64);
    for s in &all {
        *per_trace.entry(s["trace_id"].as_str().unwrap_or("").to_string()).or_default() += 1;
        let key = (s["syndicate"].as_str().unwrap_or("base").to_string(), s["agent"].as_str().unwrap_or("unknown").to_string());
        let a = &s["attrs"];
        let acc = accs.entry(key).or_default();
        match s["kind"].as_str().unwrap_or("") {
            "invoke_agent" => {
                let g = |k: &str| a[k].as_f64().unwrap_or(0.0);
                if g("harness.resource.pressure_warn_samples") > 0.0 { acc.warn_runs += 1; }
                if g("harness.reclaim.quit_count") > 0.0 { acc.reclaim_runs += 1; }
                if let Some(v) = a["harness.resource.rss_peak_bytes"].as_f64() { acc.rss_peak = acc.rss_peak.max(v); }
                if let Some(v) = a["harness.resource.available_min_bytes"].as_f64() { acc.avail_min = Some(acc.avail_min.map(|m| m.min(v)).unwrap_or(v)); }
                acc.runs.push(Run { t: s["start_unix"].as_f64().unwrap_or(0.0), dur: s["duration_ms"].as_f64().unwrap_or(0.0), status: s["status"].as_str().unwrap_or("").into(),
                    turns: a["harness.run.turns"].as_u64().unwrap_or(0), tools: a["harness.run.tool_calls"].as_u64().unwrap_or(0), tool_errors: a["harness.run.tool_errors"].as_u64().unwrap_or(0),
                    reward: a["harness.reward.kind"].as_str().unwrap_or("").into(), value: g("harness.reward.value"), conf: g("harness.reward.confidence"),
                    auto: a["harness.reward.auto"] == json!(true), overclaim: a["harness.reward.overclaim"] == json!(true), memq: a["harness.memq.state"].as_str().unwrap_or("").into(),
                    officiated: a["harness.officiated"].as_bool() });
            }
            "chat" => {
                acc.tin += a["gen_ai.usage.input_tokens"].as_u64().unwrap_or(0);
                acc.tout += a["gen_ai.usage.output_tokens"].as_u64().unwrap_or(0);
                if let Some(v) = a["harness.decode_tps"].as_f64().filter(|v| *v > 0.0) { acc.tps.push(v); }
            }
            "execute_tool" => {
                let e = acc.tools.entry(a["gen_ai.tool.name"].as_str().unwrap_or("unknown").to_string()).or_default();
                e.0 += 1;
                e.1 += u64::from(s["status"] != "ok");
                e.2.push(s["duration_ms"].as_f64().unwrap_or(0.0));
            }
            "dispatch" => {
                dispatches += 1;
                dispatches_ok += u64::from(s["status"] == "ok");
                let e = edges.entry((a["harness.dispatch.from"].as_str().unwrap_or("").into(), a["harness.dispatch.to"].as_str().unwrap_or("").into())).or_default();
                e.0 += 1;
                e.1 += u64::from(s["status"] == "ok");
                e.2.push(s["duration_ms"].as_f64().unwrap_or(0.0));
            }
            "mission" => {
                acc.missions += 1;
                acc.missions_ok += u64::from(s["status"] == "ok");
            }
            _ => {}
        }
    }
    let roster = roster(src_root);
    // every rostered agent appears, even with no runs
    for syn in roster.as_array().into_iter().flatten() {
        if syndicate.map(|x| syn["id"] != x).unwrap_or(false) { continue; }
        for m in syn["members"].as_array().into_iter().flatten() {
            accs.entry((syn["id"].as_str().unwrap_or("").into(), m["agent_id"].as_str().unwrap_or("").into())).or_default();
        }
    }
    let (mut agents, mut signals) = (vec![], vec![]);
    let mut fleet = json!({"runs": 0, "ok": 0, "error": 0, "budget": 0, "cancelled": 0, "tool_calls": 0, "tool_errors": 0, "tokens_in": 0, "tokens_out": 0,
                           "reward": {"auto": 0, "held": 0, "penalty": 0, "provisional": 0, "none": 0}, "overclaims": 0,
                           "memq": {"written": 0, "failed": 0, "off": 0, "unavailable": 0}});
    let mut all_tps: Vec<f64> = vec![];
    let bump = |f: &mut Json, path: &str, by: u64| { let c = f.pointer(path).and_then(Json::as_u64).unwrap_or(0); *f.pointer_mut(path).unwrap() = json!(c + by); };
    for ((syn, agent), acc) in accs.iter_mut() {
        acc.runs.sort_by(|a, b| a.t.partial_cmp(&b.t).unwrap_or(std::cmp::Ordering::Equal));
        let n = acc.runs.len() as u64;
        let count = |st: &str| acc.runs.iter().filter(|r| r.status == st).count() as u64;
        let (ok, err, bud, can) = (count("ok"), count("error"), count("budget"), count("cancelled"));
        let calls: u64 = acc.runs.iter().map(|r| r.tools).sum();
        let terr: u64 = acc.runs.iter().map(|r| r.tool_errors).sum();
        let rw = |k: &str| acc.runs.iter().filter(|r| r.reward == k).count() as u64;
        let (auto, held, pen, prov) = (acc.runs.iter().filter(|r| r.auto).count() as u64, rw("held_for_review"), rw("penalty"), rw("provisional"));
        let overclaims = acc.runs.iter().filter(|r| r.overclaim).count() as u64;
        let mut durs: Vec<f64> = acc.runs.iter().map(|r| r.dur).collect();
        let rated: Vec<&Run> = acc.runs.iter().filter(|r| !r.reward.is_empty()).collect();
        let last3_all_error = n >= 3 && acc.runs.iter().rev().take(3).all(|r| r.status == "error");
        let rate_bad = calls >= MIN_CALLS_FOR_RATE && (terr as f64 / calls as f64) > TOOL_ERR_RATE;
        let status = if n == 0 { "never_run" } else if last3_all_error || (n >= 3 && (ok as f64 / n as f64) < 0.5) { "failing" }
            else if err > 0 || bud > 0 || overclaims > 0 || pen > 0 || rate_bad { "degraded" } else { "healthy" };
        let mut push = |sev: &str, code: &str, text: String| signals.push(json!({"severity": sev, "syndicate": syn, "agent": agent, "code": code, "text": text}));
        if overclaims > 0 { push("warn", "overclaim", format!("{overclaims} run(s) claimed success that the evidence did not back")); }
        if held > 0 { push("info", "held_for_review", format!("{held} reward episode(s) held below the 95% bar, waiting for a grader or you")); }
        if pen > 0 { push("bad", "penalty", format!("{pen} run(s) penalised; lessons recorded")); }
        if rate_bad { push("warn", "tool_error_rate", format!("tool error rate {:.0}% over {calls} calls", terr as f64 / calls as f64 * 100.0)); }
        if bud > 0 { push("warn", "budget_exhausted", format!("{bud} run(s) ran out of turn/output budget")); }
        if acc.warn_runs > 0 { push("info", "memory_pressure", format!("memory pressure warned during {} run(s)", acc.warn_runs)); }
        let tools_json: BTreeMap<String, Json> = acc.tools.iter_mut().map(|(k, (c, e, ms))| (k.clone(), json!({"calls": *c, "errors": *e, "p95_ms": opt(pct(ms, 95.0))}))).collect();
        agents.push(json!({"syndicate": syn, "agent": agent, "status": status, "runs": n, "ok": ok, "error": err, "budget": bud, "cancelled": can,
            "success_rate": if n > 0 { json!(round(ok as f64 / n as f64)) } else { Json::Null },
            "latency_ms": {"p50": opt(pct(&mut durs, 50.0)), "p95": opt(pct(&mut durs, 95.0)), "max": opt(durs.iter().cloned().fold(None, |m: Option<f64>, v| Some(m.map(|x| x.max(v)).unwrap_or(v))))},
            "turns_avg": if n > 0 { json!(round(acc.runs.iter().map(|r| r.turns).sum::<u64>() as f64 / n as f64)) } else { Json::Null },
            "tool_calls": calls, "tool_errors": terr, "tools": tools_json,
            "tokens": {"in": acc.tin, "out": acc.tout}, "decode_tps_avg": if acc.tps.is_empty() { Json::Null } else { json!(round(acc.tps.iter().sum::<f64>() / acc.tps.len() as f64)) },
            "reward": {"rated": rated.len(), "auto": auto, "held": held, "penalty": pen, "provisional": prov, "overclaims": overclaims,
                       "avg_value": if rated.is_empty() { Json::Null } else { json!(round(rated.iter().map(|r| r.value).sum::<f64>() / rated.len() as f64)) },
                       "avg_confidence": if rated.is_empty() { Json::Null } else { json!(round(rated.iter().map(|r| r.conf).sum::<f64>() / rated.len() as f64)) }},
            "resource": {"rss_peak_bytes": if acc.rss_peak > 0.0 { json!(acc.rss_peak) } else { Json::Null }, "available_min_bytes": acc.avail_min, "pressure_warn_runs": acc.warn_runs, "reclaim_runs": acc.reclaim_runs},
            "missions": acc.missions, "missions_ok": acc.missions_ok,
            "last_run_unix": acc.runs.last().map(|r| r.t), "officiated": acc.runs.last().and_then(|r| r.officiated),
            "runs_series": series(&acc.runs, since, until)}));
        for (k, v) in [("runs", n), ("ok", ok), ("error", err), ("budget", bud), ("cancelled", can), ("tool_calls", calls), ("tool_errors", terr), ("tokens_in", acc.tin), ("tokens_out", acc.tout), ("overclaims", overclaims)] {
            bump(&mut fleet, &format!("/{k}"), v);
        }
        for (k, v) in [("auto", auto), ("held", held), ("penalty", pen), ("provisional", prov)] { bump(&mut fleet, &format!("/reward/{k}"), v); }
        bump(&mut fleet, "/reward/none", n - rated.len() as u64);
        for r in &acc.runs { if !r.memq.is_empty() { bump(&mut fleet, &format!("/memq/{}", r.memq), 1); } }
        all_tps.extend(acc.tps.iter());
    }
    fleet["success_rate"] = if fleet["runs"].as_u64().unwrap_or(0) > 0 { json!(round(fleet["ok"].as_f64().unwrap_or(0.0) / fleet["runs"].as_f64().unwrap_or(1.0))) } else { Json::Null };
    fleet["decode_tps_avg"] = if all_tps.is_empty() { Json::Null } else { json!(round(all_tps.iter().sum::<f64>() / all_tps.len() as f64)) };
    fleet["dispatches"] = json!({"total": dispatches, "ok": dispatches_ok});
    for (trace, n) in &per_trace {
        if *n > RUNAWAY_SPANS { signals.push(json!({"severity": "warn", "code": "runaway_trace", "agent": null, "syndicate": null, "text": format!("trace {trace} has {n} spans (limit {RUNAWAY_SPANS}); possible runaway loop")})); }
    }
    let edge_list: Vec<Json> = edges.iter_mut().map(|((f, t), (c, ok, ms))| json!({"from": f, "to": t, "count": *c, "ok": *ok, "p50_ms": opt(pct(ms, 50.0))})).collect();
    // recent runs + their spans, for the trace view
    let mut recent: Vec<&Json> = all.iter().filter(|s| s["kind"] == "invoke_agent").collect();
    recent.sort_by(|a, b| b["start_unix"].as_f64().partial_cmp(&a["start_unix"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));
    recent.truncate(50);
    let recent_ids: Vec<&str> = recent.iter().map(|s| s["trace_id"].as_str().unwrap_or("")).collect();
    let mut traces: BTreeMap<String, Vec<Json>> = BTreeMap::new();
    for s in &all {
        let t = s["trace_id"].as_str().unwrap_or("");
        if recent_ids.contains(&t) { let v = traces.entry(t.to_string()).or_default(); if v.len() < 200 { v.push(s.clone()); } }
    }
    let sev = |s: &Json| match s["severity"].as_str() { Some("bad") => 0, Some("warn") => 1, _ => 2 };
    signals.sort_by_key(sev);
    json!({"schema_version": SCHEMA_VERSION, "generated_unix": now(), "window": {"since": since, "until": until},
           "fleet": fleet, "agents": agents, "edges": edge_list, "signals": signals, "recent_runs": recent, "traces": traces, "roster": roster,
           "rules": {"status": "never_run: no runs in the window. failing: last 3 runs all errored, or under half succeeded (3+ runs). degraded: any error, budget stop, overclaim, penalty, or tool error rate above 25% (4+ calls). Otherwise healthy.",
                     "reward": "auto = both confidence scores at or above the bar, every non-negotiable verified, nothing overclaimed, intent declared. Below the bar = held for review.",
                     "tokens": "Summed from model-turn spans only, to avoid double counting.",
                     "timing": "Durations are measured. Start offsets of turn and tool spans inside a run are reconstructed from those durations.",
                     "runaway": format!("A trace with more than {RUNAWAY_SPANS} spans is flagged.")}})
}

/// 24 equal buckets across the window: `{runs, ok}` per bucket (counts only — no smoothing, no invention).
fn series(runs: &[Run], since: f64, until: f64) -> Json {
    let lo = if since > 0.0 { since } else { runs.first().map(|r| r.t).unwrap_or(0.0) };
    let hi = until.min(now()).max(lo + 1.0);
    let w = (hi - lo) / 24.0;
    let mut b = vec![(0u64, 0u64); 24];
    for r in runs {
        let i = (((r.t - lo) / w).floor() as i64).clamp(0, 23) as usize;
        b[i].0 += 1;
        b[i].1 += u64::from(r.status == "ok");
    }
    json!({"start": lo, "end": hi, "runs": b.iter().map(|x| x.0).collect::<Vec<_>>(), "ok": b.iter().map(|x| x.1).collect::<Vec<_>>()})
}

/// `24h`, `7d`, `90m`, or `all` -> unix seconds to subtract from now (None = all time).
pub fn parse_window(s: &str) -> Option<f64> {
    let s = s.trim();
    if s == "all" || s.is_empty() { return None; }
    let (n, unit) = s.split_at(s.len() - 1);
    let n: f64 = n.parse().ok()?;
    Some(n * match unit { "m" => 60.0, "h" => 3600.0, "d" => 86_400.0, _ => return None })
}
