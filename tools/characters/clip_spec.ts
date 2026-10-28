// A move's clip spec (D450) and what the clip step (clip.ts) and the move check (moves.ts)
// share about it: reading the spec, judging a blow's sweep and contact against it, choosing a
// take, and writing a clip's phases into a move set. No I/O here; tested in clip_spec.test.ts.
//
// A spec is an entry of a clip set's list (art/animation/sets/*.json, beside the Kimodo
// recipe motion.ts and made_clips.py read: prompt, seconds, seed, samples, once, fist) with:
//
//   guard  {"clip": the set's guard clip, "frame": the frame of its take that is the pose}:
//          keyed as the whole body at the clip's first and last frames
//   hand   right-hand keys, each {"at": seconds, "grip": [right, ahead, up], "blade": [...],
//          "edge": [...], "yaw": degrees (+ left)}: metres and directions in the body's frame
//          from the hips at the guard pose; the grip is the middle of the fist, the blade the
//          sword's line out of it, the edge the way its working edge faces (the hand socket's
//          -Z, the knuckles' side, attachment.md), which leads a strike (D565 step 3), the yaw
//          the chest's turn; kimodo_keys.py refuses a key no wrist can turn the hand to
//   left   left-hand keys, the same with "back" for "edge": the blade the forearm's line toward
//          the fist, the back the shield's face, for a shield move
//   blow   {"move": its name in the move set, "sweep": "right-to-left" | "left-to-right" |
//          "overhead", "contact": degrees from ahead the blade crosses at its fastest
//          (+ left), "next": the move it chains into, checked in the scratch set; "held":
//          true for a clip that holds the guard (it stays within a blend of the guard pose
//          throughout); "raised": [from, to] seconds for a block, the span its shield arm is
//          up in front and still}; a block or a guard has a move and no sweep
//   timing {"hit": [least, most], "back": [least, most]}: seconds the move should take, at
//          its playback speed, from its start to its hit (a blow's fastest frame, a block's
//          raise) and from there back on guard; the speed the scratch set plays the take at
//          is chosen to meet them (`speedFor`), so they can be tuned without a new take
//
// Node 22.18 or later; no dependencies.

export type V3 = [number, number, number];
export type Key = { at: number; grip: V3; blade?: V3; edge?: V3; back?: V3; yaw?: number };
export const SWEEPS = ["right-to-left", "left-to-right", "overhead"] as const;
export type Sweep = (typeof SWEEPS)[number];
export type Blow = { move: string; sweep?: Sweep; contact?: number; next?: string; held?: boolean; raised?: [number, number] };
export type Span = [number, number];
export type Timing = { hit: Span; back: Span };
export type Spec = {
  name: string;
  prompt: string;
  seconds: number;
  seed: number;
  samples: number;
  guard: { clip: string; frame: number };
  hand: Key[];
  left: Key[];
  blow: Blow;
  timing?: Timing;
};

/** A blow travels at least this far across the body, or down for an overhead, over its
 * active frames to count as sweeping that way. */
export const SWEEP_M = 0.2;
export const DOWN_M = 0.3;
/** A blow's contact this far off its spec's bearing at most. */
export const CONTACT_DEG = 25;

const isV3 = (v: unknown): v is V3 => Array.isArray(v) && v.length === 3 && v.every((x) => typeof x === "number" && Number.isFinite(x));

