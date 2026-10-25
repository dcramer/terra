// A flame's tongues tracked through a clip, for motion.ts: how long each lives, how often a tip
// pinches off and rises away on its own (shedding), how often one forks in two (splits), and how
// far each leans from vertical.
//
// The method is the storm-cell trackers' (TITAN, Dixon and Wiener 1993; tobac's tracking of
// convective cells works the same way), whose cells also grow, split, merge and drift faster than
// they keep their shape: each frame's pieces are found, each tracked piece's next position is
// forecast from its motion, and pieces are matched by how much of each forecast footprint
// (widened by REACH) they fill. A forecast filled by two pieces is a split, two forecasts filled
// by one piece a merger. A matched piece's own last step is its motion; a new piece is forecast
// to move as the clip's pieces of its kind do (a first pass, matched in place, measures that): a
// tongue still joined to the fire's body grows in place, a blob free of it rises with the gas.
// Tracking by overlap alone, as motion.ts did first, broke on every frame of 30 fps footage:
// the flame's outline is mottled with holes and clipped-white pockets, so a tongue fell apart
// into shards that came and went each frame, and a free blob rises past its own footprint.
//
// A tongue is a piece of the flame (holes filled) above TONGUE_FROM H of at least TONGUE_AREA H²;
// it is attached where its piece of the whole flame reaches below that level, free otherwise. An
// attached tongue's piece that forks into two attached pieces is a split; one that leaves a free
// piece above it, or comes off whole, sheds. A split or shed counts only if the branch it makes
// never merges back into the track it came from (else the mask's flicker made it), and nothing
// is counted by frames, so a clip slowed down by repeating its frames reads the same events.

import { filled, type Geometry, pieces } from "./motion.ts";

/// How far a forecast footprint is widened, of H: the shape change a 30 fps frame leaves.
export const REACH = 0.03;
/// The least overlap, of the smaller piece, that links two pieces across a frame.
export const LINK = 0.15;
/// The closing that joins a flame mask's cracks before its tongues are found, of H: the cut
/// leaves a tongue's dimmer streaks out, so one tongue reads as several split by hairlines.
export const CLOSE = 0.0125;

/// A mask closed by a square of radius `r` (dilated then eroded; the frame's edge erodes
/// nothing), so gaps under 2r + 1 px across fill and its outline otherwise stays.
export function closed(mask: Uint8Array, w: number, h: number, r: number): Uint8Array {
  const pass = (src: Uint8Array, grow: boolean, alongX: boolean) => {
    const out = new Uint8Array(w * h);
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        let v = grow ? 0 : 1;
        for (let d = -r; d <= r; d++) {
          const [xx, yy] = alongX ? [x + d, y] : [x, y + d];
          if (xx < 0 || yy < 0 || xx >= w || yy >= h) continue;
          const s = src[yy * w + xx];
          v = grow ? v | s : v & s;
        }
        out[y * w + x] = v;
      }
    return out;
  };
  return pass(pass(pass(pass(mask, true, true), true, false), false, true), false, false);
}

/// How far a tongue's tip must stand above the notch joining it to a taller tongue, of H.
export const PROMINENCE = 0.1;
/// A tongue must stand this tall, of H, for its lean to be measured: a stub's angle is noise.
export const LEAN_TALL = 0.15;
/// The rows at a tongue's foot and at its tip whose middles its lean is drawn through, of H.
export const LEAN_ROWS = 0.04;

export type Piece = { px: number[]; area: number; cx: number; cy: number; attached: boolean; lean: number; top: number };
export type TongueTracks = {
  /// Tongues' tracks (attached or free) wholly inside the clip: their lifetime's mean and 90th
  /// percentile, s.
  lifetime: number;
  lifetime90: number;
  tracks: number;
  /// Events per tongue a second (the mean count of attached tongues times the clip's seconds).
  sheds: number;
  splits: number;
  merges: number;
  /// Lean from vertical, degrees: the median and 90th percentile over attached tongues standing
  /// LEAN_TALL high, frame by frame.
  lean: number;
  lean90: number;
  /// Free blobs after they come off: how long they last, s, and how fast they rise, H/s (medians).
  shedLife: number;
  shedRise: number;
  /// The rise of the tracked pieces from frame to frame, H/s: the median and 90th percentile.
  rise: number;
  rise90: number;
  /// Tongues born (through the level, by a split or by shedding) a second.
  births: number;
};

const quantile = (xs: number[], q: number) => {
  const s = [...xs].sort((a, b) => a - b);
  return s.length ? s[Math.min(s.length - 1, Math.max(0, Math.floor(q * s.length)))] : NaN;
};

