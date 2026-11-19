//! Syndicates: identity (officiation), policy (what a member may do) and configuration (validate / apply).
//!
//!   syndicate.json           mission, purpose, lore, officiation, ceilings, edges (relationships), loops
//!   agents/<id>/config.json  persona, role_title, allowed_tools, allowed_hosts, max_turns, timeout_seconds
//!
//! EVERYTHING HERE IS HONORED AT RUNTIME (that is the rule for the Forge UI: no decorative settings):
//!   * a relationship A -> B is the permission for A to message B (send_message is restricted to A's neighbours)
//!   * `constraints.max_rounds`, a loop's `max_iterations` and `max_dispatch_rounds` cap delegations (dispatch.rs)
//!   * `allowed_tools` is an allowlist for the member's tools; `allowed_hosts` narrows the browser hosts it may be given
//!   * `max_turns` / `timeout_seconds` bound its runs
//!   * relationship text, condition, order and caps are written into the member's identity block, so the agent is told
//!   * a removed member is archived, so it can no longer be run by name
//!
//! Identity text is stated as fact. It must not contain framing words such as "roleplay" or "pretend" (see `lint`).
use crate::jobs::now;
use serde_json::{json, Map, Value as Json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

pub const RELATIONS: [&str; 7] = ["commands", "feeds", "verifies", "challenges", "escalates_to", "hands_off", "reports_to"];
pub const BASE_ROLES: [&str; 6] = ["coordinator", "cyber-lab", "browser", "desktop", "image-studio", "video-studio"];
const MAX_LOOP_ITERATIONS: u64 = 10;
/// Words that would frame the identity as an act. Checked case-insensitively on identity text.
pub const FRAMING: [&str; 14] = ["roleplay", "role-play", "role play", "pretend", "fictional", "fiction", "in character",
    "in-character", "make-believe", "make believe", "act as", "simulate being", "imagine you are", "persona"];

fn s<'a>(v: &'a Json, k: &str) -> &'a str {
    v.get(k).and_then(Json::as_str).unwrap_or("")
}
fn strs(v: &Json, k: &str) -> Vec<String> {
    v.get(k).and_then(Json::as_array).into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect()
}
fn read_obj(p: &Path) -> Map<String, Json> {
    fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<Json>(&t).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default()
}
fn read_json(p: &Path) -> Json {
    fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or(Json::Null)
}
fn write_pretty(p: &Path, v: &Json) -> Result<(), String> {
    fs::write(p, serde_json::to_string_pretty(v).unwrap_or_default() + "\n").map_err(|e| format!("{}: {e}", p.display()))
}

/// Framing words found in `text` (lowercased matches, deduplicated).
pub fn lint(text: &str) -> Vec<String> {
    let t = text.to_lowercase();
    FRAMING.iter().filter(|w| t.contains(*w)).map(|w| w.to_string()).collect()
}

// ---------------------------------------------------------------- the guide (single source of truth for UI + AI prompts)
/// What each relationship means, what it enforces, and what the agent is told. Served to the console and the AI helpers.
/// The "told" lines are produced by `relation_line`, the same function that writes the agent's prompt, so they cannot drift.
pub fn guide() -> Json {
    // (id, label, short, meaning, use_when, example)
    let rows = [
        ("commands", "Commands", "A directs B", "A gives B its work and B takes direction from A. The classic manager-to-worker link.",
         "A decides what happens and B carries it out.", "The Watch Commander commands the Sentinel to review today's advisories."),
        ("feeds", "Feeds", "A's findings go to B", "A produces information B needs. Neither directs the other.",
         "One agent's output is another agent's input (intel to analysis).", "The Sentinel feeds the Cartographer new advisories so exposure can be matched."),
        ("verifies", "Verifies", "A checks B's work", "B's work is not finished until A has checked it.",
         "You want an independent check before something counts as done.", "The Auditor verifies the Cartographer's map by re-checking each exposure."),
        ("challenges", "Challenges", "A argues against B's plans", "A looks for the weak point in B's plan before B acts (a devil's advocate).",
         "A plan is risky or costly to get wrong.", "The Contrarian challenges the Regent's plan with its three strongest objections."),
        ("escalates_to", "Escalates to", "A hands B what is beyond A", "When A is out of its depth, blocked, or facing something irreversible, it goes to B.",
         "You need a safe way up the chain.", "The Auditor escalates any out-of-scope finding to the Commander."),
        ("hands_off", "Hands off to", "A passes finished work to B", "A completes its stage, then B continues. Builds an ordered pipeline.",
         "The work moves through stages in a fixed order.", "The Cartographer hands the prioritised exposure list off to the Auditor."),
        ("reports_to", "Reports to", "A reports to B", "A tells B what happened. The upward counterpart of 'commands'.",
         "A manager needs status or results from a worker.", "The Tender reports to the Keeper that the lamp is lit."),
    ];
    Json::Array(rows.iter().map(|(id, label, short, meaning, use_when, example)| json!({
        "id": id, "label": label, "short": short, "meaning": meaning, "use_when": use_when, "example": example,
        "enforces": "A may message B. B's result comes back to A as a receipt.",
        "told_to_a": relation_line(id, "B", false), "told_to_b": relation_line(id, "A", true)})).collect())
}

