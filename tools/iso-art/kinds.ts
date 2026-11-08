// What each kind of asset is made to: its rig, its clips, its budget and its size. One table,
// so a new kind is a row (D136). The numbers are the iso experiment's, not main Terra's budgets
// (docs/art-style.md's are for a first-person view): an iso camera 15 to 20 m up shows a figure
// 100 to 200 pixels tall, a dozen of them at once (the decision the README names says why).

export type Kind = "hero" | "creature" | "npc" | "prop" | "scenery";
export const KINDS: Kind[] = ["hero", "creature", "npc", "prop", "scenery"];

/** The body a rigged asset has: Tripo's humanoid rig (`biped`) or what its auto-rig reads in
 * the shape (`quadruped`, `hexapod`, `octopod`, `serpentine`, `aquatic`, `avian`…). */
export type Body = "biped" | "other";

export type KindSpec = {
  /** Whether it is rigged and animated, and as what by default. */
  body?: Body;
  /** Triangles in the exported file, at most, whatever its size: the kind's share of the
   * frame (see `budget`). */
  triangles: number;
  /** A texture's side in pixels, at most, and what `export` asks Tripo for. */
  texture: number;
  /** Height in metres it may have, and the one it is made to when none is given. */
  height: [number, number];
  defaultHeight?: number;
};

export const SPECS: Record<Kind, KindSpec> = {
  hero: { body: "biped", triangles: 24_000, texture: 2048, height: [1.5, 2.2], defaultHeight: 1.85 },
  npc: { body: "biped", triangles: 16_000, texture: 1024, height: [1.3, 2.1], defaultHeight: 1.75 },
  creature: { body: "other", triangles: 24_000, texture: 2048, height: [0.2, 8] },
  prop: { triangles: 8_000, texture: 1024, height: [0.05, 3] },
  // Trees, ruins, wagons' worth of set piece taller than a figure: the largest things on screen
  // (a 7 m tree is 800 px tall at the closest camera), a dozen in view.
  scenery: { triangles: 20_000, texture: 2048, height: [3, 30] },
};

/** Pixels a metre at the closest game camera: the hero, 1.85 m, at most 0.20 of a 1080-pixel
 * frame, the most the 14 store frames of shipped iso ARPGs showed
 * (research/2026-11-05-iso-arpg-art-pipeline.md). */
export const PX_PER_M = (1080 * 0.2) / 1.85;
/** The fewest triangles a budget comes to, so a small thing keeps its shape. */
const FLOOR = 1_000;

/** Triangles an asset of this kind and size may have: one a pixel of its box at the closest
 * camera (the larger of its front, height by width, and its top, width by depth, as the camera
 * looks down on it), and at most the kind's cap. About half a box is covered and half the
 * triangles face away, so this is one visible triangle a covered pixel: past it triangles are
 * under a pixel, which the GPU shades in 2x2 quads several times over without a pixel more of
 * shape (Unreal's quad overdraw view; the cost Nanite was made for). The caps are each kind's
 * share of about a million triangles a frame at 1080p, by how many are on screen at once:
 * four heroes at 24k, a dozen creatures at 24k, eight NPCs at 16k, forty props at 8k, a dozen
 * pieces of scenery at 20k (an estimate for a bench of the ruin scene to confirm, D185). */
export function budget(kind: Kind, box: { height: number; width: number; depth: number }): number {
  const area = Math.max(box.height * box.width, box.width * box.depth);
  return Math.min(SPECS[kind].triangles, Math.max(FLOOR, Math.round((area * PX_PER_M * PX_PER_M) / 100) * 100));
}

