# Local agent harness plan — November 2, 2026

Original design retained for provenance. Implementation is installed; November 3 repair-cycle outcomes are recorded in CHECKPOINT.md and the generated report.

## Objective and selection requirement

Run local cybersecurity, browser, desktop, image and video workers on this M3 Pro/18 GiB Mac. Language-model candidates must be explicitly published as local/no-refusal. Fara and stock aligned models are excluded from the default package. Publisher no-refusal claims and measured local results must be reported separately; a finite test cannot prove all possible behavior.

## Architecture

User task → CLI supervisor → selected role → local inference endpoint → structured tool request → executor → observed result → inference continuation → verified artifact.

One Qwen3.5 9B Local HauhauCS Aggressive Q4 model, with its matching vision projector, is shared by cybersecurity, browser and desktop roles. Role folders contain configuration and prompts, not duplicate weights. The separate image worker uses an explicitly local image stack whose full components and compatibility must be verified before selection. Fara is retained only as an excluded research record.

The local server binds only to loopback. No cloud fallback. The supervisor owns the server process and unloads the language model before image or video generation, then restarts it when needed. A package-wide advisory lock serializes this package’s heavy inference workers; unrelated applications are outside that lock. Start at 8K context; measure memory and task performance before expanding. Browser context and native UI history are bounded.

## Tools and autonomy

Use OpenAI-compatible function tool schemas if the local model/server supports them. Validate tool name, argument types and required keys. Dispatch through Python functions, never eval model text. Return structured success/error results with matching call IDs. Feed tool results back to the model and permit multi-step continuation. Bounded turns, timeouts, cancellation and process cleanup are mandatory. A tool request is not a completed action: verify the filesystem, browser or desktop state after execution.

Cyber worker: workspace file reads/writes and commands for explicitly authorized lab tasks. Initial fixture is a deliberately vulnerable local function; success requires identifying the flaw, editing the fixture and passing a regression test. No external attack targets are implied by installation.

Browser worker: local Playwright-controlled Chrome, navigation, screenshot/DOM observation, click, type and page-state inspection. Initial task uses a local form with an independently checked submitted result. Native screenshot-based grounding is an additional test, separate from DOM-assisted control.

Desktop worker: macOS application/window observation and input through a local driver. Check Accessibility/Screen Recording availability. Initial task uses a new test document/window and verifies a visible change. Never report a browser test as native desktop acceptance. Missing system permission blocks only the dependent desktop test.

Image worker: local CLI/function that generates to a task output directory; record model components, seed, dimensions, elapsed time and image validity. Test a non-sensitive prompt and inspect the result. Do not substitute a cloud image service.

Agent messages: supervisor stores typed task/result envelopes locally (task ID, sender, recipient, objective, artifact paths, status, evidence). Workers can communicate through the supervisor without concurrent model residency. External messaging accounts are not part of this installation.

Video worker: pinned blackshirt/NSFW_Wan_1.3b experimental e14 with the Apple MLX Wan runtime and Q8 UMT5 encoder. Start with 320×192, 33 frames, 24 steps; verify MP4 decoding, frame count, finite pixels, temporal variation, and visually inspect representative frames. Record latency and MLX memory by phase. The small DiT requires another 6.04 GB text encoder and 0.51 GB VAE.

## Storage budget

Pre-install available: 60.04 GiB / 64.47 GB. Qwen model+projector: 6.55 GB. Reserve about 13 GB for image weights/conversion, 9.5 GB for video components, 4 GB for runtimes, 2 GB for outputs, and retain at least 15 GiB free disk. Prefer direct quantized packages and shared cache references. Recheck actual free space before each large download. Never delete unrelated files to meet this budget.

## Acceptance and package

1. Verify revisions, file sizes and available hashes for selected weights; record installed runtime versions.
2. Load with Apple Metal/MLX and verify a text response and real structured tool execution.
3. Complete and independently verify a local cyber fixture task.
4. Complete and verify a browser fixture task; separate screenshot grounding from DOM-assisted results.
5. Complete and verify a native desktop task, or record the exact permission/runtime blocker.
6. Generate and visually inspect a local image and short video; report actual latency and memory evidence where available.
7. Test malformed tool handling, turn budget and process cancellation.
8. Verify local coordinator → child execution → independently checked output → sender receipt.
9. Deliver launch commands, configs, source, pinned dependency metadata, selected-model manifest, evidence, portable HTML report and a small archive excluding multi-GB weights/caches. Installed weights remain under shared/models or linked shared caches.

No synthetic success claims: installed, loaded, tool-tested, task-tested and accepted are distinct statuses. Model-output filtering/refusal policy is not to be silently added to replace the requested no-refusal model selection; execution constraints protect concrete filesystem/tool scopes and follow the user's authorized task.
