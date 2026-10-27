// The motion gate's A/A check (D472): one build's film of one placement at one hour made twice
// (film.ts's film), and the two compared frame by frame, where the first differs and by how much
// (which pixels, how far), then each film measured as the gate does. Two films of one build
// should match to the pixel; where they do not, the measures move with no change.
//
//   node tools/fire/aa.ts [out=shots/aa] [--at 0.9] [--hour dusk] [--seconds 20] [--runs 2]
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { build, film, HOURS } from "./film.ts";
import { analyse, gate, load } from "./motion.ts";
import { writePng } from "../sandbox/png.ts";

/// Two pictures' difference: pixels that differ, the largest step, and the box round them.
export function compare(a: Uint8Array, b: Uint8Array, w: number, channels = 3) {
  let pixels = 0;
  let most = 0;
  let sum = 0;
  const box = { x0: w, y0: Infinity, x1: -1, y1: -1 };
  for (let p = 0; p * channels < a.length; p++) {
    let d = 0;
    for (let c = 0; c < channels; c++) d = Math.max(d, Math.abs(a[p * channels + c] - b[p * channels + c]));
    if (!d) continue;
    pixels++;
    sum += d;
    most = Math.max(most, d);
    const [x, y] = [p % w, Math.floor(p / w)];
    box.x0 = Math.min(box.x0, x);
    box.x1 = Math.max(box.x1, x);
    box.y0 = Math.min(box.y0, y);
    box.y1 = Math.max(box.y1, y);
  }
  return { pixels, most, mean: pixels ? sum / pixels : 0, box };
}

