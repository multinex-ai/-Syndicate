"""Build an evidence-indexed source/config archive without weights or credentials."""
import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[2]
E = ROOT / 'evidence/setup-20261102'


def load(path):
    return json.loads(path.read_text()) if path.exists() else None


def main():
    results = {}
    for name in ['cyber', 'browser', 'vision', 'communication', 'desktop', 'lifecycle', 'memory-agent', 'media-routing']:
        evidence = E / (name + '-acceptance.json')
        record = load(evidence)
        results[name] = {'passed': bool(record and record.get('passed')),
                         'state': 'pending' if record is None else record.get('status', record.get('agent_status', 'recorded')),
                         'evidence': str(evidence.relative_to(ROOT))}
    for kind in ['image', 'video']:
        evidence = ROOT / f'agents/{kind}-studio/outputs/local-smoke.evidence.json'
        record = load(evidence)
        results[kind] = {'passed': bool(record and record.get('visual_inspection') == 'passed'),
                         'state': 'pending' if record is None else record.get('status'),
                         'evidence': str(evidence.relative_to(ROOT))}
    integration = load(E/'local-agent-memory-skills.json') or {}
    integration_ready = bool(integration.get('live_identity_verified') and integration.get('live_memory_roundtrip_verified') and integration.get('skills_tests_passed') and integration.get('model_driven_memory_test_passed'))
    integration_ready = integration_ready and bool((load(E/'media-routing-acceptance.json') or {}).get('passed'))
    suite=load(ROOT/'evidence/requalification-20261103/suite.json') or {}
    subprocess.run([sys.executable, str(ROOT / 'shared/bin/build_status_report.py')], check=True)
    subprocess.run([sys.executable, str(ROOT / 'shared/bin/check_report.py')], check=True)
    complete = all(item['passed'] for item in results.values()) and integration_ready and suite.get('state')=='passed' and bool((load(E/'report-static-checks.json') or {}).get('passed'))
    status = {'created_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'state': 'locally_tested_candidate_package' if complete else 'qualification_in_progress',
              'passed': sum(item['passed'] for item in results.values()), 'required_checks': len(results),
              'denominator': 'Eight model-driven task/integration checks plus visually reviewed image and video output.',
              'checks': results,
              'memory_skills_integration_ready': integration_ready,
              'regression_suite_state':suite.get('state','not_run'),
              'no_refusal_status': 'Publisher explicitly labels selected candidates local. No universal behavioral guarantee is asserted.',
              'weights_included': False, 'credentials_included': False,
              'weights_location': str(ROOT / 'shared/models')}
    (E / 'package-status.json').write_text(json.dumps(status, indent=2))
    files = [ROOT / 'README.md', ROOT / '.gitignore', ROOT / 'reports/local-agent-models.html', ROOT / '.codex/config.toml', ROOT / '.agents/mcp_config.json']
    for folder, extensions in [('docs', {'.md'}), ('shared/config', {'.json', '.txt', '.in'}),
                               ('shared/harness', {'.py'}), ('shared/bin', {'.py', '.sh', '.mjs', '.js', '.css', '.html'}), ('shared/skills', {'.md'}),
                               ('shared/vendor/mlx-wan', {'.py', '.md', '.txt'})]:
        files.extend(p for p in (ROOT / folder).rglob('*') if p.is_file() and p.suffix in extensions and '__pycache__' not in p.parts)
    files.append(ROOT / 'shared/vendor/mlx-wan/LICENSE')
    for role in ['coordinator', 'cyber-lab', 'browser', 'desktop', 'image-studio', 'video-studio']:
        for name in ['config.json', 'candidate.json']:
            files.append(ROOT / 'agents' / role / name)
    evidence_names = ['package-status.json', 'wan-runtime-source.json', 'qwen-download.json', 'flux-quantized-base-manifest.json',
                      'image-base-download.json', 'image-encoder-download.json', 'image-encoder-conversion.json',
                      'video-download.json', 'video-download-manifest.json', 'desktop-permissions.json',
                      'tool-boundary-tests.log', 'runtime-tests.log', 'browser-driver-check.json', 'media-runtime-check.json', 'installed-tool-tests.log',
                      'installed-runtime-tests.log', 'report-static-checks.json', 'mcp-job-tests.json', 'mcp-job-tests.log', 'mcp-protocol-tests.json', 'client-integration.json', 'local-agent-memory-skills.json', 'memory-skills-tests.log']
    evidence_names += ['orchestration-tests.json']+[name + '-acceptance.json' for name in ['cyber', 'browser', 'vision', 'communication', 'desktop', 'lifecycle','memory-agent','media-routing']]
    files.extend(E / name for name in evidence_names)
    files.extend(ROOT/'evidence/requalification-20261103'/name for name in ['suite.json','infrastructure.log','memq-refresh.json','memq-checkpoint.json','status.json','report-visual.json'])
    # Synthetic visual fixture and generated media only. Desktop screenshots and
    # raw window/task logs are deliberately excluded from the portable archive.
    files.append(E / 'vision-run/fixture.png')
    for kind in ['image', 'video']:
        files.extend((ROOT / f'agents/{kind}-studio/outputs').glob('local-smoke*'))
    files = sorted(set(p for p in files if p.is_file()))
    entries = {str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest() for path in files}
    out = ROOT / 'packages'
    out.mkdir(exist_ok=True)
    target = out / ('local-agent-candidates-final.zip' if complete else 'local-agent-candidates-in-progress.zip')
    with zipfile.ZipFile(target, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
        for path in files:
            archive.write(path, path.relative_to(ROOT))
        archive.writestr('PACKAGE-MANIFEST.json', json.dumps({'status': status, 'files_sha256': entries}, indent=2))
    with zipfile.ZipFile(target) as archive:
        if archive.testzip() is not None:
            raise RuntimeError('Archive integrity check failed')
        manifest = json.loads(archive.read('PACKAGE-MANIFEST.json'))
        for name, expected in manifest['files_sha256'].items():
            if hashlib.sha256(archive.read(name)).hexdigest() != expected:
                raise RuntimeError('Archive manifest mismatch: ' + name)
        if any(name.startswith(('shared/models/', 'shared/runtime/')) or
               any(part in name for part in ('oauth-tokens', 'oauth-client', 'hf-token', 'desktop-run/', 'baseline/'))
               for name in archive.namelist()):
            raise RuntimeError('Private/runtime material unexpectedly included in archive')
    receipt = {'path': str(target), 'bytes': target.stat().st_size,
               'sha256': hashlib.sha256(target.read_bytes()).hexdigest(), 'files': len(files) + 1,
               'qualification': status['state'], 'passed_checks': status['passed'], 'required_checks': status['required_checks'],
               'archive_crc_verified': True, 'manifest_hashes_verified': True, 'excluded_material_check_passed': True}
    (out / 'package-receipt.json').write_text(json.dumps(receipt, indent=2))
    print(json.dumps(receipt, indent=2))


if __name__ == '__main__':
    main()
