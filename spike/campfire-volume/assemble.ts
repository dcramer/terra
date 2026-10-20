// Builds the spike's fire.wesl from main's: volume.wesl put in before `place`, a hearth's
// third card (its first column) made the volume's quad and its other columns and tongues
// hidden, and the quad's fragment marched (writing the film mask in the mask pass). Torches,
// the ground card, sparks, smoke and the light are main's. Each edit must match once.
// Usage: node assemble.ts <main fire.wesl> <out>
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
const [inp, out] = process.argv.slice(2);
let s = readFileSync(inp, "utf8");
const volume = readFileSync(join(import.meta.dirname, "volume.wesl"), "utf8");
const edits: [string, string][] = [
  ["fn place(corner: vec2<f32>, id: vec2<f32>) -> Out {", `${volume}\nfn place(corner: vec2<f32>, id: vec2<f32>) -> Out {`],
  [
    `    if kind == 0 && is_hearth {
        var tongue`,
    `    if kind == 0 && is_hearth && tall >= TORCH_M {
        // SPIKE: the first column the volume's quad, the other flame cards gone.
        let j = i32((phase - 1.0) * HEARTH_CARDS + 0.5);
        if j == 2 {
            return campfire_card(corner, foot, up, slot, heat);
        }
        if j > 2 {
            return nowhere();
        }
    }
    if kind == 0 && is_hearth {
        var tongue`,
  ],
  [
    `    // Faded in over the last of its way in front of what is behind it (soft particles).
    var soft = 1.0;`,
    `    if i32(fire.kind.x + 0.5) == 0 && in.uv_b.x > 5.5 {
        out.color = campfire_volume(in);
@if(FIRE_MASK)
        {
            out.color = vec4(0.0, 0.0, 0.0, saturate(CF_SHARE));
        }
        return out;
    }
    // Faded in over the last of its way in front of what is behind it (soft particles).
    var soft = 1.0;`,
  ],
];
for (const [a, b] of edits) {
  const n = s.split(a).length - 1;
  if (n !== 1) throw new Error(`edit matched ${n} times: ${a.slice(0, 60)}`);
  s = s.replace(a, b);
}
writeFileSync(out, s);
