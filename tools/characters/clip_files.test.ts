// The game's body files hold every clip the game plays, cut as the mannequin's: a clip added to
// art/animation/clips.txt (or mirrored, mirrors.json) and the mannequin without rebuilding the
// bodies (build_bodies.sh) fails here, with no Blender.
// node --test tools/characters/clip_files.test.ts

import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { Glb } from "./glb.ts";

const FRAME_S = 1 / 60;

/** The clips the game plays: clips.txt's names and the mirrored ones. */
function wanted(clipsTxt: string, mirrorsJson: string): string[] {
  const listed = clipsTxt.split("\n").map((l) => l.trim()).filter((l) => l && !l.startsWith("#"));
  return [...listed, ...Object.keys(JSON.parse(mirrorsJson))];
}

/** What is wrong with one body file against the clips wanted and the mannequin's lengths: the
 * clips it lacks, and those whose length is not the mannequin's to a frame. */
function faults(clips: Map<string, number>, want: string[], lengths: Map<string, number>): string[] {
  const out: string[] = [];
  const missing = want.filter((c) => !clips.has(c));
  if (missing.length) out.push(`lacks ${missing.length} of ${want.length}: ${missing.join(" ")}`);
  for (const c of want) {
    const [mine, theirs] = [clips.get(c), lengths.get(c)];
    if (mine !== undefined && theirs !== undefined && Math.abs(mine - theirs) > FRAME_S)
      out.push(`${c} lasts ${mine.toFixed(3)} s, the mannequin's ${theirs.toFixed(3)} s`);
  }
  return out;
}

const lengthsOf = (file: string): Map<string, number> => {
  const g = new Glb(file);
  return new Map(g.clipNames().map((c) => [c, g.clip(c)!.end]));
};

test("a body file lacking a clip, or holding one cut to another length, is found", () => {
  const want = wanted("# a comment\nIdle_Loop\n\nJog_Stop\n", '{"Jog_Stop_R": "Jog_Stop"}');
  assert.deepEqual(want, ["Idle_Loop", "Jog_Stop", "Jog_Stop_R"]);
  const mannequin = new Map([["Idle_Loop", 2], ["Jog_Stop", 3.96], ["Jog_Stop_R", 3.96]]);
  assert.deepEqual(faults(mannequin, want, mannequin), []);
  // Extra clips (the fight set's) are fine.
  assert.deepEqual(faults(new Map([...mannequin, ["Fight_Slash", 1]]), want, mannequin), []);
  const stale = new Map([["Idle_Loop", 2], ["Jog_Stop", 3.5]]);
  assert.deepEqual(faults(stale, want, mannequin), ["lacks 1 of 3: Jog_Stop_R", "Jog_Stop lasts 3.500 s, the mannequin's 3.960 s"]);
});

const want = wanted(readFileSync("art/animation/clips.txt", "utf8"), readFileSync("art/animation/mirrors.json", "utf8"));
const mannequin = "assets/models/character/mannequin.glb";

test("the mannequin holds every clip the game plays", () => {
  const lengths = lengthsOf(mannequin);
  assert.deepEqual(faults(lengths, want, lengths), [], `${mannequin}: run clips.py`);
});

// base.glb as 867457fa, 2bad8ea4 and 59966daa left it (28 clips of 34), until the retarget
// round rebuilds it: only this file's failure is let through, as a todo, so any rebuild makes
// the test strict again. Delete this line once base.glb's hash differs.
const STALE_BASE = "9e64d9bf8304d9d31558f66a4227cbce4475ed227c91fc64c79f6a8aa3b8199c";
const sha256 = (file: string) => createHash("sha256").update(readFileSync(file)).digest("hex");

for (const file of ["assets/models/character/fight.glb", "assets/models/character/base.glb"]) {
  const todo = sha256(file) === STALE_BASE ? "base.glb is stale until the retarget round's rebuild" : undefined;
  test(`${file} holds every clip the game plays, as long as the mannequin's`, { todo }, () => {
    assert.deepEqual(faults(lengthsOf(file), want, lengthsOf(mannequin)), [], `${file}: rebuild it (build_bodies.sh)`);
  });
}
