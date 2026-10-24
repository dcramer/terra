// Each duel check passes a made-up fight without its fault and fails one with it planted.
// `node --test tools/sandbox/duel/checks.test.ts`.

import assert from "node:assert/strict";
import { test } from "node:test";
import { type Contact, type Fighter, type Frame, type Moves, chainGaps, framed, hitStop, latency, misses, overlap, same, share, sliding } from "./checks.ts";
import { moments } from "./moments.ts";

const MOVES: Moves = { buffer_s: 0.35, hit_stop: new Map([["slash", 0.1], ["backhand", 0.1], ["partner_slash", 0.1]]) };

/// A fighter standing still at `x` east, its balls of the feet on the ground beside it.
function body(who: string, x: number): Fighter {
  return {
    who,
    feet: [x, 0, 0],
    facing: [who === "you" ? -1 : 1, 0, 0],
    move: null,
    t: 0,
    phase: null,
    cancels: false,
    reach: false,
    guard: false,
    stop_s: 0,
    stagger_s: 0,
    down: false,
    free: true,
    radius: who === "you" ? null : 0.35,
    balls: [
      { at: [x, 0.04, -0.15], up: 0.04 },
      { at: [x, 0.04, 0.15], up: 0.04 },
    ],
  };
}

/// So many frames of the two standing 1.3 m apart.
const fight = (frames = 60): Frame[] =>
  Array.from({ length: frames }, (_, frame) => ({ frame, step: null, asked: [], contacts: [], fighters: [body("you", 0), body("partner", -1.3)] }));

/// `who` in `move` over frames from-to, its own time going on, its phases as given by frame.
function play(frames: Frame[], who: string, move: string, from: number, to: number, phase: (n: number) => string = () => "WindUp") {
  for (let n = from; n <= to; n++) {
    const f = frames[n].fighters.find((each) => each.who === who)!;
    Object.assign(f, { move, t: (n - from) / 60, phase: phase(n) });
  }
}

const contact = (by: string, move: string, stop_s = 0.1, taken = "Hit"): Contact => ({ by, on: by === "you" ? "the partner" : "you", move, taken, stop_s });

test("a move asked starts that frame or the next; three frames late is a failure", () => {
  const prompt = fight();
  prompt[10].asked.push({ move: "slash", got: "started" });
  play(prompt, "you", "slash", 10, 40);
  assert.ok(latency(prompt, undefined, MOVES).ok);
  const late = fight();
  late[10].asked.push({ move: "slash", got: "started" });
  play(late, "you", "slash", 13, 40);
  const said = latency(late, undefined, MOVES);
  assert.ok(!said.ok);
  assert.match(said.line, /3 frames after/);
});

test("a chained press starts the frame its window opens; waiting on past it is late", () => {
  const chained = fight();
  play(chained, "you", "slash", 5, 29);
  chained[20].asked.push({ move: "backhand", got: "waits" });
  play(chained, "you", "backhand", 30, 50);
  assert.ok(latency(chained, undefined, MOVES).ok);
  // The window opened on frame 25 and the press waited on to 30.
  const waited = fight();
  play(waited, "you", "slash", 5, 29);
  for (let n = 25; n < 30; n++) waited[n].fighters[0].cancels = true;
  waited[20].asked.push({ move: "backhand", got: "waits" });
  play(waited, "you", "backhand", 30, 50);
  assert.ok(!latency(waited, undefined, MOVES).ok);
  // A press that never starts within the buffer.
  const dropped = fight(120);
  dropped[20].asked.push({ move: "backhand", got: "waits" });
  play(dropped, "you", "backhand", 100, 110);
  assert.match(latency(dropped, undefined, MOVES).line, /never started/);
});

test("a chain with idle frames between its blows is found", () => {
  const smooth = fight();
  play(smooth, "you", "slash", 5, 29);
  smooth[20].asked.push({ move: "backhand", got: "waits" });
  play(smooth, "you", "backhand", 30, 50);
  assert.ok(chainGaps(smooth).ok);
  const gap = fight();
  play(gap, "you", "slash", 5, 26);
  gap[20].asked.push({ move: "backhand", got: "waits" });
  play(gap, "you", "backhand", 30, 50);
  const said = chainGaps(gap);
  assert.ok(!said.ok);
  assert.match(said.line, /3 idle frames/);
});