/// A frame's tongues: the peaks of its flame (holes filled) above `level` standing at least
/// `prominence` px over the notch that joins each to a taller one, as a mountain's prominence
/// is reckoned (the flame's component tree by height, Salembier, Oliveras and Garrido 1998's
/// max-tree): rows are swept down from the top, joining each row's flame to the pieces above;
/// where two pieces meet, the one with the lower tip ends there, a tongue if it stood
/// `prominence` over that row, else part of the other. What is left at the level ends there
/// too. A tongue is its pixels not in a tongue ended above it, at least `least` of them; free
/// blobs (no flame joins them to the body below the level) are tongues by size alone.
export function framePieces(mask: Uint8Array, w: number, h: number, level: number, least: number, H: number, prominence = PROMINENCE * H): Piece[] {
  const solid = filled(closed(mask, w, h, Math.max(1, Math.round(CLOSE * H))), w, h);
  const whole = pieces(solid, w, h);
  const reaches = new Uint8Array(whole.area.length);
  for (let p = level * w; p < w * h; p++) if (whole.label[p]) reaches[whole.label[p]] = 1;
  const parent = new Int32Array(w * level).fill(-1);
  const tip = new Map<number, number>();
  const own = new Map<number, number[]>();
  const find = (p: number) => {
    while (parent[p] !== p) p = parent[p] = parent[parent[p]];
    return p;
  };
  const out: Piece[] = [];
  const record = (px: number[]) => {
    if (px.length < least) return;
    let [sx, sy] = [0, 0];
    for (const p of px) {
      sx += p % w;
      sy += (p / w) | 0;
    }
    const attached = reaches[whole.label[px[0]]] === 1;
    out.push({ px, area: px.length, cx: sx / px.length, cy: sy / px.length, attached, lean: attached ? leanOf(px, w, H) : NaN, top: (px[0] / w) | 0 });
  };
  const join = (a: number, b: number, y: number) => {
    const [ra, rb] = [find(a), find(b)];
    if (ra === rb) return;
    const [ta, tb] = [tip.get(ra)!, tip.get(rb)!];
    const [win, lose] = ta < tb || (ta === tb && own.get(ra)!.length >= own.get(rb)!.length) ? [ra, rb] : [rb, ra];
    const theirs = own.get(lose)!;
    if (y - tip.get(lose)! >= prominence) record(theirs);
    else {
      const mine = own.get(win)!;
      for (const p of theirs) mine.push(p);
    }
    parent[lose] = win;
    own.delete(lose);
    tip.delete(lose);
  };
  for (let y = 0; y < level; y++)
    for (let x = 0; x < w; x++) {
      const p = y * w + x;
      if (!solid[p]) continue;
      parent[p] = p;
      tip.set(p, y);
      own.set(p, [p]);
      if (x > 0 && parent[p - 1] >= 0) join(p, p - 1, y);
      if (y > 0) for (let xx = Math.max(0, x - 1); xx <= Math.min(w - 1, x + 1); xx++) if (parent[p - w + xx - x] >= 0) join(p, p - w + xx - x, y);
    }
  for (const [r, px] of own) if (level - tip.get(r)! >= prominence || reaches[whole.label[px[0]]] === 0) record(px);
  return out;
}

/// The angle from vertical, degrees, of the line from the middle of a tongue's foot rows to the
/// middle of its tip rows; NaN for a tongue under LEAN_TALL H.
export function leanOf(px: number[], w: number, H: number): number {
  let [top, foot] = [Infinity, -Infinity];
  for (const p of px) {
    const y = (p / w) | 0;
    top = Math.min(top, y);
    foot = Math.max(foot, y);
  }
  if (foot - top < LEAN_TALL * H) return NaN;
  const band = Math.max(1, Math.round(LEAN_ROWS * H));
  const mid = (y0: number, y1: number) => {
    let [s, n, sy] = [0, 0, 0];
    for (const p of px) {
      const y = (p / w) | 0;
      if (y >= y0 && y <= y1) {
        s += p % w;
        sy += y;
        n++;
      }
    }
    return [s / n, sy / n];
  };
  const [tx, ty] = mid(top, top + band - 1);
  const [fx, fy] = mid(foot - band + 1, foot);
  return (Math.atan2(Math.abs(tx - fx), fy - ty) * 180) / Math.PI;
}

/// A track as it stands: its mean step a frame over its life so far (its forecast; before its
/// first step, its kind's or its parent's), and how many steps it has taken.
type Track = { id: number; vx: number; vy: number; steps: number; attached: boolean };
type Link = { from: number; to: number; overlap: number };

