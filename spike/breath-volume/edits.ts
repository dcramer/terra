// The spike's small edits to main's fire.wesl, each an exact replacement that must match once.
import { readFileSync, writeFileSync } from "node:fs";
const [inp, out] = process.argv.slice(2);
let s = readFileSync(inp, "utf8");
const edits: [string, string][] = [
  [
    `    // The fireball where it strikes is the sprites' (\`breath_sprite\`), no longer marched.
    b.ball = 0.0;`,
    `    // SPIKE: the fireball marched again, grown over the 0.6 s after the stream's front
    // reaches the face it strikes, gone with the stream once off.
    let struck = select(0.0, saturate((flown - b.reach) / (JET_SPEED * 0.6)), on && b.face > 0.0);
    b.ball = select(struck, b.left, !on && b.face > 0.0);`,
  ],
  ["const ROD_WIDTH: f32 = 0.2;\nconst ROD_OFF: bool = true;", "const ROD_WIDTH: f32 = 1.0;\nconst ROD_OFF: bool = false;"],
  [
    `        placed = hull(vertex.position, vertex.uv_b);
    } else {`,
    `        placed = hull(vertex.position, vertex.uv_b);
    } else if i32(fire.kind.x + 0.5) == 5 {
        // SPIKE: the breath's sprites hidden; the volume is the whole breath.
        placed = nowhere();
    } else {`,
  ],
  [
    `        out.color = breath(in);
        return out;`,
    `        out.color = breath(in);
@if(FIRE_MASK)
        {
            out.color = vec4(0.0, 0.0, 0.0, saturate(FLAME_SHARE));
        }
        return out;`,
  ],
];
for (const [a, b] of edits) {
  const n = s.split(a).length - 1;
  if (n !== 1) throw new Error(`edit matched ${n} times: ${a.slice(0, 60)}`);
  s = s.replace(a, b);
}
writeFileSync(out, s);