/** The spec of the clip `name` from its list entry, or what is wrong with it. */
export function readSpec(name: string, entry: Record<string, unknown>): Spec {
  const wrong = (what: string): never => {
    throw new Error(`${name}: ${what}`);
  };
  if (typeof entry.prompt !== "string" || !entry.prompt) wrong("no prompt");
  const held = (entry.blow as { held?: unknown } | undefined)?.held === true;
  if (!entry.once && !(held && entry.still)) wrong('a move\'s clip is a one-shot: give it "once": true (a held guard\'s is "still": true)');
  const seconds = Number(entry.seconds);
  if (!(seconds > 0)) wrong("no seconds");
  const guard = entry.guard as { clip?: unknown; frame?: unknown } | undefined;
  if (!guard || typeof guard.clip !== "string" || !Number.isInteger(guard.frame)) wrong('no guard: {"clip": the guard clip, "frame": its take\'s frame}');
  const keys = (field: string): Key[] => {
    const list = entry[field] ?? [];
    if (!Array.isArray(list)) return wrong(`${field} is not a list of keys`);
    return list.map((k: Record<string, unknown>, i: number) => {
      const at = Number(k.at);
      if (!(at > 0 && at < seconds)) wrong(`${field} key ${i} at ${k.at}: inside the clip, after its first frame and before its last`);
      if (!isV3(k.grip)) wrong(`${field} key ${i}: grip is [right, ahead, up] in metres`);
      // The sword hand is turned by its edge (D565 step 3), the shield hand by its board's face.
      const turnedBy = field === "hand" ? "edge" : "back";
      const other = field === "hand" ? "back" : "edge";
      if (k[other] !== undefined) wrong(`${field} key ${i}: a ${field === "hand" ? "right" : "left"} hand is turned by its ${turnedBy}, not a ${other}`);
      for (const d of ["blade", turnedBy]) if (k[d] !== undefined && !isV3(k[d])) wrong(`${field} key ${i}: ${d} is [right, ahead, up]`);
      if (k.yaw !== undefined && typeof k.yaw !== "number") wrong(`${field} key ${i}: yaw is degrees`);
      return { at, grip: k.grip as V3, blade: k.blade as V3 | undefined, [turnedBy]: k[turnedBy] as V3 | undefined, yaw: k.yaw as number | undefined };
    });
  };
  const blow = entry.blow as { move?: unknown; sweep?: unknown; contact?: unknown; next?: unknown; held?: unknown; raised?: unknown } | undefined;
  if (!blow || typeof blow.move !== "string") wrong('no blow: {"move": its name in the move set, ...}');
  if (blow.sweep !== undefined && !SWEEPS.includes(blow.sweep as Sweep)) wrong(`sweep ${blow.sweep}: one of ${SWEEPS.join(", ")}`);
  if (blow.contact !== undefined && !(typeof blow.contact === "number" && Math.abs(blow.contact) <= 90)) wrong("contact is degrees from ahead, -90 to 90");
  if (blow.next !== undefined && typeof blow.next !== "string") wrong("next is the name of the move it chains into");
  const span = (v: unknown, inside: number) => Array.isArray(v) && v.length === 2 && v.every((x) => typeof x === "number" && x >= 0 && x <= inside) && v[0] < v[1];
  if (blow.raised !== undefined && !span(blow.raised, seconds)) wrong("raised is [from, to] seconds inside the clip");
  const timing = entry.timing as Record<string, unknown> | undefined;
  if (timing !== undefined && !(span(timing.hit, 10) && span(timing.back, 10))) wrong('timing is {"hit": [least, most], "back": [least, most]} in seconds');
  return {
    name,
    prompt: entry.prompt as string,
    seconds,
    seed: Number(entry.seed ?? 1),
    samples: Number(entry.samples ?? 3),
    guard: { clip: guard.clip as string, frame: guard.frame as number },
    hand: keys("hand"),
    left: keys("left"),
    blow: {
      move: blow.move as string,
      sweep: blow.sweep as Sweep | undefined,
      contact: blow.contact as number | undefined,
      next: blow.next as string | undefined,
      held: blow.held === true,
      raised: blow.raised as Span | undefined,
    },
    timing: timing as Timing | undefined,
  };
}

/** The playback speed that has a take, its hit `hit` seconds in and its end `end`, take its
 * spec's timing: within both its spans if a speed can, else its hit's (the back then off). */
export function speedFor(timing: Timing, hit: number, end: number) {
  const within = (x: number, [a, b]: Span) => x >= a - 1e-6 && x <= b + 1e-6;
  // Speeds that bring the hit, and the rest of the take, inside their spans.
  const [h0, h1] = [hit / timing.hit[1], hit / timing.hit[0]];
  const [b0, b1] = [(end - hit) / timing.back[1], (end - hit) / timing.back[0]];
  const [lo, hi] = [Math.max(h0, b0), Math.min(h1, b1)];
  const speed = lo <= hi ? (lo + hi) / 2 : (h0 + h1) / 2;
  const at = { hit: hit / speed, back: (end - hit) / speed };
  return { speed, ...at, ok: within(at.hit, timing.hit) && within(at.back, timing.back) };
}

/** What `kimodo_keys.py` is given for a spec. */
export function keysFor(spec: Spec) {
  return { frame: spec.guard.frame, seconds: spec.seconds, hand: spec.hand, left: spec.left };
}

/** Which way a blade's tip went over a blow's active frames, from `first` to `last` (the
 * model's space: x to the body's left, y up, z ahead), in metres to the left and down. */
export function travel(first: V3, last: V3) {
  return { left: last[0] - first[0], down: first[1] - last[1] };
}

