import {mediaView} from './tui-media-view.mjs';
import {wrapText,viewport,plainText,displayWidth} from './terminal-layout.ts';
import { CSI, C_RESET, C_BOLD, THEME, link, stripAnsi, truncateString } from './tui-theme.mjs';

export function createView(state, WORKSPACE) {
let previousTraceCount=0, previousTraceLines=0, previousTraceWidth=0;
// --- Viewport & Renderer ---
function getTermSize() {
  return {
    cols: process.stdout.columns || 100,
    rows: process.stdout.rows || 30,
  };
}

function render() {
  const { cols, rows } = getTermSize();
  if(state.mode==='MEDIA'){process.stdout.write(mediaView(state,cols,rows));return;}
  let buf = '';

  // 1. Move to home
  buf += `${CSI}H`;

  // Draw Header: Title & Version
  const headerLeft = ` ${THEME.cyan}${C_BOLD}╭─ MULTINEX SOVEREIGN HARNESS${C_RESET} ${THEME.slate}• ${state.modelName}${C_RESET}`;
  const headerRight = `${THEME.amber}${C_BOLD}[${state.mode}]${C_RESET} ${THEME.slate}${new Date().toLocaleTimeString()} ─╮${C_RESET}`;
  const fillLen1 = Math.max(0, cols - stripAnsi(headerLeft).length - stripAnsi(headerRight).length);
  buf += `${headerLeft}${'─'.repeat(fillLen1)}${headerRight}\n`;

  // 2. Simplified Cheat Sheet Ribbon (Directly above RAM / VRAM footprint)
  const cs1 = ` ${THEME.emerald}💡 CHEAT SHEET:${C_RESET} ${THEME.white}[Tab]${C_RESET} Focus Pane  │ ${THEME.white}[i]${C_RESET} Chat/Insert  │ ${THEME.white}[Esc]${C_RESET} Normal  │ ${THEME.white}[j/k]${C_RESET} Scroll`;
  const cs2 = `${THEME.white}[:set]${C_RESET} Params  │ ${THEME.white}[:role]${C_RESET} Switch  │ ${THEME.white}[:q]${C_RESET} Quit │`;
  const csFill = Math.max(0, cols - stripAnsi(cs1).length - stripAnsi(cs2).length - 3);
  buf += `│${cs1}${' '.repeat(csFill)}${cs2}\n`;

  // Divider
  buf += `├${'─'.repeat(cols - 2)}┤\n`;

  // 3. Real-Time Telemetry Bar
  const ramPercent = Math.round(((state.telemetry.totalGiB - state.telemetry.availableGiB) / state.telemetry.totalGiB) * 100);
  const barBlocks = Math.round((ramPercent / 100) * 10);
  const bar = `[${'█'.repeat(barBlocks)}${'░'.repeat(10 - barBlocks)}]`;
  const pressureLabel = state.telemetry.pressureLevel === 1 ? `${THEME.emerald}Normal${C_RESET}` : `${THEME.crimson}Warning${C_RESET}`;

  const tel1 = ` ⚡ RAM: ${THEME.emerald}${state.telemetry.availableGiB}G Free${C_RESET} ${THEME.slate}${bar} ${ramPercent}%${C_RESET} │ VRAM/Metal: ${THEME.purple}${state.telemetry.rssGiB}G${C_RESET} │ Pressure: ${pressureLabel}`;
  const tel2 = `Role: ${THEME.cyan}[${state.role}]${C_RESET} │ Lock: ${THEME.amber}${state.telemetry.lockState}${C_RESET} │`;
  const telFill1 = Math.max(0, cols - stripAnsi(tel1).length - stripAnsi(tel2).length - 3);
  buf += `│${tel1}${' '.repeat(telFill1)}${tel2}\n`;

  const tokSpeed = state.telemetry.tokensPerSec ? `${state.telemetry.tokensPerSec} tok/s` : 'not measured';
  const tel3 = ` 🎯 Tokens: ${THEME.white}${state.telemetry.tokensIn} in / ${state.telemetry.tokensOut} out (${tokSpeed})${C_RESET} │ Ctx: ${THEME.slate}${state.telemetry.contextUsed}/${state.contextSize}${C_RESET}`;
  const prettyWs = link(`file://${WORKSPACE}`, truncateString(WORKSPACE, 35));
  const tel4 = `Workspace: ${prettyWs} │`;
  const telFill2 = Math.max(0, cols - stripAnsi(tel3).length - stripAnsi(tel4).length - 3);
  buf += `│${tel3}${' '.repeat(telFill2)}${tel4}\n`;

  const attachment=state.attachedImage?` · Attached: ${state.attachedImage.split('/').at(-1)}`:'';
  buf += `│${truncateString(`Reasoning pref ${state.reasoningEnabled===false?'OFF':'ON'} [t] · Media [m]${attachment} | ${state.budgetLine}`, cols - 2).padEnd(cols - 2)}│\n`;

  // Divider above main split panes
  buf += `├${'─'.repeat(cols - 2)}┤\n`;

  // 4. Dual-Pane Canvas (Chat Left, Trace Right)
  const splitCol = Math.floor(cols * state.splitRatio);
  const leftWidth = splitCol - 1;
  const rightWidth = state.traceExpanded ? cols - 2 : cols - splitCol - 2;
  const mainHeight = Math.max(6, rows - 12);

  // Pane Headers
  const leftActive = state.activePane === 'chat';
  const rightActive = state.activePane === 'trace';

  const leftTitle = leftActive
    ? ` ${THEME.cyan}${C_BOLD}▶ CHAT THREAD (Focused)${C_RESET} `
    : ` ${THEME.slate}CHAT THREAD${C_RESET} `;
  const rightTitle = rightActive
    ? ` ${THEME.cyan}${C_BOLD}▶ ROUTING & THINKING TRACE (Focused)${C_RESET} `
    : ` ${THEME.slate}ROUTING & THINKING TRACE${C_RESET} `;

  const lFill = Math.max(0, leftWidth - stripAnsi(leftTitle).length);
  const rFill = Math.max(0, rightWidth - stripAnsi(rightTitle).length);
  const rt=`Logs ${state.traceScroll?'(scrolling)':'(live)'} · PgUp/PgDn · [/] resize · f expand`;
  buf += state.traceExpanded ? `│${truncateString(rt,rightWidth).padEnd(rightWidth)}│\n` : `│${truncateString(plainText(leftTitle),leftWidth).padEnd(leftWidth)}┬${truncateString(rt,rightWidth).padEnd(rightWidth)}│\n`;

  // Prepare Chat Lines
  let chatLines = [];
  for (const msg of state.chatHistory) {
    if (msg.role === 'user') {
      chatLines.push(`${THEME.amber}${C_BOLD}╭─ USER [${msg.time}] ─╮${C_RESET}`);
      const words = msg.text.split('\n');
      for (const w of words) {
        chatLines.push(`  ${THEME.white}${w}${C_RESET}`);
      }
      chatLines.push(`${THEME.amber}╰──────────────────────╯${C_RESET}`);
    } else if (msg.role === 'system') {
      chatLines.push(`${THEME.slate}[SYSTEM]: ${msg.text}${C_RESET}`);
    } else {
      chatLines.push(`${THEME.cyan}${C_BOLD}╭─ ${msg.role.toUpperCase()} (configured model) [${msg.time}] ─╮${C_RESET}`);
      if (msg.tools && msg.tools.length) {
        for (const t of msg.tools) {
          chatLines.push(`  ${THEME.purple}⚡ Tool: ${t}${C_RESET}`);
        }
      }
      if(msg.media&&msg.media.length)for(const item of msg.media){
        const filename=item.path.split('/').at(-1);
        chatLines.push(`  ${THEME.emerald}${C_BOLD}IMAGE READY${C_RESET} · ${filename}`);
        chatLines.push(`  ${link(`file://${item.path}`,item.path)}`);
        chatLines.push(`  ${THEME.slate}Opened in the system viewer · press [m] to browse again${C_RESET}`);
      }
      const words = msg.text.split('\n');
      for (const w of words) {
        // Detect file links and format with OSC 8
        let lineFormatted = w.replace(/(https?:\/\/[^\s]+)/g, (url) => link(url, url))
                             .replace(/(file:\/\/[^\s]+)/g, (f) => link(f, f));
        chatLines.push(`  ${THEME.white}${lineFormatted}${C_RESET}`);
      }
      chatLines.push(`${THEME.cyan}╰──────────────────────────────────────╯${C_RESET}`);
    }
    chatLines.push('');
  }

  chatLines=chatLines.flatMap(line=>wrapText(line,leftWidth));

  // Prepare Trace Lines
  const traceLines = [];
  for (const tr of state.traceLogs) {
    if(tr.type==='REASONING'&&state.reasoningEnabled===false)continue;
    let color = THEME.slate;
    if (tr.type.includes('MEMQ')) color = THEME.purple;
    if (tr.type === 'TOOL' || tr.type === 'TOOL_EXEC') color = THEME.cyan;
    if (tr.type === 'ERROR') color = THEME.crimson;
    if (tr.type === 'DONE') color = THEME.emerald;
    if (tr.type === 'DISPATCH') color = THEME.amber;

    traceLines.push(...wrapText(`[${tr.time}] [${tr.type}] ${tr.text}`,rightWidth).map(line=>`${color}${line}${C_RESET}`));
  }

  // Scroll offsets count wrapped display lines, independently for each pane.
  if(state.traceScroll>0 && previousTraceWidth===rightWidth && state.traceLogs.length>previousTraceCount) {
    state.traceScroll+=Math.max(0,traceLines.length-previousTraceLines);
  }
  previousTraceCount=state.traceLogs.length; previousTraceLines=traceLines.length; previousTraceWidth=rightWidth;
  const cv=viewport(chatLines.length,mainHeight,state.chatScroll);
  const tv=viewport(traceLines.length,mainHeight,state.traceScroll);
  state.chatScroll=cv.offset;state.traceScroll=tv.offset;
  state.chatMaxScroll=cv.max;state.traceMaxScroll=tv.max;state.pageSize=mainHeight;
  for(let r=0;r<mainHeight;r++) {
    const cl=chatLines[cv.start+r]||'',tl=traceLines[tv.start+r]||'';
    const left=cl+' '.repeat(Math.max(0,leftWidth-displayWidth(cl)));
    const right=tl+' '.repeat(Math.max(0,rightWidth-displayWidth(tl)));
    buf+=state.traceExpanded?`│${right}│\n`:`│${left}│${right}│\n`;
  }

  // 5. Divider above Input Line
  buf += `├${'─'.repeat(cols - 2)}┤\n`;

  // 6. Bottom Input & Command Line
  if (state.mode === 'COMMAND') {
    buf += `│ :${state.commandBuffer}${' '.repeat(Math.max(0, cols - state.commandBuffer.length - 4))}│\n`;
  } else {
    const promptPrefix = state.mode === 'INSERT'
      ? `${THEME.cyan}${C_BOLD}── INSERT ──${C_RESET} multinex (${state.role})> `
      : `${THEME.amber}${C_BOLD}── NORMAL ──${C_RESET} [j/k scroll, i chat, :set params]> `;
    const inp = state.inputBuffer;
    const inpFill = Math.max(0, cols - stripAnsi(promptPrefix).length - inp.length - 3);
    buf += `│ ${promptPrefix}${THEME.white}${inp}${C_RESET}${' '.repeat(inpFill)}│\n`;
  }

  // Footer Status Ribbon
  const statusStr = ` ${THEME.slate}${state.statusMessage}${C_RESET}`;
  const keysStr = `${THEME.slate}[Tab] Pane │ [:set] Tuning │ [Ctrl+C] Quit │${C_RESET}`;
  const fFill = Math.max(0, cols - stripAnsi(statusStr).length - stripAnsi(keysStr).length - 3);
  buf += `╰─${statusStr}${'─'.repeat(fFill)}${keysStr}╯`;

  // Prevent long headers and status rows from wrapping the entire terminal frame.
  buf=buf.split("\n").map(line=>displayWidth(line)>cols?wrapText(line,cols)[0]:line).join("\n");

  // Draw overlay if in SETTINGS modal
  if (state.mode === 'SETTINGS') {
    buf = renderSettingsModal(buf, cols, rows);
  }

  process.stdout.write(buf);
}

function renderSettingsModal(currentBuf, cols, rows) {
  const modalW = 56;
  const modalH = 14;
  const startX = Math.floor((cols - modalW) / 2);
  const startY = Math.floor((rows - modalH) / 2);

  const lines = currentBuf.split('\n');
  const items = [
    `Temperature:       [ ${state.temperature.toFixed(1)} ] (0.0=Code, 0.7=Creative)`,
    `Max Output Tokens: [ ${state.maxTokens} ] (512 / 1600 / 4096)`,
    `Context Size:      [ ${state.contextSize} ] (4096 / 8192 / 16384)`,
    `Active Role:       [ ${state.role} ] (cyber-lab / coordinator / browser)`,
    `Low Memory Mode:   [ ${state.lowMemory ? 'ON' : 'OFF'} ] (ubatch=64)`,
    `Vision Projector:  [ ${state.vision ? 'ON' : 'OFF'} ] (BF16 mmproj)`,
    `MemQ Compaction:   [ ${state.memqEnabled ? 'ON' : 'OFF'} ] (identity checked at use)`,
    `Local fallback:   [ ${state.localFallbackEnabled?'ON':'OFF'} ] (default OFF)`,
  ];

  let modal = '';
  modal += `${CSI}${startY};${startX}H${THEME.cyan}${C_BOLD}╭── PARAMETER TUNING MODAL (:set) ───────────────╮${C_RESET}`;

  for (let i = 0; i < items.length; i++) {
    const isSel = i === state.settingIndex;
    const selPrefix = isSel ? `${THEME.amber}${C_BOLD} ▶ ` : `   `;
    const lineContent = `${selPrefix}${items[i]}${C_RESET}`;
    const pad = Math.max(0, modalW - stripAnsi(lineContent).length - 2);
    modal += `${CSI}${startY + 1 + i};${startX}H${THEME.cyan}│${C_RESET}${lineContent}${' '.repeat(pad)}${THEME.cyan}│${C_RESET}`;
  }

  modal += `${CSI}${startY + items.length + 1};${startX}H${THEME.cyan}│${' '.repeat(modalW - 2)}│${C_RESET}`;
  modal += `${CSI}${startY + items.length + 2};${startX}H${THEME.cyan}│ ${THEME.slate}[↑/↓] Select  [Enter/Space] Toggle  [Esc] Close${C_RESET}${' '.repeat(modalW - 47)}${THEME.cyan}│${C_RESET}`;
  modal += `${CSI}${startY + items.length + 3};${startX}H${THEME.cyan}╰────────────────────────────────────────────────╯${C_RESET}`;

  return currentBuf + modal;
}

return {render};
}
