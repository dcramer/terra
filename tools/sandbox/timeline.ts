// A trailer's cut as a timeline (trailer.ts, D357): its pieces in order, title cards and
// shots, each overlapping the one before by its transition, as an editor's timeline lays
// clips; where each starts, so the sound bed's layers come in as their act does; and the
// ffmpeg filter that joins them. Pure, so trailer.test.ts can check it. Node 22.18 or
// later; no dependencies.

/// A piece of the cut: a title card or a shot, how long it runs, and how long it overlaps
/// the piece before it (a crossfade, or out of and into black for a card).
export type Piece = { name: string; seconds: number; overlap: number; act: number };

/// Where each piece starts and how long the whole cut runs: each starts where the one
/// before ends less its own overlap.
export function lay(pieces: Piece[]): { starts: number[]; seconds: number } {
  const starts: number[] = [];
  let end = 0;
  for (const [i, piece] of pieces.entries()) {
    const overlap = i === 0 ? 0 : Math.min(piece.overlap, piece.seconds / 2, pieces[i - 1].seconds / 2);
    const start = Math.max(0, end - overlap);
    starts.push(start);
    end = start + piece.seconds;
  }
  return { starts, seconds: end };
}

/// A route through `spots` smoothed into a spot every `step` metres along a centripetal
/// Catmull-Rom curve through them, which never loops or overshoots between spots, so a
/// player walking it turns a little at each rather than all at once at the corners (and the
/// camera behind them with them). It starts and ends where the spots do.
export function smoothed(spots: [number, number][], step: number): [number, number][] {
  if (spots.length < 3) return spots;
  const p = [spots[0], ...spots, spots[spots.length - 1]];
  const dense: [number, number][] = [];
  for (let i = 1; i < p.length - 2; i++) {
    const [p0, p1, p2, p3] = [p[i - 1], p[i], p[i + 1], p[i + 2]];
    const knot = (a: [number, number], b: [number, number]) => Math.max(1e-4, Math.sqrt(Math.hypot(b[0] - a[0], b[1] - a[1])));
    const t1 = knot(p0, p1);
    const t2 = t1 + knot(p1, p2);
    const t3 = t2 + knot(p2, p3);
    const mix = (a: [number, number], b: [number, number], ta: number, tb: number, t: number): [number, number] => {
      const w = (t - ta) / (tb - ta);
      return [a[0] + (b[0] - a[0]) * w, a[1] + (b[1] - a[1]) * w];
    };
    const samples = Math.max(20, Math.ceil(Math.hypot(p2[0] - p1[0], p2[1] - p1[1]) / 0.25));
    for (let k = 0; k < samples; k++) {
      const t = t1 + ((t2 - t1) * k) / samples;
      const a1 = mix(p0, p1, 0, t1, t);
      const a2 = mix(p1, p2, t1, t2, t);
      const a3 = mix(p2, p3, t2, t3, t);
      const b1 = mix(a1, a2, 0, t2, t);
      const b2 = mix(a2, a3, t1, t3, t);
      dense.push(mix(b1, b2, t1, t2, t));
    }
  }
  dense.push(spots[spots.length - 1]);
  // Every `step` metres along it.
  const out: [number, number][] = [dense[0]];
  let since = 0;
  for (let i = 1; i < dense.length; i++) {
    since += Math.hypot(dense[i][0] - dense[i - 1][0], dense[i][1] - dense[i - 1][1]);
    if (since >= step) {
      out.push(dense[i]);
      since = 0;
    }
  }
  const end = spots[spots.length - 1];
  if (out[out.length - 1] !== end) out.push(end);
  return out;
}

/// ffmpeg's filter joining the pieces, input i being piece i: each crossfaded into the next
/// with `xfade` at the time `lay` gives, every input first brought to one frame rate, size
/// and time base, as xfade needs. The joined picture is `[cut]`.
export function joined(pieces: Piece[], fps: number, size: string): string {
  const { starts } = lay(pieces);
  const [w, h] = size.split("x");
  const parts = pieces.map((_, i) => `[${i}:v]fps=${fps},scale=${w}:${h},setsar=1,format=yuv420p,settb=AVTB[p${i}]`);
  if (pieces.length === 1) return [...parts, "[p0]copy[cut]"].join(";");
  let last = "p0";
  for (let i = 1; i < pieces.length; i++) {
    const end = i === pieces.length - 1 ? "cut" : `x${i}`;
    const overlap = starts[i - 1] + pieces[i - 1].seconds - starts[i];
    parts.push(`[${last}][p${i}]xfade=transition=fade:duration=${overlap.toFixed(3)}:offset=${starts[i].toFixed(3)}[${end}]`);
    last = end;
  }
  return parts.join(";");
}

/// The bars a letterbox lays over a picture `width` by `height` to show it `ratio` to one,
/// each so many pixels high, even, as video wants; none for a picture already as wide.
export function bars(width: number, height: number, ratio: number): number {
  const shown = width / ratio;
  if (shown >= height) return 0;
  return 2 * Math.round((height - shown) / 4);
}
