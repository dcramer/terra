// BC5 (two BC4 channels) block compression and a DDS writer for 3D textures, for the fire's baked
// volume (volume.ts; D493). The block format is Microsoft's (Direct3D 10 "Block Compression",
// BC4 and BC5 UNORM): a 4x4 block of one channel is two 8-bit endpoints and sixteen 3-bit indices
// into a palette of eight; with e0 > e1 the six between are interpolated in sevenths, otherwise
// four in fifths and the last two are 0 and 1. BC5 is a red block then a green one, 16 bytes.
// The encoder is ours: both palettes tried from the block's range (the second with the range of
// what is neither near 0 nor near 1, so a block at a flame's edge spends its steps on the flame),
// each refined by least squares on its indices, the one with the smaller squared error kept;
// a block holding zeros keeps them exactly 0 (an eight-entry fit from 0 keeps its low end there).
// A 3D texture's slices are compressed one by one (blocks are 4x4x1). Node 22.18; no dependencies.

/// The palette a BC4 block's endpoints make, 0 to 1.
export function palette(e0: number, e1: number): Float32Array {
  const p = new Float32Array(8);
  p[0] = e0 / 255;
  p[1] = e1 / 255;
  if (e0 > e1) {
    for (let i = 2; i < 8; i++) p[i] = ((8 - i) * e0 + (i - 1) * e1) / 7 / 255;
  } else {
    for (let i = 2; i < 6; i++) p[i] = ((6 - i) * e0 + (i - 1) * e1) / 5 / 255;
    p[6] = 0;
    p[7] = 1;
  }
  return p;
}

/// Each value's nearest palette entry and the block's squared error.
function assign(v: ArrayLike<number>, p: Float32Array, idx: Uint8Array): number {
  let sse = 0;
  for (let t = 0; t < 16; t++) {
    let best = 0;
    let bd = Infinity;
    for (let i = 0; i < 8; i++) {
      const d = (v[t] - p[i]) * (v[t] - p[i]);
      if (d < bd) {
        bd = d;
        best = i;
      }
    }
    idx[t] = best;
    sse += bd;
  }
  return sse;
}

// Where each index of a palette lies between its endpoints (0 at e0, 1 at e1), or -1 if fixed.
const W8 = [0, 1, 1 / 7, 2 / 7, 3 / 7, 4 / 7, 5 / 7, 6 / 7];
const W6 = [0, 1, 1 / 5, 2 / 5, 3 / 5, 4 / 5, -1, -1];

const clamp8 = (x: number) => Math.min(255, Math.max(0, Math.round(x)));

type Try = { e0: number; e1: number; idx: Uint8Array; sse: number };

/// The best endpoints of one mode from a start, refined twice by least squares on the indices.
function fit(v: ArrayLike<number>, lo: number, hi: number, six: boolean): Try {
  // Eight-entry mode needs e0 > e1, six-entry e0 <= e1.
  let e0 = six ? clamp8(lo * 255) : clamp8(hi * 255);
  let e1 = six ? clamp8(hi * 255) : clamp8(lo * 255);
  if (!six && e0 === e1) {
    if (e0 < 255) e0++;
    else e1--;
  }
  const w = six ? W6 : W8;
  let best: Try = { e0, e1, idx: new Uint8Array(16), sse: Infinity };
  for (let iter = 0; iter < 3; iter++) {
    const idx = new Uint8Array(16);
    const sse = assign(v, palette(e0, e1), idx);
    if (sse < best.sse) best = { e0, e1, idx, sse };
    // Least squares: v = e0 (1 - w) + e1 w over the interpolated texels, in 0-255.
    let a = 0;
    let b = 0;
    let c = 0;
    let x = 0;
    let y = 0;
    for (let t = 0; t < 16; t++) {
      const wt = w[idx[t]];
      if (wt < 0) continue;
      const u = 1 - wt;
      const val = v[t] * 255;
      a += u * u;
      b += u * wt;
      c += wt * wt;
      x += u * val;
      y += wt * val;
    }
    const det = a * c - b * b;
    if (Math.abs(det) < 1e-9) break;
    let n0 = clamp8((c * x - b * y) / det);
    // A block holding none (a flame's edge) keeps its low endpoint at 0, so none stays none.
    let n1 = !six && e1 === 0 ? 0 : clamp8((a * y - b * x) / det);
    if (six ? n0 > n1 : n0 <= n1) {
      if (six) [n0, n1] = [n1, n0];
      else if (n0 === n1) {
        if (n0 < 255) n0++;
        else n1--;
      } else [n0, n1] = [n1, n0];
    }
    if (n0 === e0 && n1 === e1) break;
    e0 = n0;
    e1 = n1;
  }
  return best;
}

