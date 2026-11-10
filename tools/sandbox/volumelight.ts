#!/usr/bin/env node
// The light of a patch of a still with a light volume on against off, as a ratio (D71), and
// the den's continuous check made from it (tools/continuous/checks.json, D485). One running
// game: each view is held once unshot (a run's first stills are of a world still arriving:
// they read 0.4 and 3.9 of the later ones), then held again and, with the clock stopped
// (`sandbox.step` leaves it so: no bough or blade moves between a view's stills), shot with
// the volume, with the named volume moved out of the world (`sandbox.place`, so what it held
// is lit by the sky's environment map, as everything outside a volume is), and with it put
// back. The ratio is the mean of on and back over off, and back over on says how steady the
// view was. Before a view is shot the camera is asked where it stands (`den.state`'s
// camera): a still from anywhere but the place asked is held again, not judged.
//
//   cargo xtask gpu still -- node tools/sandbox/volumelight.ts [--out target/check/volumelight]
//   node tools/sandbox/volumelight.ts --from <dir>
//   cargo xtask gpu still -- node tools/sandbox/volumelight.ts --volume "Den light" --view x,y,z,x,y,z \
//       [--patch x0,y0,x1,y1] [--out <dir>] [-- <sandbox options>]
//   node tools/sandbox/volumelight.ts --from <dir> --volume "Den light" [--patch x0,y0,x1,y1]
//
// With no --volume it is the check, on seed 3's den at 13:00. Three things well inside the
// den's box (`den.state`'s light_box) are each shot with a patch of ground outside the box
// in the same still, and judged as their ratio over that ground's: the ground outside is lit
// the same either way, so what it reads apart from 1 is the eye's own change between the
// stills (the exposure), which the division takes out. They are a trunk's bark, grass, and
// the bare ground by the den's mouth; the places are found from the world (`world.trunks`,
// `world.ground`), the eye and its sight line as far from every trunk as the land allows.
// Each must be lit as it is without the volume, within its bar (the volume once held the
// land's own shade of the sky there and a trunk read 0.8); and the chamber looking toward the
// mouth must stay dark and not blue (the sky must not leak in), and be lit by the volume at
// all (so the switch is known to have switched). --view is an eye and what it looks at,
// metres east, up and south of the start, and --patch fractions of the picture (all of it by
// default); a volume's name is `r find`'s; the world is the check's unless sandbox options
// follow `--`. A line a stage; exits 1 when the check fails. Node 22.18 or later; no
// dependencies.

import { existsSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { call, ROOT, shot, start, status, step, stop, waitUntil } from "./client.ts";
import { type Picture, readPng } from "./png.ts";

type Box = [number, number, number, number];
type Place = [number, number, number];
/// A thing in a view whose patch of the picture is read: a square of ground so far across,
/// or (`tall`) an upright strip so wide and so tall, facing the eye.
type Spot = { name: string; at: Place; wide: number; tall?: number };
type View = { name: string; vantage: number[]; spots?: Spot[]; note?: string; clear?: number };
type LightBox = { middle: Place; sides: Place[] };
type Den = { mouth: Place; floor: Place; ahead: Place; light_box: LightBox };
export type Camera = { at: Place; toward: Place; up: Place; fov: number | null };
type State = { den?: Den; camera?: Camera };
type Trunk = { at: [number, number]; radius: number; height: number };
/// What a run leaves beside its stills, for `--from`: each view's patches.
type Shot = Record<string, { note: string; spots: { name: string; box: Box }[] }>;

/// The den's check. The world, the player 20 m back from the den's mouth (`world.trunks`
/// knows the trees round the player, and the den is 500 m from the start), the volume's name
/// (`den_light`), the frames a view is held before its stills (the exposure eases), and the
/// frames drawn after the volume is moved.
const DEN = ["--experiment", "play", "--fresh", "--save", "shots/volumelight.ron", "--radius", "30000", "--seed", "3", "--at", "-216,-422", "--time", "13", "--mute", "--clean"];
const DEN_VOLUME = "Den light";
const HELD_FRAMES = 240;
const MOVED_FRAMES = 60;
/// Far outside any world: a volume there holds nothing, and its light is still worked out.
const AWAY_FROM_ALL = [1e7, 1e7, 1e7];
const WORDS = ["on", "off", "back"];
const WHOLE: Box = [0, 0, 1, 1];
/// The check's places. What is judged is at least IN metres inside the den's box (a cell of
/// its light is 3 m, `den_light`'s CELL_M) and the ground it is judged over at least OUT
/// outside it; the eye stands BEHIND that ground, UP over it, looking at the middle of the
/// two; grass and the ground outside are AWAY or more from the den's open mouth (22 m of its
/// passage, `den.rs`'s MOUTH_M, cleared 5.5 m either side); the bare ground is BARE behind
/// the mouth, inside what is cleared.
const [IN, OUT, BEHIND, UP, AWAY, BARE, MOUTH_M] = [3, 2.5, 4, 3, 9, 4.2, 22];
/// A square of ground is seen from RISE metres over it for each metre off, the eye no more
/// than HIGH over its own ground (under the crowns), and every patch is at least TALL of the
/// picture's height.
const [RISE, HIGH, TALL] = [0.5, 9, 0.03];
/// How far each judged thing may stand from itself without the volume, over the ground
/// outside the box, and its still with the volume put back from its first (a view less
/// steady than that has measured nothing). Five runs on 2026-11-10 (light-fragment j13) read
/// the trunk 0.999 to 1.000, the grass 1.000 to 1.002 and the floor 0.995 to 1.000, back
/// over on 1.000 to 1.002, 0.995 to 1.001 and 1.003 to 1.014; with the fault planted
/// (den_light's `whose` giving open air the den's light) 0.844, 1.018 and 1.013. So the
/// trunk and the grass are judged (D632); the floor's fault is under three of its own
/// spreads from its good runs, and it is read only.
const SAME: Record<string, number> = { trunk: 0.02, grass: 0.01 };
const STEADY = 0.02;
/// The chamber toward the mouth with the volume: its luma and blue share at most these, and
/// without the volume at least LIT times as bright.
const DARK = 0.02;
const BLUE = 0.25;
const LIT = 2;

const linear = (v: number) => (v <= 0.04045 * 255 ? v / 255 / 12.92 : ((v / 255 + 0.055) / 1.055) ** 2.4);
const sub = (a: Place, b: Place): Place => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const add = (a: Place, b: Place, k = 1): Place => [a[0] + b[0] * k, a[1] + b[1] * k, a[2] + b[2] * k];
const dot = (a: Place, b: Place) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: Place, b: Place): Place => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const unit = (a: Place): Place => add([0, 0, 0], a, 1 / (Math.hypot(...a) || 1));

/// How far inside a box a point is, in metres; under nought, that far outside its nearest
/// side.
export function inside(box: LightBox, point: Place): number {
  return Math.min(
    ...box.sides.map((side) => {
      const long = Math.hypot(...side);
      const along = dot(sub(point, box.middle), side) / (long * long) + 0.5;
      return Math.min(along, 1 - along) * long;
    }),
  );
}

/// Where a point is in a camera's picture, as fractions from its left and top; nothing when
/// it is behind the eye. `aspect` is the picture's width over its height.
export function pictured(camera: Camera, aspect: number, point: Place): [number, number] | undefined {
  const [toward, up] = [unit(camera.toward), unit(camera.up)];
  const from = sub(point, camera.at);
  const ahead = dot(from, toward);
  if (ahead < 0.1) return undefined;
  const half = Math.tan((camera.fov ?? Math.PI / 4) / 2) * ahead;
  return [0.5 + dot(from, cross(toward, up)) / (half * aspect) / 2, 0.5 - dot(from, up) / half / 2];
}

/// The patch of a picture a spot fills: the box round its corners, nothing when any is out
/// of the picture.
export function patchOf(camera: Camera, aspect: number, spot: Spot): Box | undefined {
  const side = unit(cross(camera.toward, camera.up));
  const half = spot.wide / 2;
  const corners: Place[] = spot.tall
    ? [-1, 1].flatMap((a) => [-1, 1].map((b) => add(add(spot.at, side, a * half), [0, 1, 0], (b * spot.tall!) / 2)))
    : [-1, 1].flatMap((a) => [-1, 1].map((b) => add(spot.at, [a * half, 0, b * half])));
  const seen = corners.map((corner) => pictured(camera, aspect, corner));
  if (seen.some((at) => !at || at[0] < 0.01 || at[0] > 0.99 || at[1] < 0.01 || at[1] > 0.99)) return undefined;
  const [xs, ys] = [seen.map((at) => at![0]), seen.map((at) => at![1])];
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
}