/// The links between one frame's pieces and the next's: for each earlier piece, its footprint
/// moved by its forecast and widened by `reach` px; a link where a later piece fills at least
/// LINK of the smaller of the two.
export function links(a: Piece[], forecast: [number, number][], b: Piece[], w: number, h: number, reach: number): Link[] {
  const owner = new Int32Array(w * h).fill(-1);
  b.forEach((piece, j) => piece.px.forEach((p) => (owner[p] = j)));
  const out: Link[] = [];
  const seen = new Uint8Array(w * h);
  a.forEach((piece, i) => {
    const [dx, dy] = forecast[i].map(Math.round);
    const hits = new Map<number, number>();
    const marked: number[] = [];
    for (const p of piece.px) {
      const x = (p % w) + dx;
      const y = ((p / w) | 0) + dy;
      for (let yy = Math.max(0, y - reach); yy <= Math.min(h - 1, y + reach); yy++)
        for (let xx = Math.max(0, x - reach); xx <= Math.min(w - 1, x + reach); xx++) {
          const q = yy * w + xx;
          if (seen[q]) continue;
          seen[q] = 1;
          marked.push(q);
          if (owner[q] >= 0) hits.set(owner[q], (hits.get(owner[q]) ?? 0) + 1);
        }
    }
    for (const q of marked) seen[q] = 0;
    for (const [j, n] of hits) if (n >= LINK * Math.min(piece.area, b[j].area)) out.push({ from: i, to: j, overlap: n });
  });
  return out;
}


export type Event = { kind: "split" | "shed"; parent: number; branch: number; at: number };
export type Span = {
  start: number;
  end: number;
  /// Where its centroid was first and last, and when and where it came free (a shed), px.
  y0: number;
  y1: number;
  freedAt?: number;
  freedY?: number;
  attached: boolean;
  born: "level" | "split" | "shed" | "start";
  mergedInto?: number;
  rejoined?: boolean;
};

/// Tracks every frame's pieces: `prior` the forecast step (px a frame) of a piece with no track
/// history, attached and free. Returns each piece's track, the tracks' spans and fates, the
/// events, and the steps taken by each kind.
export function follow(frames: Piece[][], w: number, h: number, reach: number, prior: { attached: [number, number]; free: [number, number] }) {
  let next = 0;
  const tracks: Track[][] = [];
  const span = new Map<number, Span>();
  const events: Event[] = [];
  const steps = { attached: [] as [number, number][], free: [] as [number, number][] };
  frames.forEach((now, t) => {
    const mine: Track[] = new Array(now.length);
    const before = t > 0 ? frames[t - 1] : [];
    const had = t > 0 ? tracks[t - 1] : [];
    const forecast = had.map((tr) => [tr.vx, tr.vy] as [number, number]);
    const ls = t > 0 ? links(before, forecast, now, w, h, reach) : [];
    // Each later piece's parent: the earlier piece whose forecast it fills most.
    const parent = new Array(now.length).fill(-1);
    const best = new Array(now.length).fill(0);
    for (const l of ls)
      if (l.overlap > best[l.to]) {
        best[l.to] = l.overlap;
        parent[l.to] = l.from;
      }
    // An earlier piece goes on as the child that fills its forecast most; its others branch.
    const heir = new Array(before.length).fill(-1);
    const heirOverlap = new Array(before.length).fill(0);
    for (const l of ls)
      if (parent[l.to] === l.from && l.overlap > heirOverlap[l.from]) {
        heirOverlap[l.from] = l.overlap;
        heir[l.from] = l.to;
      }
    now.forEach((piece, j) => {
      const i = parent[j];
      if (i >= 0 && heir[i] === j) {
        const tr = had[i];
        const step: [number, number] = [piece.cx - before[i].cx, piece.cy - before[i].cy];
        (piece.attached ? steps.attached : steps.free).push(step);
        const k = tr.steps + 1;
        mine[j] = { id: tr.id, vx: tr.steps ? tr.vx + (step[0] - tr.vx) / k : step[0], vy: tr.steps ? tr.vy + (step[1] - tr.vy) / k : step[1], steps: k, attached: piece.attached };
        const s = span.get(tr.id)!;
        s.end = t;
        s.y1 = piece.cy;
        if (before[i].attached && !piece.attached && s.freedAt === undefined) {
          events.push({ kind: "shed", parent: tr.id, branch: tr.id, at: t });
          [s.freedAt, s.freedY] = [t, piece.cy];
        }
        if (!before[i].attached && piece.attached) s.rejoined = true;
        s.attached ||= piece.attached;
        return;
      }
      const id = next++;
      let born: Span["born"] = t === 0 ? "start" : "level";
      // A branch of an attached tongue: a fork if attached too; a shed if free and above the
      // tongue it left (a blob come off its side is neither).
      if (i >= 0 && before[i].attached) {
        if (piece.attached) born = "split";
        else if (piece.cy < now[heir[i]].top) born = "shed";
        if (born !== "level") events.push({ kind: born as Event["kind"], parent: had[i].id, branch: id, at: t });
      }
      const v = i >= 0 && had[i].steps > 0 ? [had[i].vx, had[i].vy] : piece.attached ? prior.attached : prior.free;
      mine[j] = { id, vx: v[0], vy: v[1], steps: 0, attached: piece.attached };
      span.set(id, { start: t, end: t, y0: piece.cy, y1: piece.cy, attached: piece.attached, born, ...(born === "shed" ? { freedAt: t, freedY: piece.cy } : {}) });
    });
    // An earlier piece with no heir whose forecast a later piece fills merged into its track.
    for (const l of ls) {
      if (heir[l.from] >= 0) continue;
      const s = span.get(had[l.from].id)!;
      if (s.mergedInto === undefined) s.mergedInto = mine[l.to].id;
    }
    tracks.push(mine);
  });
  return { tracks, span, events, steps };
}

