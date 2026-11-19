#!/usr/bin/env node
import {toggleReasoning,toggleFallback,writeControls,inferenceLines} from './tui-reasoning.mjs';
import {createMediaBrowser} from './tui-media.mjs';
import {mediaFile} from './media-viewer.ts';
import {executionPlan} from './tui-routing.mjs';
import {planRoute} from './tui-route-planner.mjs';
import {importClipboardImage} from './clipboard-image.mjs';

/**
 * MULTINEX SOVEREIGN HARNESS — Interactive Terminal UI (TUI)
 *
 * Architecture:
 * - Alternate screen buffer, non-canonical raw keyboard mode, double-buffered rendering
 * - Top navigation cheat sheet ribbon (simplified keybindings)
 * - Real-time telemetry HUD (RAM gauge, VRAM/footprint, tok/s, context window, lock status)
 * - Dual-pane layout: Active Chat Thread (left) & Routing / Thinking Trace (right)
 * - Native OSC 8 clickable hyperlinks (Cmd-Click to open in editor or browser)
 * - In-app parameter tuning modal (:set or Ctrl+O)
 * - Smart workspace auto-binding to $PWD
 * - MemQ closed-loop context compaction & episodic checkpoint recording
 */

import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { PassThrough } from 'node:stream';
import { paneKey, mouseInput } from './terminal-layout.ts';
import { explainRunFailure } from '../private-inference/run-failure.ts';
import { createRuntime } from './tui-runtime.mjs';
import { createView } from './tui-view.mjs';
import { createBudgetMonitor } from './tui-budget.mjs';
import fs from 'node:fs';
import path from 'node:path';
import readline from 'node:readline';
import { fileURLToPath } from 'node:url';

// --- Paths & Workspace ---
const ROOT = path.resolve(fileURLToPath(new URL('.', import.meta.url)), '../..');
const WORKSPACE = process.env.MNX_WORKSPACE || process.cwd();
const workspaceBinary = path.join(ROOT, 'target/release/local-agents-rs');
const RUST_HARNESS = fs.existsSync(workspaceBinary) ? workspaceBinary : path.join(ROOT, 'shared/rust-harness/target/release/local-agents-rs');
const MEMQ_PYTHON = path.join(ROOT, 'shared/runtime/venv/bin/python');
const MEMQ_CLIENT = path.join(ROOT, 'shared/harness/memq_client.py');

import { ESC, CSI, OSC, C_RESET, C_BOLD, C_DIM, C_UNDERLINE, rgb, bgRgb, THEME, link, stripAnsi, truncateString } from './tui-theme.mjs';

// --- Application State ---
const state = {
  reasoningEnabled: true,
  localFallbackEnabled: process.env.LOCAL_AGENTS_ALLOW_LOCAL_FALLBACK === '1',
  controlFile: null,
  mediaFiles: [], mediaIndex: 0,
  attachedImage: null,
  mode: 'INSERT', // 'NORMAL' | 'INSERT' | 'SETTINGS' | 'COMMAND'
  activePane: 'chat', // 'chat' | 'trace'
  budgetLine: 'Runpod credit: checking...',
  budgetWarning: '',
  splitRatio: 0.55,
  traceExpanded: false,
  traceMaxScroll: 0,
  chatMaxScroll: 0,
  pageSize: 10,
  role: 'coordinator',
  modelName: process.env.PRIVATE_INFERENCE_URL ? 'Private inference / fallback OFF' : 'Local Qwen 3.5 9B',
  temperature: 0.0,
  maxTokens: 1600,
  contextSize: 8192,
  vision: false,
  lowMemory: true,
  memqEnabled: true,
  
  // Hardware Telemetry
  telemetry: {
    availableGiB: 6.5,
    totalGiB: 18.0,
    rssGiB: 5.62,
    pressureLevel: 1, // 1=Normal, 2=Warning, 4=Critical
    swapUsedMiB: 0,
    lockState: 'IDLE',
    tokensIn: 0,
    tokensOut: 0,
    tokensPerSec: 0,
    contextUsed: 0,
  },

  // Chat & Log Streams
  chatHistory: [
    {
      role: 'system',
      text: `Multinex Sovereign Harness initialized under MNX-AGI v1.4. Workspace bound to ${WORKSPACE}. MemQ checks identity at use time.`,
      time: new Date().toLocaleTimeString(),
    }
  ],
  traceLogs: [
    {
      type: 'INIT',
      text: `Harness initialized. Smart workspace: ${WORKSPACE}`,
      time: new Date().toLocaleTimeString(),
    },
    {
      type: 'MEMQ',
      text: 'MemQ configured; live identity is not yet checked for this session.',
      time: new Date().toLocaleTimeString(),
    }
  ],

  // Scroll positions
  chatScroll: 0,
  traceScroll: 0,

  // Command & Input Line
  inputBuffer: '',
  inputCursor: 0,
  commandBuffer: '',
  isProcessing: false,
  statusMessage: 'Ready. Press Tab to switch panes, :set for tuning.',

  // Settings focus index
  settingIndex: 0,
};