/// Whether a camera stands where a view asked and looks the way it asked.
export function asked(view: View, camera: Camera): boolean {
  const [eye, at] = [view.vantage.slice(0, 3) as Place, view.vantage.slice(3) as Place];
  return Math.hypot(...sub(camera.at, eye)) < 0.1 && dot(unit(camera.toward), unit(sub(at, eye))) > 0.9995;
}

/// A patch's mean linear luma and the blue's share of its light.
export function patch(p: Picture, [x0, y0, x1, y1]: Box): { luma: number; blue: number } {
  let [r, g, b, n] = [0, 0, 0, 0];
  for (let y = Math.floor(y0 * p.height); y < Math.floor(y1 * p.height); y++)
    for (let x = Math.floor(x0 * p.width); x < Math.floor(x1 * p.width); x++) {
      const i = (y * p.width + x) * 4;
      r += linear(p.rgba[i]);
      g += linear(p.rgba[i + 1]);
      b += linear(p.rgba[i + 2]);
      n++;
    }
  return { luma: n ? (0.2126 * r + 0.7152 * g + 0.0722 * b) / n : 0, blue: r + g + b > 0 ? b / (r + g + b) : 0 };
}

export type Reading = { on: number; off: number; ratio: number; steady: number; blue: number };

/// A patch of a view's three stills: its luma with the volume (the mean of on and back) and
/// without, the one over the other, back over on, and the blue share with the volume.
export function reading(out: string, view: string, box: Box): Reading {
  const [on, off, back] = WORDS.map((word) => patch(readPng(join(out, `${view}-${word}.png`)), box));
  const lit = (on.luma + back.luma) / 2;
  return { on: lit, off: off.luma, ratio: lit / Math.max(off.luma, 1e-6), steady: back.luma / Math.max(on.luma, 1e-6), blue: back.blue };
}

/// The same with the ratio and the steadiness the medians of each pixel's own, not the ratio
/// of the patch's means: a pixel whose light is not the volume's to change (a sunfleck, a
/// highlight) is left out by a median, and moves a mean by its whole light.
export function shaded(out: string, view: string, [x0, y0, x1, y1]: Box): Reading {
  const [on, off, back] = WORDS.map((word) => readPng(join(out, `${view}-${word}.png`)));
  const luma = (p: Picture, i: number) => 0.2126 * linear(p.rgba[i]) + 0.7152 * linear(p.rgba[i + 1]) + 0.0722 * linear(p.rgba[i + 2]);
  const median = (all: number[]) => all.sort((a, b) => a - b)[all.length >> 1] ?? 0;
  const [ratios, steadies]: number[][] = [[], []];
  for (let y = Math.floor(y0 * on.height); y < Math.floor(y1 * on.height); y++)
    for (let x = Math.floor(x0 * on.width); x < Math.floor(x1 * on.width); x++) {
      const i = (y * on.width + x) * 4;
      const [a, f, c] = [luma(on, i), luma(off, i), luma(back, i)];
      // A black pixel has no ratio.
      if (a < 1e-4 || f < 1e-4) continue;
      ratios.push((a + c) / 2 / f);
      steadies.push(c / a);
    }
  return { ...reading(out, view, [x0, y0, x1, y1]), ratio: median(ratios), steady: median(steadies) };
}

/// A judged thing's reading over the ground outside the box in the same stills: the ratio
/// and the steadiness each over the ground's, the rest the thing's own.
export function over(thing: Reading, ground: Reading): Reading {
  return { ...thing, ratio: thing.ratio / Math.max(ground.ratio, 1e-6), steady: thing.steady / Math.max(ground.steady, 1e-6) };
}