// ---------------------------------------------------------------- tools
/// (name, category, note). Categories: core (always on), workspace, exec, browser, desktop.
const TOOLS: [(&str, &str); 22] = [
    ("send_message", "core"), ("list_workers", "core"), ("receive_messages", "core"), ("list_skills", "core"), ("read_skill", "core"),
    ("memq_status", "core"), ("memq_tools", "core"), ("memq_call", "core"),
    ("read_file", "workspace"), ("write_file", "workspace"), ("list_files", "workspace"),
    ("run_command", "exec"),
    ("browser_open", "browser"), ("browser_observe", "browser"), ("browser_click", "browser"), ("browser_fill", "browser"), ("browser_screenshot", "browser"),
    ("desktop_observe", "desktop"), ("desktop_screenshot", "desktop"), ("desktop_click", "desktop"), ("desktop_type", "desktop"), ("desktop_key", "desktop"),
];

/// `list_dir` is the older spelling of `list_files`.
pub fn canonical_tool(name: &str) -> &str {
    if name == "list_dir" { "list_files" } else { name }
}
pub fn tool_category(name: &str) -> &'static str {
    let n = canonical_tool(name);
    if n == "desktop_focus" { return "desktop"; }
    TOOLS.iter().find(|(t, _)| *t == n).map(|(_, c)| *c).unwrap_or("unknown")
}

/// The tool catalog the console shows, with what each category needs in order to run.
pub fn tool_catalog() -> Json {
    let mut v: Vec<Json> = TOOLS.iter().map(|(t, c)| json!({"name": t, "category": c, "grantable": *c != "desktop" && *c != "core",
        "needs": match *c { "core" => "always available", "workspace" => "in the member's allowlist and the syndicate ceiling",
            "exec" => "allowlist + ceiling + the member's explicit exec acknowledgement (runs programs in its workspace; not network-sandboxed)",
            "browser" => "allowlist + ceiling, and hosts supplied for the task (hosts must be in the member's allowed hosts)",
            _ => "only the built-in desktop role" }})).collect();
    v.push(json!({"name": "desktop_focus", "category": "desktop", "grantable": false, "needs": "only the built-in desktop role"}));
    Json::Array(v)
}

/// What a member is actually given, decided from its config and its syndicate's ceiling.
#[derive(Debug, Default, Clone)]
pub struct Grants {
    pub workspace: Vec<String>,
    pub exec: bool,
    pub browser: Vec<String>,
    /// (tool, reason) for tools the config names that the runtime will not hand over.
    pub declared_only: Vec<(String, String)>,
}

pub fn grants(config: &Json, manifest: &Json) -> Grants {
    let allowed: Vec<String> = match config.get("allowed_tools").and_then(Json::as_array) {
        Some(a) => a.iter().filter_map(|x| x.as_str()).map(|t| canonical_tool(t).to_string()).collect(),
        None => vec!["read_file".into(), "write_file".into(), "list_files".into()],
    };
    let ceiling: BTreeSet<String> = strs(manifest, "allowed_tools_ceiling").iter().map(|t| canonical_tool(t).to_string()).collect();
    let acknowledged = config.get("exec_acknowledged") == Some(&json!(true));
    let mut g = Grants::default();
    for t in allowed {
        match tool_category(&t) {
            "core" => {}
            "workspace" if ceiling.contains(&t) || ceiling.is_empty() && config.get("allowed_tools").is_none() => g.workspace.push(t),
            "workspace" => g.declared_only.push((t, "above the syndicate ceiling".into())),
            "exec" if !ceiling.contains(&t) => g.declared_only.push((t, "above the syndicate ceiling".into())),
            "exec" if !acknowledged => g.declared_only.push((t, "execution is not acknowledged for this member".into())),
            "exec" => g.exec = true,
            "browser" if ceiling.contains(&t) => g.browser.push(t),
            "browser" => g.declared_only.push((t, "above the syndicate ceiling".into())),
            "desktop" => g.declared_only.push((t, "desktop control is only available to the built-in desktop role".into())),
            _ => g.declared_only.push((t, "the runtime has no such tool".into())),
        }
    }
    g.workspace.sort();
    g.workspace.dedup();
    g.browser.sort();
    g.browser.dedup();
    g
}

