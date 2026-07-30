// Generate the DMG background from the design tokens.
//
// The window a user sees before they have ever run Tome should already look
// like Tome. Reading the colours out of `public/tokens.css` rather than
// retyping them means the installer cannot drift from the app, for the same
// reason `site/assets/site.css` has no palette of its own.
//
// PNG is written by hand -- zlib is in Node's standard library and the image
// is a flat fill with two rectangles, so a dependency would buy nothing. See
// the `check-contrast.mjs` argument about CSS parsers.
//
//   node scripts/make-dmg-background.mjs
//
// Writes src-tauri/dmg/background.png (540x380) and background@2x.png. Both
// are committed: the DMG has to be reproducible on a machine with no Node,
// and regenerating them is this script's job, not the bundler's.

import { deflateSync } from 'node:zlib';
import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

/** Pull a token's light-theme value out of tokens.css. */
function token(css, name) {
  // The first definition is the light theme; the dark overrides come later
  // under a media query, and the DMG window is chrome-coloured by the Finder
  // regardless of theme, so light is the right one.
  const m = css.match(new RegExp(`${name}:\\s*(#[0-9a-fA-F]{6})`));
  if (!m) throw new Error(`token ${name} not found in tokens.css`);
  return [1, 3, 5].map((i) => parseInt(m[1].slice(i, i + 2), 16));
}

const css = readFileSync(join(root, 'public/tokens.css'), 'utf8');
const BG = token(css, '--color-bg-primary');
const RULE = token(css, '--color-border');
const ACCENT = token(css, '--color-accent');

const W = 540;
const H = 380;

function render(scale) {
  const w = W * scale;
  const h = H * scale;
  const px = new Uint8Array(w * h * 3);

  const put = (x, y, [r, g, b]) => {
    const i = (y * w + x) * 3;
    px[i] = r;
    px[i + 1] = g;
    px[i + 2] = b;
  };

  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) put(x, y, BG);

  // A hairline under the icon row, at the same optical weight as the app's
  // panel dividers. The icons are centred at y=180 by `bundle.macOS.dmg` in
  // tauri.conf.json, and their names sit below them, so the rule clears both.
  const ruleY = Math.round(288 * scale);
  for (let t = 0; t < scale; t++)
    for (let x = Math.round(56 * scale); x < Math.round((W - 56) * scale); x++)
      put(x, ruleY + t, RULE);

  // The arrow between the app and the Applications alias: the one instruction
  // the window has to give, drawn rather than written so it needs no
  // translation. A shaft and a chevron, in the accent.
  const midY = Math.round(180 * scale);
  const from = Math.round(214 * scale);
  const to = Math.round(326 * scale);
  const weight = Math.max(1, Math.round(1.5 * scale));
  for (let t = 0; t < weight; t++) for (let x = from; x < to; x++) put(x, midY + t, ACCENT);
  const head = Math.round(7 * scale);
  for (let d = 0; d < head; d++)
    for (let t = 0; t < weight; t++) {
      put(to - d, midY - d + t, ACCENT);
      put(to - d, midY + d + t, ACCENT);
    }

  return png(w, h, px);
}

/** Minimal truecolour PNG: IHDR, IDAT, IEND. */
function png(w, h, rgb) {
  const raw = Buffer.alloc(h * (w * 3 + 1));
  for (let y = 0; y < h; y++) {
    raw[y * (w * 3 + 1)] = 0; // filter type 0
    Buffer.from(rgb.buffer, y * w * 3, w * 3).copy(raw, y * (w * 3 + 1) + 1);
  }

  const chunk = (type, data) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(crc32(body) >>> 0);
    return Buffer.concat([len, body, crc]);
  };

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 2; // colour type: truecolour
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

const CRC_TABLE = (() => {
  const t = new Int32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c;
  }
  return t;
})();

function crc32(buf) {
  let c = -1;
  for (const b of buf) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return c ^ -1;
}

const out = join(root, 'src-tauri/dmg');
mkdirSync(out, { recursive: true });
writeFileSync(join(out, 'background.png'), render(1));
writeFileSync(join(out, 'background@2x.png'), render(2));
console.log(`wrote ${out}/background.png and background@2x.png`);