/// The check's verdict from its readings: what fails, in words. `things` are the judged
/// things by name, each already over the ground outside the box.
export function judged(things: Record<string, Reading>, chamber: Reading): string[] {
  const failed: string[] = [];
  for (const [name, thing] of Object.entries(things)) {
    if (Math.abs(thing.steady - 1) > STEADY) failed.push(`the ${name} with the volume put back is ${thing.steady.toFixed(3)} of its first still (within ${STEADY} of 1): the view was not steady and measured nothing, run it again`);
    if (Math.abs(thing.ratio - 1) > SAME[name]) failed.push(`the ${name} in the den's box is ${thing.ratio.toFixed(3)} of itself without the volume (within ${SAME[name]} of 1)`);
  }
  if (chamber.on > DARK) failed.push(`the chamber's luma is ${chamber.on.toFixed(4)} (at most ${DARK})`);
  if (chamber.blue > BLUE) failed.push(`the chamber's blue share is ${chamber.blue.toFixed(3)} (at most ${BLUE})`);
  if (1 / chamber.ratio < LIT) failed.push(`the chamber without the volume is ${(1 / chamber.ratio).toFixed(2)} times as bright (at least ${LIT}): the volume was not switched off, or lights nothing`);
  return failed;
}

const stills = (out: string, views: string[]) => views.flatMap((view) => WORDS.map((word) => join(out, `${view}-${word}.png`)));
const camera = async () => ((await call("den.state").catch(() => ({}))) as State).camera;
const drawn = async (frames: number) => {
  const from = (await status()).frame;
  await waitUntil(`${frames} frames`, status, (now) => now.frame >= from + frames);
};

/// Holds a view until the exposure has eased, and says where the camera then stands. A
/// camera anywhere else is said and the view held again: a still has been shot from another
/// direction than its view's (light-fragment j8, j10).
async function hold(view: View): Promise<Camera | undefined> {
  for (let tries = 0; ; tries++) {
    await call("sandbox.look", { vantage: view.vantage });
    await step(HELD_FRAMES);
    const now = await camera();
    if (!now || asked(view, now)) return now;
    const told = `the ${view.name} view's camera stands at ${now.at.map((v) => v.toFixed(1))} looking ${now.toward.map((v) => v.toFixed(2))}, asked ${view.vantage.map((v) => v.toFixed(1))}`;
    if (tries === 2) throw new Error(`${told}, three times: something else holds the camera (see \`hold_camera\` in crates/terra_world/src/remote.rs)`);
    console.log(`volumelight: ${told}; held again`);
  }
}

/// Shoots each view with the volume, without it and with it back, in one game; leaves each
/// view's patches in `views.json` beside the stills.
async function shoot(volume: string, sandboxArgs: string[], views: (state: State) => Promise<View[]>, out: string) {
  try {
    await start([...sandboxArgs, "--offscreen"], { release: true });
    await waitUntil("the world to be played", async () => (await call("title.state").catch(() => ({ screen: "Playing" }))) as { screen: string }, (now) => now.screen === "Playing", 600);
    // The land's trees are still arriving when the world is first played.
    await step(HELD_FRAMES);
    const all = await views((await call("den.state").catch(() => ({}))) as State);
    const { translation: home } = (await call("sandbox.place", { name: volume })) as { translation: Place };
    for (const view of all) await hold(view);
    // The first frames of a view that has just lost its volume draw the grass and the
    // needles black (trunk-off.png, 2026-11-10: the picture's shaders are made again for a
    // view without one): lose it once unshot, the clock running.
    await call("sandbox.place", { name: volume, translation: AWAY_FROM_ALL });
    await step(HELD_FRAMES);
    await call("sandbox.place", { name: volume, translation: home });
    await step(HELD_FRAMES);
    const patches: Shot = {};
    for (const view of all) {
      const eye = await hold(view);
      for (const word of WORDS) {
        if (word === "off") await call("sandbox.place", { name: volume, translation: AWAY_FROM_ALL });
        if (word === "back") await call("sandbox.place", { name: volume, translation: home });
        if (word !== "on") await drawn(MOVED_FRAMES);
        const file = await shot(join(out, `${view.name}-${word}.png`));
        if (word !== "on" || !view.spots) continue;
        if (!eye) throw new Error("this game does not say where its camera is (den.state's camera): build it from this checkout");
        const { width, height } = readPng(file);
        const note = view.note ?? "";
        const spots = view.spots.map((spot) => {
          const box = patchOf(eye, width / height, spot);
          if (!box) throw new Error(`the ${view.name} view's ${spot.name} is not in its picture (${view.note}): change how the view is found (\`found\`), not the bars`);
          return { name: spot.name, box };
        });
        patches[view.name] = { note, spots };
      }
      const after = await camera();
      if (after && !asked(view, after)) throw new Error(`the ${view.name} view's camera moved while its stills were shot`);
    }
    writeFileSync(join(out, "views.json"), JSON.stringify(patches));
  } catch (error) {
    // The game's own refusal or the client's error says what to do; no stack with it.
    await stop().catch(() => undefined);
    console.log(`volumelight: ✗ the run failed: ${(error as Error).message}`);
    process.exit(1);
  }
  await stop().catch(() => undefined);
}