const { sampleTelemetry, queryMemQContext, commitMemQEpisode } = createRuntime({ state, ROOT, WORKSPACE, RUST_HARNESS, MEMQ_PYTHON, MEMQ_CLIENT });

const {render}=createView(state, WORKSPACE);
const refreshBudget=createBudgetMonitor(state,render);

// --- Agent Execution Dispatcher ---
const mediaBrowser=createMediaBrowser(state,[WORKSPACE,ROOT],render);

async function executeAgentTask(promptText) {
  state.isProcessing = true;
  state.statusMessage = 'Executing through configured inference with native tools...';
  render();

  // Add user prompt to chat
  state.chatHistory.push({
    role: 'user',
    text: promptText,
    time: new Date().toLocaleTimeString(),
  });
  const runDirectory = path.join(ROOT, 'runs', `terminal-${randomUUID()}`);
  const routePlan=await planRoute(promptText,state.role,state.attachedImage);
  const plan=executionPlan(promptText,state.role,runDirectory,state.attachedImage,routePlan.terminalTarget);
  state.traceLogs.push({
    type: 'ROUTE_PLAN',
    text: `${routePlan.source}: ${routePlan.stages.map(stage=>stage.target).join(' → ')}${routePlan.blockedReason?` · BLOCKED: ${routePlan.blockedReason}`:''}`,
    time: new Date().toLocaleTimeString(),
  });
  render();

  fs.mkdirSync(runDirectory,{recursive:true,mode:0o700});
  const routePlanFile=path.join(runDirectory,'route-plan.json');
  fs.writeFileSync(routePlanFile,JSON.stringify(routePlan,null,2),{mode:0o600});
  if(!routePlan.executable){
    state.isProcessing=false;
    state.chatHistory.push({role:'system',text:`Route selected: ${routePlan.stages.map(stage=>stage.target).join(' → ')}. ${routePlan.blockedReason}. No inference job was submitted.`,time:new Date().toLocaleTimeString()});
    state.statusMessage=routePlan.blockedReason;render();return;
  }

  // 1. MemQ Historical Recall (Bounded Context)
  const memqContext = await queryMemQContext(promptText);

  // Construct augmented objective
  let finalObjective = promptText;
  if (memqContext) {
    finalObjective = `[Relevant Historical Memory]:\n${memqContext}\n\n[Current Task]:\n${promptText}`;
  }

  const structuredTargets=new Set(['image','video','video-edit','speech','effects','music']);
  const isMediaTerminal=structuredTargets.has(routePlan.terminalTarget);
  const requiresPipeline=routePlan.stages.length>1||isMediaTerminal;
  if(requiresPipeline){
    plan.executable=process.execPath;
    // Pass raw promptText to media pipelines so MemQ recall headers never pollute diffusion/audio prompts.
    const pipelinePrompt=isMediaTerminal?promptText:finalObjective;
    plan.args=[path.join(ROOT,'local-inference-gateway'),routePlanFile,pipelinePrompt,runDirectory,...(state.attachedImage?[state.attachedImage]:[])];
  }

  // 2. Spawn Rust Harness Agent
  const startTime = Date.now();
  state.controlFile=path.join(runDirectory,'inference-controls.json');
  fs.writeFileSync(state.controlFile,JSON.stringify({reasoningEnabled:state.reasoningEnabled,localFallbackEnabled:state.localFallbackEnabled}),{mode:0o600});
  const args = plan.args ?? [
    'agent',
    state.role,
    finalObjective,
    '--workspace', WORKSPACE,
    '--run-dir', runDirectory,
    '--max-turns', '10',
    '--timeout', '1200',
  ];
  if (state.vision) args.push('--vision');

  state.traceLogs.push({
    type: 'SPAWN',
    text: `Trying configured private inference; local fallback is ${state.localFallbackEnabled?'ON':'OFF'}.\nRun files: ${runDirectory}`,
    time: new Date().toLocaleTimeString(),
  });
  render();

  const child = spawn(plan.executable??RUST_HARNESS, args, { cwd: ROOT, env: { ...process.env, LOCAL_AGENTS_CONTEXT_SIZE: String(state.contextSize), PRIVATE_INFERENCE_EVENTS:'1', LOCAL_AGENTS_ALLOW_LOCAL_FALLBACK:state.localFallbackEnabled?'1':'0', LOCAL_AGENTS_MIN_DISK_RESERVE_GIB:process.env.LOCAL_AGENTS_MIN_DISK_RESERVE_GIB??'5.0', PRIVATE_INFERENCE_CONTROL_FILE:state.controlFile } });
  let stdout = '';
  let stderr = '';

  child.stdout.on('data', data => {
    stdout += data.toString();
  });

  const progress=inferenceLines(state,render);
  child.stderr.on('data',data=>{stderr+=data.toString();progress.push(data.toString());});

  child.on('close', code => {
    progress.finish();
    let parsedOutput;
    try {parsedOutput=JSON.parse(stdout);}catch{/* The failure path may not emit JSON. */}
    const discovered=mediaBrowser.collect(parsedOutput??stdout,runDirectory);
    state.controlFile=null;
    if(plan.referenceImage)state.attachedImage=null;
    state.isProcessing = false;
    const durSec = ((Date.now() - startTime) / 1000).toFixed(1);

    if (code === 0) {
      try {
        const parsed = JSON.parse(stdout);
        const answer = parsed.answer || (plan.modality&&parsed.path?`Generated ${plan.modality}: ${parsed.path}`:'Task completed successfully.');
        const toolsUsed = parsed.tools || [];
        const turns = parsed.turns || [];
        
        // Update telemetry stats
        if (turns.length) {
          const lastTurn = turns[turns.length - 1];
          if (lastTurn.usage) {
            state.telemetry.tokensIn = lastTurn.usage.prompt_tokens || 0;
            state.telemetry.tokensOut = lastTurn.usage.completion_tokens || 0;
            state.telemetry.contextUsed = lastTurn.usage.total_tokens || 0;
          }
          if (lastTurn.timings?.predicted_per_second) {
            state.telemetry.tokensPerSec = lastTurn.timings.predicted_per_second.toFixed(1);
          }
        }

        // Add assistant reply to chat history
        state.chatHistory.push({
          role: state.role,
          text: answer,
          time: new Date().toLocaleTimeString(),
          tools: toolsUsed.map(t => `${t.tool}(${JSON.stringify(t.arguments || {})})`),
          media: discovered,
        });

        // Record tools to trace log
        for (const t of toolsUsed) {
          state.traceLogs.push({
            type: 'TOOL_EXEC',
            text: `⚙ ${t.tool} -> ${t.response?.ok ? 'OK' : 'Error'} (${t.seconds}s)`,
            time: new Date().toLocaleTimeString(),
          });
        }

        state.traceLogs.push({
          type: 'DONE',
          text: `Finished in ${durSec}s (${state.telemetry.tokensPerSec} tok/s). Status: model_finished`,
          time: new Date().toLocaleTimeString(),
        });

        // Commit episodic memory to MemQ
        void commitMemQEpisode(promptText, answer, turns, toolsUsed, runDirectory).then(() => render());

        const deliveryOnly=toolsUsed.some(t=>t.tool==='send_message')&&!state.currentRunMediaCount;
        state.statusMessage = deliveryOnly?'Message queued in local inbox; generation has NOT been submitted.':`Completed in ${durSec}s. Inference turn finished.`;
        if(deliveryOnly)state.traceLogs.push({type:'DELIVERY_ONLY',text:'A local inbox message is not a Runpod job. No generated media file was found.',time:new Date().toLocaleTimeString()});
        if(plan.modality==='image'&&discovered.length)void mediaBrowser.open(discovered.at(-1));
      } catch (err) {
        state.chatHistory.push({
          role: state.role,
          text: stdout || 'Task completed (non-JSON output).',
          time: new Date().toLocaleTimeString(),
        });
        state.statusMessage = 'Finished with non-JSON response.';
      }
    } else {
      let routing = '';
      for (const directory of [runDirectory, path.join(runDirectory, 'planner')]) {
        try { routing += fs.readFileSync(path.join(directory, 'inference-routing.jsonl'), 'utf8'); } catch { /* Optional receipt. */ }
      }
      const failure = explainRunFailure(stdout, stderr, code, routing);
      state.chatHistory.push({
        role: 'system',
        text: `${failure.message}\nRun details: ${runDirectory}`,
        time: new Date().toLocaleTimeString(),
      });
      state.traceLogs.push({
        type: 'ERROR',
        text: `${failure.summary}\nRun files: ${runDirectory}`,
        time: new Date().toLocaleTimeString(),
      });
      state.statusMessage = failure.summary;
    }
    sampleTelemetry();
    render();
  });
}

