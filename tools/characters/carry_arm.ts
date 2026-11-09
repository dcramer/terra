// The carried arm held in the body's frame (D560): a carry pose (a look's `carry_hold`, laid on
// the arm holding it by the hold layer, terra_walker::layer) keeps its turn against the body,
// not the chest, so a torch held flame up stays up through a sprint's lean, a crouch's hunch and
// a jump's landing. It is the mesh space rotation blend shipped overlay layers lay an arm with
// (Unreal's Layered Blend per Bone "Mesh Space Rotation Blend", which ALS's overlays use for
// the arms, docs only): the shoulder blade's turn in the body is the carry pose's on the chest
// as it stands at rest, whatever the clip does to the chest; the shoulder still goes where the
// chest takes it.
//
// An upright pose's arm (the torch's) is held so and no more. A hanging one (a tool's, a
// sword's, a knife's) is held so too and gives as the legs fold (`fold`): the elbow bends and
// the arm comes out from the side, so a landing's crouch, which brings the hand down to the
// knees, lifts the look clear of the ground and the legs instead of driving a pickaxe 34.6 cm
// under it. Twin of terra_walker::carry_arm, whose numbers it reads; change both.
// Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { type Glb, type Pose, type Q, len, qinv, qmul, sub, turn } from "./glb.ts";

/** The give's numbers, read from the game's source (terra_walker::carry_arm) so the two
 * cannot differ: the elbow's and the arm's turn (degrees) with the legs folded all the way,
 * and the fold a give starts at and is whole at. */
export const GIVE = (() => {
  const source = readFileSync(join(import.meta.dirname, "../../crates/terra_walker/src/carry_arm.rs"), "utf8");
  const of = (name: string) => {
    const m = source.match(new RegExp(`pub const ${name}: f32 = ([0-9.]+);`));
    if (!m) throw new Error(`carry_arm.ts: no ${name} in crates/terra_walker/src/carry_arm.rs: the give's numbers are read from it; keep the constant or change both`);
    return Number(m[1]);
  };
  return { elbow: of("GIVE_ELBOW_DEG"), out: of("GIVE_OUT_DEG"), from: of("FOLD_FROM"), to: of("FOLD_TO") };
})();

/** How far the legs are folded at `pose`, 0 standing to 1: of the straighter leg (each its
 * hip, knee and ankle), how much shorter hip to ankle is than thigh and shin laid end to end,
 * from `GIVE.from` to `GIVE.to`, eased. It has no scale, so it is one on every body. */
export function fold(pose: Pose[], legs: readonly (readonly [number, number, number])[]): number {
  const bent = Math.min(
    ...legs.map(([hip, knee, ankle]) => 1 - len(sub(pose[hip].t, pose[ankle].t)) / (len(sub(pose[hip].t, pose[knee].t)) + len(sub(pose[knee].t, pose[ankle].t)))),
  );
  const x = Math.max(0, Math.min(1, (bent - GIVE.from) / (GIVE.to - GIVE.from)));
  return x * x * (3 - 2 * x);
}

/** The turns, in the body, that hold each carried arm at `pose`: at each of `upright` and
 * `hanging` (a carried arm's clavicle, by its carry pose's kind) the chest's turn from its rest
 * (`rest`, the body's rest pose) undone, as hold.py's and posed's bends turn a bone with what
 * is below it; and a hanging arm's give by the legs' fold, at its upper arm and forearm. */
export function carryArm(body: Glb, rest: Pose[], upright: readonly string[], hanging: readonly string[] = []): (pose: Pose[]) => Map<string, Q> {
  const parentOf = new Map<number, number>();
  body.gltf.nodes.forEach((n, i) => n.children?.forEach((c) => parentOf.set(c, i)));
  const chests = [...upright, ...hanging].map((root) => [root, parentOf.get(body.named(root))!] as const);
  const legs = hanging.length ? ["l", "r"].map((s) => [body.named(`thigh_${s}`), body.named(`calf_${s}`), body.named(`foot_${s}`)] as const) : [];
  return (pose) => {
    const turns = new Map<string, Q>(chests.map(([root, chest]) => [root, qmul(rest[chest].r, qinv(pose[chest].r))]));
    if (!hanging.length) return turns;
    const g = fold(pose, legs);
    for (const root of hanging) {
      const s = root.slice(-1);
      // The model faces +Z, +X its left: a right arm goes out from the side turning about -Z.
      turns.set(`upperarm_${s}`, turn([0, 0, (s === "r" ? -1 : 1) * GIVE.out * g]));
      turns.set(`lowerarm_${s}`, turn([-GIVE.elbow * g, 0, 0]));
    }
    return turns;
  };
}
