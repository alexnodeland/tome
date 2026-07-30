// A minimal PNG encoder.
//
// Node ships zlib, and the images this repository generates are flat fills
// with a few rectangles — so a dependency would buy nothing but a supply
// chain. Shared by the DMG background and the menu bar icon, because two
// copies of a CRC table is one copy too many.

import { deflateSync } from 'node:zlib';

/** An 8-bit truecolour PNG from packed RGB bytes. */
export function rgbPng(w, h, rgb) {
  const raw = Buffer.alloc(h * (w * 3 + 1));
  for (let y = 0; y < h; y++) {
    raw[y * (w * 3 + 1)] = 0; // filter type 0
    Buffer.from(rgb.buffer, y * w * 3, w * 3).copy(raw, y * (w * 3 + 1) + 1);
  }
  return assemble(w, h, raw, 2);
}

/**
 * An 8-bit RGBA PNG from packed RGBA bytes.
 *
 * macOS template images are black pixels plus an alpha channel: the system
 * recolours them for the menu bar's appearance — light or dark, normal or
 * highlighted. Supplying colour defeats that and produces an icon that is
 * invisible in one of the two.
 */
export function rgbaPng(w, h, rgba) {
  const raw = Buffer.alloc(h * (w * 4 + 1));
  for (let y = 0; y < h; y++) {
    raw[y * (w * 4 + 1)] = 0;
    Buffer.from(rgba.buffer, y * w * 4, w * 4).copy(raw, y * (w * 4 + 1) + 1);
  }
  return assemble(w, h, raw, 6);
}

/** IHDR, IDAT, IEND. `colourType` is 2 (RGB) or 6 (RGBA). */
function assemble(w, h, raw, colourType) {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = colourType;
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', ihdr),
    chunk('IDAT', deflateSync(raw, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

function chunk(type, data) {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body) >>> 0);
  return Buffer.concat([len, body, crc]);
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