// --- Keyboard Event Dispatcher ---
function setupKeyboard() {
  const keys=new PassThrough();
  readline.emitKeypressEvents(keys);
  let mousePending='',dragging=false;
  process.stdin.on('data',chunk=>{
    const parsed=mouseInput(mousePending+chunk.toString());mousePending=parsed.pending;
    for(const event of parsed.events){
      const columns=process.stdout.columns||100,divider=Math.floor(columns*state.splitRatio);
      if(event.button===64||event.button===65){
        const right=state.traceExpanded||event.x>divider;
        state.activePane=right?'trace':'chat';
        const field=right?'traceScroll':'chatScroll';
        state[field]=Math.max(0,state[field]+(event.button===64?3:-3));
      }else if(event.released){dragging=false;}
      else if(event.button===0){dragging=!state.traceExpanded&&Math.abs(event.x-divider)<=2;state.activePane=state.traceExpanded||event.x>divider?'trace':'chat';}
      else if(event.button===32&&dragging){state.splitRatio=Math.max(.2,Math.min(.8,event.x/columns));}
      render();
    }
    if(parsed.text)keys.write(parsed.text);
  });
  if (process.stdin.isTTY) {
    process.stdin.setRawMode(true);
  }

  keys.on('keypress', (str, key) => {
    // Global Exit
    if (key.ctrl && key.name === 'c') {
      cleanupAndExit();
    }

    if(state.mode==='MEDIA'){mediaBrowser.key(key);return;}
    if(state.mode==='NORMAL'&&str==='t'){toggleReasoning(state);render();return;}
    if(state.mode==='NORMAL'&&str==='m'){mediaBrowser.show();return;}

    // Modal SETTINGS Mode
    if (state.mode === 'SETTINGS') {
      if (key.name === 'escape') {
        state.mode = 'NORMAL';
        render();
        return;
      }
      if (key.name === 'up' || key.name === 'k') {
        state.settingIndex = Math.max(0, state.settingIndex - 1);
        render();
        return;
      }
      if (key.name === 'down' || key.name === 'j') {
        state.settingIndex = Math.min(7, state.settingIndex + 1);
        render();
        return;
      }
      if (key.name === 'return' || key.name === 'space') {
        // Toggle selected setting
        switch (state.settingIndex) {
          case 0:
            state.temperature = state.temperature === 0.0 ? 0.2 : (state.temperature === 0.2 ? 0.7 : 0.0);
            break;
          case 1:
            state.maxTokens = state.maxTokens === 1600 ? 4096 : (state.maxTokens === 4096 ? 512 : 1600);
            break;
          case 2:
            state.contextSize = state.contextSize === 8192 ? 16384 : (state.contextSize === 16384 ? 4096 : 8192);
            break;
          case 3:
            const roles = ['cyber-lab', 'coordinator', 'browser', 'desktop', 'image-studio', 'video-studio'];
            const curIdx = roles.indexOf(state.role);
            state.role = roles[(curIdx + 1) % roles.length];
            break;
          case 4:
            state.lowMemory = !state.lowMemory;
            break;
          case 5:
            state.vision = !state.vision;
            break;
          case 6:
            state.memqEnabled = !state.memqEnabled;
            break;
          case 7:
            toggleFallback(state);
            break;
        }
        render();
        return;
      }
      return;
    }

    // Command Mode (:command)
    if (state.mode === 'COMMAND') {
      if (key.name === 'escape') {
        state.mode = 'NORMAL';
        state.commandBuffer = '';
        render();
        return;
      }
      if (key.name === 'return') {
        const command=state.commandBuffer;
        state.commandBuffer = '';
        state.mode = 'NORMAL';
        void executeCommand(command);
        render();
        return;
      }
      if (key.name === 'backspace') {
        state.commandBuffer = state.commandBuffer.slice(0, -1);
        if (!state.commandBuffer) state.mode = 'NORMAL';
        render();
        return;
      }
      if (str && str.length === 1) {
        state.commandBuffer += str;
        render();
        return;
      }
      return;
    }

    // Tab: Switch Panes (Global)
    if (key.name === 'tab') {
      state.activePane = state.activePane === 'chat' ? 'trace' : 'chat';
      if(state.activePane==='trace')state.mode='NORMAL';
      render();
      return;
    }

    if(paneKey(state,str,key)){render();return;}

    // NORMAL Mode
    if (state.mode === 'NORMAL') {
      if (key.name === 'i' || key.name === 'a') {
        state.mode = 'INSERT';
        render();
        return;
      }
      if (str === ':') {
        state.mode = 'COMMAND';
        state.commandBuffer = '';
        render();
        return;
      }
      if (str === 'o' && key.ctrl) {
        state.mode = 'SETTINGS';
        render();
        return;
      }
      if (key.name === 'j' || key.name === 'down') {
        if (state.activePane === 'chat') state.chatScroll = Math.max(0, state.chatScroll - 1);
        else state.traceScroll = Math.max(0, state.traceScroll - 1);
        render();
        return;
      }
      if (key.name === 'k' || key.name === 'up') {
        if (state.activePane === 'chat') state.chatScroll++;
        else state.traceScroll++;
        render();
        return;
      }
      if (str === 'q') {
        cleanupAndExit();
      }
      return;
    }

    // INSERT Mode
    if (state.mode === 'INSERT') {
      if (key.name === 'escape') {
        state.mode = 'NORMAL';
        render();
        return;
      }
      if (key.name === 'return') {
        const text = state.inputBuffer.trim();
        if (text && !state.isProcessing) {
          state.inputBuffer = '';
          executeAgentTask(text);
        }
        return;
      }
      if (key.name === 'backspace') {
        state.inputBuffer = state.inputBuffer.slice(0, -1);
        render();
        return;
      }
      if (str && str.length === 1 && !key.ctrl && !key.meta) {
        state.inputBuffer += str;
        render();
        return;
      }
    }
  });
}

