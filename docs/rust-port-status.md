# Rust port status (2026-11-10)

The control plane, agent loop, tools and media workers now run in Rust (`shared/rust-harness`, binary `local-agents-rs`). Python is left only where a Python-only library does the actual work.

```
Codex ──MCP──► run-mcp.sh ─► local-agents-rs mcp ─► supervisor ─┬─► local-agents-rs agent  (loop + native tools)
                                                                └─► local-agents-rs image|video ─► media_kernel.py (MLX only)
```

## Moved to Rust in this pass

| Was (Python) | Now (Rust) | Verified by |
|---|---|---|
| `tools.py`, `tool_host.py`, `skill_catalog.py`, `worker_catalog.py` | `src/tools/` (file, shell, messages, skills, workers, MemQ config, browser over CDP, desktop via Quartz FFI) | 17 unit tests, including live headless Chrome; live agent run |
| `agent.py` | `local-agents-rs agent` (already ported; the Python tool-host spawn is now removed) | live MCP job: `write_file`/`read_file` wrote and read back `native tools ok` |
| `coordinator.py` | `src/dispatch.rs` (earlier port) | existing dispatch tests |
| `jobs.py`, `mcp_server.py` | `local-agents-rs mcp` (earlier port); `run-mcp.sh` now points at it | MCP initialize/tools list, live job |
| `image_worker.py`, `video_worker.py`, the MediaGuard used by workers | `src/media.rs` plus a shared watchdog in `runtime.rs` (lock, reclaim, admission, watchdog, evidence) | 4 unit tests, live 512 px FLUX render (16.5 s), live 320×192×33 Wan clip (135 s) |

`cargo test --release`: 49 passed. The remaining Python tests (`test_runtime`, `test_media_policy`, `test_memory_skills`, `test_memq_mcp_server`, `test_chat_inference`) pass, and so does `shared/bin/check_mcp.py` against the Rust bridge. The 14 deleted Python files are kept in `evidence/rust-port-20261110/python-retired.tar.gz`.

Tool calls can now be cancelled mid-call: SIGTERM or the run deadline kills a running command's process group, aborts a browser action and closes Chrome, or kills a MemQ call.

Browser: Playwright is gone. The Rust client speaks the Chrome DevTools Protocol straight to headless Chrome, with requests outside the allowlist blocked through `Fetch`. Chrome is still the biggest memory cost (about 820–910 MB RSS across 9–10 processes). Because the client only needs CDP, Lightpanda or chrome-headless-shell can be tried later via `CHROME_BIN`.

## Acceptance suite on the Rust path (`node shared/bin/requalify.mjs`)

9 of 10 steps pass: unit tests, cyber, browser, vision, delegation, cancellation, memory (live MemQ read, identity checked before and after), image routing, MCP protocol. Desktop is skipped for now. Accessibility is granted, but Screen Recording only takes effect after the host app (Claude, or Terminal) is restarted. It also failed in the earlier evidence. Run `node shared/bin/requalify.mjs desktop` after that restart. The first full run failed most model steps only because another session held the heavy-model lock while rendering images. Evidence from before the rerun is in `evidence/rust-port-20261110/qualification-evidence-before-rust-requalify.tar.gz`.

## Browser engine

The browser tool now opens one browser-level CDP connection and a flat session on a new page target. That works with Chrome (tested) and is the shape Lightpanda needs. Set `BROWSER_BIN=/path/to/lightpanda` to switch; with Lightpanda, `browser_screenshot` returns a clear error because it does not render.

Lightpanda `1.1.0-nightly.11975+bfa07e3e4` is in `shared/vendor/lightpanda/lightpanda` (sha256 `1d2ac488…d527`). All three browser tests pass on it unchanged: open, the strict-match click error, fill, submit through a click, blocking a disallowed host, cancelling mid-navigation, and closing cleanly. Live test, three runs each, counting only the test's own browser process group:

| Engine | Test time | Peak RSS | Processes |
|---|---|---|---|
| Chrome (headless) | 1.2–3.3 s | 550–830 MB | 6–9 |
| Lightpanda | 0.19–0.30 s | 10–24 MB | 1 |

The Lightpanda peaks are rough, because the run is so short that the sampler only caught 1–2 samples. It is still roughly 30× less memory and 5–10× faster. It is not the default yet: it has no screenshots, so the vision step still needs Chrome.

## Seen in the live runs

- The first video attempt was stopped by the guard. During model load, available RAM fell to 2.5 GiB and pressure hit warning level. The watchdog quit ChatGPT normally and freed about 2.7 GiB, but swap had already grown 0.97 GiB, over the 0.75 GiB limit, so it stopped the kernel and reopened ChatGPT. That is the same rule the Python guard had. The retry passed without quitting anything.
- Memory limit fixed: RSS missed MLX's GPU memory (1.5 GiB RSS against a 3.4 GB MLX peak), while for llama.cpp RSS overstates it (4.95 GiB RSS, 1.13 GiB physical footprint, because the model file is memory-mapped). Samples now record `worker_footprint_bytes` (from `proc_pid_rusage`, the same number as Apple's `footprint` tool), and the 8 GiB worker limit checks the larger of the two. Live check on a FLUX render: MLX peak 3.17 GiB, max RSS 2.13 GiB, max footprint 3.50 GiB.
- Codex has to restart its MCP server to pick up the Rust bridge in `run-mcp.sh`.

## Still Python, and why

| File | Why it stays |
|---|---|
| `media_kernel.py` | The FLUX.2 klein (mflux) and Wan pipelines exist only as Python on MLX. Only model load, generate and save remain here. |
| `memq_client.py`, `memq_mcp_server.py` | MemQ uses an MCP OAuth (PKCE) client over TLS from the Python `mcp` SDK. `memq_status` and `memq_tools` are native; `memq_call` runs `memq_client.py call` as a killable child. |
| `runtime.py` | Shared `ROOT`, lock and terminate helpers for the remaining Python files. |
| `chat_inference.py`, `media_policy.py` | Separate chat-inference path (not part of this port). It still uses the Python MediaGuard. |
| `acceptance*.py`, `rust_agent.py`, `desktop_fixture.py` | Qualification scripts and fixtures. They now drive the Rust binary through `rust_agent.py`. |
| `convert_image_encoder.py`, `shared/bin/*.py` setup scripts | One-time model conversion and installation tools. |
| `test_*.py` (trimmed) | Tests for the Python files above. |

## Image route on Apple Silicon (research)

| Route | Fit | Cost |
|---|---|---|
| Keep MLX behind a process boundary (chosen) | Same weights and quality, MLX is the fastest Metal path for diffusion here | Python stays for the kernel only |
| `mlx-rs` (Rust bindings to mlx-c) | Can load the same MLX 4-bit weights | Would need FLUX.2 klein, its Qwen3 encoder, VAE and sampler re-implemented in Rust; bindings trail MLX releases |
| `candle` with Metal | Pure Rust | No FLUX.2 klein; MLX 4-bit weights would need re-quantising to GGUF; slower on Metal for diffusion |
| `stable-diffusion.cpp` (ggml, Metal) | No Python, same pattern as llama.cpp | FLUX.2 klein support and the local encoder conversion must be confirmed first |

Next step if Python must go entirely: try `stable-diffusion.cpp` with FLUX.2 klein GGUF weights. Fall back to `mlx-rs` only if that fails.
