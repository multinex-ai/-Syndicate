"""Compatibility entry point for report structure and interaction tests."""
from pathlib import Path
import subprocess
ROOT=Path(__file__).resolve().parents[2]
subprocess.run(['node',str(ROOT/'shared/bin/check-report.mjs')],cwd=ROOT,check=True)
