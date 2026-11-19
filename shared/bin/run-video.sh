#!/bin/bash
# video worker: Rust owns validation, the heavy lock, memory reclaim and the resource guard;
# only the MLX compute runs in shared/harness/media_kernel.py.
set -euo pipefail
AGENT_ROOT="$(cd -P "$(dirname "$0")/../.." && pwd)"
export PATH="/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin:${PATH:-}"
HARNESS_BINARY="$AGENT_ROOT/target/release/local-agents-rs"
if [ ! -x "$HARNESS_BINARY" ]; then
  HARNESS_BINARY="$AGENT_ROOT/shared/rust-harness/target/release/local-agents-rs"
fi
PRIVATE_DIR="${INFERENCE_PRIVATE_DIR:-$AGENT_ROOT/shared/runtime/private-inference}"
if [ "${LOCAL_AGENTS_FORCE_LOCAL:-0}" != "1" ] && [ "${PRAECO_INFERENCE_MODE:-}" != "local" ] && [ -z "${PRIVATE_INFERENCE_URL:-}" ] && [ -f "$PRIVATE_DIR/harness.env" ]; then
  exec node "$AGENT_ROOT/local-inference-gateway" harness "$HARNESS_BINARY" video "$@"
fi
exec "$HARNESS_BINARY" video "$@"
