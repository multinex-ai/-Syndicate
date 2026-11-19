#!/bin/bash
# Local Agents MCP bridge: the native Rust control plane (jobs, supervisor, agent loop, tools).
set -euo pipefail
AGENT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
export PATH="/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin:${PATH:-}"
export HF_HUB_OFFLINE=1 HF_HUB_DISABLE_TELEMETRY=1
HARNESS_BINARY="$AGENT_ROOT/target/release/local-agents-rs"
if [ ! -x "$HARNESS_BINARY" ]; then
  HARNESS_BINARY="$AGENT_ROOT/shared/rust-harness/target/release/local-agents-rs"
fi
PRIVATE_DIR="${INFERENCE_PRIVATE_DIR:-$AGENT_ROOT/shared/runtime/private-inference}"
if [ -z "${PRIVATE_INFERENCE_URL:-}" ] && [ -f "$PRIVATE_DIR/harness.env" ]; then
  exec node "$AGENT_ROOT/local-inference-gateway" harness "$HARNESS_BINARY" mcp
fi
exec "$HARNESS_BINARY" mcp
