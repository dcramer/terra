// The cabin the building clips are of (tools/sandbox/qa.ts): where it stands, and how far it
// is put up, at once and off camera, before a clip films the stage it is about. Places are
// the base's cubes, half a metre each, east, up a storey and south of the claim stone, and
// metres for the camera.

import type { KitSet, Script, Vec3 } from "../script.ts";

/// Its corner tiles: five metres east to west and four north to south.
export const [X0, X1, Z0, Z1] = [4, 13, 4, 11];
/// Its middle on the ground, in metres.
export const C: Vec3 = [4.5, 0, 4];
export const DOOR = [
  [8, 0, Z1],
  [9, 0, Z1],
];
export const WINDOW = [
  [X1, 0, 7],
  [X1, 0, 8],
];
/// A window cut into the north wall once it is built.
export const CUT = [
  [8, 0, Z0],
  [9, 0, Z0],
];
/// The deck's posts, east of the cabin.
export const POSTS = [
  [17, 0, 5],
  [22, 0, 5],
  [17, 0, 10],
  [22, 0, 10],
];

/// The sandbox a clip runs in: the play experiment by the lake, building free (D333).
export const SANDBOX = ["--experiment", "play", "--free-build", "--time", "10", "--at", "0,3", "--face", "-8,14"];

/// How far the cabin is put up before a clip: its plan; its walls built; its roof on too.
export type Stage = "plan" | "walls" | "roof";

/// The claim stone set down, and the cabin put up as far as `upTo`, all at once in the first
/// frames, then time for its sections to be laid and its textures to come in, unfilmed. A
/// clip's own stage starts where this leaves the cursor.
export function putUp(s: Script, set: KitSet, upTo: Stage) {
  s.call("storage.place", { item: "Claim stone", at: [0, 0, 0] });
  s.baseMovesAt(s.frame + 1, [0, -0.465, 0]);
  s.wait(2);
  plan(s, set);
  if (upTo !== "plan") {
    buildAll(s);
    if (upTo === "roof") {
      s.call("build.lay", { what: "roof", over: [8, 0, 7], kind: set.wall(0), style: "Gable" });
      s.wait(1);
      buildAll(s);
    }
  }
  s.wait(240);
}

/// The cabin's plan: its four walls (a mix lays each in its own kind), a stone floor, a
/// doorway and a window.
export function plan(s: Script, set: KitSet, gap = 0) {
  const walls: [number[], number[]][] = [
    [[X0, 0, Z1], [X1, 0, Z1]],
    [[X1, 0, Z0], [X1, 0, Z1]],
    [[X0, 0, Z0], [X1, 0, Z0]],
    [[X0, 0, Z0], [X0, 0, Z1]],
  ];
  walls.forEach(([min, max], i) => {
    s.call("build.lay", { what: "wall", min, max, kind: set.wall(i) });
    s.wait(gap);
  });
  s.call("build.lay", { what: "floor", min: [X0 + 1, 0, Z0 + 1], max: [X1 - 1, 0, Z1 - 1], kind: "stone" });
  s.wait(gap);
  s.call("build.opening", { tiles: DOOR, door: true });
  s.wait(gap);
  s.call("build.opening", { tiles: WINDOW, door: false });
  s.wait(Math.max(1, gap));
}

/// The cabin's gable roof taken off as a player takes it apart, last in first out: from the
/// ridge down to the eaves, each row on both slopes through its four stages, a row every
/// `gap` frames.
export function roofOff(s: Script, gap = 15) {
  const rows: [number, number][] = [
    [9, 7],
    [9, 8],
    [8, 6],
    [8, 9],
    [7, 5],
    [7, 10],
    [6, Z0],
    [6, Z1],
  ];
  for (const [y, z] of rows) {
    s.call("build.take_apart", { at: [8, y, z], steps: 4 });
    s.wait(gap);
  }
}

/// Everything planned built, at once.
export function buildAll(s: Script) {
  for (let i = 0; i < 3; i++) {
    s.call("build.fill", { steps: 100000 });
    s.wait(1);
  }
}
