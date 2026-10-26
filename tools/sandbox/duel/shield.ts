// The shield through a stagger, on a duel's fight record (`shield`: the board's middle each
// frame, terra_creatures::melee::record): how far the board jumps in the fighter's own frame
// (its feet, facing and up) while a parried blow staggers it, research/2026-10-25-shield-prior-
// art.md fault 4 (the shield up at the left to down by the hip in 6 frames). duel.ts runs it
// beside checks.ts; checks.test.ts plants the fault. Node 22.18 or later; no dependencies.

import type { Check, Fighter, Frame } from "./checks.ts";

/// The span a jump is measured over: 0.1 s at the record's 60 frames a second, the 6 frames
/// the developer's eye caught the stagger's shield crossing in.
export const JUMP_FRAMES = 6;
/// The farthest the board may move in that span through a stagger, m: a quick, eased lift of
/// the arm, not a snap (a smoothstep over 0.25 s at most moving the board 0.25 m).
export const JUMP_M = 0.15;
/// How far before a stagger starts and after it ends the jump is looked for, frames: its blend
/// in, and the blend back to the guard.
const BEFORE = 6;
const AFTER = 18;

const sub = (a: number[], b: number[]) => a.map((x, i) => x - b[i]);
const dot = (a: number[], b: number[]) => a.reduce((s, x, i) => s + x * b[i], 0);
const unit = (a: number[]) => {
  const l = Math.hypot(...a) || 1;
  return a.map((x) => x / l);
};
const cross = (a: number[], b: number[]) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];

/// A fighter's board middle in its own frame: right, up and ahead of its feet, m.
export function inBody(f: Fighter, up: number[]): number[] | undefined {
  if (!f.shield) return undefined;
  const ahead = unit(sub(f.facing, up.map((x) => x * dot(f.facing, up))));
  const right = cross(ahead, up);
  const rel = sub(f.shield.mid, f.feet);
  return [dot(rel, right), dot(rel, up), dot(rel, ahead)];
}

/// Each stagger's frames: a parried blow's striker (a flinch's reel is not one), from the
/// parry's frame to where its `stagger_s` is spent.
function staggers(frames: Frame[]): { who: string; from: number; to: number }[] {
  const found: { who: string; from: number; to: number }[] = [];
  frames.forEach((frame, from) => {
    for (const parried of frame.contacts.filter((c) => c.taken === "Parried")) {
      const who = parried.by.replace(/^the /, "");
      let to = from + 1;
      while (to < frames.length - 1 && (frames[to].fighters.find((f) => f.who === who)?.stagger_s ?? 0) > 0) to++;
      found.push({ who, from, to });
    }
  });
  return found;
}

/// The worst shield jump through a stagger: the farthest the board middle moves in the
/// fighter's frame within JUMP_FRAMES, from BEFORE frames before the stagger to AFTER frames
/// past it. Its value is that distance (m), what the combat scorecard holds; a fault each
/// stagger past JUMP_M. None on a record with no shield (unmeasured, `?` on the scorecard).
export function staggerJump(frames: Frame[]): Check | undefined {
  if (!frames.some((frame) => frame.fighters.some((f) => f.shield))) return undefined;
  const name = "stagger shield jump";
  const up = frames.find((frame) => frame.up)?.up ?? [0, 1, 0];
  const each = staggers(frames).map(({ who, from, to }) => {
    let worst = { m: 0, at: frames[from]?.frame ?? 0 };
    const lo = Math.max(0, from - BEFORE);
    const hi = Math.min(frames.length - 1, to + AFTER);
    for (let i = lo; i + JUMP_FRAMES <= hi; i++) {
      const a = frames[i].fighters.find((f) => f.who === who);
      const b = frames[i + JUMP_FRAMES].fighters.find((f) => f.who === who);
      const pa = a && inBody(a, up);
      const pb = b && inBody(b, up);
      if (!pa || !pb) continue;
      const m = Math.hypot(...sub(pb, pa));
      if (m > worst.m) worst = { m, at: frames[i].frame };
    }
    return { who, from: frames[from].frame, ...worst };
  });
  if (!each.length) return { name, ok: true, line: "no stagger in the fight", faults: 0, value: 0 };
  const worst = each.reduce((a, b) => (b.m > a.m ? b : a));
  const over = each.filter((s) => s.m > JUMP_M);
  const whose = worst.who === "you" ? "the player's" : `the ${worst.who}'s`;
  const line = `${each.length} staggers, worst ${worst.m.toFixed(2)} m in ${JUMP_FRAMES} frames (bar ${JUMP_M} m), ${whose} from frame ${worst.at} (stagger at ${worst.from})`;
  return { name, ok: over.length === 0, line, faults: over.length, value: Math.round(worst.m * 1000) / 1000 };
}
