// The statistics of a judged panel (tools/review/judge.ts): a build's mean over fresh judges, the difference of two
// builds with a bootstrap interval and a permutation test, how many judges a difference needs, and for forced choice
// a rate with its Wilson interval and an exact binomial test. Seeded, so a panel scores the same every time.

/// A seeded generator (mulberry32): the same seed, the same resamples.
export function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export const mean = (xs: number[]) => xs.reduce((s, x) => s + x, 0) / xs.length;

/// The sample standard deviation (n - 1); nought for one value.
export function sd(xs: number[]): number {
  if (xs.length < 2) return 0;
  const m = mean(xs);
  return Math.sqrt(xs.reduce((s, x) => s + (x - m) ** 2, 0) / (xs.length - 1));
}

/// The pooled standard deviation of two samples.
export function pooledSd(a: number[], b: number[]): number {
  const df = a.length + b.length - 2;
  if (df < 1) return 0;
  return Math.sqrt(((a.length - 1) * sd(a) ** 2 + (b.length - 1) * sd(b) ** 2) / df);
}

const quantile = (sorted: number[], q: number) => sorted[Math.min(sorted.length - 1, Math.max(0, Math.round(q * (sorted.length - 1))))];

/// The 95% percentile-bootstrap interval of mean(b) - mean(a), each sample resampled with replacement on its own.
export function bootstrapDiff(a: number[], b: number[], iterations = 10000, seed = 1): [number, number] {
  const r = rng(seed);
  const pick = (xs: number[]) => {
    let s = 0;
    for (let i = 0; i < xs.length; i++) s += xs[Math.floor(r() * xs.length)];
    return s / xs.length;
  };
  const diffs: number[] = [];
  for (let i = 0; i < iterations; i++) diffs.push(pick(b) - pick(a));
  diffs.sort((x, y) => x - y);
  return [quantile(diffs, 0.025), quantile(diffs, 0.975)];
}

/// The two-sided permutation p of mean(b) - mean(a): the share of relabellings at least as far from nought.
export function permutationP(a: number[], b: number[], iterations = 10000, seed = 2): number {
  const r = rng(seed);
  const all = [...a, ...b];
  const seen = Math.abs(mean(b) - mean(a));
  let hits = 0;
  for (let i = 0; i < iterations; i++) {
    for (let k = all.length - 1; k > 0; k--) {
      const j = Math.floor(r() * (k + 1));
      [all[k], all[j]] = [all[j], all[k]];
    }
    const d = Math.abs(mean(all.slice(a.length)) - mean(all.slice(0, a.length)));
    if (d >= seen - 1e-9) hits++;
  }
  return (hits + 1) / (iterations + 1);
}

function logGamma(x: number): number {
  // Lanczos (g 7, 9 terms), as Numerical Recipes.
  const c = [0.99999999999980993, 676.5203681218851, -1259.1392167224028, 771.32342877765313, -176.61502916214059,
    12.507343278686905, -0.13857109526572012, 9.9843695780195716e-6, 1.5056327351493116e-7];
  if (x < 0.5) return Math.log(Math.PI / Math.sin(Math.PI * x)) - logGamma(1 - x);
  x -= 1;
  let a = c[0];
  const t = x + 7.5;
  for (let i = 1; i < 9; i++) a += c[i] / (x + i);
  return 0.5 * Math.log(2 * Math.PI) + (x + 0.5) * Math.log(t) - t + Math.log(a);
}

