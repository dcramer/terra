import assert from "node:assert/strict";
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { ROOT } from "./attach_contract.ts";
import { type Mesh, detect, detectItem, heldData } from "./detect.ts";
import type { V3 } from "./glb.ts";

// Bars, from measuring every committed detector item against the turn and grip a person set
// (2026-10-27): turns 0 or 1 degree off; grips 0.1 to 2.1 cm off (the torch's 10 cm is held
// higher, as it is carried upright), and the spear's 6.7 cm above the middle of its shaft.
const TURN_DEG = 2;
const GRIP_M = 0.025;
const POLEARM_GRIP_SHARE = 0.04;
// The arming sword's source lies with its guard and blade 7.7 to 9.3 degrees off its X; a
// person rounded its "out" to [0, 0, 1].
const OUT_DEG = 8;

const angle = (a: V3, b: V3) => (Math.acos(Math.min(1, Math.abs(a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / (Math.hypot(...a) * Math.hypot(...b)))) * 180) / Math.PI;
const off = (a: number, b: number) => Math.abs((((a - b) % 360) + 540) % 360 - 180);

test("every committed hafted, blade and polearm item: the detector finds the turn and grip a person set", () => {
  const items = readdirSync(join(ROOT, "art", "held"))
    .map((f) => f.replace(/\.json$/, ""))
    .filter((i) => ["hafted", "blade", "polearm"].includes(heldData(i).kind as string));
  assert.ok(items.length >= 9, `only ${items.length} items`);
  for (const item of items) {
    const d = heldData(item);
    const f = detectItem(item);
    assert.ok(f.ok, `${item}: ${!f.ok && f.why}`);
    if (d.fit === "length") {
      assert.ok(off(f.turn!, (d.turn as number) ?? 0) <= TURN_DEG, `${item}: turn ${f.turn}, set ${d.turn ?? 0}`);
      const bar = d.kind === "polearm" ? POLEARM_GRIP_SHARE * (d.length as number) : GRIP_M;
      assert.ok(Math.abs(f.grip_from_end! - (d.grip_from_end as number)) <= bar, `${item}: grip ${f.grip_from_end}, set ${d.grip_from_end}`);
    } else {
      const [grip, toward, out] = [d.grip, d.toward, d.out] as V3[];
      assert.ok(Math.hypot(...f.grip!.map((x, i) => x - grip[i])) <= GRIP_M, `${item}: grip ${f.grip}, set ${grip}`);
      const along = (a: V3, b: V3): V3 => [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
      assert.ok(angle(along(f.grip!, f.toward!), along(grip, toward)) <= TURN_DEG, `${item}: toward ${f.toward}, set ${toward}`);
      assert.ok(angle(f.out!, out) <= OUT_DEG, `${item}: out ${f.out}, set ${out}`);
    }
  }
});

/** Boxes [x0, x1, y0, y1, z0, z1] in metres, each 8 corners and 12 triangles. */
function boxes(...bs: number[][]): Mesh {
  const mesh: Mesh = { points: [], tris: [] };
  for (const [x0, x1, y0, y1, z0, z1] of bs) {
    const at = mesh.points.length;
    for (const x of [x0, x1]) for (const y of [y0, y1]) for (const z of [z0, z1]) mesh.points.push([x, y, z]);
    // Corners numbered x*4 + y*2 + z; two triangles a face.
    for (const [a, b, c, e] of [[0, 1, 3, 2], [4, 6, 7, 5], [0, 4, 5, 1], [2, 3, 7, 6], [0, 2, 6, 4], [1, 5, 7, 3]]) mesh.tris.push(at + a, at + b, at + c, at + a, at + c, at + e);
  }
  return mesh;
}

/** Turned `deg` about +Y. */
function turned(m: Mesh, deg: number): Mesh {
  const [c, s] = [Math.cos((deg * Math.PI) / 180), Math.sin((deg * Math.PI) / 180)];
  return { points: m.points.map(([x, y, z]) => [x * c + z * s, y, -x * s + z * c] as V3), tris: m.tris };
}

const HANDLE = [-0.015, 0.015, 0, 0.6, -0.015, 0.015];
/** An axe of boxes: a 3 cm haft, its bit reaching 15 cm toward -X and 6 toward +X. */
const AXE = boxes(HANDLE, [-0.15, 0.06, 0.55, 0.68, -0.02, 0.02]);

test("an axe made of boxes: its bit turned to -Z, wherever it starts, and gripped a fist up its haft", () => {
  for (const deg of [0, 30, -45, 120]) {
    const f = detect(turned(AXE, deg), "hafted", "length", 0.68);
    assert.ok(f.ok, !f.ok ? f.why : "");
    // Its bit starts at -X, which is -90 degrees from +Z toward +X; turned by deg it is at -90 + deg.
    assert.ok(off(f.turn!, -90 - deg) <= 1, `turned ${deg}: ${f.turn}`);
    assert.ok(Math.abs(f.grip_from_end! - 0.075) <= 0.01, `grip ${f.grip_from_end}`);
  }
});

test("the detector refuses what it cannot decide, naming the measure", () => {
  const refused = (m: Mesh, kind: string, fit: string, length: number | undefined, why: RegExp) => {
    const f = detect(m, kind, fit, length);
    assert.ok(!f.ok, `found ${JSON.stringify(f)}`);
    assert.match(f.why, why);
  };
  refused(boxes([0, 0.7, 0, 0.1, 0, 0.05]), "hafted", "length", 0.7, /stands along \+Y/);
  refused(boxes([0, 0.1, 0, 0.08, 0, 0.06]), "blade", "points", undefined, /long axis, 10\.0 cm, is under 3 times/);
  refused(boxes([-0.1, 0.1, 0, 0.6, -0.01, 0.01]), "hafted", "length", 0.6, /no handle/);
  refused(boxes(HANDLE, [-0.15, 0.06, 0, 0.1, -0.02, 0.02], [-0.15, 0.06, 0.5, 0.6, -0.02, 0.02]), "hafted", "length", 0.6, /near its middle/);
  refused(boxes(HANDLE, [-0.15, 0.06, 0, 0.13, -0.02, 0.02]), "hafted", "length", 0.6, /stands on its working end/);
  refused(boxes(HANDLE, [-0.025, 0.025, 0.55, 0.68, -0.005, 0.005]), "hafted", "length", 0.68, /head's section, 5\.\d cm, is under 1\.5 times/);
  refused(boxes(HANDLE, [-0.03, 0.03, 0.55, 0.68, -0.02, 0.02]), "blade", "length", 0.68, /blade is not flat/);
  refused(boxes(HANDLE, [-0.1075, 0.0925, 0.55, 0.68, -0.02, 0.02]), "hafted", "length", 0.68, /neither one-sided by 2\.0 cm nor even within 1\.0 cm/);
  refused(AXE, "bow", "length", 0.68, /a bow is not measured/);
});
