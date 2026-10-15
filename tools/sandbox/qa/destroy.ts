// The destruction film: what harm does to what is built in a set (D193, D305). A house
// cracks under blows; a freestanding wall's foot is knocked out and the wall above falls into
// debris; a house's walls go and its roof comes down; a tower falls whole; the ground gives
// way under a corner; the debris is gathered and the cracked wall mended. Places as in
// build.ts: the base's cubes for what is done, metres for the camera.

import type { KitSet, Scenario, Script, Vec3 } from "./script.ts";

/// A kind's health by its pattern (D305: its pattern's file; a material may say otherwise).
const HEALTH: Record<string, number> = { block: 600, coursed: 400, frame: 300 };

const HOUSE: Vec3 = [4.5, 0, 4];
const SHED: Vec3 = [12, 0, 3.5];
const TOWER: Vec3 = [-2.75, 0, 3.5];
const WALL: Vec3 = [4.5, 0, 8.5];
const ALL: Vec3 = [4.5, 1, 5];

function script(s: Script, set: KitSet) {
  const health = (n: number) => HEALTH[set.walls[n % set.walls.length].pattern] ?? 400;
  // The house, the shed, the tower and the wall: the camera keeps out.
  s.solid([2, -1, 2], [7, 5.5, 6]);
  s.solid([10, -1, 2], [14, 5, 5]);
  s.solid([-4, -1, 2], [-1, 14, 5]);
  s.solid([2, -1, 8.5], [7, 3, 9]);

  s.stage("built", "What stands: a house, a shed, a tower and a wall", () => {
    s.call("storage.place", { item: "Claim stone", at: [0, 0, 0] });
    s.baseMovesAt(s.frame + 1, [0, -0.465, 0]);
    s.wait(2);
    s.call("build.lay", { what: "room", min: [4, 0, 4], max: [13, 0, 11], kind: set.wall(0), floor: "stone" });
    s.call("build.opening", { tiles: [[8, 0, 11], [9, 0, 11]], door: true });
    s.call("build.lay", { what: "roof", over: [8, 0, 7], kind: set.wall(0), style: "Gable" });
    s.call("build.lay", { what: "room", min: [20, 0, 4], max: [27, 0, 9], kind: set.wall(1), floor: "stone" });
    s.call("build.lay", { what: "roof", over: [23, 0, 6], kind: set.wall(1), style: "Hip" });
    for (let storey = 0; storey < 4; storey++) s.call("build.lay", { what: "room", min: [-8, storey, 4], max: [-3, storey, 9], kind: set.wall(2) });
    s.call("build.lay", { what: "roof", over: [-6, 3, 6], kind: set.wall(2), style: "Hip" });
    s.call("build.lay", { what: "wall", min: [4, 0, 17], max: [13, 0, 17], kind: set.wall(3) });
    s.wait(2);
    // Built at once, before the film's first frame is worth looking at.
    s.call("build.fill", { steps: 100000 });
    s.wait(2);
    s.call("build.fill", { steps: 100000 });
    s.orbit(ALL, { radius: 22, height: 9, from: 150, to: 205, frames: 420, look: 1 });
    s.wait(420);
  });

  s.stage("cracks", "Blows crack a wall", () => {
    s.glide([4.5, 1.9, -4], [4.5, 1.2, 2], 120);
    s.wait(120);
    const hit = Math.ceil(health(0) / 5);
    for (let i = 0; i < 4; i++) {
      s.call("build.damage", { at: [5, 0, 4], to: [8, 4, 4], amount: hit });
      s.wait(60);
    }
    s.wait(60);
  });

  s.stage("wall", "A wall's foot knocked out: the wall above falls", () => {
    s.glide([4.5, 9, 4], [4.5, 1, 6], 70);
    s.glide([4.5, 3.0, 15.5], [4.5, 0.6, 8.5], 90, 50);
    s.wait(120);
    s.call("build.damage", { at: [4, 0, 17], to: [13, 0, 17] });
    s.orbit(WALL, { radius: 7, height: 2.5, from: 0, to: 30, frames: 240, look: 0.3 });
    s.wait(240);
  });

  s.stage("roof", "A shed's walls knocked out: its roof comes down", () => {
    s.glide([18, 6, 13], [12, 1.5, 3.5], 150);
    s.wait(150);
    for (const [min, max] of [
      [[20, 0, 4], [27, 5, 4]],
      [[20, 0, 9], [27, 5, 9]],
      [[20, 0, 5], [20, 5, 8]],
      [[27, 0, 5], [27, 5, 8]],
    ]) {
      s.call("build.damage", { at: min, to: max });
      s.wait(45);
    }
    s.orbit(SHED, { radius: 10, height: 4, from: 45, to: 80, frames: 240 });
    s.wait(240);
  });

  s.stage("tower", "A tower's foot knocked out: the tower comes down", () => {
    s.glide([6, 10, 18], [2, 3, 4], 80);
    s.glide([-12, 9, 15], [-2.75, 5, 3.5], 110, 60);
    s.wait(150);
    s.call("build.damage", { at: [-8, 0, 4], to: [-3, 1, 9] });
    s.orbit(TOWER, { radius: 15, height: 6, from: 320, to: 345, frames: 300, look: 3 });
    s.wait(300);
  });

  s.stage("ground", "The ground gives way under a corner", () => {
    s.glide([12, 3.5, 12], [6, 1, 5], 150);
    s.wait(150);
    const columns: [number, number][] = [];
    for (let x = 11; x <= 14; x++) for (let z = 9; z <= 12; z++) columns.push([x, z]);
    s.call("build.subside", { columns, by_m: 1.5 });
    s.wait(300);
  });

  s.stage("gather", "The debris gathered, the cracked wall mended", () => {
    s.glide([8, 7, 16], [4.5, 0.5, 4], 150);
    s.wait(150);
    s.call("build.gather", {});
    s.wait(120);
    s.glide([15, 7.5, 0], [4.5, 1.2, 2], 70);
    s.glide([4.5, 1.9, -4], [4.5, 1.2, 2], 90, 50);
    s.wait(140);
    for (const at of [[5, 1, 4], [9, 1, 4], [5, 5, 4], [9, 5, 4]]) s.call("build.repair", { at, brush: 4 });
    s.wait(180);
  });
}

export default {
  about: "harm in a set: cracks, a wall's foot knocked out, a roof losing its walls, a tower brought down, the ground giving way, debris gathered and a wall mended",
  sandbox: ["--experiment", "play", "--free-build", "--time", "10", "--at", "0,3", "--face", "-8,14"],
  script,
} satisfies Scenario;