const meanOf = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : NaN);

/// Every track and event of the tongues in a clip's flame masks: tongues above `tongueFrom` H
/// of at least `tongueArea` H².
export function trackTongues(masks: Uint8Array[], w: number, h: number, g: Geometry, fps: number, tongueFrom: number, tongueArea: number): TongueTracks {
  const H = g.height;
  const level = Math.max(1, Math.min(h, Math.round(g.base - tongueFrom * H)));
  const least = Math.max(6, Math.round(tongueArea * H * H));
  const frames = masks.map((m) => framePieces(m, w, h, level, least, H));
  const reach = Math.max(1, Math.round(REACH * H));
  // A first pass in place, with twice the reach, gives each kind's typical step.
  const first = follow(frames, w, h, 2 * reach, { attached: [0, 0], free: [0, 0] });
  const typical = (s: [number, number][]): [number, number] => (s.length ? [0, quantile(s.map((v) => v[1]), 0.5)] : [0, 0]);
  const { span, events } = follow(frames, w, h, reach, { attached: typical(first.steps.attached), free: typical(first.steps.free) });
  const last = frames.length - 1;
  // A split counts when its branch never merges back into the track it came from; a shed when
  // its blob never joins an attached tongue again: it rises away and fades.
  const attachedTrack = (id: number | undefined) => id !== undefined && span.get(id)!.attached;
  const counted = events.filter((e) => {
    const b = span.get(e.branch)!;
    if (e.kind === "split") return b.mergedInto !== e.parent;
    return !(e.branch === e.parent ? b.rejoined : b.attached) && !attachedTrack(b.mergedInto);
  });
  const tongueSeconds = Math.max(1e-9, frames.reduce((n, f) => n + f.filter((p) => p.attached).length, 0) / fps);
  const perTongue = (kind: Event["kind"]) => counted.filter((e) => e.kind === kind).length / tongueSeconds;
  const spans = [...span.values()];
  const whole = spans.filter((s) => s.born !== "start" && s.end < last);
  const life = whole.map((s) => (s.end - s.start + 1) / fps);
  const shed = counted.filter((e) => e.kind === "shed").map((e) => span.get(e.branch)!);
  // A track's rise, H/s: its centroid's from first to last over that time, where it moved.
  const riseOf = (y0: number, y1: number, frames: number) => ((y0 - y1) * fps) / frames / H;
  const rises = spans.filter((s) => s.end > s.start && s.y0 !== s.y1).map((s) => riseOf(s.y0, s.y1, s.end - s.start));
  const leans = frames.flatMap((f) => f.filter((p) => p.attached && Number.isFinite(p.lean)).map((p) => p.lean));
  return {
    lifetime: meanOf(life),
    lifetime90: quantile(life, 0.9),
    tracks: life.length,
    sheds: perTongue("shed"),
    splits: perTongue("split"),
    merges: spans.filter((s) => s.attached && s.mergedInto !== undefined).length / tongueSeconds,
    lean: quantile(leans, 0.5),
    lean90: quantile(leans, 0.9),
    shedLife: meanOf(shed.filter((s) => s.end < last).map((s) => (s.end - s.freedAt! + 1) / fps)),
    shedRise: quantile(
      shed.filter((s) => s.end > s.freedAt! && s.freedY !== s.y1).map((s) => riseOf(s.freedY!, s.y1, s.end - s.freedAt!)),
      0.5,
    ),
    rise: quantile(rises, 0.5),
    rise90: quantile(rises, 0.9),
    births: spans.filter((s) => s.start > 0).length / (frames.length / fps),
  };
}
