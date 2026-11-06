// What each kind of asset is made to: its rig, its clips, its budget and its size. One table,
// so a new kind is a row (D136). The numbers are the iso experiment's, not main Terra's budgets
// (docs/art-style.md's are for a first-person view): an iso camera 15 to 20 m up shows a figure
// 100 to 200 pixels tall, a dozen of them at once (the decision the README names says why).

export type Kind = "hero" | "creature" | "npc" | "prop";
export const KINDS: Kind[] = ["hero", "creature", "npc", "prop"];

/** The body a rigged asset has: Tripo's humanoid rig (`biped`) or what its auto-rig reads in
 * the shape (`quadruped`, `hexapod`, `octopod`, `serpentine`, `aquatic`, `avian`…). */
export type Body = "biped" | "other";

export type KindSpec = {
  /** Whether it is rigged and animated, and as what by default. */
  body?: Body;
  /** Triangles in the exported file, at most. */
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
  prop: { triangles: 8_000, texture: 1024, height: [0.05, 10] },
};

/** A clip the game asks for by our name, the Tripo preset it is made from, and what it must be.
 * Tripo's presets for its V3 rig (developers.tripo3d.com, animations-retarget, 2026-11-05):
 * a biped has idle, walk, run, dive, climb, jump, slash, shoot, hurt, fall and turn; every other
 * body has one, a walk (`march` for serpentine and aquatic bodies). There is no attack, hit or
 * death preset by those names: `slash`, `hurt` and `fall` are the nearest. */
export type ClipSpec = { name: string; preset: string; loop: boolean; seconds: [number, number]; moves?: "walk" | "run"; ends?: "down" };

const BIPED: ClipSpec[] = [
  { name: "idle", preset: "idle", loop: true, seconds: [1, 12] },
  { name: "walk", preset: "walk", loop: true, seconds: [0.5, 3], moves: "walk" },
  { name: "run", preset: "run", loop: true, seconds: [0.3, 2], moves: "run" },
  { name: "attack", preset: "slash", loop: false, seconds: [0.4, 4] },
  { name: "hit", preset: "hurt", loop: false, seconds: [0.2, 3] },
  { name: "death", preset: "fall", loop: false, seconds: [0.4, 6], ends: "down" },
];

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
  if (kind === "prop") return `${whole.replace(" from head to foot", "")} Its shapes read from above at a distance: a clear outline and big simple masses, no thin parts that vanish.`;
  const posed = body === "biped"
    ? "Standing in a T-pose for rigging: arms straight out to the sides, hands open, legs straight and a little apart, feet flat, nothing touching between the arms and the body."
    : "Standing square on all its feet in a neutral pose for rigging, legs straight and apart, mouth closed, tail and limbs clear of the body.";
  return `${whole} ${above} ${gear} ${posed}`;
}