/** What is wrong with a blow's sweep and contact against its spec, if anything. */
export function againstSpec(blow: Pick<Blow, "sweep" | "contact">, first: V3, last: V3, bearing: number): string[] {
  const bad: string[] = [];
  const { left, down } = travel(first, last);
  const went = `${(Math.abs(left) * 100).toFixed(0)} cm to the ${left >= 0 ? "left" : "right"}, ${(down * 100).toFixed(0)} cm down`;
  if (blow.sweep === "right-to-left" && left < SWEEP_M) bad.push(`sweeps ${went}, not right to left (spec)`);
  if (blow.sweep === "left-to-right" && -left < SWEEP_M) bad.push(`sweeps ${went}, not left to right (spec)`);
  if (blow.sweep === "overhead" && (down < DOWN_M || down < Math.abs(left))) bad.push(`sweeps ${went}, not down (overhead, spec)`);
  if (blow.contact !== undefined && Math.abs(bearing - blow.contact) > CONTACT_DEG) {
    const side = (d: number) => `${Math.abs(d).toFixed(0)}° ${d >= 0 ? "left" : "right"}`;
    bad.push(`contact ${side(bearing)} of ahead, spec ${side(blow.contact)} (bar ${CONTACT_DEG}°)`);
  }
  return bad;
}

export type Scored = { take: number; failures: string[]; off: number };

/** The take kept (D450): the first that passes the move check; if none does, the one with
 * fewest failures, then least off (metres from the guard, and so on: the caller's measure). */
export function choose(takes: Scored[]): Scored | undefined {
  const sorted = [...takes].sort((a, b) => a.take - b.take);
  return sorted.find((t) => t.failures.length === 0) ?? [...sorted].sort((a, b) => a.failures.length - b.failures.length || a.off - b.off)[0];
}

const round2 = (x: number) => (Math.round(x * 100) / 100).toFixed(2);

/** A move set's text with one move's clip and phases written in, the rest of the file (its
 * comments, the other moves) as it was: each field replaced where the move has it, added after
 * its clip where not; a move the set lacks is added at the end of its moves. */
export function writePhases(
  ron: string,
  move: string,
  phases: { clip: string; from: number; done: number; active?: [number, number]; speed?: number; hold?: number; next?: string },
): string {
  const fields: [string, string][] = [
    ["clip", `"${phases.clip}"`],
    ["from", round2(phases.from)],
    ...(phases.active ? [["active", `(${round2(phases.active[0])}, ${round2(phases.active[1])})`] as [string, string]] : []),
    ["done", round2(phases.done)],
    ...(phases.hold !== undefined ? [["hold", round2(phases.hold)] as [string, string]] : []),
    ...(phases.next !== undefined ? [["next", `"${phases.next}"`] as [string, string]] : []),
    ...(phases.speed !== undefined ? [["speed", round2(phases.speed)] as [string, string]] : []),
  ];
  const head = new RegExp(`"${move.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}"\\s*:\\s*\\(`);
  const found = head.exec(ron);
  if (!found) {
    const moves = /\bmoves\s*:\s*\{/.exec(ron);
    if (!moves) throw new Error("no moves: { ... } in the set");
    const close = matching(ron, moves.index + moves[0].length - 1, "{", "}");
    const before = ron.slice(0, close).replace(/\s*$/, "");
    const all = phases.speed === undefined ? [...fields, ["speed", "1.0"]] : fields;
    const entry = `\n        "${move}": (${all.map(([k, v]) => `${k}: ${v}`).join(", ")}),`;
    return before + (before.endsWith(",") || before.endsWith("{") ? "" : ",") + entry + "\n    " + ron.slice(close);
  }
  const open = found.index + found[0].length - 1;
  const close = matching(ron, open, "(", ")");
  let body = ron.slice(open + 1, close);
  for (const [key, value] of fields) {
    const at = new RegExp(`\\b${key}\\s*:\\s*(\\([^)]*\\)|"[^"]*"|-?[\\d.]+)`);
    if (at.test(body)) body = body.replace(at, `${key}: ${value}`);
    else body = body.replace(/\bclip\s*:\s*"[^"]*"\s*,?/, (m) => `${m.replace(/,?$/, ",")} ${key}: ${value},`);
  }
  return ron.slice(0, open + 1) + body + ron.slice(close);
}

/** The index of the bracket closing the one at `open`. */
function matching(text: string, open: number, a: string, b: string): number {
  let depth = 0;
  for (let i = open; i < text.length; i++) {
    if (text[i] === "/" && text[i + 1] === "/") i = text.indexOf("\n", i);
    if (i < 0) break;
    if (text[i] === a) depth++;
    if (text[i] === b && --depth === 0) return i;
  }
  throw new Error(`no ${b} closing the ${a} at ${open}`);
}