async function executeCommand(cmd) {
  const parts = cmd.trim().split(/\s+/);
  const verb = parts[0];
  const arg = cmd.trim().slice(verb.length).trim();
  if(parts[0]==='media'){mediaBrowser.show(arg);return;}
  if(parts[0]==='attach'){
    const candidate=path.isAbsolute(arg)?arg:path.resolve(WORKSPACE,arg);
    const item=mediaFile(candidate,[WORKSPACE,ROOT]);
    if(!item||item.kind!=='image'){state.statusMessage='Attach a readable PNG, JPEG, WebP, GIF, or AVIF inside this workspace.';render();return;}
    state.attachedImage=item.path;state.statusMessage=`Image attached for the next edit request: ${item.path}`;render();return;
  }
  if(parts[0]==='paste-image'){
    try{
      const directory=path.join(ROOT,'runs','attachments');fs.mkdirSync(directory,{recursive:true,mode:0o700});
      const destination=path.join(directory,`clipboard-${randomUUID()}.png`);
      await importClipboardImage(destination);
      const item=mediaFile(destination,[ROOT]);if(!item||item.kind!=='image')throw new Error('Clipboard did not contain a supported image');
      state.attachedImage=item.path;mediaBrowser.collect({path:item.path});state.statusMessage=`Clipboard image attached for the next edit request: ${item.path}`;
    }catch(error){state.statusMessage=error instanceof Error?error.message:'Unable to import clipboard image';}
    render();return;
  }

  switch (verb) {
    case 'q':
    case 'quit':
      cleanupAndExit();
      break;
    case 'set':
      state.mode = 'SETTINGS';
      break;
    case 'role':
      if (arg) {
        state.role = arg;
        state.statusMessage = `Role switched to [${arg}].`;
      }
      break;
    case 'clear':
      state.chatHistory = [];
      state.traceLogs = [];
      state.statusMessage = 'History cleared.';
      break;
    case 'help':
      state.statusMessage = 'Use :paste-image or :attach path, then type the edit. Logs: Tab then PgUp/PgDn; media: m; quit: :q';
      break;
    default:
      state.statusMessage = `Unknown command: :${verb}`;
      break;
  }
}

function cleanupAndExit() {
  process.stdout.write(`${CSI}?1002l${CSI}?1006l${CSI}?1049l${CSI}?25h`); // Restore mouse, screen & cursor
  console.log('Multinex Sovereign Harness exited cleanly. Heavy lock released.');
  process.exit(0);
}

// --- Main Entrypoint ---
function main() {
  // Enter alternate buffer and hide cursor
  process.stdout.write(`${CSI}?1049h${CSI}?25l${CSI}?1002h${CSI}?1006h`);

  // Handle window resize
  process.stdout.on('resize', () => {
    render();
  });

  // Handle process signals
  process.on('SIGINT', cleanupAndExit);
  process.on('SIGTERM', cleanupAndExit);
  process.on('exit', () => {
    process.stdout.write(`${CSI}?1002l${CSI}?1006l${CSI}?1049l${CSI}?25h`);
  });

  setupKeyboard();
  sampleTelemetry();
  render();

  // Periodic telemetry refresh (every 2 seconds)
  void refreshBudget();
  setInterval(refreshBudget,30000);
  setInterval(() => {
    sampleTelemetry();
    render();
  }, 2000);
}

main();
