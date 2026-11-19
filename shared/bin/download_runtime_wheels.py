"""Fetch slow runtime wheels with bounded ranges and official PyPI SHA-256 hashes."""
import concurrent.futures
import json
from pathlib import Path
import urllib.request

from packaging.tags import sys_tags
from packaging.utils import parse_wheel_filename
from download_parallel import fetch

ROOT = Path(__file__).resolve().parents[2]
PACKAGES = ['torch', 'mlx-metal', 'playwright', 'opencv-python', 'transformers',
            'matplotlib', 'pyobjc-core', 'sympy', 'numpy', 'mlx', 'einops']


def main():
    pins = {}
    for line in (ROOT / 'shared/config/requirements.lock.txt').read_text().splitlines():
        if '==' in line and not line.lstrip().startswith('#'):
            name, version = line.strip().split('==')
            pins[name] = version
    ranking = {tag: i for i, tag in enumerate(sys_tags())}
    manifest = []
    for name in PACKAGES:
        version = pins[name]
        with urllib.request.urlopen(f'https://pypi.org/pypi/{name}/{version}/json', timeout=60) as response:
            metadata = json.load(response)
        eligible = []
        for candidate in metadata['urls']:
            if not candidate['filename'].endswith('.whl'):
                continue
            wheel_tags = parse_wheel_filename(candidate['filename'])[3]
            priorities = [ranking[tag] for tag in wheel_tags if tag in ranking]
            if priorities:
                eligible.append((min(priorities), candidate))
        if not eligible:
            raise ValueError('No compatible wheel for ' + name)
        item = min(eligible, key=lambda x: x[0])[1]
        item.update(package=name, version=version)
        manifest.append(item)
    (ROOT / 'evidence/setup-20261102/runtime-wheel-manifest.json').write_text(json.dumps(manifest, indent=2))

    def download(item):
        return fetch('pypi/' + item['package'], item['version'], item['filename'],
                     item['size'], item['digests']['sha256'],
                     ROOT / 'shared/runtime/wheels' / item['filename'],
                     workers=64, url=item['url'], chunk_bytes=1024*1024)
    with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
        results = list(pool.map(download, manifest))
    (ROOT / 'evidence/setup-20261102/runtime-wheels-download.json').write_text(json.dumps(results, indent=2))


if __name__ == '__main__':
    main()