// ---------------------------------------------------------------- membership (runtime policy)
#[derive(Debug, Clone)]
pub struct Edge {
    pub to: String,
    pub relation: String,
    pub condition: String,
    pub max_rounds: Option<u64>,
    pub order: u64,
}

pub struct Member {
    pub syndicate_dir: PathBuf,
    pub agent_id: String,
    pub config: Json,
    pub manifest: Json,
}

/// The syndicate a role belongs to, if it is a syndicate member (base roles are not).
pub fn membership(root: &Path, role: &str) -> Option<Member> {
    let d = crate::agent::role_dir(root, role)?;
    let agents = d.parent()?;
    if agents.file_name()? != "agents" {
        return None;
    }
    let syn_dir = agents.parent()?;
    if syn_dir.parent()?.file_name()? != "syndicates" {
        return None;
    }
    let manifest = read_json(&syn_dir.join("syndicate.json"));
    if manifest.is_null() {
        return None;
    }
    Some(Member { syndicate_dir: syn_dir.to_path_buf(), agent_id: role.to_string(), config: read_json(&d.join("config.json")), manifest })
}

fn edges_of(manifest: &Json) -> Vec<(usize, &Json)> {
    manifest.get("edges").and_then(Json::as_array).into_iter().flatten().enumerate().collect()
}

impl Member {
    pub fn syndicate_name(&self) -> String {
        self.syndicate_dir.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
    }
    fn edge(&self, i: usize, e: &Json) -> Edge {
        Edge { to: s(e, "to").into(), relation: e.get("relation").and_then(Json::as_str).unwrap_or("commands").into(), condition: s(e, "condition").into(),
               max_rounds: e.pointer("/constraints/max_rounds").and_then(Json::as_u64), order: e.get("order").and_then(Json::as_u64).unwrap_or(i as u64 + 1) }
    }
    /// Outgoing relationships, in flow order.
    pub fn outgoing(&self) -> Vec<Edge> {
        let mut v: Vec<Edge> = edges_of(&self.manifest).into_iter().filter(|(_, e)| s(e, "from") == self.agent_id).map(|(i, e)| self.edge(i, e)).collect();
        v.sort_by_key(|e| e.order);
        v
    }
    pub fn can_message(&self, to: &str) -> Option<Edge> {
        self.outgoing().into_iter().find(|e| e.to == to)
    }
    pub fn max_dispatch_rounds(&self) -> u64 {
        self.manifest.get("max_dispatch_rounds").and_then(Json::as_u64).unwrap_or(3).clamp(1, 20)
    }
    /// How many times this member may delegate to `to` in one mission: the relationship's own cap, else the
    /// tightest loop cap that runs through this pair, else the syndicate-wide cap.
    pub fn cap_to(&self, to: &str) -> u64 {
        if let Some(r) = self.can_message(to).and_then(|e| e.max_rounds) {
            return r;
        }
        let mut cap = self.max_dispatch_rounds();
        for l in self.manifest.get("loops").and_then(Json::as_array).into_iter().flatten() {
            let path = strs(l, "path");
            let in_loop = path.windows(2).any(|w| w[0] == self.agent_id && w[1] == to);
            if in_loop {
                cap = cap.min(l.get("max_iterations").and_then(Json::as_u64).unwrap_or(cap));
            }
        }
        cap
    }
    pub fn coordinator(&self) -> String {
        s(&self.manifest, "coordinator_agent_id").to_string()
    }
    pub fn grants(&self) -> Grants {
        grants(&self.config, &self.manifest)
    }
    /// Hosts this member may be given: the request's hosts narrowed by the member's allowed_hosts (when it has any).
    pub fn narrow_hosts(&self, requested: &[String]) -> Vec<String> {
        let allowed = strs(&self.config, "allowed_hosts");
        if allowed.is_empty() {
            return requested.to_vec();
        }
        requested.iter().filter(|h| allowed.contains(h)).cloned().collect()
    }
    pub fn max_turns(&self) -> Option<u64> { self.config.get("max_turns").and_then(Json::as_u64) }
    pub fn timeout_seconds(&self) -> Option<u64> { self.config.get("timeout_seconds").and_then(Json::as_u64) }
    pub fn workspace(&self, root: &Path) -> PathBuf {
        match self.config.get("workspace_rel_path").and_then(Json::as_str) {
            Some(p) => root.join(p),
            None => self.syndicate_dir.join("agents").join(&self.agent_id).join("workspace"),
        }
    }
}

