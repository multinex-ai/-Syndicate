"""Model-driven skill read and exact MemQ setup-plan recall; no new remote write."""
import json
from pathlib import Path
from rust_agent import run_agent
from memq_client import PRIVATE,ROOT,check_scope

def main():
 check_scope(ROOT)
 setup=json.loads((PRIVATE/'setup-roundtrip.json').read_text())
 if not setup.get('passed'):raise RuntimeError('Independent memory round trip must pass first')
 directory=ROOT/'evidence/setup-20261102/memory-agent-run';workspace=directory/'workspace'
 objective=(f'Read the installed memq-operations skill, check MemQ status, inspect its available operations, and use plan_state_read to read plan {setup["plan_id"]}. '
            'Write memory-receipt.json in your workspace with JSON key verification_nonce containing the exact verification_nonce from that remote plan. '
            'Do not create or update any memory. Report limitations if the read fails.')
 record=run_agent('coordinator',objective,workspace,directory,max_turns=12,timeout=900)
 events=record.get('tools',[])
 skill=any(e['tool']=='read_skill' and e['response'].get('ok') for e in events)
 read=any(e['tool']=='memq_call' and e['response'].get('ok') and e['response'].get('result',{}).get('live_identity_verified') is True and e.get('arguments',{}).get('name')=='plan_state_read' for e in events if isinstance(e.get('arguments'),dict))
 try:nonce=json.loads((workspace/'memory-receipt.json').read_text()).get('verification_nonce')
 except (OSError,ValueError):nonce=None
 passed=record.get('status')=='model_finished' and skill and read and nonce==setup['nonce']
 evidence={'passed':passed,'skill_read_observed':skill,'live_memory_read_observed':read,'live_scope_receipt_verified':read,'nonce_matches':nonce==setup['nonce'],'status':record.get('status'),'model':'HauhauCS/Qwen3.5-9B-Local-HauhauCS-Aggressive Q4_K_M'}
 (directory.parent/'memory-agent-acceptance.json').write_text(json.dumps(evidence,indent=2))
 path=directory.parent/'local-agent-memory-skills.json';data=json.loads(path.read_text());data['model_driven_memory_test_passed']=passed;path.write_text(json.dumps(data,indent=2))
 print(json.dumps(evidence));return 0 if passed else 1

if __name__=='__main__':raise SystemExit(main())