test("a planted foot that stays put passes; one sliding 5 cm fails; a roll is left out", () => {
  assert.ok(sliding(fight()).ok);
  const slid = fight();
  for (let n = 20; n < 30; n++) slid[n].fighters[0].balls[0].at[0] = 0.005 * (n - 20);
  for (let n = 30; n < 60; n++) slid[n].fighters[0].balls[0].at[0] = 0.05;
  const said = sliding(slid);
  assert.ok(!said.ok);
  assert.match(said.line, /the player's left foot/);
  const rolled = fight();
  play(rolled, "you", "roll", 20, 59, () => "Recovery");
  for (let n = 20; n < 60; n++) rolled[n].fighters[0].balls[0].at[0] = 0.05 * (n - 20);
  assert.ok(sliding(rolled).ok);
});

test("bodies closer than their pushboxes allow are found", () => {
  assert.ok(overlap(fight()).ok);
  const pressed = fight();
  pressed[30].fighters[1].feet = [-0.6, 0, 0];
  const said = overlap(pressed);
  assert.ok(!said.ok);
  assert.match(said.line, /10\.0 cm at frame 30/);
});

test("a blow in reach in its active frames must land, unless rolled through", () => {
  const active = (n: number) => (n >= 20 && n <= 25 ? "Active" : n < 20 ? "WindUp" : "Recovery");
  const reaching = () => {
    const frames = fight();
    play(frames, "partner", "partner_slash", 5, 40, active);
    for (let n = 20; n <= 25; n++) frames[n].fighters[1].reach = true;
    return frames;
  };
  const landed = reaching();
  landed[22].contacts.push(contact("the partner", "partner_slash"));
  assert.ok(misses(landed).ok);
  const missed = reaching();
  assert.ok(!misses(missed).ok);
  const rolled = reaching();
  play(rolled, "you", "roll", 15, 30, () => "Recovery");
  assert.ok(misses(rolled).ok);
  // A parry ends the blow: its contact is on the frame after the blow's last.
  const parried = fight();
  play(parried, "partner", "partner_slash", 5, 22, active);
  for (let n = 20; n <= 22; n++) parried[n].fighters[1].reach = true;
  parried[23].contacts.push(contact("the partner", "partner_slash", 0.12, "Parried"));
  assert.ok(misses(parried).ok);
});

test("a hit holds the striker for its hit-stop; too long, or not the set's, fails", () => {
  const held = (frames: number, stop_s = 0.1) => {
    const out = fight();
    out[20].contacts.push(contact("you", "slash", stop_s));
    for (let n = 20; n < 20 + frames; n++) out[n].fighters[0].stop_s = stop_s;
    return out;
  };
  assert.ok(hitStop(held(6), MOVES).ok);
  assert.ok(hitStop(held(7), MOVES).ok);
  const long = hitStop(held(10), MOVES);
  assert.ok(!long.ok);
  assert.match(long.line, /10 frames for 6\.0/);
  const wrong = hitStop(held(3, 0.05), MOVES);
  assert.ok(!wrong.ok);
  assert.match(wrong.line, /the set's hit_stop 0\.1/);
});

test("records of one fight agree; a contact a frame later is another fight", () => {
  const one = fight();
  play(one, "you", "slash", 10, 40);
  one[25].contacts.push(contact("you", "slash"));
  const two = structuredClone(one);
  assert.ok(same([{ name: "side", frames: one }, { name: "above", frames: two }]).ok);
  const later = structuredClone(one);
  later[25].contacts = [];
  later[26].contacts.push(contact("you", "slash"));
  const said = same([{ name: "side", frames: one }, { name: "above", frames: later }]);
  assert.ok(!said.ok);
  assert.match(said.line, /above differs from side/);
});

test("the contact sheet shows a blow's start, its landing and its recovery", () => {
  const frames = fight(80);
  frames[10].step = "light 1";
  frames[10].asked.push({ move: "slash", got: "started" });
  play(frames, "you", "slash", 10, 60, (n) => (n < 30 ? "WindUp" : n <= 36 ? "Active" : "Recovery"));
  frames[33].contacts.push(contact("you", "slash"));
  frames[70].step = "end";
  assert.deepEqual(moments(frames), [
    { step: "light 1", subject: "slash", frames: [10, 33, 43] },
    { step: "end", subject: "", frames: [70, 130, 190] },
  ]);
});

test("a fight away from where the cameras look is found", () => {
  assert.ok(framed(fight(), [-0.65, 1, 0], 1.5).ok);
  assert.ok(!framed(fight(), [5, 1, 5], 1.5).ok);
});

test("the player's camera framing: the share of the frame's height, centred, the partner in view", () => {
  // Behind the player (x = 0), looking west along the line to the partner (x = -1.3), at
  // 55 degrees: the body's 1.8 m fills 1.8 / (2 d tan 27.5) of the frame's height.
  const filmed = (back: number, aside = 0, looking = [-1, 0, 0]) =>
    fight().map((frame) => ({ ...frame, camera: { at: [back, 0.9, aside], looking, up: [0, 1, 0], vfov: 55, aspect: 16 / 9 } }));
  const at = (share: number) => 1.8 / (2 * share * Math.tan((27.5 * Math.PI) / 180));
  const held = share(filmed(at(0.235)), [0.2, 0.27]);
  assert.ok(held.ok, held.line);
  assert.match(held.line, /0\.235 of the frame's height/);
  assert.ok(!share(filmed(at(0.38)), [0.2, 0.27]).ok, "too near");
  assert.ok(!share(filmed(4.6, 0.6), [0.2, 0.27]).ok, "over the shoulder, too near");
  assert.ok(!share(filmed(at(0.235), 4.0), [0.2, 0.27]).ok, "off to the side");
  const away = share(filmed(at(0.235), 0, [-0.6, 0, -0.8]), [0.2, 0.27]);
  assert.ok(!away.ok, away.line);
  assert.ok(!share(fight(), [0.2, 0.27]).ok, "no camera recorded");
});
