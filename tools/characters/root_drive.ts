#!/usr/bin/env node
// A move's drive from its clip's root motion: the root's speed ahead (glTF +Z, the way the body
// faces) over the clip, as the `drive` keys of a move in assets/data/moves/*.ron (clip seconds,
// metres a second, joined by straight lines), so the body travels exactly as the clip steps and
// its planted feet stay put (docs/movement.md "Root motion"; Unreal's root motion from
// animation). Clips joined by + are one after another, as glb_clips.ts joins them.
//
//   node tools/characters/root_drive.ts RM.glb CLIP[+CLIP...] [--every=SECONDS] [--body=BODY.glb]
//
// --body scales the drive to that body's skeleton against the pack's, as its in-place feet are.
//
// RM.glb is a file whose clips keep their root motion on the node "root" (a pack's _RM file).
// Prints the drive and how far it goes; a root that turns or strays sideways is named, since a
// drive moves the body straight ahead only.

import { Glb, posed } from "./glb.ts";

/** The root's speed ahead every `every` seconds over `clips` played one after another, m/s. */
export function rootDrive(g: Glb, clips: string[], every: number): { drive: [number, number][]; ahead: number; aside: number } {
  const root = g.named("root");
  if (root < 0) throw new Error(`root_drive: ${g.file} has no node "root"; give the file whose clips keep their root motion (a pack's _RM file)`);
  const samples: [number, number][] = [];
  let t0 = 0;
  let ahead = 0;
  let aside = 0;
  for (const name of clips) {
    const clip = g.clip(name);
    if (!clip) throw new Error(`root_drive: ${g.file} has no clip ${name}`);
    const at = (t: number) => posed(g, clip, Math.min(clip.end, Math.max(0, t)))[root].t;
    const first = at(0);
    const steps = Math.max(1, Math.round(clip.end / every));
    // Each step's mean speed at its middle, so the drive goes as far as the root does.
    for (let k = 0; k < steps; k++) {
      const [a, b] = [(k * clip.end) / steps, ((k + 1) * clip.end) / steps];
      samples.push([t0 + (a + b) / 2, (at(b)[2] - at(a)[2]) / (b - a)]);
    }
    const last = at(clip.end);
    ahead += last[2] - first[2];
    aside = Math.max(aside, Math.abs(last[0] - first[0]));
    t0 += clip.end;
  }
  return { drive: samples, ahead, aside };
}

/** How much bigger `body`'s skeleton stands than `pack`'s: their pelvises' parents' scale. */
export function scaleAgainst(body: Glb, pack: Glb): number {
  const scale = (g: Glb) => {
    const rest = posed(g, g.clip(g.clipNames()[0])!, 0);
    return rest[g.parentOf[g.named("pelvis")]]?.s[0] ?? 1;
  };
  return scale(body) / scale(pack);
}

/** How far a drive goes: its keys joined by straight lines, held at its ends from 0 to `end`. */
export function driven(drive: [number, number][], end: number): number {
  let d = drive[0][0] * drive[0][1];
  for (let k = 1; k < drive.length; k++) d += ((drive[k][1] + drive[k - 1][1]) / 2) * (drive[k][0] - drive[k - 1][0]);
  return d + (end - drive[drive.length - 1][0]) * drive[drive.length - 1][1];
}


if (import.meta.main) {
  const args = process.argv.slice(2);
  const every = Number(args.find((a) => a.startsWith("--every="))?.split("=")[1] ?? 1 / 30);
  const [file, clips] = args.filter((a) => !a.startsWith("--"));
  if (!file || !clips) {
    console.log("usage: node tools/characters/root_drive.ts RM.glb CLIP[+CLIP...] [--every=SECONDS] [--body=BODY.glb]");
    process.exit(2);
  }
  const g = new Glb(file);
  const body = args.find((a) => a.startsWith("--body="))?.split("=")[1];
  const { drive: raw, ahead, aside } = rootDrive(g, clips.split("+"), every);
  // In the body's metres: the pack's skeleton scaled as the body's is (the fight body's 1.078),
  // so the drive goes as far as the body's in-place feet go back.
  const by = body ? scaleAgainst(new Glb(body), g) : 1;
  const drive = raw.map(([t, v]): [number, number] => [t, v * by]);
  if (body) console.log(`// scaled ${by.toFixed(3)} to ${body}'s skeleton`);
  console.log(`drive: [${drive.map(([t, v]) => `(${t.toFixed(2)}, ${v.toFixed(2)})`).join(", ")}],`);
  const end = clips.split("+").reduce((s, c) => s + (g.clip(c)?.end ?? 0), 0);
  console.log(`// ${clips}: the root goes ${ahead.toFixed(2)} m ahead in ${end.toFixed(2)} s, the drive ${driven(drive, end).toFixed(2)} m${aside > 0.05 ? `; the root strays ${aside.toFixed(2)} m aside, which a drive does not follow` : ""}`);
}
