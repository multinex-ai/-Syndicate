#!/bin/bash
set -euo pipefail
AGENT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
exec "$AGENT_ROOT/shared/runtime/venv/bin/python" "$AGENT_ROOT/shared/harness/memq_client.py" "${1:-status}"
