// Generate the menu bar icon (S4-6, P5-008).
//
// A **template image**: black pixels and an alpha channel, nothing else. macOS
// recolours it for the menu bar's current appearance — light or dark, normal
// or highlighted, and inverted while the menu is open. An icon with colour in
// it defeats all four and ends up invisible in at least one.
//
// The glyph is an open book seen from above: two facing pages with a gutter.
// Drawn rather than drafted in a vector tool because it is eleven rectangles
// at this size, and because a committed binary whose source is a script can be
// regenerated when the size or the metrics change.
//
//   node scripts/make-tray-icon.mjs
//
// Writes src-tauri/icons/tray.png at 44x44, which is the @2x size for the
// 22pt menu bar. Committed, so the bundle builds on a machine with no Node.

import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { rgbaPng } from './lib/png.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

// 44px = 22pt at @2x. The glyph occupies about 18pt of it; menu bar icons are
// inset from their bounds or they crowd their neighbours.
const SIZE = 44;
const px = new Uint8Array(SIZE * SIZE * 4);

/** Black at `alpha`. Everything here is black — see the template note above. */
function set(x, y, alpha) {
  if (x < 0 || y < 0 || x >= SIZE || y >= SIZE) return;
  const i = (y * SIZE + x) * 4;
  px[i] = 0;
  px[i + 1] = 0;
  px[i + 2] = 0;
  px[i + 3] = alpha;
}

function fill(x0, y0, x1, y1, alpha = 255) {
  for (let y = y0; y < y1; y++) for (let x = x0; x < x1; x++) set(x, y, alpha);
}

const LEFT = 5;
const RIGHT = SIZE - 5;
const TOP = 9;
const BOTTOM = SIZE - 9;
const MID = SIZE / 2;
const STROKE = 3;

// One outlined rectangle, wider than tall, with a single centre stroke.
//
// Two earlier attempts are worth recording, because both looked reasonable
// full size and failed at 22 points, which is the only size that matters:
//
// * An outline with *two* inner strokes and dashes of "text" read as window
//   panes — the dashes gave it mullions.
// * Two separate page rectangles read as two tall boxes: the gap dominated,
//   and nothing tied them into one object.
//
// A single frame with one gutter stroke is unambiguous at any size, and at
// 22 points it is the only version that is.
fill(LEFT, TOP, RIGHT, TOP + STROKE); // top
fill(LEFT, BOTTOM - STROKE, RIGHT, BOTTOM); // bottom
fill(LEFT, TOP, LEFT + STROKE, BOTTOM); // left
fill(RIGHT - STROKE, TOP, RIGHT, BOTTOM); // right
fill(MID - 1, TOP, MID + 2, BOTTOM); // the gutter

// The corners come off, which is a two-pixel rounding at @2x. Sharp corners
// on a 22pt glyph sit uneasily beside the system's own menu bar icons, all of
// which are drawn on a rounded grid.
for (const [x, y] of [
  [LEFT, TOP],
  [RIGHT - 1, TOP],
  [LEFT, BOTTOM - 1],
  [RIGHT - 1, BOTTOM - 1],
]) {
  set(x, y, 0);
}

const out = join(root, 'src-tauri/icons/tray.png');
writeFileSync(out, rgbaPng(SIZE, SIZE, px));
console.log(`wrote ${out}`);
