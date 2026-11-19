"""Compatibility entry point for the portable Node report builder."""
from pathlib import Path
import subprocess
ROOT=Path(__file__).resolve().parents[2]
subprocess.run(['node',str(ROOT/'shared/bin/build-report.mjs')],cwd=ROOT,check=True)