/// Encodes sixteen values (0 to 1, x fastest) as one BC4 block into out at o.
export function encodeBc4(v: ArrayLike<number>, out: Uint8Array, o: number): void {
  let lo = Infinity;
  let hi = -Infinity;
  let mlo = Infinity;
  let mhi = -Infinity;
  const NEAR = 0.5 / 255;
  for (let t = 0; t < 16; t++) {
    const x = Math.min(1, Math.max(0, v[t]));
    lo = Math.min(lo, x);
    hi = Math.max(hi, x);
    if (x > NEAR && x < 1 - NEAR) {
      mlo = Math.min(mlo, x);
      mhi = Math.max(mhi, x);
    }
  }
  let best: Try;
  if (Math.round(lo * 255) === Math.round(hi * 255)) {
    const e = Math.round(lo * 255);
    best = { e0: e, e1: e, idx: new Uint8Array(16), sse: 0 };
  } else {
    best = fit(v, lo, hi, false);
    const six = fit(v, mlo === Infinity ? lo : mlo, mhi === -Infinity ? hi : mhi, true);
    if (six.sse < best.sse) best = six;
  }
  out[o] = best.e0;
  out[o + 1] = best.e1;
  let bits0 = 0;
  let bits1 = 0;
  for (let t = 0; t < 8; t++) bits0 |= best.idx[t] << (3 * t);
  for (let t = 0; t < 8; t++) bits1 |= best.idx[8 + t] << (3 * t);
  out[o + 2] = bits0 & 255;
  out[o + 3] = (bits0 >> 8) & 255;
  out[o + 4] = (bits0 >> 16) & 255;
  out[o + 5] = bits1 & 255;
  out[o + 6] = (bits1 >> 8) & 255;
  out[o + 7] = (bits1 >> 16) & 255;
}

/// Decodes one BC4 block at o into sixteen values, 0 to 1, x fastest.
export function decodeBc4(b: Uint8Array, o: number, out: Float32Array, at = 0): void {
  const p = palette(b[o], b[o + 1]);
  const bits0 = b[o + 2] | (b[o + 3] << 8) | (b[o + 4] << 16);
  const bits1 = b[o + 5] | (b[o + 6] << 8) | (b[o + 7] << 16);
  for (let t = 0; t < 8; t++) out[at + t] = p[(bits0 >> (3 * t)) & 7];
  for (let t = 0; t < 8; t++) out[at + 8 + t] = p[(bits1 >> (3 * t)) & 7];
}

/// A 3D texture of two channels (each w*h*d, x fastest, then y, then z), 0 to 1, as BC5:
/// slice by slice, blocks left to right and top to bottom, red's 8 bytes then green's.
export function encodeBc5(r: Float32Array, g: Float32Array, w: number, h: number, d: number): Uint8Array {
  if (w % 4 || h % 4) throw new Error(`BC5 needs sides of 4: ${w} x ${h}`);
  const bw = w / 4;
  const bh = h / 4;
  const out = new Uint8Array(bw * bh * d * 16);
  const vr = new Float32Array(16);
  const vg = new Float32Array(16);
  let o = 0;
  for (let z = 0; z < d; z++) {
    for (let by = 0; by < bh; by++) {
      for (let bx = 0; bx < bw; bx++) {
        for (let t = 0; t < 16; t++) {
          const i = (z * h + by * 4 + (t >> 2)) * w + bx * 4 + (t & 3);
          vr[t] = r[i];
          vg[t] = g[i];
        }
        encodeBc4(vr, out, o);
        encodeBc4(vg, out, o + 8);
        o += 16;
      }
    }
  }
  return out;
}

