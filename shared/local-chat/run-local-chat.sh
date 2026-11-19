#!/bin/bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Discover Codex CLI executable
if [ -n "${CODEX_BIN:-}" ] && [ -x "$CODEX_BIN" ]; then
  : # Use user-specified CODEX_BIN
elif command -v codex >/dev/null 2>&1; then
  CODEX_BIN="$(command -v codex)"
elif [ -x "$REPO_ROOT/dist/codex" ]; then
  CODEX_BIN="$REPO_ROOT/dist/codex"
elif [ -x "$REPO_ROOT/evidence/terminal-chat-20261109/codex-update/candidate/codex-aarch64-apple-darwin" ]; then
  CODEX_BIN="$REPO_ROOT/evidence/terminal-chat-20261109/codex-update/candidate/codex-aarch64-apple-darwin"
else
  echo "Error: 'codex' CLI not found. Please install Codex (e.g. 'brew install codex' or npm i -g @openai/codex) or set CODEX_BIN." >&2
  exit 1
fi

if /usr/sbin/lsof -nP -iTCP:18087 -sTCP:LISTEN >/dev/null 2>&1; then
  echo "Local chat port 18087 is already in use. Please close the existing chat session first." >&2
  exit 1
fi

cleanup() {
  if [ -n "${ADAPTER_PID:-}" ]; then
    kill -TERM "$ADAPTER_PID" 2>/dev/null || true
    wait "$ADAPTER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

export LOCAL_AGENTS_ROOT="$REPO_ROOT"
export LOCAL_AGENTS_LOW_MEMORY=1
export LOCAL_AGENTS_TOLERATE_WARNING_GIB=2.0
export LOCAL_AGENTS_CONTEXT_SIZE=24576
export LOCAL_AGENTS_MAX_SWAP_GROWTH_GIB=3.0

MNX_MANAGED_INFERENCE=1 node "$SCRIPT_DIR/local-responses-adapter.mjs" >"$SCRIPT_DIR/local-responses-adapter.log" 2>&1 &
ADAPTER_PID=$!

for _ in $(seq 1 50); do
  kill -0 "$ADAPTER_PID" 2>/dev/null || { cat "$SCRIPT_DIR/local-responses-adapter.log" >&2; exit 1; }
  if curl --silent --fail http://127.0.0.1:18087/health >/dev/null; then break; fi
  sleep .1
done
curl --silent --fail http://127.0.0.1:18087/health >/dev/null

echo "Local Qwen Chat • Standalone Syndicate Harness²"
echo "Qwen 3.5 9B loads on-demand per turn, then unloads before worker tools execute."
echo "Exit with /quit. Resume later with: agents-chat resume --last"

env CODEX_HOME="$SCRIPT_DIR/isolated-home" "$CODEX_BIN" --model local-agent -c 'model_provider="local_uncensored"' -C "$REPO_ROOT" "$@"