/// A view of a thing in the den's box and a spot of ground outside it: the best of the
/// spots round the thing, 16 ways and from `near` to `far` metres off, that lie OUT outside
/// the box and AWAY from the mouth, by how far every trunk is from that ground, from the eye
/// and from the line between the eye and the thing.
function found(name: string, thing: Spot, [near, far]: number[], land: { box: LightBox; up: (east: number, south: number) => number | undefined; trunks: Trunk[]; mouthward: (at: Place) => number; fov: number | null }): View {
  const flat = (a: Place, b: Place) => Math.hypot(a[0] - b[0], a[2] - b[2]);
  const others = land.trunks.filter((trunk) => Math.hypot(trunk.at[0] - thing.at[0], trunk.at[1] - thing.at[2]) > 0.5);
  // How far the trunks are from a line between two places, less each one's own radius.
  const clear = (a: Place, b: Place) =>
    Math.min(
      99,
      ...others.map((trunk) => {
        const at: Place = [trunk.at[0], 0, trunk.at[1]];
        const [line, to] = [sub(b, a), sub(at, a)];
        const along = Math.max(0, Math.min(1, (line[0] * to[0] + line[2] * to[2]) / Math.max(line[0] ** 2 + line[2] ** 2, 1e-6)));
        return flat(at, add(a, line, along)) - trunk.radius;
      }),
    );
  let best: { score: number; view: View } | undefined;
  const refused = { inside: 0, high: 0, small: 0 };
  for (let turn = 0; turn < 16; turn++)
    for (let off = near; off <= far; off++) {
      const way: Place = [Math.cos((turn * Math.PI) / 8), 0, Math.sin((turn * Math.PI) / 8)];
      const [ground, eye] = [add(thing.at, way, off), add(thing.at, way, off + BEHIND)];
      const [groundUp, eyeUp] = [land.up(ground[0], ground[2]), land.up(eye[0], eye[2])];
      if (groundUp === undefined || eyeUp === undefined) continue;
      // A square of ground is looked down on (RISE), however the land between falls: from
      // UP over the eye's own ground alone, a hillside's was seen edge on, 4 pixels high.
      [ground[1], eye[1]] = [groundUp, Math.max(eyeUp, groundUp) + UP];
      if (!thing.tall) eye[1] = Math.max(eye[1], thing.at[1] + RISE * (off + BEHIND));
      if (eye[1] - eyeUp > HIGH && ++refused.high) continue;
      if ((inside(land.box, ground) > -OUT || inside(land.box, add(ground, [0, 1, 0])) > -OUT || land.mouthward(ground) < AWAY) && ++refused.inside) continue;
      const score = Math.min(clear(ground, ground), clear(eye, eye), clear(eye, thing.at));
      if (best && best.score >= score) continue;
      // The eye looks midway between the two by angle: at the middle of the line between
      // them, the ground 4 m off lay on the picture's lower edge or under it.
      const middle = add(eye, unit(add(unit(sub(thing.at, eye)), unit(sub(ground, eye)))), 5);
      // Both patches as the game's camera will picture them (`sandbox.look` keeps the eye
      // level), each at least TALL of the picture's height.
      const toward = unit(sub(middle, eye));
      const planned: Camera = { at: eye, toward, up: cross(unit(cross(toward, [0, 1, 0])), toward), fov: land.fov };
      const tall = [thing, { name: "ground outside", at: ground, wide: 1 }].map((spot) => patchOf(planned, 16 / 9, spot)).map((box) => (box ? box[3] - box[1] : 0));
      if (Math.min(...tall) < TALL && ++refused.small) continue;
      const note = `${inside(land.box, thing.at).toFixed(1)} m inside the box, its ground ${(-inside(land.box, ground)).toFixed(1)} m outside, ${score.toFixed(1)} m clear of trunks`;
      best = { score, view: { name, clear: score, vantage: [...eye, ...middle], spots: [thing, { name: "ground outside", at: ground, wide: 1 }], note } };
    }
  if (!best) throw new Error(`no ground outside the den's box to judge the ${name} over within ${far} m of it (of the spots tried, ${refused.inside} in the box or by the mouth, ${refused.high} with the eye over ${HIGH} m up, ${refused.small} with a patch under ${TALL} of the picture's height or out of it)`);
  return best.view;
}

