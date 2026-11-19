import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
from media_policy import GIB, MediaGuard, admission_reason, runtime_reason

# media_policy.MediaGuard remains only for chat_inference.py; the media workers use the Rust guard (src/media.rs).
class MediaPolicyTests(unittest.TestCase):
    def setUp(self):
        policy = patch('media_policy.memory_guard_enabled', return_value=True)
        policy.start()
        self.addCleanup(policy.stop)
        for k in ['LOCAL_AGENTS_LOW_MEMORY', 'LOCAL_AGENTS_TOLERATE_WARNING_GIB', 'LOCAL_AGENTS_MAX_SWAP_GROWTH_GIB']:
            os.environ.pop(k, None)

    def sample(self, **updates):
        return dict(pressure_level=1, available_bytes=5*GIB, disk_free_bytes=20*GIB,
                    worker_rss_bytes=2*GIB, swap_used_bytes=10*GIB, **updates)

    def test_admission_blocks_pressure_low_memory_and_disk(self):
        baseline=self.sample()
        self.assertIsNone(admission_reason(baseline))
        for changes in ({'pressure_level':2},{'available_bytes':3*GIB},{'disk_free_bytes':11*GIB}):
            self.assertIsNotNone(admission_reason(baseline | changes))

    def test_existing_swap_is_not_a_rejection_but_growth_is(self):
        baseline=self.sample()
        self.assertIsNone(runtime_reason(baseline,baseline))
        for changes in ({'pressure_level':4},{'worker_rss_bytes':9*GIB},{'swap_used_bytes':11*GIB}):
            self.assertIsNotNone(runtime_reason(baseline | changes,baseline))

    def test_memory_opt_out_preserves_disk_reserve(self):
        with patch('media_policy.memory_guard_enabled', return_value=False):
            sample = self.sample() | {'pressure_level': 4, 'available_bytes': 0, 'worker_rss_bytes': 12*GIB}
            self.assertIsNone(admission_reason(sample))
            self.assertIsNone(runtime_reason(sample, sample))
            self.assertIsNotNone(admission_reason(sample | {'disk_free_bytes': GIB}))

    def test_watchdog_stops_only_its_real_subprocess(self):
        with tempfile.TemporaryDirectory() as directory:
            script='''from media_policy import *
from unittest.mock import patch
import time,sys
baseline={'pressure_level':1,'available_bytes':5*GIB,'disk_free_bytes':20*GIB,'worker_rss_bytes':GIB,'swap_used_bytes':0}
calls=0
def fixture():
 global calls
 calls+=1
 return baseline if calls==1 else baseline | {'pressure_level':4}
with patch('media_policy.snapshot',side_effect=fixture), patch('media_policy.memory_guard_enabled',return_value=True):
 with MediaGuard(sys.argv[1]): time.sleep(30)
'''
            env={**os.environ,'PYTHONPATH':str(Path(__file__).resolve().parent)}
            p=subprocess.Popen([sys.executable,'-c',script,str(Path(directory)/'out.png')],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,env=env)
            try:
                self.assertNotEqual(p.wait(timeout=12),0)
                import json
                evidence=json.loads((Path(directory)/'out.resources.json').read_text())
                self.assertEqual(evidence['status'],'resource_stopped')
                self.assertIn('Critical',evidence['reason'])
            finally:
                if p.poll() is None:p.kill();p.wait()


if __name__=='__main__':unittest.main()
