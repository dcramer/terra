// What the scripted fight (../fight.ts) knows of the troll's blows and the sword's, read from
// the game's data, and what it decides from them: when its guard is up and whether a swing
// can be begun. No number of either is copied here: a change to data/creatures.ron's Troll or
// to the move set changes the plan.
//
// The rule that matters (terra_creatures melee::drive): a held guard is raised only once the
// arm is in no move, so a swing keeps the guard down until it is done, not until its
// `cancel`; only the chain's next blow (or a roll) cuts one short. A swing is begun only if
// the guard can be up again before the troll's next blow may land.

/// A blow of a creature's: when it lands and how long it lasts, seconds from its beginning.
export type Blow = { lands: number; lasts: number };
/// A blow of the light chain: how long it lasts and from when the next may cut it short,
/// seconds of its own time.
export type Swing = { name: string; lasts: number; cancel: number };

/// How late the script sees the troll is the machine's (a look took 0.1 to 0.7 s on a busy
/// one, 2026-11-09), so it is measured by the caller: a time through a blow is given as the
/// least and the most it can be now, `[low, high]`.
export type Through = [number, number];
/// The guard is kept this long past the latest landing, seconds of the least it can be.
export const KEPT_AFTER_S = 0.1;
/// A swing must leave the arm free this long before a blow may land, seconds, besides what
/// the caller measures of its own lateness: a guard up longer than the set's parry.
export const SPARE_S = 0.3;

const without = (ron: string) => ron.replace(/\/\/.*$/gm, "");
const field = (text: string, name: string) => {
  const found = new RegExp(`\\b${name}:\\s*([\\d.]+)`).exec(text);
  return found ? Number(found[1]) : undefined;
};

/// Every blow of a kind in data/creatures.ron: its temper's own (`bite_at_s`, `attack_s`)
/// and each of its `blows` (`lands_at_s`, `lasts_s`).
export function readBlows(ron: string, kind: string): Blow[] {
  const text = without(ron);
  const from = text.indexOf(`name: "${kind}"`);
  if (from < 0) throw new Error(`data/creatures.ron has no kind named ${kind}: the fight's plan reads its blows from that row`);
  const next = text.indexOf("name: \"", from + 1);
  const row = text.slice(from, next < 0 ? text.length : next);
  const blows: Blow[] = [];
  const [lands, lasts] = [field(row, "bite_at_s"), field(row, "attack_s")];
  if (lands !== undefined && lasts !== undefined) blows.push({ lands, lasts });
  for (const each of row.matchAll(/\([^()]*lands_at_s:[^()]*\)/g)) {
    const [at, long] = [field(each[0], "lands_at_s"), field(each[0], "lasts_s")];
    if (at !== undefined && long !== undefined) blows.push({ lands: at, lasts: long });
  }
  if (blows.length === 0) throw new Error(`data/creatures.ron's ${kind} names no blow (bite_at_s and attack_s, or blows with lands_at_s and lasts_s): the fight's plan needs when each lands`);
  return blows;
}

/// The light chain of a move set (data/moves/*.ron): the main button's move and each `next`
/// after it, with how long each lasts and from when the next cuts it short.
export function readChain(ron: string): Swing[] {
  const text = without(ron);
  const first = /buttons:\s*\([^)]*\bmain:\s*"([\w-]+)"/.exec(text)?.[1];
  if (!first) throw new Error("the move set names no main button's move (buttons: (main: ...)): the fight's plan swings by it");
  const body = text.slice(text.indexOf("moves:"));
  const names = [...body.matchAll(/"([\w-]+)":\s*\(/g)];
  const chain: Swing[] = [];
  for (let name: string | undefined = first; name && !chain.some((each) => each.name === name); ) {
    const at = names.findIndex((each) => each[1] === name);
    if (at < 0) throw new Error(`the move set's chain names ${name}, which is no move of it`);
    const chunk = body.slice(names[at].index, names[at + 1]?.index ?? body.length);
    const [from, speed, done] = [field(chunk, "from") ?? 0, field(chunk, "speed") ?? 1, field(chunk, "done")];
    if (done === undefined) throw new Error(`the move set's ${name} has no done: the fight's plan needs how long it lasts`);
    const lasts = (done - from) / speed;
    const cancel = field(chunk, "cancel");
    chain.push({ name, lasts, cancel: cancel === undefined ? lasts : (cancel - from) / speed });
    name = /\bnext:\s*"([\w-]+)"/.exec(chunk)?.[1];
  }
  return chain;
}

/// Whether a blow under way may still land: until the latest of its blows surely has. Which
/// blow is coming the script cannot tell, as a player new to the troll cannot. The guard is
/// wanted through all of it that no swing fits in.
export function threatens(blows: Blow[], [low]: Through): boolean {
  return low < Math.max(...blows.map((each) => each.lands)) + KEPT_AFTER_S;
}

/// The least time until a blow may land, seconds: in no blow, the soonest any lands; before
/// the soonest can have, what is left to it; while one may land, none; after, what is left of
/// the shortest blow this one may still be, and then the soonest of the next.
export function untilLanding(blows: Blow[], through: Through | undefined): number {
  const soonest = Math.min(...blows.map((each) => each.lands));
  if (through === undefined) return soonest;
  const [low, high] = through;
  if (high < soonest) return soonest - high;
  if (threatens(blows, through)) return 0;
  const left = blows.filter((each) => each.lasts > low).map((each) => Math.max(0, each.lasts - high));
  return (left.length > 0 ? Math.min(...left) : 0) + soonest;
}

/// The arm as the script believes it: the chain's blow it is in and for how long, seconds.
export type Arm = { swing: number; for: number } | undefined;

/// The blow a press now would begin, if the guard could still be up `SPARE_S` and the
/// caller's own lateness `late` before a blow may land `until` seconds from now: the chain's
/// first from a free arm; its next from a blow past its `cancel` (never pressed sooner: a
/// press waits only the set's buffer). None if there is not the time, or the arm cannot be
/// cut short yet.
export function swing(chain: Swing[], arm: Arm, until: number, late = 0): number | undefined {
  const busy = arm !== undefined && arm.for < chain[arm.swing].lasts;
  if (busy && arm.for < chain[arm.swing].cancel) return undefined;
  const next = busy ? (arm.swing + 1) % chain.length : 0;
  return chain[next].lasts + SPARE_S + late <= until ? next : undefined;
}
