// The fire's colour ramp (docs/fire.md, "The colour ramp"; D415): a flame's emission at each
// heat, from blackbody glow, as data with a source rather than a painted file. A row is a fuel;
// heat 0 to 1 runs over its temperatures, its colour is a blackbody's at that temperature (so
// it stays saturated as it dims: a thin flame goes deep red, never grey), its brightness a
// softened power of the blackbody's own luminance, and its A how far that heat occludes rather
// than adds light (hot near 0, cool near 1).
//
// The blackbody's colour is Planck's law integrated against the CIE 1931 observer, through
// Wyman, Sloan and Shirley's multi-lobe fit of it ("Simple Analytic Approximations to the CIE
// XYZ Color Matching Functions", JCGT 2013), into linear sRGB. Node 22.18 or later; no
// dependencies. `node tools/fire/ramp.ts` writes the texture the engine reads
// (assets/textures/fire/ramp.png and ramp.json); its machine tuning against the references is
// still to come.

import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { type Picture, writePng } from "../sandbox/png.ts";

/// A fuel's row: the temperatures heat 0 and 1 stand for, in kelvin, and how steeply its
/// brightness falls with heat (1 is the blackbody's own luminance, smaller is softer).
export type Fuel = { cold: number; hot: number; contrast: number };

/// The rows (D377's 950-1670 K for wood).
export const FUELS: Record<string, Fuel> = {
  wood: { cold: 950, hot: 1670, contrast: 0.35 },
  oil: { cold: 900, hot: 1500, contrast: 0.35 },
  gas: { cold: 1100, hot: 1900, contrast: 0.3 },
};

const lobe = (l: number, mu: number, s1: number, s2: number) => {
  const t = (l - mu) / (l < mu ? s1 : s2);
  return Math.exp(-0.5 * t * t);
};

/// The CIE 1931 2° observer at a wavelength in nanometres (Wyman, Sloan and Shirley 2013).
export function observer(l: number): [number, number, number] {
  return [
    1.056 * lobe(l, 599.8, 37.9, 31.0) + 0.362 * lobe(l, 442.0, 16.0, 26.7) - 0.065 * lobe(l, 501.1, 20.4, 26.2),
    0.821 * lobe(l, 568.8, 46.9, 40.5) + 0.286 * lobe(l, 530.9, 16.3, 31.1),
    1.217 * lobe(l, 437.0, 11.8, 36.0) + 0.681 * lobe(l, 459.0, 26.0, 13.8),
  ];
}

/// A blackbody's XYZ at a temperature, in arbitrary but consistent units.
export function planckXYZ(kelvin: number): [number, number, number] {
  const xyz: [number, number, number] = [0, 0, 0];
  for (let l = 380; l <= 780; l += 5) {
    const b = l ** -5 / Math.expm1(1.4388e7 / (l * kelvin));
    const o = observer(l);
    for (let k = 0; k < 3; k++) xyz[k] += o[k] * b;
  }
  return xyz;
}

/// A blackbody's colour in linear sRGB, out of gamut clipped, at a luminance of 1.
export function blackbody(kelvin: number): [number, number, number] {
  const [x, y, z] = planckXYZ(kelvin);
  const rgb = [
    3.2406 * x - 1.5372 * y - 0.4986 * z,
    -0.9689 * x + 1.8758 * y + 0.0415 * z,
    0.0557 * x - 0.204 * y + 1.057 * z,
  ].map((c) => Math.max(0, c));
  const lum = 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2];
  return [rgb[0] / lum, rgb[1] / lum, rgb[2] / lum];
}

/// The ramp at a heat for a fuel: linear RGB (luminance 1 at the hottest) and its A.
export function ramp(heat: number, fuel: Fuel = FUELS.wood): [number, number, number, number] {
  const h = Math.min(1, Math.max(0, heat));
  const t = fuel.cold + h * (fuel.hot - fuel.cold);
  const lum = (planckXYZ(t)[1] / planckXYZ(fuel.hot)[1]) ** fuel.contrast;
  const c = blackbody(t);
  return [c[0] * lum, c[1] * lum, c[2] * lum, (1 - h) * (1 - h)];
}

/// The ramp tabulated at `n` heats, as games sample it from a texture.
export function table(fuel: Fuel = FUELS.wood, n = 256): Float32Array {
  const out = new Float32Array(n * 4);
  for (let i = 0; i < n; i++) out.set(ramp(i / (n - 1), fuel), i * 4);
  return out;
}

/// The texture's rows, top to bottom, as the engine names them (fire.rs `RAMP_ROWS`).
export const ROWS = ["wood", "oil", "gas"] as const;

/// The ramp as the engine's texture: 256 wide, a row a fuel, its RGB over `scale` (the ramp's
/// brightest channel, as the blackbody's red passes 1) and sRGB-encoded so the dim red end keeps
/// its precision in 8 bits, its A linear. The engine multiplies the scale back (`ramp.json`).
export function texture(n = 256): { picture: Picture; scale: number } {
  const rows = ROWS.map((name) => table(FUELS[name], n));
  let scale = 0;
  for (const t of rows) for (let i = 0; i < t.length; i++) if (i % 4 !== 3) scale = Math.max(scale, t[i]);
  const encode = (c: number) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);
  const rgba = new Uint8Array(n * rows.length * 4);
  rows.forEach((t, y) => {
    for (let i = 0; i < n * 4; i++) {
      const v = i % 4 === 3 ? t[i] : encode(t[i] / scale);
      rgba[y * n * 4 + i] = Math.round(Math.min(1, Math.max(0, v)) * 255);
    }
  });
  return { picture: { width: n, height: rows.length, rgba }, scale };
}

if (import.meta.main) {
  const dir = join(dirname(fileURLToPath(import.meta.url)), "../../assets/textures/fire");
  const { picture, scale } = texture();
  writePng(join(dir, "ramp.png"), picture);
  const about = "The fire's colour ramp (tools/fire/ramp.ts): a row a fuel, RGB sRGB-encoded over scale, A how far a heat occludes";
  writeFileSync(join(dir, "ramp.json"), `${JSON.stringify({ about, rows: ROWS, scale: +scale.toFixed(4) }, null, 2)}\n`);
  console.log(`ramp: ${ROWS.length} rows of ${picture.width}, scale ${scale.toFixed(3)}`);
}