/// The regularised incomplete beta I_x(a, b), by its continued fraction (Numerical Recipes' betacf).
function incompleteBeta(x: number, a: number, b: number): number {
  if (x <= 0) return 0;
  if (x >= 1) return 1;
  const front = Math.exp(logGamma(a + b) - logGamma(a) - logGamma(b) + a * Math.log(x) + b * Math.log(1 - x));
  if (x > (a + 1) / (a + b + 2)) return 1 - incompleteBeta(1 - x, b, a);
  let [c, d] = [1, 1 - ((a + b) * x) / (a + 1)];
  d = 1 / (Math.abs(d) < 1e-30 ? 1e-30 : d);
  let f = d;
  for (let m = 1; m <= 300; m++) {
    for (const num of [(m * (b - m) * x) / ((a + 2 * m - 1) * (a + 2 * m)), (-(a + m) * (a + b + m) * x) / ((a + 2 * m) * (a + 2 * m + 1))]) {
      d = 1 + num * d;
      d = 1 / (Math.abs(d) < 1e-30 ? 1e-30 : d);
      c = 1 + num / c;
      if (Math.abs(c) < 1e-30) c = 1e-30;
      f *= c * d;
    }
    if (Math.abs(c * d - 1) < 1e-12) break;
  }
  return (front * f) / a;
}

/// The two-sided p of Student's t at `df` degrees of freedom.
export function tP(t: number, df: number): number {
  return incompleteBeta(df / (df + t * t), df / 2, 0.5);
}

/// The t that leaves `p` two-sided at `df` degrees of freedom (1.96 as df grows, at 0.05).
export function tCritical(df: number, p = 0.05): number {
  let [lo, hi] = [0, 1000];
  for (let i = 0; i < 100; i++) {
    const mid = (lo + hi) / 2;
    if (tP(mid, df) > p) lo = mid;
    else hi = mid;
  }
  return (lo + hi) / 2;
}

/// Welch's test of mean(b) - mean(a): the difference, its 95% interval and the two-sided p. A spread of nought on
/// both sides gives an interval of nought width and p 0 or 1.
export function welch(a: number[], b: number[]): { diff: number; lo: number; hi: number; p: number; df: number } {
  const diff = mean(b) - mean(a);
  const va = sd(a) ** 2 / a.length;
  const vb = sd(b) ** 2 / b.length;
  const se = Math.sqrt(va + vb);
  if (se === 0) return { diff, lo: diff, hi: diff, p: diff === 0 ? 1 : 0, df: a.length + b.length - 2 };
  const df = (va + vb) ** 2 / (va ** 2 / (a.length - 1) + vb ** 2 / (b.length - 1));
  const half = tCritical(df) * se;
  return { diff, lo: diff - half, hi: diff + half, p: tP(diff / se, df), df };
}

const Z_95 = 1.959964;
const Z_80 = 0.841621;

/// Judges a build needs for a difference `delta` to show at 5% two-sided with 80% power, at judge spread `s`.
export function judgesNeeded(s: number, delta: number): number {
  return Math.max(2, Math.ceil((2 * (Z_95 + Z_80) ** 2 * s * s) / (delta * delta)));
}

/// The least difference `n` judges a build show at 5% two-sided with 80% power, at judge spread `s`.
export function detectable(s: number, n: number): number {
  return (Z_95 + Z_80) * s * Math.sqrt(2 / n);
}

/// The 95% Wilson interval of k successes in n.
export function wilson(k: number, n: number): [number, number] {
  if (n === 0) return [0, 1];
  const p = k / n;
  const z2 = Z_95 * Z_95;
  const centre = (p + z2 / (2 * n)) / (1 + z2 / n);
  const half = (Z_95 * Math.sqrt((p * (1 - p)) / n + z2 / (4 * n * n))) / (1 + z2 / n);
  return [Math.max(0, centre - half), Math.min(1, centre + half)];
}

/// The exact two-sided binomial p of k in n at one half: the probability of an outcome no likelier than k.
export function binomialP(k: number, n: number): number {
  const pmf: number[] = [];
  let c = 2 ** -n;
  for (let i = 0; i <= n; i++) {
    pmf.push(c);
    c = (c * (n - i)) / (i + 1);
  }
  const at = pmf[k];
  return Math.min(1, pmf.filter((p) => p <= at * (1 + 1e-9)).reduce((s, p) => s + p, 0));
}
