// The key moments of a duel for its contact sheet, read from the fight record: a row a step
// of the script, each at the move the step made (the player's), or the partner's blow it
// met (a block, a parry), or the step alone (a draw, a strafe), and three moments of it:
// the wind-up (the move starting), the contact (the blow landing, or the first active
// frame of a blow while it runs) and the recovery. Node 22.18 or later; no dependencies.

import { type Frame, type Run, runs } from "./checks.ts";

export type Row = { step: string; subject: string; frames: [number, number, number] };
export const COLUMNS = ["wind-up", "contact", "recovery"];
/// The recovery shown so long after a blow's recovery begins, its follow-through read.
const INTO_RECOVERY = 6;
/// A step with no move is shown at it and this long after, twice.
const STEP_ALONE = 60;

export function moments(frames: Frame[]): Row[] {
  const all = runs(frames);
  const byFrame = new Map(frames.map((frame) => [frame.frame, frame]));
  const rows: Row[] = [];
  for (const frame of frames) {
    if (frame.step === null) continue;
    const subject = mine(frame, all) ?? met(frame, all, frames);
    if (!subject) {
      rows.push({ step: frame.step, subject: "", frames: [frame.frame, frame.frame + STEP_ALONE, frame.frame + 2 * STEP_ALONE] });
      continue;
    }
    const span = frames.filter((each) => each.frame >= subject.from && each.frame <= subject.to + 1);
    const hit = span.find((each) => each.contacts.some((c) => c.move === subject.move));
    const active = span.find((each) => each.fighters.some((f) => f.phase === "Active"));
    const contact = hit?.frame ?? active?.frame ?? Math.round((subject.from + subject.to) / 2);
    // A blow's recovery a little after it begins; a move with no blow (a roll) at its end;
    // a blow cut short where it met the guard (a parry), the reeling after.
    const struck = span.some((each) => each.fighters.some((f) => f.who === subject.who && f.phase === "Active"));
    let recovery = subject.to;
    for (let n = contact; struck && n <= subject.to; n++) {
      const me = byFrame.get(n)?.fighters.find((f) => f.who === subject.who);
      if (me?.phase === "Recovery") {
        recovery = Math.min(subject.to, n + INTO_RECOVERY);
        break;
      }
    }
    if (recovery <= contact) recovery = contact + 2 * INTO_RECOVERY;
    rows.push({ step: frame.step, subject: `${subject.who === "you" ? "" : "the partner's "}${subject.move}`, frames: [subject.from, contact, recovery] });
  }
  return rows;
}

/// The player's move a step asked for.
function mine(frame: Frame, all: Run[]): Run | undefined {
  const asked = frame.asked.find((each) => each.got === "started" || each.got === "waits");
  return asked && all.find((run) => run.who === "you" && run.move === asked.move && run.from >= frame.frame);
}

/// The partner's blow running at a step that lands on the player after it: what a guard met.
function met(frame: Frame, all: Run[], frames: Frame[]): Run | undefined {
  return all.find(
    (run) =>
      run.who !== "you" &&
      run.from <= frame.frame &&
      run.to >= frame.frame &&
      frames.some((each) => each.frame >= frame.frame && each.frame <= run.to + 1 && each.contacts.some((c) => c.move === run.move)),
  );
}
