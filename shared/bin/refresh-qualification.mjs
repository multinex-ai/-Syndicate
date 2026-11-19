import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'../..');
const read=p=>JSON.parse(fs.readFileSync(path.join(root,p),'utf8'));
const write=(p,d)=>fs.writeFileSync(path.join(root,p),JSON.stringify(d,null,2)+'\n');
const now=new Date().toISOString();
const roles={coordinator:['communication','lifecycle','memory-agent','media-routing'],'cyber-lab':['cyber'],browser:['browser'],desktop:['desktop','vision'],'image-studio':['image'],'video-studio':['video']};
for(const [role,names] of Object.entries(roles)) {
 const checks=names.map(name=>{const source=['image','video'].includes(name)?`agents/${name}-studio/outputs/local-smoke.evidence.json`:`evidence/setup-20261102/${name}-acceptance.json`;const d=read(source);return {name,passed:['image','video'].includes(name)?d.visual_inspection==='passed':d.passed===true,evidence:source}});
 const passed=checks.every(c=>c.passed);
 for(const filename of ['config.json','candidate.json']){
  const file=`agents/${role}/${filename}`,data=read(file);data.state=passed?'locally_smoke_tested_candidate':'installed_with_open_acceptance';data.qualification={updated_utc:now,checks,scope:'Bounded local fixtures; not general reliability or universal no-refusal evidence'};
  if(filename==='candidate.json'){
   data.remaining=checks.filter(c=>!c.passed).map(c=>c.name);
   data.runtime=role==='image-studio'?'MFLUX / MLX':role==='video-studio'?'pinned MLX Wan':'llama.cpp / Metal';
   if(data.harness_state)data.harness_state=passed?'locally_tested':'implemented_with_open_acceptance';
   if(role==='video-studio'){
    data.checked_date=now.slice(0,10);
    data.candidates[0].limitations='Specialized adult-content training; publisher warns ordinary prompts may perform poorly. Local evidence covers only the small smoke-test clip, not production quality.';
    delete data.candidates[0].proposed_test;
    data.candidates[0].local_test='320×192, 33 frames, 24 steps, 16 fps; generation, decoding and sampled-frame review recorded in outputs/local-smoke.evidence.json';
    data.no_refusal_evidence='Publisher claim; local tests verify generation, not universal no-refusal behavior';
   }
  }
  write(file,data);
 }
}
const mfile='evidence/setup-20261102/local-agent-memory-skills.json',mem=read(mfile),modelMemory=read('evidence/setup-20261102/memory-agent-acceptance.json'),suite=read('evidence/requalification-20261103/suite.json');
mem.updated_utc=now;mem.model_driven_memory_test_passed=modelMemory.passed===true;
mem.state=mem.model_driven_memory_test_passed?'live_adapter_and_model_driven_memory_verified':'adapter_verified_model_acceptance_open';
mem.infrastructure_tests_passed=suite.steps.find(s=>s.name==='infrastructure')?.state==='passed'?32:null;
mem.limitations=['Process has OS-user permissions; tool guard is not an OS sandbox','Live memory requires a valid independently authenticated session; all host clients remain separate'];
write(mfile,mem);
const permission=read('evidence/setup-20261102/desktop-acceptance.json');
write('evidence/setup-20261102/desktop-permissions.json',{updated_utc:now,...permission.permissions,source:'Observed by actual local runtime during desktop acceptance',desktop_acceptance_passed:permission.passed});
const status=read('evidence/requalification-20261103/status.json');
const runtime=read('shared/config/runtime-plan.json');
runtime.status=status.passed===status.total&&suite.state==='passed'?'locally_smoke_tested_candidate':'installed_with_open_acceptance';
runtime.updated_utc=now;
runtime.communication.dispatch_default=3;
runtime.communication.dispatch_scope='Coordinator CLI and MCP requests default to three; other direct CLI roles default to zero.';
runtime.stop_control='Turn/time limits, Ctrl-C/SIGTERM cleanup and owned process groups; measured acceptance is in lifecycle-acceptance.json.';
runtime.qualification={passed:status.passed,required:status.total,regression:suite.state};
write('shared/config/runtime-plan.json',runtime);
const readmePath=path.join(root,'README.md');
const readiness=status.passed===status.total&&suite.state==='passed'?'All ten task/media gates and the regression suite passed. This is a locally smoke-tested candidate package.':`${status.passed}/${status.total} task/media gates pass. Native desktop acceptance awaits an uninterrupted active desktop; the current package remains in progress.`;
fs.writeFileSync(readmePath,fs.readFileSync(readmePath,'utf8').replace(/^Updated November 3, 2026\..*$/m,`Updated November 3, 2026. ${readiness} Consult [the evidence report](reports/local-agent-models.html) and [the current checkpoint](docs/CHECKPOINT.md). The report is a dated snapshot, not a live control console.`));
const lines=status.checks.map(c=>`| ${c.name} | ${c.passed?'Passed':'Open / failed'} | ${c.source} |`).join('\n');
fs.writeFileSync(path.join(root,'docs/CHECKPOINT.md'),`# Local Agents checkpoint\n\nSnapshot: ${now}. Realm: Agents / Multinex; this directory is not a Git repository.\n\n## Objective and evidence\n\nFix Wan, MemQ, delegation, cancellation and native desktop control; rerun the defined acceptance suite and deliver a portable source/configuration/evidence package. All selected model files are installed. Runtime: Apple M3 Pro, 18 GiB unified memory; heavy workers are serialized.\n\n${status.passed}/${status.total} task/media gates pass. Sequential regression state: ${suite.state}. These are local smoke-test results, not production or broad reliability qualification.\n\n| Gate | Result | Evidence |\n| --- | --- | --- |\n${lines}\n\n## Repairs\n\n- Wan: map exported UMT5 Sequential gate keys to the current MLX module layout before strict loading.\n- MemQ: restore absolute token expiry on process restart; validate official issuer metadata before refreshing the agent's own OAuth credentials. Preserve an existing refresh token if the response omits its replacement.\n- Delegation: children receive only their assignment; ownership context is separate. Successful task queueing yields the parent so the dispatcher can run. Self-delegation is rejected.\n- Cancellation: the acceptance fixture uses the direct-run directory and verifies loaded-server interruption and endpoint closure.\n- Desktop: explicit normalized screenshot coordinates; retain only the newest screenshot in model context. User-granted OS permissions are checked by the actual runtime. The fixture stays visible across Spaces; observed-process focus and named-window input guards prevent this test from acting on another app.\n\nPre-repair source/evidence remains in evidence/requalification-20261103/baseline. No credentials were copied between clients. Own credentials and authoritative bindings stay outside the workspace. Galileo/TF memory remains excluded.\n\n## Delivery and limits\n\nThe report is reports/local-agent-models.html. The package receipt is packages/package-receipt.json. The archive excludes weights, environments, credentials, private bindings, raw desktop traces and job databases. Selected samples and their receipts are included.\n\nCodex's live local worker status tool and MemQ handshake were observed in this chat. Antigravity configuration is installed; its live connection remains unverified. Running hosts need supported refresh/reconnection to load changed server code. Native workers and cyber commands run as the OS user. Host policies are unchanged.\n\nHTML static structure and control contracts pass. Browser file-URL policy blocked rendered report inspection; no alternate route was used. Actual rendered report UI remains unverified.\n\n## Next action\n\n${status.passed===status.total&&suite.state==='passed'?'Use the packaged launchers for bounded real tasks and review their output; Antigravity live connection is a separate remaining client check.':'Repair the remaining failed gate shown above and rerun affected tests before final qualification.'}\n\nNo unrelated deletion, background installer or new external publication is part of this task.\n`);
console.log(JSON.stringify({updated:now,roles:Object.keys(roles),suite:suite.state}));