const raw = (png: string, grey: boolean) => {
  const out = execFileSync("ffmpeg", ["-v", "error", "-i", png, "-f", "rawvideo", "-pix_fmt", grey ? "gray" : "rgb24", "-"], { maxBuffer: 2 ** 28 });
  return new Uint8Array(out.buffer, out.byteOffset, out.length);
};
const size = (png: string) => {
  const [w, h] = execFileSync("ffprobe", ["-v", "error", "-i", png, "-show_entries", "stream=width,height", "-of", "csv=p=0"], { encoding: "utf8" }).trim().split(",").map(Number);
  return { w, h };
};
/// The script frame the gate's films roll from (film.ts's `--cine …@120`).
const ROLL = 120;
const sha = (path: string) => createHash("sha1").update(readFileSync(path)).digest("hex");

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const opt = (name: string, or: string) => (argv.includes(name) ? argv[argv.indexOf(name) + 1] : or);
  const out = argv.find((a, i) => !a.startsWith("--") && !argv[i - 1]?.startsWith("--")) ?? "shots/aa";
  const [dx, hourName, seconds, runs] = [+opt("--at", "0.9"), opt("--hour", "dusk"), +opt("--seconds", "20"), +opt("--runs", "2")];
  const hour = HOURS.find(([h]) => h === hourName);
  if (!hour) throw new Error(`no hour ${hourName}: ${HOURS.map(([h]) => h).join(", ")}`);
  const root = process.cwd();
  build(root);
  const bin = join(root, "target/release/terra_sandbox");
  const dirs = Array.from({ length: runs }, (_, r) => join(out, `r${r}`));
  const from = +opt("--from", "120");
  for (const d of dirs) film(root, bin, d, dx, seconds, [hour], true, from);
  console.log(`✓ filmed placement ${dx} at ${hourName} ${runs} times, ${seconds} s`);
  const lines: string[] = [];
  const frames = (d: string) => readdirSync(join(d, `${hourName}-frames`)).filter((f) => f.endsWith(".png")).sort();
  const names = frames(dirs[0]);
  for (let r = 1; r < runs; r++) {
    const [a, b] = [join(dirs[0], `${hourName}-frames`), join(dirs[r], `${hourName}-frames`)];
    const other = new Set(frames(dirs[r]));
    const differ = names.filter((n) => other.has(n) && sha(join(a, n)) !== sha(join(b, n)));
    lines.push(`r0 against r${r}: ${names.length} files (${other.size} in r${r}), ${differ.length} differ; first ${differ.slice(0, 6).join(" ")}`);
    // Frame 120 always: the gate's films start there (film.ts's roll), so it is what they see first.
    const roll = names.filter((n) => +(/f-0*(\d+)/.exec(n)?.[1] ?? -1) === ROLL);
    const sampled = [...differ.slice(0, 4), ...differ.filter((_, i) => i % 50 === 0).slice(1, 8)];
    for (const n of [...new Set([...sampled, ...roll])]) {
      if (!differ.includes(n)) {
        lines.push(`  ${n}: matches`);
        continue;
      }
      const grey = n.endsWith(".matte.png");
      const { w, h } = size(join(a, n));
      const [pa, pb] = [raw(join(a, n), grey), raw(join(b, n), grey)];
      const c = compare(pa, pb, w, grey ? 1 : 3);
      lines.push(`  ${n}: ${c.pixels} of ${w * h} px differ, most ${c.most}, mean ${c.mean.toFixed(1)}, in x ${c.box.x0}-${c.box.x1} y ${c.box.y0}-${c.box.y1}`);
      if (c.mean > 4) {
        const rgba = new Uint8Array(w * h * 4);
        const ch = grey ? 1 : 3;
        for (let p = 0; p < w * h; p++) {
          let d = 0;
          for (let k = 0; k < ch; k++) d = Math.max(d, Math.abs(pa[p * ch + k] - pb[p * ch + k]));
          const v = d ? Math.min(255, 64 + d * 8) : pa[p * ch] >> 2;
          rgba.set(d ? [v, 0, 0, 255] : [v, v, v, 255], 4 * p);
        }
        writePng(join(out, `diff-r${r}-${n}`), { width: w, height: h, rgba });
        copyFileSync(join(a, n), join(out, `r0-${n}`));
        copyFileSync(join(b, n), join(out, `r${r}-${n}`));
      }
    }
  }
  const probes = dirs.map((d) => readFileSync(join(d, `${hourName}-run.log`), "utf8").split("\n").filter((l) => l.includes("PROBE")).map((l) => l.slice(l.indexOf("PROBE"))));
  const byScript = probes.map((ps) => {
    const m = new Map<number, string[]>();
    for (const p of ps) {
      const s = +(/script (\d+)/.exec(p)?.[1] ?? -1);
      m.set(s, [...(m.get(s) ?? []), p]);
    }
    return m;
  });
  // A throwaway `info!("PROBE script {} ...")` in the game (its script frame first) is compared
  // script frame by script frame, to tell what the game did apart from what it drew.
  for (let r = 1; r < runs && probes[0].length; r++) {
    const differ = [...byScript[0].keys()].filter((s) => s > 0 && byScript[0].get(s)!.join("|") !== byScript[r].get(s)?.join("|"));
    lines.push(`probes: r0 ${probes[0].length}, r${r} ${probes[r].length}; script frames differing ${differ.length}: ${differ.slice(0, 30).join(" ")}`);
    for (const s of [1, 2, 3, 4, ...differ.slice(0, 3), 120, 122]) {
      for (const [k, m] of [[0, byScript[0]], [r, byScript[r]]] as const) lines.push(`  ${s} r${k}: ${(m.get(s) ?? ["-"]).join(" || ")}`);
    }
  }
  for (let r = 1; r < runs; r++) {
    const [a, b] = [join(dirs[0], `${hourName}-frames`), join(dirs[r], `${hourName}-frames`)];
    const kind = (n: string) => (n.endsWith(".matte.png") ? "matte" : "picture");
    for (const k of ["picture", "matte"]) {
      const differ = names.filter((n) => kind(n) === k && sha(join(a, n)) !== sha(join(b, n))).map((n) => +/f-0*(\d+)/.exec(n)![1]);
      lines.push(`${k}s differing ${differ.length}: ${differ.slice(0, 40).join(" ")}`);
    }
  }
  for (const d of from === 120 ? dirs : []) {
    const src = (input: string) => ({ input, fps: 30, from: 0, seconds });
    const { clip, cut } = load(src(join(d, `${hourName}.mkv`)), src(join(d, `${hourName}-matte.mkv`)));
    const g = gate(analyse(clip, cut));
    const log = readFileSync(join(d, `${hourName}-run.log`), "utf8").split("\n").filter((l) => /held|Script frame/.test(l));
    lines.push(`${d}: cut ${cut.toFixed(1)}, box ${clip.w}x${clip.h}: ${g.map((x) => `${x.name} ${x.value.toFixed(3)}`).join(" ")}`);
    for (const l of log.slice(0, 6)) lines.push(`  log: ${l.slice(0, 200)}`);
  }
  writeFileSync(join(out, "aa.txt"), lines.join("\n") + "\n");
  console.log(lines.join("\n"));
  for (const d of dirs) rmSync(join(d, `${hourName}-frames`), { recursive: true, force: true });
}
