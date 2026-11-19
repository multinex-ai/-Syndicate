"""Verify native desktop input against an independently instrumented local window."""
import json
import secrets
import subprocess
import sys
import time

from rust_agent import run_agent
from runtime import ROOT, terminate


def main():
    import ApplicationServices
    import Quartz
    evidence = ROOT / 'evidence/setup-20261102'
    permissions = {'accessibility': bool(ApplicationServices.AXIsProcessTrusted()),
                   'screen_recording': bool(Quartz.CGPreflightScreenCaptureAccess())}
    if not all(permissions.values()):
        result = {'passed': False, 'status': 'blocked_by_macos_permissions', 'permissions': permissions,
                  'native_input_attempted': False}
        (evidence / 'desktop-acceptance.json').write_text(json.dumps(result, indent=2))
        print(json.dumps(result))
        return 2
    run_dir = evidence / 'desktop-run'
    run_dir.mkdir(parents=True, exist_ok=True)
    observed_path = run_dir / 'native-observed.json'
    marker = 'DESK-' + secrets.token_hex(4).upper()
    observed_path.unlink(missing_ok=True)
    observed_path.with_suffix('.ready').unlink(missing_ok=True)
    with (run_dir / 'fixture.log').open('w') as log:
        process = subprocess.Popen([sys.executable, str(ROOT / 'shared/harness/desktop_fixture.py'),
                                    str(observed_path), marker], stdout=log, stderr=subprocess.STDOUT,
                                   start_new_session=True)
        try:
            deadline = time.monotonic() + 30
            while not observed_path.with_suffix('.ready').exists():
                if process.poll() is not None or time.monotonic() > deadline:
                    raise RuntimeError('Native fixture did not become ready')
                time.sleep(.2)
            agent = run_agent('desktop',
                'Use desktop_observe and desktop_screenshot to find the window titled Local Agent Desktop Test. '
                'Read the DESK- code visibly displayed inside that window. Click its input, type that code exactly, '
                'and click its Save button. Observe the Saved confirmation. Use only this fixture window and desktop tools; '
                'do not write files or send messages for this test. For visual clicks use coordinate_space=normalized_1000 with x,y in 0–1000 across the full screenshot.',
                ROOT / 'agents/desktop/workspace/acceptance', run_dir, vision=True, max_turns=16, timeout=900,
                desktop_window_title='Local Agent Desktop Test')
            observed = json.loads(observed_path.read_text()) if observed_path.exists() else None
            result = {'passed': bool(observed and observed.get('matched') and agent['status'] == 'model_finished'),
                      'agent_status': agent['status'], 'seconds': agent['seconds'], 'permissions': permissions,
                      'native_observation': observed, 'tool_calls': len(agent['tools']),
                      'scope': 'Actual native screenshot, code reading, mouse/keyboard entry and button activation in a dedicated fixture.'}
            (evidence / 'desktop-acceptance.json').write_text(json.dumps(result, indent=2))
            print(json.dumps(result, indent=2))
            return 0 if result['passed'] else 1
        finally:
            terminate(process)


if __name__ == '__main__':
    raise SystemExit(main())
