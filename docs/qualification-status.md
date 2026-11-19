# Qualification status (snapshot)

> Moved from the workspace README on 2026-11-10 so the README can serve as an entry point. Content below is the November 3-8, 2026 snapshot and has not been re-verified. Commands run from the Agents root.

For a map of this workspace, see the [folder hierarchy](folder-hierarchy.md) or the [interactive folder guide](folder-hierarchy.html), including the shared toolkit and the separate Praeco repository.

Updated November 3, 2026. 9/10 task/media gates pass. Native desktop acceptance awaits an uninterrupted active desktop; the current package remains in progress. Consult [the evidence report](../reports/local-agent-models.html) and [the current checkpoint](CHECKPOINT.md). The report is a dated snapshot, not a live control console.

## Selected models

| Role | Installed model | Runtime |
| --- | --- | --- |
| Coordinator, cyber, browser, desktop | HauhauCS/Qwen3.5-9B-Local-HauhauCS-Aggressive, Q4_K_M; matching vision projector for screenshots | llama.cpp / Metal |
| Image studio | Runpod FLUX.2 Klein 4B Q4 diffusion/VAE with ponpoke local encoder converted to MFLUX Q4 | MFLUX / MLX |
| Video studio | blackshirt/NSFW_Wan_1.3b experimental e14, Q8 UMT5 encoder and Wan VAE | pinned MLX Wan runtime |

The four reasoning roles share one copy of Qwen. Each specialist has its own folder, executable config, workspace and output directory. The selected publishers describe these models or components as local; no finite test can guarantee universal no-refusal behavior. No refusal-reduced fallback is substituted. Fara and CyberTiel are not installed defaults. The image encoder lists flux-non-commercial-v2.1; review component licenses before commercial use.

## Interactive terminal chat

The isolated local Qwen chat launcher is `shared/bin/local-chat.command`. It exposes the full worker MCP tools and bounded MemQ recall, retains chat history, and unloads Qwen between replies. See [the launch guide](terminal-chat.md). Client discovery and infrastructure checks passed on November 8; actual conversation is still memory-blocked and native Terminal must be opened manually.

## Run from this folder

```sh
bash shared/bin/run-agent.sh cyber-lab 'Review the files in this workspace.'
bash shared/bin/run-agent.sh browser 'Open the authorized test page.' --allow-host example.test
bash shared/bin/run-agent.sh desktop 'Observe the visible native windows.'
bash shared/bin/run-agent.sh coordinator 'Ask image-studio to create a red robot reading beside a blue lamp.'
bash shared/bin/run-image.sh 'A small red robot reading beside a blue lamp.'
bash shared/bin/run-video.sh 'A yellow toy car rolling across a wooden table.'
```

Coordinator dispatch defaults to three subtasks. The model sees worker capabilities and can inspect installation readiness and the last recorded qualification. It queues a structured task, yields its process, and lets the supervisor load the specialist. A child executes its own assignment and returns an evidence envelope. Simple media requests avoid another Qwen load; a dependent continuation is explicit and bounded. Use --dispatch-limit 0 for planning only. External messaging accounts are not connected.

Only one package-managed heavy model runs at a time. Every server binds to loopback; there is no cloud inference fallback. Unrelated apps and model servers are outside the package lock. Browser contexts have explicit host allowlists. Native control uses the visible desktop and requires Accessibility and Screen Recording. Cyber commands run as your OS user: tool path checks are not an OS sandbox.

## Memory, skills and clients

Four packaged skills are available through list_skills/read_skill. MemQ uses this local agent's independent OAuth enrollment and a verified Multinex principal/namespace binding. The adapter checks ownership and scope around private operations and blocks Galileo/TF work; those projects require their own verified Umbrage connection. Credentials remain outside this workspace. A missing or invalid identity blocks memory, not independent local work.

The local MCP bridge supplies status/start/job/log/cancel tools. Codex and Antigravity configurations are installed; actual host connection evidence is separate. [Client integration](client-integration.md) explains setup, job semantics and remaining host-specific limits. Adding worker tools does not replace the host's main model or rules.

## Verification and repair evidence

[The report](../reports/local-agent-models.html) tracks ten task/media gates: cyber, browser, screenshot grounding, delegation, cancellation, native desktop, model-driven MemQ/skills, routed image generation, image review and video review. The separate infrastructure suite covers 32 contract/process tests. Independent fixtures verify outcomes rather than trusting a model's completion claim. A single pass establishes a smoke-tested candidate, not broad reliability or production readiness.

The November 3 repairs cover Wan encoder-key compatibility, persisted OAuth expiry and issuer discovery, delegation scope separation and yielding, screenshot coordinate units/history and native focus/target bounds, and the cancellation fixture path. Recoverable pre-repair source and results are under evidence/requalification-20261103/baseline. The sequential runner is shared/bin/requalify.mjs; do not run a second suite or another inference job concurrently.

## Portable package and rebuild

The ZIP includes source, role configs, pinned dependency metadata, selected evidence and generated samples. It excludes weights, runtime environments, credentials, private identity bindings, raw desktop captures and job databases. Installed weights remain in shared/models. The final filename is emitted only when all defined task/media, memory, regression and report-structure gates pass.

On Apple Silicon macOS with Node.js, Homebrew llama.cpp/FFmpeg and uv installed:

```sh
uv venv --python 3.12 shared/runtime/venv  # Only if absent.
uv pip install --python shared/runtime/venv/bin/python -r shared/config/requirements.lock.txt
python3 shared/bin/download_parallel.py
python3 shared/bin/download_image_base.py
python3 shared/bin/download_image_encoder.py
python3 shared/bin/download_video.py
shared/runtime/venv/bin/python shared/harness/convert_image_encoder.py
node shared/bin/build-report.mjs
node shared/bin/check-report.mjs
```

Downloaders verify pinned hashes before finalizing files. The gated encoder requires approved repository access and the local owner-only credential; never put credentials in commands or archives. Do not redownload intact weights. The earlier authorized MasClean cleanup is complete; no additional unrelated deletion belongs to this task.
