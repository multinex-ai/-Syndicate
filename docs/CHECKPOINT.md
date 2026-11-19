# Local Agents checkpoint

Snapshot: 2026-11-03T18:42:10.359Z. Realm: Agents / Multinex; this directory is not a Git repository.

## Objective and evidence

Fix Wan, MemQ, delegation, cancellation and native desktop control; rerun the defined acceptance suite and deliver a portable source/configuration/evidence package. All selected model files are installed. Runtime: Apple M3 Pro, 18 GiB unified memory; heavy workers are serialized.

9/10 task/media gates pass. Sequential regression state: requires_repairs. These are local smoke-test results, not production or broad reliability qualification.

| Gate | Result | Evidence |
| --- | --- | --- |
| Cyber / code | Passed | evidence/setup-20261102/cyber-acceptance.json |
| Browser | Passed | evidence/setup-20261102/browser-acceptance.json |
| Screenshot grounding | Passed | evidence/setup-20261102/vision-acceptance.json |
| Delegation | Passed | evidence/setup-20261102/communication-acceptance.json |
| Cancellation / budgets | Passed | evidence/setup-20261102/lifecycle-acceptance.json |
| Native desktop | Open / failed | evidence/setup-20261102/desktop-acceptance.json |
| MemQ + skills | Passed | evidence/setup-20261102/memory-agent-acceptance.json |
| Qwen → image | Passed | evidence/setup-20261102/media-routing-acceptance.json |
| FLUX image | Passed | agents/image-studio/outputs/local-smoke.evidence.json |
| Wan video | Passed | agents/video-studio/outputs/local-smoke.evidence.json |

## Repairs

- Wan: map exported UMT5 Sequential gate keys to the current MLX module layout before strict loading.
- MemQ: restore absolute token expiry on process restart; validate official issuer metadata before refreshing the agent's own OAuth credentials. Preserve an existing refresh token if the response omits its replacement.
- Delegation: children receive only their assignment; ownership context is separate. Successful task queueing yields the parent so the dispatcher can run. Self-delegation is rejected.
- Cancellation: the acceptance fixture uses the direct-run directory and verifies loaded-server interruption and endpoint closure.
- Desktop: explicit normalized screenshot coordinates; retain only the newest screenshot in model context. User-granted OS permissions are checked by the actual runtime. The fixture stays visible across Spaces; observed-process focus and named-window input guards prevent this test from acting on another app.

Pre-repair source/evidence remains in evidence/requalification-20261103/baseline. No credentials were copied between clients. Own credentials and authoritative bindings stay outside the workspace. Galileo/TF memory remains excluded.

## Delivery and limits

The report is reports/local-agent-models.html. The package receipt is packages/package-receipt.json. The archive excludes weights, environments, credentials, private bindings, raw desktop traces and job databases. Selected samples and their receipts are included.

Codex's live local worker status tool and MemQ handshake were observed in this chat. Antigravity configuration is installed; its live connection remains unverified. Running hosts need supported refresh/reconnection to load changed server code. Native workers and cyber commands run as the OS user. Host policies are unchanged.

HTML static structure and control contracts pass. Browser file-URL policy blocked rendered report inspection; no alternate route was used. Actual rendered report UI remains unverified.

## Next action

Repair the remaining failed gate shown above and rerun affected tests before final qualification.

No unrelated deletion, background installer or new external publication is part of this task.

## User pause

The user explicitly paused this work on November 3, 2026 because they are using the Mac. Desktop testing stays pending. No further computer control or tests should start until the user resumes. The current review package has 9/10 task/media gates and all 32 infrastructure tests passed. Resume with an uninterrupted desktop window, then rerun only native acceptance and refresh final qualification/package artifacts.

## November 4: media workflow resumed within machine limits