/** A clip the game asks for by our name, the Tripo preset it is made from, and what it must be.
 * Tripo's presets for its V3 rig (developers.tripo3d.com, animations-retarget, 2026-11-05):
 * a biped has idle, walk, run, dive, climb, jump, slash, shoot, hurt, fall and turn; every other
 * body has one, a walk (`march` for serpentine and aquatic bodies). There is no attack, hit or
 * death preset by those names: `slash` and `fall` are the nearest. `hurt` is no hit: on the
 * knight (2026-11-05) it is 13.8 s lying wounded on the ground from its first frame, so a hit is
 * the game's flinch and hit-stop, or a `--prompt` motion (20 credits).
 *
 * `window` cuts a clip to the part the game plays, in the preset's own seconds: `slash` is 6.6 s,
 * a wind-up from 0.75 s, the swing at 1.5 to 2 s and a slow return to guard from 2.75 to 5.5 s,
 * which an ARPG's attack does not wait for (the game blends back to idle). */
export type ClipSpec = { name: string; preset: string; loop: boolean; seconds: [number, number]; moves?: "walk" | "run"; ends?: "down"; window?: [number, number] };

const BIPED: ClipSpec[] = [
  // Tripo's idle is 15.3 s, a still stance with two fidgets in it: a long idle is a usual one.
  { name: "idle", preset: "idle", loop: true, seconds: [1, 20] },
  { name: "walk", preset: "walk", loop: true, seconds: [0.5, 3], moves: "walk" },
  { name: "run", preset: "run", loop: true, seconds: [0.3, 2], moves: "run" },
  { name: "attack", preset: "slash", loop: false, seconds: [0.4, 4], window: [0.75, 2.75] },
  { name: "death", preset: "fall", loop: false, seconds: [0.4, 6], ends: "down" },
];

/** A biped on the game's skeleton (`convert`) plays the game's own clips (art/animation/clips.txt,
 * Quaternius's libraries, re-sampled on its skeleton by tools/characters/export_character.py)
 * under the names iso asks for, not Tripo's presets: the developer, 2026-11-08, "better
 * animations ... (no auto magic)". Each is in place and loops as made, so no cut is needed. The
 * attack is the asset's: `Sword_Attack` for anything held, `Punch_Cross` for a brawler's fists. */
export const GAME_CLIPS: Record<string, string> = {
  idle: "Idle_Loop",
  walk: "Walk_Loop",
  run: "Jog_Fwd_Loop",
  attack: "Sword_Attack",
  hit: "Hit_Chest",
  death: "Death01",
};
export const UNARMED_ATTACK = "Punch_Cross";

/** The one preset every other body Tripo rigs has. */
const MARCH = new Set(["serpentine", "aquatic"]);

/** The clips an asset gets, by its rig's type as Tripo reports it (`biped`, `quadruped`…). */
export function clipsFor(rigType: string | undefined): ClipSpec[] {
  if (!rigType) return [];
  if (rigType === "biped") return BIPED;
  return [{ name: "walk", preset: MARCH.has(rigType) ? "march" : "walk", loop: true, seconds: [0.4, 4], moves: "walk" }];
}

/** The concept's iso part, between the subject and the style guide's paragraph: what a model
 * for this view needs that a model for a first-person view does not. */
export function isoPart(kind: Kind, body: Body | undefined): string {
  const whole = "The whole subject from head to foot, seen straight from the front at eye level, as a model sheet.";
  const above = "Its shapes read from above at a distance: a clear outline, big simple masses on the head, shoulders and back, light and dark parts set against each other, no thin parts that vanish.";
  const gear = "Everything it wears and carries is part of the one figure and fixed to it: armour and clothes close to the body, a weapon held firmly in a hand or sheathed, a shield strapped on, nothing loose, floating or hanging free.";
  if (!SPECS[kind].body) return `${whole.replace(" from head to foot", "")} Its shapes read from above at a distance: a clear outline and big simple masses, no thin parts that vanish.`;
  const posed = body === "biped"
    ? "Standing in a T-pose for rigging: arms straight out to the sides, hands open, legs straight and a little apart, feet flat, nothing touching between the arms and the body."
    : "Standing square on all its feet in a neutral pose for rigging, legs straight and apart, mouth closed, tail and limbs clear of the body.";
  return `${whole} ${above} ${gear} ${posed}`;
}