// ---------------------------------------------------------------- identity
fn humanize(c: &str) -> String {
    c.replace('_', " ")
}

fn relation_line(rel: &str, other: &str, incoming: bool) -> String {
    match (rel, incoming) {
        ("commands", false) => format!("You direct {other}."),
        ("commands", true) => format!("You take direction from {other}."),
        ("feeds", false) => format!("Your findings feed {other}."),
        ("feeds", true) => format!("{other} feeds you its findings."),
        ("verifies", false) => format!("You verify the work of {other}; nothing of theirs is done until you have checked it."),
        ("verifies", true) => format!("{other} verifies your work; it is not done until they have checked it."),
        ("challenges", false) => format!("You challenge the plans of {other} before they act."),
        ("challenges", true) => format!("{other} challenges your plans before you act; answer them with evidence."),
        ("escalates_to", false) => format!("When something is beyond you, you escalate to {other}."),
        ("escalates_to", true) => format!("{other} escalates to you when something is beyond them."),
        ("hands_off", false) => format!("When your stage is complete you hand off to {other}."),
        ("hands_off", true) => format!("{other} hands off to you when their stage is complete."),
        ("reports_to", false) => format!("You report to {other}."),
        (_, _) => format!("{other} reports to you."),
    }
}

/// Render a member's identity block from in-memory pieces (used for the on-disk files and for UI previews).
/// `people` lists (agent_id, persona name) for naming counterparts.
fn render_identity(me: &str, persona: &Json, syn: &Json, people: &dyn Fn(&str) -> String) -> String {
    let mut out = String::new();
    let title = s(persona, "title");
    out += &format!("You are {}{}, of {}.\n", s(persona, "name"), if title.is_empty() { String::new() } else { format!(", {title}") }, s(syn, "display_name"));
    if !s(persona, "backstory").is_empty() {
        out += &format!("{}\n", s(persona, "backstory"));
    }
    out += &format!("\nThe mission of {}: {}\n", s(syn, "display_name"), s(syn, "mission"));
    if !s(syn, "purpose").is_empty() {
        out += &format!("Why it exists: {}\n", s(syn, "purpose"));
    }
    if !s(syn, "lore").is_empty() {
        out += &format!("{}\n", s(syn, "lore"));
    }
    let creed = strs(persona, "creed");
    if !creed.is_empty() {
        out += "\nWhat you hold to:\n";
        for c in creed { out += &format!("- {c}\n"); }
    }
    let never = strs(persona, "never");
    if !never.is_empty() {
        out += "\nWhat you never do:\n";
        for c in never { out += &format!("- {c}\n"); }
    }
    if !s(persona, "voice").is_empty() {
        out += &format!("\nHow you speak: {}\n", s(persona, "voice"));
    }
    // Relationships in flow order, each with its condition and cap.
    let mut rels: Vec<(u64, String)> = vec![];
    let mut can_message: Vec<String> = vec![];
    for (i, e) in edges_of(syn) {
        let rel = e.get("relation").and_then(Json::as_str).unwrap_or("commands");
        let order = e.get("order").and_then(Json::as_u64).unwrap_or(i as u64 + 1);
        let cond = if s(e, "condition").is_empty() { String::new() } else { format!(" When: {}.", humanize(s(e, "condition"))) };
        let cap = e.pointer("/constraints/max_rounds").and_then(Json::as_u64).map(|r| format!(" At most {r} rounds.")).unwrap_or_default();
        if s(e, "from") == me {
            rels.push((order, format!("{}{cond}{cap}", relation_line(rel, &people(s(e, "to")), false))));
            can_message.push(people(s(e, "to")));
        }
        if s(e, "to") == me {
            rels.push((order, format!("{}{cond}{cap}", relation_line(rel, &people(s(e, "from")), true))));
        }
    }
    rels.sort_by_key(|(o, _)| *o);
    let mut place: Vec<String> = rels.into_iter().map(|(_, t)| t).collect();
    for l in syn.get("loops").and_then(Json::as_array).into_iter().flatten() {
        let path = strs(l, "path");
        if path.iter().any(|p| p == me) {
            place.push(format!("You take part in the {} cycle ({}), at most {} rounds, until {}.",
                s(l, "name"), path.iter().map(|p| people(p)).collect::<Vec<_>>().join(" → "), l.get("max_iterations").and_then(Json::as_u64).unwrap_or(1), s(l, "exit_when")));
        }
    }
    place.push(if can_message.is_empty() { "You do not delegate to anyone; return your result to whoever gave you the task.".into() }
               else { format!("You may send messages only to: {}.", can_message.join(", ")) });
    out += "\nYour place in the syndicate:\n";
    for p in place { out += &format!("- {p}\n"); }
    out
}

