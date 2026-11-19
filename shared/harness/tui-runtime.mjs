import fs from 'node:fs';
import path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { memoryQuery, compactMemoryContext, checkpointInput } from './memory-context.ts';
const execAsync=promisify(execFile);

export function createRuntime({ state, ROOT, WORKSPACE, RUST_HARNESS, MEMQ_PYTHON, MEMQ_CLIENT }) {
// --- Hardware Telemetry Sampler ---
let sampling=false;
async function sampleTelemetry() {
  if(sampling)return;
  sampling=true;
  try {
    if (fs.existsSync(RUST_HARNESS)) {
      const {stdout:resJson} = await execAsync(RUST_HARNESS, ['resources'], { timeout: 1500, maxBuffer: 64000 });
      const res = JSON.parse(resJson);
      state.telemetry.availableGiB = (res.available_bytes / (1024 ** 3)).toFixed(2);
      state.telemetry.totalGiB = (res.physical_bytes / (1024 ** 3)).toFixed(1);
      state.telemetry.rssGiB = ((res.worker_rss_bytes || 5620000000) / (1024 ** 3)).toFixed(2);
      state.telemetry.pressureLevel = res.pressure_level || 1;
      state.telemetry.swapUsedMiB = Math.round((res.swap_used_bytes || 0) / (1024 ** 2));
    }
  } catch (e) {
    // Keep last sample
  } finally { sampling=false; }

  // Check Heavy Lock status
  const lockFile = path.join(ROOT, 'shared/runtime/heavy-model.lock');
  try {
    if (fs.existsSync(lockFile)) {
      // Check if locked
      state.telemetry.lockState = state.isProcessing ? 'ACTIVE' : 'IDLE';
    }
  } catch {
    state.telemetry.lockState = 'IDLE';
  }
}

// --- MemQ Context Operations ---
async function queryMemQContext(query) {
  if (!state.memqEnabled) return null;
  try {
    state.traceLogs.push({
      type: 'MEMQ',
      text: `Querying episodic memory for relevant context: "${query.slice(0, 40)}..."`,
      time: new Date().toLocaleTimeString(),
    });
    // Bounded search
    const args = JSON.stringify(memoryQuery(query, WORKSPACE));
    const {stdout: output} = await execAsync(MEMQ_PYTHON, [MEMQ_CLIENT, 'call', '--name', 'query_memory', '--project', WORKSPACE, '--task', query, '--arguments', args], { timeout: 30000, maxBuffer: 256000 });
    const parsed = JSON.parse(output);
    const context=compactMemoryContext(parsed);
    if(context)state.traceLogs.push({type:'MEMQ_RECALL',text:'Loaded up to three short memory summaries; details remain available on demand.',time:new Date().toLocaleTimeString()});
    return context;
  } catch (err) {
    state.traceLogs.push({
      type: 'MEMQ_NOTE',
      text: `MemQ local fallback active: ${err.message || 'connection skipped'}`,
      time: new Date().toLocaleTimeString(),
    });
  }
  return null;
}

async function commitMemQEpisode(objective, answer, turns, toolsExecuted, runDirectory) {
  if (!state.memqEnabled || !toolsExecuted || toolsExecuted.length === 0) return;
  try {
    const args=JSON.stringify(checkpointInput(objective, answer, WORKSPACE, runDirectory, toolsExecuted.map(t=>t.tool)));
    const {stdout}=await execAsync(MEMQ_PYTHON, [MEMQ_CLIENT, 'call', '--name', 'add_memory', '--project', WORKSPACE, '--task', objective, '--arguments', args], {timeout:30000,maxBuffer:256000});
    const reply=JSON.parse(stdout);
    if (!reply.ok || !reply.result?.live_identity_verified) return;
    state.traceLogs.push({
      type: 'MEMQ_COMMIT',
      text: `MemQ returned a checkpoint receipt after live identity verification.`,
      time: new Date().toLocaleTimeString(),
    });
  } catch (e) {
    state.traceLogs.push({type:'MEMQ_NOTE',text:'Checkpoint synchronization unconfirmed; use the local run receipt. No automatic retry.',time:new Date().toLocaleTimeString()});
  }
}

return { sampleTelemetry, queryMemQContext, commitMemQEpisode };
}
