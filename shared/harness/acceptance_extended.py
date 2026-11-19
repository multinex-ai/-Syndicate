"""Visual grounding and inter-agent checks using independently observed fixtures."""
import json
from pathlib import Path
import re
import secrets
import sys
import time
import signal
import socket
import subprocess

from rust_agent import run_agent
from runtime import ROOT, model_server, request_json, terminate

E = ROOT / 'evidence/setup-20261102'


def vision():
    import base64
    from PIL import Image, ImageDraw, ImageFont
    directory = E / 'vision-run'
    directory.mkdir(parents=True, exist_ok=True)
    marker = 'VIS-' + secrets.token_hex(3).upper()
    canvas = Image.new('RGB', (900, 500), '#f7f9fc')
    draw = ImageDraw.Draw(canvas)
    font = ImageFont.truetype('/System/Library/Fonts/Supplemental/Arial.ttf', 44)
    draw.text((50, 35), 'Local visual fixture', font=font, fill='#142341')
    draw.text((50, 110), marker, font=font, fill='#142341')
    draw.rectangle((60, 245, 330, 405), fill='#2068cf')
    draw.text((110, 295), 'SAVE', font=font, fill='white')
    draw.rectangle((520, 245, 840, 405), fill='#cf3046')
    draw.text((570, 295), 'CANCEL', font=font, fill='white')
    image_path = directory / 'fixture.png'
    canvas.save(image_path)
    started = time.monotonic()
    with model_server(directory, vision=True):
        response = request_json('/v1/chat/completions', {
            'model': 'local-agent', 'temperature': 0, 'max_tokens': 400,
            'messages': [{'role': 'user', 'content': [
                {'type': 'text', 'text': 'Read this screenshot. Return only JSON with keys marker (the VIS- code), save_center (x,y normalized to 0–1000 across the full image, NOT pixels), and cancel_color (a color word).'},
                {'type': 'image_url', 'image_url': {'url': 'data:image/png;base64,' + base64.b64encode(image_path.read_bytes()).decode()}}
            ]}]}, timeout=240)
    answer = response['choices'][0]['message']['content']
    try:
        parsed = json.loads(re.search(r'\{.*\}', answer, re.S).group())
        center = parsed['save_center']
        nx, ny = (center['x'], center['y']) if isinstance(center, dict) else center
        if any(isinstance(v, bool) or not isinstance(v, (int, float)) for v in (nx, ny)):
            raise ValueError('Coordinates must be numbers')
        if not 0<=nx<=1000 or not 0<=ny<=1000:raise ValueError('Coordinates outside the declared normalized grid')
        x, y = nx*900/1000, ny*500/1000
        passed = parsed['marker'] == marker and 60 <= x <= 330 and 245 <= y <= 405 and 'red' in parsed['cancel_color'].lower()
    except Exception:
        parsed = None
        passed = False
    result = {'passed': passed, 'expected_marker': marker, 'response': answer,
              'parsed': parsed, 'coordinate_space':'normalized_1000; independently mapped to 900×500 pixels', 'seconds': round(time.monotonic() - started, 2),
              'scope': 'Synthetic screenshot reading and button localization; native input tested separately.'}
    (E / 'vision-acceptance.json').write_text(json.dumps(result, indent=2))
    return result


def communication():
    from rust_agent import orchestrate
    marker = 'LOCAL-MESSAGE-' + secrets.token_hex(6)
    filename = 'communication-' + secrets.token_hex(6) + '.txt'
    directory = E / 'communication-run'
    objective = f'Use send_message to ask the cyber-lab agent to write a file named {filename} containing exactly {marker} in its workspace, then read it back and report. Queue this one task. Do not write that file yourself.'
    # Planner run then one dispatched child, both on the Rust loop.
    orchestrate('coordinator', objective, ROOT / 'agents/coordinator/workspace', directory, 6, 420, [], False, 1)
    parent = json.loads((directory / 'planner/result.json').read_text())
    dispatched = json.loads((directory / 'dispatch-0/dispatch.json').read_text())
    target = ROOT / 'agents/cyber-lab/workspace' / filename
    inbox = [json.loads(p.read_text()) for p in (ROOT / 'agents/coordinator/inbox').glob('*.json')]
    receipts = [m for m in inbox if m.get('status') == 'receipt' and
                m.get('in_reply_to') in {p['id'] for p in dispatched['processed']}]
    result = {'passed': parent['status'] == 'model_finished' and target.is_file() and
              target.read_text().strip() == marker and bool(receipts) and
              any(p['status'] == 'completed' for p in dispatched['processed']),
              'parent_status': parent['status'], 'child_receipts': receipts,
              'file_exists': target.is_file(), 'expected_marker': marker,
              'target': str(target), 'external_messages_sent': False,
              'scope': 'Local coordinator task → selected-model child execution → independently checked file → return receipt.'}
    (E / 'communication-acceptance.json').write_text(json.dumps(result, indent=2))
    return result


def lifecycle():
    directory = E / 'budget-run'
    bounded = run_agent('cyber-lab',
        'Use write_file to create budget-test.txt containing BUDGET-729, then read it back and report its contents.',
        ROOT / 'agents/cyber-lab/workspace/budget-test', directory, max_turns=1, timeout=300)
    budget_ok = bounded['status'] == 'turn_budget_exhausted' and bool(bounded['tools'])
    cancel_dir = E / 'cancellation-run'
    cancel_dir.mkdir(parents=True, exist_ok=True)
    state_path = cancel_dir / 'server-process.json'
    state_path.unlink(missing_ok=True)
    with (cancel_dir / 'worker.log').open('w') as log:
        worker = subprocess.Popen([
            str(ROOT / 'shared/rust-harness/target/release/local-agents-rs'), 'agent', 'coordinator',
            'Write a detailed fictional travelogue of at least 1500 words. Start writing immediately and do not use tools.',
            '--run-dir', str(cancel_dir), '--dispatch-limit', '0'], stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            deadline = time.monotonic() + 220
            healthy = False
            while time.monotonic() < deadline and worker.poll() is None:
                try:
                    healthy = json.loads(state_path.read_text())['state'] == 'healthy'
                except (OSError, ValueError, KeyError):
                    pass
                if healthy:
                    break
                time.sleep(.25)
            if not healthy:
                raise RuntimeError('Cancellation fixture never reached a healthy model server')
            time.sleep(3)
            worker.send_signal(signal.SIGINT)
            worker.wait(timeout=20)
        finally:
            terminate(worker)
    cancelled = json.loads((cancel_dir / 'result.json').read_text())
    server = json.loads(state_path.read_text())
    with socket.socket() as probe:
        port_closed = probe.connect_ex(('127.0.0.1', 8087)) != 0
    result = {'passed': budget_ok and cancelled['status'] == 'cancelled' and
              server['state'] == 'terminated' and port_closed,
              'budget_status': bounded['status'], 'budget_tool_calls': len(bounded['tools']),
              'cancellation_status': cancelled['status'], 'server': server,
              'endpoint_closed': port_closed,
              'scope': 'Selected-model turn-budget enforcement and interrupt cleanup with a loaded Metal server.'}
    (E / 'lifecycle-acceptance.json').write_text(json.dumps(result, indent=2))
    return result


if __name__ == '__main__':
    selected = {'vision': vision, 'communication': communication, 'lifecycle': lifecycle}[sys.argv[1]]
    result = selected()
    print(json.dumps(result, indent=2))
    raise SystemExit(0 if result['passed'] else 1)
