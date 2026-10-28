// The motion gate's pass rule, paired (D472): a build's films are judged beside its base's, filmed
// in the same job by film.ts, as shipped regression gates hold a change against its parent run on
// the same machine (Chromium's Pinpoint, Mozilla's Perfherder) rather than against a fixed number
// alone. A measure passes when the build meets the real clips' bar (motion.ts GATE), or when it is
// no further outside that bar than the base is, within the run-to-run spread of the gate's value
// that repeat films of one flame show (TOLERANCE, derived from gate-aa.json by `tolerance`).
// Without it main's night splits, which sit on the 1.2 bar (pooled 1.18 over 28 films), failed
// main itself about one run in three (fire round g85).
import { GATE, held } from "./motion.ts";

/// One film: its placement (m along x) and its value of each GATE measure, in GATE's order.
export type Film = { dx: number; v: number[] };
/// What film.ts keeps of a side's films at an hour (`<side>/<hour>.json`).
export type Side = { rev: string; hour: string; films: (Film & { dir: string })[] };

/// The gate's value of each measure over a side's films: its middle-half mean, to two decimals,
/// as `gate` gives it.
export const heldOf = (films: Film[]) => GATE.map((_, i) => Math.round(100 * held(films.map((f) => f.v[i]))) / 100);

/// How far a value lies outside [lo, hi]: 0 inside.
export const outside = (v: number, lo: number, hi: number) => Math.max(lo - v, v - hi, 0);

/// Each measure judged: the build's value and the base's, whether the build meets the bar (`met`),
/// and, where it does not, whether it is no further outside it than the base plus the hour's
/// tolerance (`kept`). Without a base a measure passes only by its bar.
export function paired(build: Film[], base: Film[] | undefined, tol: Record<string, number>) {
  const b = heldOf(build);
  const a = base && heldOf(base);
  return GATE.map((g, i) => {
    const met = b[i] >= g.lo && b[i] <= g.hi;
    const t = tol[g.name];
    if (!met && a && t === undefined) throw new Error(`no tolerance for ${g.name}`);
    const kept = !met && !!a && outside(b[i], g.lo, g.hi) <= outside(a[i], g.lo, g.hi) + t + 1e-9;
    return { name: g.name, value: b[i], base: a?.[i], met, kept, pass: met || kept };
  });
}

/// A judged hour in a line's words: each measure's value, `x` where it fails, and the base's value
/// after a `~` where it passed only as no worse than the base.
export const told = (j: ReturnType<typeof paired>) =>
  j.map((x) => `${x.name} ${x.value.toFixed(2)}${x.pass ? (x.kept ? `~${x.base!.toFixed(2)}` : "") : `x${x.base === undefined ? "" : `(base ${x.base.toFixed(2)})`}`}`).join(" ");

/// The run-to-run sd of each measure's gate value for a build filmed `takes` times a placement, from
/// repeat films of one flame (A/A): drawn `draws` times, each placement's takes picked from its
/// pooled films with replacement (a seeded LCG, so the answer is the same each time), as the
/// gate's own films are a draw of `takes` a placement.
export function spread(films: Film[], takes: number, draws = 4000, seed = 1): number[] {
  const dxs = [...new Set(films.map((f) => f.dx))].sort((p, q) => p - q);
  const by = dxs.map((dx) => films.filter((f) => f.dx === dx));
  let s = seed;
  const rand = () => (s = (s * 1103515245 + 12345) % 2147483648) / 2147483648;
  const vals = GATE.map(() => [] as number[]);
  for (let d = 0; d < draws; d++) {
    const pick = by.flatMap((of) => Array.from({ length: takes }, () => of[Math.floor(rand() * of.length)]));
    GATE.forEach((_, i) => vals[i].push(held(pick.map((f) => f.v[i]))));
  }
  return vals.map((xs) => {
    const m = xs.reduce((a, x) => a + x, 0) / xs.length;
    return Math.sqrt(xs.reduce((a, x) => a + (x - m) ** 2, 0) / xs.length);
  });
}

/// Standard deviations of the difference between two runs a build may be worse than its base by.
/// Two independent runs differ with sd sqrt(2) times one run's, so at 2 a build no worse than its
/// base fails a measure it does not meet about one run in 44 (one-sided 2.3%).
export const SDS = 2;

/// Each measure's tolerance at an hour from A/A films: SDS x sqrt(2) x `spread`, rounded up to the
/// gate's two decimals, at least 0.01 (the values are rounded to it).
export const tolerance = (films: Film[], takes: number) =>
  Object.fromEntries(spread(films, takes).map((sd, i) => [GATE[i].name, Math.max(0.01, Math.ceil(100 * SDS * Math.SQRT2 * sd - 1e-9) / 100)]));

/// The tolerances the gate uses, by hour: `tolerance` of gate-aa.json's films at film.ts's TAKES
/// (film.test checks they are). `node tools/fire/film.ts --aa <side dirs>...` pools new A/A films
/// into gate-aa.json and prints the table.
export const TOLERANCE: Record<string, Record<string, number>> = {
  dusk: { flick: 0.08, area: 0.01, life: 0.01, splits: 0.11, lean: 0.15, topred: 0.07 },
  night: { flick: 0.08, area: 0.01, life: 0.02, splits: 0.11, lean: 0.13, topred: 0.05 },
};
