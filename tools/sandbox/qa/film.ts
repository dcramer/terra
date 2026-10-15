// The building film: a cabin built in a set the way a player builds one, from claiming the
// ground to a storey, a deck and a corner knocked down and mended (tools/sandbox/qa.ts).
// Places are the base's cubes, half a metre each, east, up a storey and south of the claim
// stone for plans, and metres for the camera.

import type { KitSet, Scenario, Script, Step, Vec3 } from "./script.ts";

/// The cabin's corner tiles: five metres east to west and four north to south.
const [X0, X1, Z0, Z1] = [4, 13, 4, 11];
/// Its middle on the ground, in metres.
const C: Vec3 = [4.5, 0, 4];
const DOOR = [
  [8, 0, Z1],
  [9, 0, Z1],
];
const WINDOW = [
  [X1, 0, 7],
  [X1, 0, 8],
];
const CUT = [
  [8, 0, Z0],
  [9, 0, Z0],
];
const POSTS = [
  [17, 0, 5],
  [22, 0, 5],
  [17, 0, 10],
  [22, 0, 10],
];

/// The first frame of a step matching, or the end.
const first = (steps: Step[], test: (what: string) => boolean, or: number) => steps.find((step) => test(step.what))?.frame ?? or;
/// How far up a step's cubes are, in cubes, as its words give it ("(10 cubes, 3 up)").
const up = (what: string) => Number(/(-?\d+) up\)/.exec(what)?.[1] ?? 0);

