# Local media quality workflow

Updated November 4, 2026. Use installed FLUX.2 Klein 4B and Wan 1.3B only. No model downloads or paid services are authorized for this comparison.

Status: implementation and 38 infrastructure tests passed. Actual new outputs are unverified because the first image attempt was blocked before loading. Desktop testing remains paused.

## Machine limits

- One package-managed heavy worker at a time. Other applications remain outside this lock.
- Require normal macOS memory pressure, 4 GiB available memory and 12 GiB disk reserve before loading.
- Stop the owned worker for critical pressure, resident memory over 8 GiB, system swap growth over 768 MiB, or loss of disk reserve. Warning pressure or less than 1.5 GiB available for six seconds also stops it.
- MLX memory guideline: 7 GiB; image cache: 256 MiB; video cache disabled. These are sampled safeguards, not a hard OS memory reservation.
- FLUX releases the text encoder before denoising and transformer before decoding. One seed per process.
- Image dimensions are capped at 768 per side. One PNG/JPEG/WebP reference at most 768 per side and 20 MiB.
- Video is capped at 33 frames at up to 384×224; above that, 17 frames with at most 512×288 pixels of area. No image-to-video capability has been added.

## Client interface

MCP start_local_agent accepts media_preset (draft, quality; video also detail-short) and reference_image (image role only, absolute local path). Coordinator send_message accepts the same options. Existing running client sessions may need to refresh their tool catalog; the fresh STDIO server was checked.

## Sequential tests

Run the smallest case first, inspect its resource receipt, and stop on a blocked or resource-stopped result. Inspect image-quality.png before using it as the reference. A generated file is not a visual-quality pass. Keep the same prompt and seed for the draft/quality comparison.

### 512px low-memory baseline

```sh
bash shared/bin/run-image.sh 'Generate a detailed studio photograph of a small red robot reading a book beside a blue lamp. Clean composition, high quality, studio lighting.' --preset draft --seed 729 --output evidence/media-quality-20261104/image-draft.png
```

### 768px image quality

```sh
bash shared/bin/run-image.sh 'Generate a detailed studio photograph of a small red robot reading a book beside a blue lamp. Clean composition, high quality, studio lighting.' --preset quality --seed 729 --output evidence/media-quality-20261104/image-quality.png
```

### Reference edit: blue lamp to green

```sh
bash shared/bin/run-image.sh 'A detailed studio photograph of the same small red robot reading the same open book, now beside a green lamp instead of a blue lamp. Preserve the robot, book, background, camera angle and composition.' --preset quality --seed 729 --reference-image evidence/media-quality-20261104/image-quality.png --output evidence/media-quality-20261104/image-reference.png
```

### Short video: 384 × 224

```sh
bash shared/bin/run-video.sh 'A yellow toy car rolling slowly from left to right on a plain wooden table, natural daylight, fixed camera, steady motion.' --preset quality --seed 729 --output evidence/media-quality-20261104/video-quality.mp4
```

### Short video: 512 × 288

```sh
bash shared/bin/run-video.sh 'A yellow toy car rolling slowly from left to right on a plain wooden table, natural daylight, fixed camera, steady motion.' --preset detail-short --seed 729 --output evidence/media-quality-20261104/video-detail-short.mp4
```

## Evidence

Each successful worker writes a .evidence.json and a .resources.json. Failed/admission-blocked runs retain resource evidence. Compare subject/composition/detail and inspect video frames across time; do not equate pixel count with improved quality. Framework peak memory and process RSS are different measurements.

The structured state is docs/media-quality-state.json. The maintained HTML report is reports/local-agent-models.html; regenerate with node shared/bin/build-report.mjs. Report UI rendering is separately unverified. The previous candidate ZIP predates these changes and has not been rebuilt.
