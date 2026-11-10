// What heard.ts judges a walk among wildlife by, apart from the game so it is tested alone
// (heard.test.ts): the kinds' voices read from data/creatures.ron, and the two faults.

/** A kind's voices by its data: the sound a fright names, and the one it calls with at rest. */
export type Kind = { name: string; cry?: string; call?: string; calls?: [number, number] };

/** `sounds.heard`: each sound's name and how many times it has been heard. */
export type Heard = Record<string, number>;

/** A kind stepped up to, and the sounds heard more often in the seconds after than before. */
export type Met = { kind: string; after: string[] };

/**
 * No one voice more than this many times a minute: one every three seconds. A guess (D654),
 * and so this check is a reading, not a gate (D632): the walk the developers refused heard the
 * crow's caw 326 times in three minutes, 109 a minute, which it fails, but no walk has yet
 * been heard and found good, whose number would be the bar. On main 2026-11-10 (seed 3) the
 * crow's caw is still 109 a minute with calls_s at 90 to 300 s, so its calls were not the cause.
 */
export const KNOWN_BAD_PER_MIN = 109;
export const BAR_PER_MIN = 20;

/** The names the sounds made in code are heard by (terra_game play/sounds.rs sounds_like). */
const MADE: Record<string, string | undefined> = {
  Snort: "snort",
  Hiss: "hiss",
  Chatter: "chatter",
  Chirp: "chirp",
  Quack: "quack",
  Croak: "croak",
  Bark: "bark",
  Growl: "cry.growl",
  Silent: undefined,
};

/** Every kind of data/creatures.ron with what it is heard as (terra_game play/noises.rs). */
export function readKinds(ron: string): Kind[] {
  const text = ron.replace(/\/\/.*$/gm, "");
  const starts = [...text.matchAll(/^ {8}name: "([^"]+)"/gm)];
  return starts.map((start, i) => {
    const body = text.slice(start.index, starts[i + 1]?.index ?? text.length);
    const named = (field: string) => new RegExp(`^ {8}${field}: Some\\("([^"]+)"\\)`, "m").exec(body)?.[1];
    const voice = /^ {8}voice: (\w+)/m.exec(body)?.[1] ?? "Silent";
    if (!(voice in MADE)) throw new Error(`${start[1]}'s voice ${voice} is not one heard.ts knows: add it to MADE in tools/sandbox/heard/judge.ts from sounds_like`);
    const calls = /^ {8}calls_s: Some\(\(([\d.]+), ([\d.]+)\)\)/m.exec(body);
    const cry = named("cry") ?? MADE[voice];
    return { name: start[1], cry, call: named("call") ?? cry, calls: calls ? [Number(calls[1]), Number(calls[2])] : undefined };
  });
}

/** The faults of a walk: a voice heard too often, or a kind met that named no sound. */
export function faults(kinds: Kind[], before: Heard, after: Heard, minutes: number, met: Met[], bar = BAR_PER_MIN): string[] {
  const found: string[] = [];
  const voices = new Set(kinds.flatMap((kind) => [kind.cry, kind.call]).filter((name) => name !== undefined));
  for (const voice of voices) {
    const rate = ((after[voice] ?? 0) - (before[voice] ?? 0)) / minutes;
    if (rate > bar) {
      const whose = kinds.filter((kind) => kind.cry === voice || kind.call === voice).map((kind) => `${kind.name}${kind.calls ? ` (calls_s ${kind.calls.join(" to ")})` : ""}`);
      found.push(`${voice} was heard ${rate.toFixed(1)} times a minute, over the bar of ${bar}: it is ${whose.join(", ")}'s in assets/data/creatures.ron; raise calls_s or thin the kind, do not raise the bar`);
    }
  }
  for (const one of met) {
    const kind = kinds.find((kind) => kind.name === one.kind);
    if (!kind) found.push(`${one.kind} was met and is not in assets/data/creatures.ron`);
    else if (kind.cry && !one.after.includes(kind.cry)) found.push(`${one.kind} was stepped up to and ${kind.cry} was not heard (heard: ${one.after.join(", ") || "nothing"}): its cry in assets/data/creatures.ron must name a row of terra_game play/sounds/clips.rs or a voice made in code`);
  }
  return found;
}

/** The most often heard voice of a walk, to say on the passing line. */
export function busiest(kinds: Kind[], before: Heard, after: Heard, minutes: number): { voice: string; rate: number } {
  const voices = new Set(kinds.flatMap((kind) => [kind.cry, kind.call]).filter((name) => name !== undefined));
  let most = { voice: "none", rate: 0 };
  for (const voice of voices) {
    const rate = ((after[voice] ?? 0) - (before[voice] ?? 0)) / minutes;
    if (rate > most.rate) most = { voice, rate };
  }
  return most;
}