function script(s: Script, set: KitSet) {
  // The cabin and its storey and roofs, the deck, and the garden wall: the camera keeps out.
  s.solid([2, -1, 2], [7, 8.5, 6]);
  s.solid([-0.5, -2, 6.5], [4.5, 0.7, 8.5]);
  const wall = set.wall(0);

  s.stage("claim", "Claiming the ground", () => {
    s.orbit([1.5, 0, 2.5], { radius: 7, height: 2.2, from: 280, to: 230, frames: 480, look: 0.7 });
    s.wait(120);
    // The claim stone set down; the base, until then at the start, half a metre above the
    // ground, stands at the stone from the next frame.
    s.call("storage.place", { item: "Claim stone", at: [0, 0, 0] });
    s.baseMovesAt(s.frame + 1, [0, -0.465, 0]);
    s.wait(360);
  });

  s.stage("plan", "Laying the plan: walls, floor, openings, blocks", () => {
    s.glide([-3.5, 5, 13], [4.5, 0.8, 4.5], 240);
    s.press("backspace");
    // Wall by wall, the blueprint showing each as it is laid; a mix lays each in its own kind.
    const walls: [number[], number[]][] = [
      [[X0, 0, Z1], [X1, 0, Z1]],
      [[X1, 0, Z0], [X1, 0, Z1]],
      [[X0, 0, Z0], [X1, 0, Z0]],
      [[X0, 0, Z0], [X0, 0, Z1]],
    ];
    walls.forEach(([min, max], i) => {
      s.call("build.lay", { what: "wall", min, max, kind: set.wall(i) });
      s.wait(45);
    });
    s.call("build.lay", { what: "floor", min: [X0 + 1, 0, Z0 + 1], max: [X1 - 1, 0, Z1 - 1], kind: "stone" });
    s.wait(45);
    s.call("build.opening", { tiles: DOOR, door: true });
    s.wait(40);
    s.call("build.opening", { tiles: WINDOW, door: false });
    s.wait(40);
    // A low garden wall west of the door in blocks (D226): in boxes of the metre brush, a
    // course of them from below the ground, which falls away a metre here, and a course
    // over it, then a post at each end a cube at a time.
    const garden = set.wall(4);
    s.glide([1.0, 2.0, 11.5], [1.8, -0.6, 6.8], 120);
    for (const y of [-4, -2]) {
      for (const x of [0, 2, 4, 6]) {
        s.call("build.lay", { what: "block", min: [x, y, 14], max: [x, y, 14], brush: 2, kind: garden });
        s.wait(15);
      }
    }
    for (const x of [0, 7]) {
      for (const z of [14, 15]) s.call("build.lay", { what: "block", min: [x, 0, z], max: [x, 0, z], brush: 1, kind: garden });
      s.wait(30);
    }
    s.wait(60);
  });

  // Built, what goes in next each time: the footing whole, the floor, the walls course by
  // course, the doorway's steps and the garden wall. Footings and floor quicker than walls.
  const start = s.frame;
  s.cut([10.5, 1.5, 9.5], [6.5, -0.3, 5.5]);
  s.glide([8.5, 1.1, 11.5], [4.5, -0.2, 6.5], 420);
  const steps = s.fill("build", { seconds: 36, gap: 14, weigh: (what) => (up(what) < 0 ? 0.3 : 1) });
  const walls = first(steps, (what) => up(what) >= 0 && what.includes(wall), s.frame);
  if (s.known) {
    s.mark("footing", "Footings first, then the floor", start, walls - 1);
    s.mark("walls", "Walls course by course", walls, s.frame - 1);
  }
  // The walls going up from round the cabin, then in close at a corner where they meet.
  const going = s.frame;
  s.frame = walls;
  s.orbit([C[0], 1.2, C[2]], { radius: 11, height: 3.5, from: 30, to: -60, frames: Math.max(60, Math.round((going - walls) * 0.6)) });
  s.wait(Math.round((going - walls) * 0.6));
  s.glide([9.6, 2.6, 8.2], [6.7, 1.2, 5.6], Math.max(60, going - s.frame));
  s.frame = going;
  s.wait(60);

  s.stage("openings", "A window cut, the door and shutters fitted, the door opened", () => {
    s.glide([11.5, 3, 1], [4.5, 1.2, 3], 70);
    s.glide([4.5, 2.2, -5.5], [4.5, 1.2, 2.5], 90, 50);
    s.call("build.opening", { tiles: CUT, door: false });
    s.wait(30);
    s.fill("cut", { seconds: 3, gap: 10 });
    s.wait(30);
    s.call("build.fit", { at: [CUT[0][0], 2, Z0], kind: "Louvred shutters" });
    s.wait(60);
    s.glide([11.5, 3, 1], [4.5, 1.2, 3], 70);
    s.glide([7.5, 2.0, 11.0], [4.5, 1.1, 5.0], 90, 50);
    s.wait(100);
    s.call("build.fit", { at: DOOR[0], kind: "Plank door" });
    s.wait(50);
    s.call("build.fit", { at: [X1, 2, WINDOW[0][2]], kind: "Board shutters" });
    s.wait(70);
    s.call("build.door", { at: DOOR[0], open: true });
    s.wait(120);
  });

  s.stage("roof", "Add a roof: frame, battens, covering, trims", () => {
    s.glide([12.5, 7.5, 12.5], [4.5, 2.2, 4], 150);
    s.call("build.lay", { what: "roof", over: [8, 0, 7], kind: wall, style: "Gable" });
    s.wait(60);
    const roof = s.fill("roof", { seconds: 16, gap: 18 });
    if (roof.length) s.orbit([C[0], 2.5, C[2]], { radius: 12, height: 5, from: 135, to: 215, frames: roof.at(-1)!.frame - roof[0].frame });
    s.wait(60);
  });

  s.stage("material", `One wall changed to ${set.other(0)} in place`, () => {
    s.glide([-6, 5, -1], [2.5, 1.5, 4], 60);
    s.glide([-5.5, 2.5, 8.5], [2.5, 1.2, 4], 70, 45);
    for (const z of [4, 6, 8, 10]) for (const y of [0, 2, 4]) s.call("build.replace", { at: [X0, y, z], brush: 2, kind: set.other(0) });
    s.wait(30);
    s.fill("material", { seconds: 8, gap: 10 });
    s.wait(45);
  });

  s.stage("carve", "A window carved out with the brush", () => {
    s.glide([-4.5, 2.0, 5.5], [2.0, 1.3, 3.8], 120);
    s.wait(60);
    s.call("build.carve", { at: [X0, 2, 6], brush: 2 });
    s.wait(30);
    s.fill("carve", { seconds: 3, gap: 10 });
    s.wait(60);
  });

  s.stage("storey", "The roof off, a storey on, a roof over it and a dormer", () => {
    s.glide([14, 9, 14], [4.5, 3, 4], 120);
    // As a player would: the roof taken apart, last in first out.
    s.call("build.take_apart", { at: [8, 6, 4], steps: 400 });
    s.wait(40);
    s.call("build.lay", { what: "room", min: [X0, 1, Z0], max: [X1, 1, Z1], kind: set.wall(1) });
    s.wait(40);
    s.fill("storey", { seconds: 10, gap: 12 });
    // The roof over the storey, and a dormer on its south slope: a gable of its own running
    // out to the eaves, lower than the ridge, its end a gable wall.
    s.call("build.lay", { what: "roof", over: [8, 1, 7], kind: set.wall(1), style: "Gable" });
    s.wait(20);
    s.call("build.lay", { what: "roof", min: [7, 1, 8], max: [10, 1, Z1], kind: set.wall(1), style: "Gable", along_x: false });
    s.wait(40);
    s.fill("upper roof", { seconds: 8, gap: 14 });
    s.orbit([C[0], 4, C[2]], { radius: 13, height: 5, from: 130, to: 200, frames: 300 });
    s.wait(120);
  });

  s.stage("deck", "A deck on posts, and stairs up to it", () => {
    s.solid([8.5, 0, 2.5], [11.5, 3.5, 5.5], s.frame);
    s.glide([17.5, 5, 16.5], [9.8, 1.5, 4.5], 120);
    for (const at of POSTS) {
      s.call("build.lay", { what: "post", min: at, max: at, kind: set.wall(2) });
      s.wait(15);
    }
    s.call("build.lay", { what: "floor", min: [17, 1, 5], max: [22, 1, 10], kind: set.wall(2) });
    s.wait(20);
    s.call("build.lay", { what: "flight", min: [18, -1, 17], max: [18, -1, 17], kind: set.wall(2), facing: "North", wide: 1.0 });
    s.wait(40);
    s.fill("deck", { seconds: 10, gap: 10 });
    s.wait(60);
  });

  s.stage("collapse", "A corner knocked down, built again and mended", () => {
    s.glide([-4.5, 3.6, 12.5], [2.6, 1.6, 5.2], 120);
    s.wait(30);
    // Cracked, then its foot knocked out: what hangs from nothing breaks into fragments.
    s.call("build.damage", { at: [X0, 0, Z1 - 3], to: [X0, 4, Z1 - 1], amount: 30 });
    s.wait(90);
    s.call("build.damage", { at: [X0, 0, Z1 - 1], to: [X0 + 1, 3, Z1] });
    s.wait(210);
    // Laid again where it fell, built, and the cracked wall mended.
    s.call("build.lay", { what: "wall", min: [X0, 0, Z1 - 1], max: [X0, 0, Z1], kind: set.other(0) });
    s.wait(20);
    s.call("build.lay", { what: "wall", min: [X0 + 1, 0, Z1], max: [X0 + 1, 0, Z1], kind: wall });
    s.wait(40);
    s.fill("mend", { seconds: 5, gap: 10 });
    s.call("build.repair", { at: [X0, 2, Z1 - 2], brush: 4 });
    s.wait(90);
    s.wait(60);
  });

  s.stage("finished", "Finished: round the cabin", () => {
    s.orbit([C[0] + 2, 2.5, C[2]], { radius: 15, height: 6, from: 250, to: -40, frames: 900, look: 0.5 });
    s.wait(900);
  });
}

export default { about: "a cabin built as a player builds one, in a set: claim, plan, footings, walls, openings, roof, a wall changed, a window carved, a storey, a deck, a corner knocked down and mended", sandbox: ["--experiment", "play", "--free-build", "--time", "10", "--at", "0,3", "--face", "-8,14"], script } satisfies Scenario;
