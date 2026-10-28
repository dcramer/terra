// The carried arm held in the body's frame (D560): a carry pose (a look's `carry_hold`, laid on
// the arm holding it by the hold layer, terra_walker::layer) keeps its turn against the body,
// not the chest, so a torch held flame up stays up through a sprint's lean, a crouch's hunch and
// a jump's landing. It is the mesh space rotation blend shipped overlay layers lay an arm with
// (Unreal's Layered Blend per Bone "Mesh Space Rotation Blend", which ALS's overlays use for
// the arms, docs only): the shoulder blade's turn in the body is the carry pose's on the chest
// as it stands at rest, whatever the clip does to the chest; the shoulder still goes where the
// chest takes it. Twin of terra_walker::carry_arm; change both.
// Node 22.18 or later; no dependencies.

import { type Glb, type Pose, type Q, qinv, qmul } from "./glb.ts";

/** The turn, in the body, laid at each of `roots` (a carried arm's clavicle) that holds it as
 * the carry pose does on the chest at rest (`rest`, the body's rest pose): the chest's turn
 * from its rest undone, as hold.py's and posed's bends turn a bone with what is below it. */
export function carryArm(body: Glb, rest: Pose[], roots: readonly string[]): (pose: Pose[]) => Map<string, Q> {
  const parentOf = new Map<number, number>();
  body.gltf.nodes.forEach((n, i) => n.children?.forEach((c) => parentOf.set(c, i)));
  const chests = roots.map((root) => [root, parentOf.get(body.named(root))!] as const);
  return (pose) => new Map(chests.map(([root, chest]) => [root, qmul(rest[chest].r, qinv(pose[chest].r))]));
}