/// The identity block for a syndicate member on disk, or None if it has no persona or officiation is off.
pub fn officiation(role_dir: &Path) -> Option<String> {
    let cfg: Json = serde_json::from_str(&fs::read_to_string(role_dir.join("config.json")).ok()?).ok()?;
    let persona = cfg.get("persona")?;
    let syn_path = role_dir.parent()?.parent()?.join("syndicate.json");
    let syn: Json = serde_json::from_str(&fs::read_to_string(syn_path).ok()?).ok()?;
    if syn.pointer("/officiation/enabled") == Some(&json!(false)) {
        return None;
    }
    let me = s(&cfg, "agent_id").to_string();
    let name = |id: &str| -> String {
        role_dir.parent().map(|a| read_json(&a.join(id).join("config.json")))
            .and_then(|c| c.pointer("/persona/name").and_then(Json::as_str).map(|n| format!("{n} ({id})"))).unwrap_or_else(|| id.to_string())
    };
    Some(render_identity(&me, persona, &syn, &name))
}

/// Identity blocks for every member of an unsaved bundle: `{agent_id: text}`.
pub fn preview(root: &Path, bundle: &Json) -> Json {
    let syn = merged_manifest(root, bundle);
    let members: Vec<&Json> = bundle["members"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    if syn.pointer("/officiation/enabled") == Some(&json!(false)) {
        return json!({});
    }
    let name = |id: &str| -> String {
        members.iter().find(|m| s(m, "agent_id") == id).and_then(|m| m.pointer("/persona/name").and_then(Json::as_str).filter(|n| !n.is_empty()).map(|n| format!("{n} ({id})")))
            .unwrap_or_else(|| id.to_string())
    };
    let mut out = Map::new();
    for m in &members {
        out.insert(s(m, "agent_id").to_string(), json!(render_identity(s(m, "agent_id"), &m["persona"], &syn, &name)));
    }
    Json::Object(out)
}

// ---------------------------------------------------------------- validation
fn slug_ok(id: &str) -> bool {
    !id.is_empty() && id.len() <= 48 && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') && !id.starts_with('-')
}
/// A hostname (letters, digits, dashes, dots; no scheme, path, port or wildcard) or an IP literal.
pub fn host_ok(h: &str) -> bool {
    if h.is_empty() || h.len() > 253 { return false; }
    if h == "::1" || h.parse::<std::net::Ipv4Addr>().is_ok() { return true; }
    h.split('.').all(|l| !l.is_empty() && l.len() <= 63 && !l.starts_with('-') && !l.ends_with('-') && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
}
fn tool_name_ok(t: &str) -> bool {
    !t.is_empty() && t.len() <= 64 && t.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The manifest as it will be after apply: the file on disk overlaid with the bundle's syndicate object.
fn merged_manifest(root: &Path, bundle: &Json) -> Json {
    let dir = s(bundle, "dir");
    let mut merged = if slug_ok(dir) { read_obj(&root.join("syndicates").join(dir).join("syndicate.json")) } else { Map::new() };
    for (k, v) in bundle["syndicate"].as_object().cloned().unwrap_or_default() { merged.insert(k, v); }
    Json::Object(merged)
}

/// Every problem with a bundle; empty means it can be applied.
pub fn validate(root: &Path, bundle: &Json) -> Vec<String> {
    let mut errs = vec![];
    let dir = s(bundle, "dir");
    let merged = merged_manifest(root, bundle);
    let syn = &merged;
    if !slug_ok(dir) { errs.push(format!("dir \"{dir}\" must be lowercase letters, digits and dashes")); }
    if s(syn, "syndicate_id").is_empty() { errs.push("syndicate.syndicate_id is required".into()); }
    if s(syn, "mission").trim().is_empty() { errs.push("syndicate.mission is required".into()); }
    let members: Vec<&Json> = bundle["members"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    if members.is_empty() { errs.push("at least one member is required".into()); }
    let ids: Vec<String> = members.iter().map(|m| s(m, "agent_id").to_string()).collect();
    let tool_ceiling = strs(syn, "allowed_tools_ceiling");
    let host_ceiling = strs(syn, "allowed_hosts_ceiling");
    for t in &tool_ceiling { if !tool_name_ok(t) { errs.push(format!("tool ceiling: \"{t}\" is not a valid tool name (lowercase letters, digits, underscores)")); } }
    for h in &host_ceiling { if !host_ok(h) { errs.push(format!("host ceiling: \"{h}\" is not a valid host (a bare hostname or IP, no scheme, port or wildcard)")); } }
    for (i, id) in ids.iter().enumerate() {
        if !slug_ok(id) { errs.push(format!("member \"{id}\": id must be lowercase letters, digits and dashes")); }
        if ids[..i].contains(id) { errs.push(format!("member \"{id}\" appears twice")); }
        if BASE_ROLES.contains(&id.as_str()) || root.join("agents").join(id).is_dir() {
            errs.push(format!("member \"{id}\" collides with a base role; base roles always win, so this member would never run"));
        }
        // an id may live in only one syndicate, or role_dir would pick one arbitrarily
        if let Ok(rd) = fs::read_dir(root.join("syndicates")) {
            for e in rd.flatten() {
                let other = e.file_name().to_string_lossy().into_owned();
                if other != dir && e.path().join("agents").join(id).is_dir() {
                    errs.push(format!("member \"{id}\" already belongs to syndicate \"{other}\"; a member id must be unique across syndicates"));
                }
            }
        }
    }
    let coord = s(syn, "coordinator_agent_id");
    if !coord.is_empty() && !ids.iter().any(|i| i == coord) { errs.push(format!("coordinator \"{coord}\" is not a member")); }
    for m in &members {
        let id = s(m, "agent_id");
        for t in strs(m, "allowed_tools") {
            if !tool_name_ok(&t) { errs.push(format!("{id}: \"{t}\" is not a valid tool name")); }
            else if !tool_ceiling.iter().any(|c| canonical_tool(c) == canonical_tool(&t)) { errs.push(format!("{id}: tool \"{t}\" is above the syndicate ceiling")); }
        }
        for h in strs(m, "allowed_hosts") {
            if !host_ok(&h) { errs.push(format!("{id}: \"{h}\" is not a valid host (a bare hostname or IP, no scheme, port or wildcard)")); }
            else if !host_ceiling.contains(&h) { errs.push(format!("{id}: host \"{h}\" is above the syndicate ceiling")); }
        }
        if let Some(t) = m.get("max_turns") {
            if !t.as_u64().map(|n| (1..=32).contains(&n)).unwrap_or(false) { errs.push(format!("{id}: max_turns must be a whole number from 1 to 32")); }
        }
        if let Some(t) = m.get("timeout_seconds") {
            if !t.as_u64().map(|n| (30..=3600).contains(&n)).unwrap_or(false) { errs.push(format!("{id}: timeout_seconds must be a whole number from 30 to 3600")); }
        }
        let p = &m["persona"];
        if !p.is_object() || s(p, "name").trim().is_empty() { errs.push(format!("{id}: persona.name is required")); }
        for (field, text) in identity_texts(syn, p) {
            let hits = lint(&text);
            if !hits.is_empty() { errs.push(format!("{id}: {field} contains framing words {hits:?}; state the identity as fact")); }
        }
    }
    for (i, e) in syn["edges"].as_array().into_iter().flatten().enumerate() {
        for end in ["from", "to"] {
            if !ids.iter().any(|x| x == s(e, end)) { errs.push(format!("edge {i}: {end} \"{}\" is not a member", s(e, end))); }
        }
        if s(e, "from") == s(e, "to") { errs.push(format!("edge {i}: a member cannot point at itself (use a loop)")); }
        let rel = e.get("relation").and_then(Json::as_str).unwrap_or("commands");
        if !RELATIONS.contains(&rel) { errs.push(format!("edge {i}: unknown relation \"{rel}\"")); }
        if let Some(r) = e.pointer("/constraints/max_rounds").and_then(Json::as_u64) {
            if r == 0 || r > 20 { errs.push(format!("edge {i}: max_rounds must be 1-20")); }
        }
        if s(e, "condition").len() > 240 { errs.push(format!("edge {i}: condition is longer than 240 characters")); }
        let hits = lint(s(e, "condition"));
        if !hits.is_empty() { errs.push(format!("edge {i}: condition contains framing words {hits:?}")); }
        for t in strs(&e["constraints"], "tools") {
            if !tool_ceiling.contains(&t) { errs.push(format!("edge {i}: tool \"{t}\" is above the syndicate ceiling")); }
        }
    }
    for (i, l) in syn["loops"].as_array().into_iter().flatten().enumerate() {
        let path = strs(l, "path");
        if path.len() < 2 { errs.push(format!("loop {i}: needs at least two members")); }
        for p in &path {
            if !ids.contains(p) { errs.push(format!("loop {i}: \"{p}\" is not a member")); }
        }
        let n = l.get("max_iterations").and_then(Json::as_u64).unwrap_or(0);
        if n == 0 || n > MAX_LOOP_ITERATIONS { errs.push(format!("loop {i}: max_iterations must be 1-{MAX_LOOP_ITERATIONS}")); }
        if s(l, "exit_when").trim().is_empty() { errs.push(format!("loop {i}: exit_when is required so the loop can end")); }
    }
    let rounds = syn.get("max_dispatch_rounds").and_then(Json::as_u64).unwrap_or(0);
    if rounds == 0 || rounds > 20 { errs.push("syndicate.max_dispatch_rounds must be 1-20".into()); }
    errs
}

/// Things that are valid but will not behave the way the config suggests. Shown as warnings, never blocks apply.
pub fn notes(root: &Path, bundle: &Json) -> Vec<Json> {
    let mut out = vec![];
    let syn = merged_manifest(root, bundle);
    let members: Vec<&Json> = bundle["members"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let coord = s(&syn, "coordinator_agent_id").to_string();
    let edges = edges_of(&syn);
    for m in &members {
        let id = s(m, "agent_id");
        let g = grants(m, &syn);
        for (t, why) in &g.declared_only {
            out.push(json!({"member": id, "code": "declared_only", "text": format!("{id}: \"{t}\" is listed but will not be granted: {why}.")}));
        }
        if m.get("allowed_tools").and_then(Json::as_array).map(|a| a.iter().any(|t| t == "list_dir")).unwrap_or(false) {
            out.push(json!({"member": id, "code": "alias", "text": format!("{id}: \"list_dir\" is the older name for \"list_files\"; both are treated as list_files.")}));
        }
        if !g.browser.is_empty() && strs(m, "allowed_hosts").is_empty() {
            out.push(json!({"member": id, "code": "browser_no_hosts", "text": format!("{id}: has browser tools but no allowed hosts, so it can open nothing.")}));
        }
        let outgoing = edges.iter().any(|(_, e)| s(e, "from") == id);
        let incoming = edges.iter().any(|(_, e)| s(e, "to") == id);
        if !outgoing && id != coord {
            out.push(json!({"member": id, "code": "cannot_delegate", "text": format!("{id}: has no outgoing relationship, so it cannot message anyone (send_message is removed).")}));
        }
        if !incoming && id != coord {
            out.push(json!({"member": id, "code": "unreachable", "text": format!("{id}: has no incoming relationship, so nobody can send it work.")}));
        }
    }
    if coord.is_empty() {
        out.push(json!({"member": null, "code": "no_coordinator", "text": "No coordinator is set, so a mission has no entry point."}));
    }
    out
}

/// Every identity string that ends up in a member's prompt.
fn identity_texts(syndicate: &Json, persona: &Json) -> Vec<(String, String)> {
    let mut out = vec![];
    for k in ["display_name", "mission", "purpose", "lore"] {
        out.push((format!("syndicate.{k}"), s(syndicate, k).to_string()));
    }
    for k in ["name", "title", "backstory", "voice"] {
        out.push((format!("persona.{k}"), s(persona, k).to_string()));
    }
    for k in ["creed", "never"] {
        for (i, x) in strs(persona, k).into_iter().enumerate() {
            out.push((format!("persona.{k}[{i}]"), x));
        }
    }
    out
}

// ---------------------------------------------------------------- load / list / apply
/// Syndicate folder names under `syndicates/`.
pub fn list_dirs(root: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(root.join("syndicates")).into_iter().flatten().flatten()
        .filter(|e| e.path().join("syndicate.json").is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    v.sort();
    v
}

/// An editable bundle read back from disk: the manifest plus each member's config.
pub fn load_bundle(root: &Path, dir: &str) -> Result<Json, String> {
    if !slug_ok(dir) { return Err("bad syndicate folder name".into()); }
    let sd = root.join("syndicates").join(dir);
    let manifest = read_json(&sd.join("syndicate.json"));
    if manifest.is_null() { return Err(format!("no syndicate named {dir}")); }
    let members: Vec<Json> = strs(&manifest, "agent_ids").iter().map(|id| {
        let c = read_json(&sd.join("agents").join(id).join("config.json"));
        let mut m = json!({"agent_id": id});
        for k in ["role_title", "persona", "allowed_tools", "allowed_hosts", "max_turns", "timeout_seconds", "exec_acknowledged", "model_profile_id"] {
            if let Some(v) = c.get(k) { m[k] = v.clone(); }
        }
        m
    }).collect();
    Ok(json!({"dir": dir, "syndicate": manifest, "members": members}))
}

/// Validate, back up, and write. Unknown keys already in the files (mcp_servers, provenance…) are kept.
/// Members dropped from the bundle are archived under `.history/<ts>/removed/`, so they can no longer be run.
pub fn apply(root: &Path, bundle: &Json, dry_run: bool) -> Result<Json, String> {
    let errs = validate(root, bundle);
    if !errs.is_empty() {
        return Err(format!("bundle rejected:\n- {}", errs.join("\n- ")));
    }
    let dir = root.join("syndicates").join(s(bundle, "dir"));
    let syn_path = dir.join("syndicate.json");
    let mut writes: Vec<(PathBuf, Json, bool)> = vec![];
    let mut syn = read_obj(&syn_path);
    let old_ids = strs(&Json::Object(syn.clone()), "agent_ids");
    for (k, v) in bundle["syndicate"].as_object().cloned().unwrap_or_default() {
        syn.insert(k, v);
    }
    syn.entry("schema_version").or_insert(json!(1));
    let new_ids: Vec<String> = bundle["members"].as_array().unwrap().iter().map(|m| s(m, "agent_id").to_string()).collect();
    syn.insert("agent_ids".into(), json!(new_ids));
    writes.push((syn_path.clone(), Json::Object(syn.clone()), syn_path.exists()));
    for m in bundle["members"].as_array().unwrap() {
        let id = s(m, "agent_id");
        let p = dir.join("agents").join(id).join("config.json");
        let existed = p.exists();
        let mut cfg = read_obj(&p);
        if !existed {
            cfg.insert("schema_version".into(), json!(1));
            cfg.insert("model_profile_id".into(), json!("local-qwen35-9b-local"));
            cfg.insert("prompt_file".into(), json!("PROMPT.md"));
            cfg.insert("system_instructions_file".into(), json!("SYSTEM_INSTRUCTIONS.md"));
            cfg.insert("vision".into(), json!(false));
            cfg.insert("workspace_rel_path".into(), json!(format!("syndicates/{}/agents/{id}/workspace", s(bundle, "dir"))));
        }
        cfg.insert("agent_id".into(), json!(id));
        cfg.insert("syndicate_id".into(), syn["syndicate_id"].clone());
        for k in ["role_title", "persona", "allowed_tools", "allowed_hosts", "max_turns", "timeout_seconds", "model_profile_id", "exec_acknowledged"] {
            if let Some(v) = m.get(k) { cfg.insert(k.into(), v.clone()); }
        }
        writes.push((p, Json::Object(cfg), existed));
    }
    let removed: Vec<&String> = old_ids.iter().filter(|o| !new_ids.contains(o)).collect();
    let mut plan: Vec<Json> = writes.iter().map(|(p, _, e)| json!({"path": p.strip_prefix(root).unwrap_or(p), "action": if *e { "update" } else { "create" }})).collect();
    for r in &removed {
        plan.push(json!({"path": format!("syndicates/{}/agents/{r}", s(bundle, "dir")), "action": "archive (member removed)"}));
    }
    if dry_run {
        return Ok(json!({"dry_run": true, "writes": plan}));
    }
    let backup = dir.join(".history").join(format!("{}", now() as u64));
    for (p, v, existed) in &writes {
        if *existed {
            let rel = p.strip_prefix(&dir).unwrap_or(p);
            let b = backup.join(rel);
            fs::create_dir_all(b.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::copy(p, &b).map_err(|e| e.to_string())?;
        }
        let parent = p.parent().unwrap();
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        if parent.file_name().and_then(|n| n.to_str()) != Some(s(bundle, "dir")) {
            for sub in ["workspace", "inbox"] { fs::create_dir_all(parent.join(sub)).map_err(|e| e.to_string())?; }
            let prompt = parent.join("PROMPT.md");
            if !prompt.exists() {
                fs::write(&prompt, format!("Carry out your part of the {} mission with the tools you have, and report what actually happened.\n",
                    s(&bundle["syndicate"], "display_name"))).map_err(|e| e.to_string())?;
            }
        }
        write_pretty(p, v)?;
    }
    for r in &removed {
        let from = dir.join("agents").join(r);
        if from.is_dir() {
            let to = backup.join("removed").join(r);
            fs::create_dir_all(to.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::rename(&from, &to).map_err(|e| format!("archive {r}: {e}"))?;
        }
    }
    Ok(json!({"applied": true, "writes": plan, "backup": if backup.exists() { json!(backup.strip_prefix(root).unwrap_or(&backup)) } else { Json::Null }}))
}
