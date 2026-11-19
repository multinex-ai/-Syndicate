// --- ANSI Escape Sequences & Palettes ---
const ESC = '\x1b';
const CSI = '\x1b[';
const OSC = '\x1b]';

// Colors (Accords Sovereign Neutral & Emerald/Cyan Accents)
const C_RESET = `${CSI}0m`;
const C_BOLD = `${CSI}1m`;
const C_DIM = `${CSI}2m`;
const C_UNDERLINE = `${CSI}4m`;

// TrueColor (24-bit) Hex
const rgb = (r, g, b) => `${CSI}38;2;${r};${g};${b}m`;
const bgRgb = (r, g, b) => `${CSI}48;2;${r};${g};${b}m`;

const THEME = {
  bgCanvas: bgRgb(5, 7, 11),
  bgCard: bgRgb(13, 17, 23),
  bgElevated: bgRgb(21, 28, 37),
  cyan: rgb(61, 240, 255),
  emerald: rgb(52, 211, 153),
  amber: rgb(245, 158, 11),
  crimson: rgb(244, 63, 94),
  purple: rgb(167, 139, 250),
  slate: rgb(110, 127, 142),
  slateLight: rgb(159, 176, 191),
  white: rgb(240, 244, 248),
  border: rgb(45, 55, 72),
  borderActive: rgb(61, 240, 255),
};

// OSC 8 Hyperlink Formatter (Cmd-Clickable)
function link(url, label) {
  const display = label || url;
  return `${OSC}8;;${url}${ESC}\\${THEME.cyan}${C_UNDERLINE}${display}${C_RESET}${OSC}8;;${ESC}\\`;
}

// Strip ANSI for width calculations
function stripAnsi(str) {
  return str.replace(/\x1b\[[0-9;]*m/g, '').replace(/\x1b\]8;;[^\x1b]*\x1b\\/g, '');
}

function truncateString(str, maxLen) {
  const plain = stripAnsi(str);
  if (plain.length <= maxLen) return str;
  return plain.slice(0, maxLen - 1) + '…';
}


export { ESC, CSI, OSC, C_RESET, C_BOLD, C_DIM, C_UNDERLINE, rgb, bgRgb, THEME, link, stripAnsi, truncateString };
