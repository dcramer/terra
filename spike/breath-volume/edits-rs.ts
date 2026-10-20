// The spike's edit to crates/terra_world/src/fire.rs (130d62b3): the breaths' hulls drawn a
// second time into the picture's alpha only, so the tonemapper takes the marched flame through
// the film as it does a hearth's (crates/terra_world/src/fire/film.rs). With main's fire.wesl
// the extra hull discards every fragment (its rod is off), so the binary draws main.
// Usage: node edits-rs.ts <fire.rs>
import { readFileSync, writeFileSync } from "node:fs";
const [path] = process.argv.slice(2);
let s = readFileSync(path, "utf8");
const edits: [string, string][] = [
  ["const KINDS: usize = 7;", "const KINDS: usize = 8;"],
  [
    "const MASK: usize = 6;",
    "const MASK: usize = 6;\n/// SPIKE (spike/breath-volume): the breaths' hulls drawn again, into the alpha only.\nconst JET_MASK: usize = 7;",
  ],
  ["    if material == MASK {", "    if material == JET_MASK {\n        2.0\n    } else if material == MASK {"],
  ["                mask: i == MASK,", "                mask: i == MASK || i == JET_MASK,"],
  ["                depth_bias: if kind == 2.0 {", "                depth_bias: if i == JET_MASK {\n                    3.0\n                } else if kind == 2.0 {"],
  [
    `    spawn("Fire: jets", hulls(JETS), fire[2].clone());`,
    `    spawn("Fire: jets", hulls(JETS), fire[2].clone());\n    spawn("Fire: jets' mask", hulls(JETS), fire[JET_MASK].clone());`,
  ],
];
for (const [a, b] of edits) {
  const n = s.split(a).length - 1;
  if (n !== 1) throw new Error(`edit matched ${n} times: ${a.slice(0, 60)}`);
  s = s.replace(a, b);
}
writeFileSync(path, s);
