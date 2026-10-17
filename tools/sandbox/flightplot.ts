// A flight-path plot of a scene's flier (trailer/flight.ts, as `play::fliers` flies it), to
// check a course before anything is filmed: from above, coloured by speed, a dot a second;
// its height; and its speed, bank, climb and how far off the camera's aim it is, through the
// film. Orange is where it breathes fire (after its wind-up).
//
//   node tools/sandbox/flightplot.ts <scene> <out.png>
//
// <scene> is a scene's name (trailer/scenes/<name>.ts). The film's seconds count from the
// frame it rolls on.

import { execFileSync } from "node:child_process";
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { Script, type Vec3 } from "./qa/script.ts";
import { Course, wings } from "./trailer/flight.ts";

const [name, out] = process.argv.slice(2);
if (!name || !out) {
  console.error("usage: node tools/sandbox/flightplot.ts <scene> <out.png>");
  process.exit(1);
}
const scene = (await import(pathToFileURL(join(import.meta.dirname, "trailer/scenes", `${name}.ts`)).href)).default as { rolls?: number; script: (s: Script) => void };
const s = new Script();
scene.script(s);
const calls = s.all();
const rolls = scene.rolls ?? 0;
const w = wings();
const reach = Number(/reach_m:\s*([\d.]+)/.exec(String(execFileSync("cat", [join(import.meta.dirname, "../../assets/data/fliers.ron")])))?.[1] ?? 30);
const windup = Number(/windup_s:\s*([\d.]+)/.exec(String(execFileSync("cat", [join(import.meta.dirname, "../../assets/data/fliers.ron")])))?.[1] ?? 0);

// The course, replayed from the calls, as the game is told it.
let course: Course | undefined;
const breaths: [number, number][] = [];
// Where each breath is aimed, as it sweeps.
const aims: [number, Vec3][] = [];
let breathing: number | undefined;
const looks = new Map<number, [Vec3, Vec3]>();
let last = 0;
for (const call of calls) {
  const p = call.params as Record<string, unknown>;
  last = Math.max(last, call.frame);
  if (call.method === "flight.spawn") course = new Course(p.at as Vec3, (p.facing as Vec3) ?? (p.at as Vec3), w);
  else if (call.method === "flight.path") course?.fly(call.frame, p.through as Vec3[], p.facing as Vec3 | undefined, p.speed as number | undefined);
  else if (call.method === "flight.hover") course?.hover(call.frame, p.at as Vec3, p.facing as Vec3);
  else if (call.method === "flight.breathe") {
    if (p.on && breathing === undefined) breathing = call.frame;
    if (p.on && p.at) aims.push([call.frame, p.at as Vec3]);
    if (!p.on && breathing !== undefined) {
      breaths.push([breathing + windup * 60, call.frame]);
      breathing = undefined;
    }
  } else if (call.method === "build.look") looks.set(call.frame, [p.from as Vec3, p.at as Vec3]);
}
if (!course) {
  console.error(`${name} spawns no flier`);
  process.exit(1);
}

type Row = { t: number; at: Vec3; speed: number; bank: number; climb: number; off?: number; fire: boolean };
const rows: Row[] = [];
let look: [Vec3, Vec3] | undefined;
for (let frame = rolls; frame <= last; frame++) {
  look = looks.get(frame) ?? look;
  const st = course.state(frame);
  const leg = st.leg?.plan;
  let bend = 0;
  if (leg) {
    const t = (frame - (st.leg?.frame ?? 0)) / 60;
    const i = Math.min(leg.time.findIndex((x) => x > t), leg.bend.length - 1);
    bend = i >= 0 ? leg.bend[i] : 0;
  }
  const bank = (Math.atan((st.speed * st.speed * bend) / 9.81) * 180) / Math.PI;
  const climb = (Math.asin(Math.max(-1, Math.min(1, st.way[1]))) * 180) / Math.PI;
  let off: number | undefined;
  if (look) {
    const [eye, at] = look;
    const a = [at[0] - eye[0], at[1] - eye[1], at[2] - eye[2]];
    const b = [st.at[0] - eye[0], st.at[1] - eye[1], st.at[2] - eye[2]];
    const cos = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / (Math.hypot(...a) * Math.hypot(...b));
    off = (Math.acos(Math.max(-1, Math.min(1, cos))) * 180) / Math.PI;
  }
  rows.push({ t: (frame - rolls) / 60, at: st.at, speed: st.speed, bank: Math.max(-60, Math.min(60, bank)), climb, off, fire: breaths.some(([a, b]) => frame >= a && frame < b) });
}

const W = 1600;
const H = 1100;
const parts: string[] = [];
const text = (x: number, y: number, t: string, size = 13, fill = "#ddd", anchor = "start") =>
  parts.push(`<text x="${x.toFixed(1)}" y="${y.toFixed(1)}" font-size="${size}" fill="${fill}" text-anchor="${anchor}" font-family="sans-serif">${t}</text>`);
const speedColour = (v: number) => `hsl(${Math.round(240 - (Math.min(v, w.cruise * 1.2) / (w.cruise * 1.2)) * 240)},85%,55%)`;