User authorized image quality, reference editing and short-video preset tests with installed models only, preserving the desktop pause. Added media presets, single-reference FLUX editing, phase-wise release, MCP/coordinator parameters, and sampled resource admission/watchdog. 38 infrastructure tests and fresh STDIO MCP checks passed. First real image attempt was blocked before model loading: under 4 GiB available; subsequent checks showed macOS warning pressure. No new media output or quality improvement is claimed. See docs/media-quality-state.json and docs/media-quality.md for acceptance and exact sequential replay commands. Do not lower thresholds or download larger models; wait for normal pressure and 4 GiB available. Previous ZIP remains the pre-change snapshot.

November 4 follow-up: investigated Codex helper counts. No orphaned Codex process found. Closed this thread’s obsolete Hugging Face setup tab. Temporarily archived three idle unpinned chats using the supported app connector, then restored all three and verified their presence. No forced process termination or history/project-file deletion. Smallest image retry remained admission-blocked (warning pressure; about 3.3 GiB available). All media models remain unloaded. Evidence: evidence/media-quality-20261104/process-review.json and image-draft-after-cleanup.resources.json.

## November 5 — coordinated render pause and guarded test attempts

Vision: qualify installed image/video models within the Mac limits. Status: both other media chats confirmed saved checkpoints, paused rendering and no owned worker remaining. No forced process termination or work deletion. Abstract: image512 draft and Wan320x192/33-frame draft both rejected before loading on warning pressure; available memory was 3.53 and 3.67 GiB respectively. A 13-sample, roughly 60-second watch stayed at warning pressure and did not launch its conditional image retry. No new image/video output; higher presets/reference editing remain pending.

Evidence: evidence/media-quality-20261105/coordination.json, image-draft.resources.json, video-draft.resources.json, admission-watch.json. Prior report/state preserved in that dated directory. Both other chats remain paused; no resumption authorized here. Next: normal pressure and >=4GiB available, then serial baseline tests before quality comparisons. Existing 38-test result is historical, not rerun for this evidence-only update. No spending/downloads; desktop testing remains paused.

## November 8 — interactive local Qwen CLI integration

Vision: persistent isolated Codex terminal chat using local Qwen and all local-agent MCP job tools. Status: configured; 45 harness tests, adapter transforms/cancellation and live app-server provider/MCP discovery passed. Actual Qwen chat remains admission-blocked on warning memory pressure; no two-turn continuity or routed media success claimed. Abstract: managed per-request inference shares heavy_lock, checks process-tree resources and unloads before tool calls; all Qwen workers now receive admission checks by default. Existing media limits preserved.

Launcher: shared/bin/local-chat.command. State/guide: docs/terminal-chat-state.json and docs/terminal-chat.md. Evidence and backups: evidence/terminal-chat-20261108. Native Terminal UI control was explicitly denied by the computer-use tool; user must open the launcher. No alternative UI automation attempted. No model downloads, paid calls, credential copying or change to active ~/.codex settings. Local bridge discovery is not a fresh private-memory authentication proof. Next: normal pressure and >=4GiB available, then status tool call + two-turn code-recall test and one specialist route. Existing native desktop/media quality gates remain open.

## November 9 — terminal resume503 repair

User-reported resume failure was traced to actual Qwen loading followed by sustained memory pressure during roughly14k-token prompt processing. Kept guards unchanged. Terminal inference now uses batch256/microbatch64, Q8 key/value caches and no prompt cache. Live retry loaded the model but still resource-stopped without an answer; evidence runs/terminal-d38522feed6544b49a5f63326af00d2a and evidence/terminal-chat-20261109/low-memory-chat.log. Errors now retain guard reason and the triggering memory sample rather than post-cleanup readings.46 harness tests pass. User adapter was left running; owned model server terminated. No chat history deleted or other apps closed. Next: more memory headroom, then saved-chat reply/continuity and routing acceptance.

November9 CLI upgrade: isolated Codex0.166.0 ->0.167.0. Official release archive digest and macOS signature passed; new app-server still selects local_uncensored/local-agent and discovers5 worker+3 MemQ tools. Running user session preserved; exit/relaunch needed to use new binary. Versioned rollback copy in lab/dist. Evidence: evidence/terminal-chat-20261109/codex-update. Separate runtime memory blocker remains open.
