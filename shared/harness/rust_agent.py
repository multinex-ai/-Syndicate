"""Acceptance-suite adapter: runs the Rust agent loop (local-agents-rs agent) and
returns its result.json, so the existing qualification scripts exercise the same
native loop and tools that MCP jobs use. Test glue only; no agent logic lives here."""
import json
import subprocess
from pathlib import Path

from runtime import ROOT

BINARY = ROOT / 'shared/rust-harness/target/release/local-agents-rs'


def command(role, objective, workspace, run_dir, max_turns=16, timeout=600, allowed_hosts=(), vision=False,
            dispatch_limit=0, scope_task=None, desktop_window_title=None):
    argv = [str(BINARY), 'agent', role, objective, '--workspace', str(workspace), '--run-dir', str(run_dir),
            '--max-turns', str(max_turns), '--timeout', str(timeout), '--dispatch-limit', str(dispatch_limit)]
    for host in allowed_hosts:
        argv += ['--allow-host', host]
    if vision:
        argv.append('--vision')
    if scope_task:
        argv += ['--scope-task', scope_task]
    if desktop_window_title:
        argv += ['--desktop-window-title', desktop_window_title]
    return argv


def _run(argv, run_dir, timeout):
    run_dir = Path(run_dir)
    run_dir.mkdir(parents=True, exist_ok=True)
    with (run_dir / 'rust-agent.log').open('w') as log:
        subprocess.run(argv, stdout=log, stderr=subprocess.STDOUT, timeout=timeout + 120, check=False)
    return json.loads((run_dir / 'result.json').read_text())


def run_agent(role, objective, workspace, run_dir, max_turns=16, timeout=600, allowed_hosts=(), vision=False,
              scope_task=None, desktop_window_title=None):
    return _run(command(role, objective, workspace, run_dir, max_turns, timeout, allowed_hosts, vision,
                        0, scope_task, desktop_window_title), run_dir, timeout)


def orchestrate(role, objective, workspace, run_dir, max_turns, timeout, allowed_hosts, vision, limit):
    return _run(command(role, objective, workspace, run_dir, max_turns, timeout, allowed_hosts, vision, limit), run_dir, timeout)