// From above: x east, z south (north up).
{
  const [x0, y0, size] = [20, 40, 760];
  const xs = rows.map((r) => r.at[0]);
  const zs = rows.map((r) => r.at[2]);
  const [minX, maxX, minZ, maxZ] = [Math.min(...xs), Math.max(...xs), Math.min(...zs), Math.max(...zs)];
  const span = Math.max(maxX - minX, maxZ - minZ) * 1.08;
  const [cx, cz] = [(minX + maxX) / 2, (minZ + maxZ) / 2];
  const px = (x: number) => x0 + size / 2 + ((x - cx) / span) * size;
  const pz = (z: number) => y0 + size / 2 + ((z - cz) / span) * size;
  parts.push(`<rect x="${x0}" y="${y0}" width="${size}" height="${size}" fill="#1b1f24" stroke="#444"/>`);
  text(x0 + 6, y0 - 10, `From above (north up), ${Math.round(span)} m across; colour is speed, blue slow to red ${Math.round(w.cruise * 1.2)} m/s`, 14);
  // A 10 m grid, the base's middle marked.
  for (let g = Math.ceil((cx - span / 2) / 10) * 10; g < cx + span / 2; g += 10) parts.push(`<line x1="${px(g)}" y1="${y0}" x2="${px(g)}" y2="${y0 + size}" stroke="${g === 0 ? "#555" : "#262b31"}"/>`);
  for (let g = Math.ceil((cz - span / 2) / 10) * 10; g < cz + span / 2; g += 10) parts.push(`<line x1="${x0}" y1="${pz(g)}" x2="${x0 + size}" y2="${pz(g)}" stroke="${g === 0 ? "#555" : "#262b31"}"/>`);
  for (let i = 1; i < rows.length; i++) {
    const [a, b] = [rows[i - 1], rows[i]];
    parts.push(`<line x1="${px(a.at[0])}" y1="${pz(a.at[2])}" x2="${px(b.at[0])}" y2="${pz(b.at[2])}" stroke="${b.fire ? "#ff8a00" : speedColour(b.speed)}" stroke-width="${b.fire ? 6 : 2.5}"/>`);
  }
  // Within its breath's reach of where it is then, red; past it, grey: its flame ends short.
  for (const [frame, a] of aims) {
    const from = course.at(frame);
    const far = Math.hypot(a[0] - from[0], a[1] - from[1], a[2] - from[2]) > reach;
    parts.push(`<circle cx="${px(a[0])}" cy="${pz(a[2])}" r="2.2" fill="${far ? "#888" : "#ff3b00"}"/>`);
  }
  for (const r of rows.filter((r) => Math.abs(r.t - Math.round(r.t)) < 1e-6)) {
    parts.push(`<circle cx="${px(r.at[0])}" cy="${pz(r.at[2])}" r="3" fill="#fff"/>`);
    if (Math.round(r.t) % 2 === 0) text(px(r.at[0]) + 5, pz(r.at[2]) - 4, `${Math.round(r.t)}`, 11, "#fff");
  }
}

// Through the film: height, speed, bank, climb and how far off the camera's aim.
const series: { label: string; get: (r: Row) => number | undefined; lo: number; hi: number; colour: string }[] = [
  { label: "height (m)", get: (r) => r.at[1], lo: 0, hi: Math.max(...rows.map((r) => r.at[1])) * 1.1, colour: "#9fd" },
  { label: "speed (m/s)", get: (r) => r.speed, lo: 0, hi: w.cruise * 1.3, colour: "#fd6" },
  { label: "bank asked (°, + left)", get: (r) => r.bank, lo: -60, hi: 60, colour: "#f9a" },
  { label: "climb (°)", get: (r) => r.climb, lo: -45, hi: 45, colour: "#9cf" },
  { label: "off the camera's aim (°)", get: (r) => r.off, lo: 0, hi: 60, colour: "#ccc" },
];
{
  const [x0, x1] = [830, W - 20];
  const end = rows[rows.length - 1].t;
  const tx = (t: number) => x0 + (t / end) * (x1 - x0);
  const band = (H - 80) / series.length;
  series.forEach((sr, k) => {
    const y0 = 40 + k * band;
    const h = band - 30;
    const py = (v: number) => y0 + h - ((v - sr.lo) / (sr.hi - sr.lo)) * h;
    parts.push(`<rect x="${x0}" y="${y0}" width="${x1 - x0}" height="${h}" fill="#1b1f24" stroke="#444"/>`);
    for (const [a, b] of breaths) parts.push(`<rect x="${tx((a - rolls) / 60)}" y="${y0}" width="${tx((b - rolls) / 60) - tx((a - rolls) / 60)}" height="${h}" fill="#ff8a00" opacity="0.18"/>`);
    if (sr.lo < 0) parts.push(`<line x1="${x0}" y1="${py(0)}" x2="${x1}" y2="${py(0)}" stroke="#555"/>`);
    for (let t = 0; t <= end; t += 5) {
      parts.push(`<line x1="${tx(t)}" y1="${y0}" x2="${tx(t)}" y2="${y0 + h}" stroke="#2c3238"/>`);
      text(tx(t), y0 + h + 13, `${t}s`, 10, "#888", "middle");
    }
    const pts = rows.map((r) => [r, sr.get(r)] as const).filter(([, v]) => v !== undefined).map(([r, v]) => `${tx(r.t).toFixed(1)},${py(Math.max(sr.lo, Math.min(sr.hi, v as number))).toFixed(1)}`);
    parts.push(`<polyline points="${pts.join(" ")}" fill="none" stroke="${sr.colour}" stroke-width="1.6"/>`);
    text(x0 + 6, y0 + 15, `${sr.label}  ${sr.lo} to ${Math.round(sr.hi)}`, 13, sr.colour);
  });
}
text(20, H - 20, `${name}: ${rows.length} frames from the roll; orange is fire, after a ${windup} s wind-up; red dots are where it is aimed within its reach, grey past it`, 14, "#aaa");

const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}"><rect width="100%" height="100%" fill="#111"/>${parts.join("")}</svg>`;
const svgFile = out.replace(/\.png$/, "") + ".svg";
writeFileSync(svgFile, svg);
execFileSync("rsvg-convert", ["-o", out, svgFile]);
console.log(`Wrote ${out}.`);