/// The two channels of a BC5 3D texture, as encodeBc5's inputs.
export function decodeBc5(data: Uint8Array, w: number, h: number, d: number): [Float32Array, Float32Array] {
  const r = new Float32Array(w * h * d);
  const g = new Float32Array(w * h * d);
  const vr = new Float32Array(16);
  const vg = new Float32Array(16);
  const bw = w / 4;
  const bh = h / 4;
  let o = 0;
  for (let z = 0; z < d; z++) {
    for (let by = 0; by < bh; by++) {
      for (let bx = 0; bx < bw; bx++) {
        decodeBc4(data, o, vr);
        decodeBc4(data, o + 8, vg);
        for (let t = 0; t < 16; t++) {
          const i = (z * h + by * 4 + (t >> 2)) * w + bx * 4 + (t & 3);
          r[i] = vr[t];
          g[i] = vg[t];
        }
        o += 16;
      }
    }
  }
  return [r, g];
}

// DXGI formats and the DDS header's flags (Microsoft's DDS_HEADER and DDS_HEADER_DXT10).
export const DXGI_R8G8_UNORM = 49;
export const DXGI_BC5_UNORM = 83;
const DDSD_CAPS = 0x1;
const DDSD_HEIGHT = 0x2;
const DDSD_WIDTH = 0x4;
const DDSD_PITCH = 0x8;
const DDSD_PIXELFORMAT = 0x1000;
const DDSD_MIPMAPCOUNT = 0x20000;
const DDSD_LINEARSIZE = 0x80000;
const DDSD_DEPTH = 0x800000;
const DDPF_FOURCC = 0x4;
const DDSCAPS_COMPLEX = 0x8;
const DDSCAPS_TEXTURE = 0x1000;
const DDSCAPS2_VOLUME = 0x200000;
const DIMENSION_TEXTURE3D = 4;

/// A DDS file of one 3D texture, no mipmaps, with the DX10 header naming its format.
export function dds3d(format: number, w: number, h: number, d: number, data: Uint8Array): Buffer {
  const bc = format === DXGI_BC5_UNORM;
  const pitch = bc ? (w / 4) * (h / 4) * 16 : w * 2;
  const head = Buffer.alloc(4 + 124 + 20);
  head.write("DDS ", 0, "ascii");
  const u = (o: number, x: number) => head.writeUInt32LE(x >>> 0, o);
  u(4, 124);
  u(8, DDSD_CAPS | DDSD_HEIGHT | DDSD_WIDTH | DDSD_PIXELFORMAT | DDSD_MIPMAPCOUNT | DDSD_DEPTH | (bc ? DDSD_LINEARSIZE : DDSD_PITCH));
  u(12, h);
  u(16, w);
  u(20, pitch);
  u(24, d);
  u(28, 1);
  // 11 reserved words, then the pixel format at 76.
  u(76, 32);
  u(80, DDPF_FOURCC);
  head.write("DX10", 84, "ascii");
  u(108, DDSCAPS_TEXTURE | DDSCAPS_COMPLEX);
  u(112, DDSCAPS2_VOLUME);
  // The DX10 header at 128.
  u(128, format);
  u(132, DIMENSION_TEXTURE3D);
  u(136, 0);
  u(140, 1);
  u(144, 0);
  const expect = bc ? pitch * d : w * h * d * 2;
  if (data.length !== expect) throw new Error(`a ${w}x${h}x${d} texture of format ${format} is ${expect} bytes, not ${data.length}`);
  return Buffer.concat([head, data]);
}

/// The size, format and data of a DDS file dds3d wrote.
export function readDds3d(file: Buffer): { format: number; w: number; h: number; d: number; data: Uint8Array } {
  if (file.toString("ascii", 0, 4) !== "DDS " || file.toString("ascii", 84, 88) !== "DX10") throw new Error("not a DX10 DDS file");
  const r = (o: number) => file.readUInt32LE(o);
  return { format: r(128), h: r(12), w: r(16), d: r(24), data: new Uint8Array(file.buffer, file.byteOffset + 148, file.length - 148) };
}
