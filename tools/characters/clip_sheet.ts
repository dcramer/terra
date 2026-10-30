#!/usr/bin/env node
// Clips as made, on our body, for the developer's eye (a bought pack's, a take's): a film of
// each played once at its own speed from two sides, the body carried by the clip's root motion
// over a ground marked every half metre, a sword and shield in its hands, and a sheet of six
// moments of each. Blender renders (clip_sheet.py); run it through the GPU queue:
//
//   cargo xtask gpu film -- node tools/characters/clip_sheet.ts OUT BODY.glb [--root=RM.glb]
//       [--held=RIGHT.glb,LEFT.glb|none] ENTRY...
//
// BODY.glb is a body exported with the clips (export_character.py --clips=). ENTRY is a clip or
// clips joined by + (a blow and its recovery). --root reads each clip's root motion from a file
// of the same clips with it kept (the packs' _RM files); without it the body stands in place.
// Writes OUT/film.mp4 (the review desk's encoding), OUT/sheet.jpg and OUT/roots.json; the
// frames and Blender's log stay in OUT/frames/.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { Glb, posed } from "./glb.ts";

const args = process.argv.slice(2);
const opt = (name: string) => args.find((a) => a.startsWith(`--${name}=`))?.split("=")[1];
const [out, body, ...entries] = args.filter((a) => !a.startsWith("--"));
if (!out || !body || !entries.length) {
  console.log("usage: node tools/characters/clip_sheet.ts OUT BODY.glb [--root=RM.glb] [--held=RIGHT.glb,LEFT.glb|none] ENTRY...");
  process.exit(2);
}
const held = opt("held") ?? "assets/models/held/arming_sword.glb,assets/models/held/heater_shield.glb";

/** Each clip's root travel from its first frame, a sample each 1/30 s, in glTF's axes. */
export function rootTravel(file: string, clips: string[]): Record<string, number[][]> {
  const g = new Glb(file);
  const root = g.named("root");
  if (root < 0) throw new Error(`clip_sheet: ${file} has no root node; give a file whose clips keep their root motion on "root"`);
  const travel: Record<string, number[][]> = {};
  for (const name of clips) {
    const clip = g.clip(name);
    if (!clip) throw new Error(`clip_sheet: ${file} has no clip ${name}`);
    const at = (t: number) => posed(g, clip, t)[root].t;
    const first = at(0);
    travel[name] = [];
    for (let k = 0; k <= Math.round(clip.end * 30); k++) travel[name].push(at(k / 30).map((x, i) => x - first[i]));
  }
  return travel;
}

mkdirSync(out, { recursive: true });
const frames = join(out, "frames");
rmSync(frames, { recursive: true, force: true });
mkdirSync(frames);
const clips = [...new Set(entries.flatMap((e) => e.split("+")))];
const roots = opt("root") ? rootTravel(opt("root")!, clips) : {};
writeFileSync(join(out, "roots.json"), JSON.stringify(roots));
const log = join(frames, "blender.log");
try {
  execFileSync("bash", ["-c", `tools/blender/run.sh tools/characters/clip_sheet.py "$@" > "${log}" 2>&1`, "_", frames, body, join(out, "roots.json"), held, ...entries], { stdio: "inherit" });
} catch {
  console.log(`✗ clip_sheet: Blender failed; read ${log}`);
  process.exit(1);
}
const count = readdirSync(frames).filter((f) => f.startsWith("a_")).length;
console.log(`✓ rendered ${count} frames of ${entries.length} entries`);
const ff = (...a: string[]) => execFileSync("ffmpeg", ["-y", "-loglevel", "error", ...a], { stdio: "inherit" });
ff("-framerate", "30", "-i", join(frames, "a_%05d.png"), "-framerate", "30", "-i", join(frames, "b_%05d.png"), "-filter_complex", "hstack=inputs=2,scale=1600:-2", "-an", "-c:v", "libx264", "-crf", "28", "-preset", "slow", "-pix_fmt", "yuv420p", "-movflags", "+faststart", join(out, "film.mp4"));
ff("-i", join(frames, "sheet.png"), "-vf", "scale=1680:-2", "-q:v", "4", join(out, "sheet.jpg"));
if (!existsSync(join(out, "film.mp4"))) process.exit(1);
console.log(`✓ ${join(out, "film.mp4")} and ${join(out, "sheet.jpg")}`);