/// The den's views: a trunk, grass and the bare ground by the mouth, each inside the box with
/// ground outside it; and from the chamber's floor toward the mouth.
async function denViews({ den, camera }: State): Promise<View[]> {
  if (!den?.light_box) throw new Error("this world has no den, or its game does not say its light's box (den.state): build it from this checkout");
  const { mouth, floor, ahead, light_box: box } = den;
  const flatly = (from: Place, to: Place) => unit([to[0] - from[0], 0, to[2] - from[2]]);
  const [inward, outward] = [flatly(mouth, ahead), flatly(floor, mouth)];
  // The land round the box, a spot every 1.5 m, and each trunk's foot.
  const reach = Math.hypot(...box.sides.map((side) => Math.hypot(...side))) / 2 + 24;
  const [apart, across] = [1.5, Math.ceil((2 * reach) / 1.5)];
  const node = (k: number) => (k - across / 2) * apart;
  const spots = Array.from({ length: across * across }, (_, k) => [box.middle[0] + node(k % across), box.middle[2] + node(Math.floor(k / across))]);
  const trunks = (await call("world.trunks", { at: [box.middle[0], box.middle[2]], within: reach })) as Trunk[];
  if (!trunks.length) throw new Error(`no tree is known within ${reach.toFixed(0)} m of the den's box (world.trunks knows those round the player): the den's mouth is ${mouth.map((v) => v.toFixed(0))} from the player, so put DEN's --at 20 m back from it`);
  const ups = ((await call("world.ground", { at: [...spots, ...trunks.map((trunk) => trunk.at)] })) as { up: number }[]).map((spot) => spot.up);
  const up = (east: number, south: number) => {
    const [x, z] = [Math.round((east - box.middle[0]) / apart + across / 2), Math.round((south - box.middle[2]) / apart + across / 2)];
    return x < 0 || z < 0 || x >= across || z >= across ? undefined : ups[z * across + x];
  };
  // How far a place is, flatly, from the open stretch of the den's passage.
  const mouthward = (at: Place) => {
    const along = Math.max(0, Math.min(MOUTH_M, dot(sub(at, mouth), inward)));
    const nearest = add(mouth, inward, along);
    return Math.hypot(at[0] - nearest[0], at[2] - nearest[2]);
  };
  const land = { box, up, trunks, mouthward, fov: camera?.fov ?? null };
  let why = "none stands there";
  const best = (views: View[], what: string) => {
    if (!views.length) throw new Error(`no ${what} at least ${IN} m inside the den's box with ground outside it near: ${why}`);
    return views.reduce((a, b) => (b.clear! > a.clear! ? b : a));
  };
  const tried = (make: () => View) => {
    try {
      return [make()];
    } catch (error) {
      why = `the last tried, ${(error as Error).message}`;
      return [];
    }
  };
  // A trunk's bark from 0.8 to 1.6 m up, the middle half of its width.
  const barks = trunks
    .map((trunk, k): Spot => ({ name: "trunk", at: [trunk.at[0], ups[spots.length + k] + 1.2, trunk.at[1]], wide: trunk.radius, tall: 0.8 }))
    .filter((bark) => bark.wide >= 0.15 && inside(box, bark.at) >= IN && inside(box, add(bark.at, [0, -1.2, 0])) >= IN);
  const trunk = best(barks.flatMap((bark) => tried(() => found("trunk", bark, [6, 14], land))), "trunk");
  // Grass: the land's own spots a little inside the box, away from the mouth.
  const lawns = spots
    .map(([east, south], k): Spot => ({ name: "grass", at: [east, ups[k], south], wide: 1 }))
    .filter((lawn) => inside(box, lawn.at) >= IN && inside(box, lawn.at) < IN + 2 && inside(box, add(lawn.at, [0, 1, 0])) >= IN && mouthward(lawn.at) >= AWAY);
  const grass = best(lawns.flatMap((lawn) => tried(() => found("grass", lawn, [6, 10], land))), "grass");
  const bare = add(mouth, inward, -BARE);
  bare[1] = up(bare[0], bare[2]) ?? mouth[1];
  const ground = found("floor", { name: "floor", at: bare, wide: 1 }, [6, 24], land);
  return [trunk, grass, ground, { name: "chamber", vantage: [floor[0], floor[1] + 1.6, floor[2], floor[0] + outward[0] * 4, floor[1] + 0.4, floor[2] + outward[2] * 4] }];
}

if (import.meta.main) {
  const { values, positionals } = parseArgs({
    options: { out: { type: "string" }, from: { type: "string" }, volume: { type: "string" }, view: { type: "string" }, patch: { type: "string" } },
    allowPositionals: true,
  });
  const out = resolve(values.from ?? values.out ?? join(ROOT, "target/check/volumelight"));
  const things = ["trunk", "grass", "floor"];
  const views = values.volume ? ["view"] : [...things, "chamber"];
  if (!values.from) {
    if (values.volume && !values.view) {
      console.log("volumelight: ✗ --volume needs --view x,y,z,x,y,z, an eye and what it looks at from the start (the script's header), or --from a folder already shot");
      process.exit(1);
    }
    for (const still of [...stills(out, views), join(out, "views.json")]) rmSync(still, { force: true });
    if (values.volume) await shoot(values.volume, positionals.length ? positionals : DEN, async () => [{ name: "view", vantage: values.view!.split(",").map(Number) }], out);
    else await shoot(DEN_VOLUME, DEN, denViews, out);
  }
  const missing = [...stills(out, views), join(out, "views.json")].find((s) => !existsSync(s));
  if (missing) {
    console.log(`volumelight: ✗ no ${missing}; the run's log is shots/remote.log`);
    process.exit(1);
  }
  const told = (r: Reading) => `${r.ratio.toFixed(3)} (luma ${r.on.toFixed(4)} over ${r.off.toFixed(4)}, back over on ${r.steady.toFixed(3)}, blue share ${r.blue.toFixed(3)})`;
  if (values.volume) {
    const box = (values.patch ? values.patch.split(",").map(Number) : WHOLE) as Box;
    console.log(`volumelight: "${values.volume}" on against off, patch ${box.join(",")}: ${told(reading(out, "view", box))}; stills in ${out}`);
    process.exit(0);
  }
  const patches = JSON.parse(readFileSync(join(out, "views.json"), "utf8")) as Shot;
  const read: Record<string, Reading> = {};
  const said: string[] = [];
  for (const thing of things) {
    const [own, ground] = patches[thing].spots.map(({ box }) => shaded(out, thing, box));
    read[thing] = over(own, ground);
    said.push(`the ${thing} ${read[thing].ratio.toFixed(3)} (back over on ${read[thing].steady.toFixed(3)}; its own ${own.ratio.toFixed(3)}, the ground's ${ground.ratio.toFixed(3)}; ${patches[thing].note})`);
  }
  const chamber = reading(out, "chamber", WHOLE);
  // The trunk and the grass are judged, the floor read only (SAME's note, D632).
  const failed = judged(Object.fromEntries(Object.keys(SAME).map((name) => [name, read[name]])), chamber);
  const fix = failed.length ? `: ${failed.join("; ")}. The den's volume holds the wrong light in those cells: see den_light.rs's \`Whose\` (open air and the rock under it hold the open sky's, D485) and \`thrown_on\`, and do not widen the bars` : "";
  console.log(`volumelight: ${failed.length ? "✗" : "✓"} the den's volume on against off, each thing in its box over ground outside it in the same stills, the trunk within ${SAME.trunk} of 1, the grass within ${SAME.grass} and the floor read only: ${said.join(", ")}; the chamber toward the mouth luma ${chamber.on.toFixed(4)} (at most ${DARK}), blue share ${chamber.blue.toFixed(3)} (at most ${BLUE}), ${(1 / chamber.ratio).toFixed(2)} times as bright without it (at least ${LIT})${fix}; stills in ${out}`);
  process.exit(failed.length ? 1 : 0);
}
