// build_bodies.sh's verdict: every failure any of its steps can print, read from the whole
// build's output (the morphs' logs are in it), the base's rig check and whether the build ran
// to its end. A step that prints nothing when it fails is caught by its exit: tools/blender/run.sh
// says when a Blender script exits non-zero.
//
//   node tools/characters/build_verdict.ts BUILD.log RIG-CHECK.json [STATUS]
//
// Prints BUILD pass, or BUILD fail and each failure (to 160 characters); exits 1 on a failure.

import { existsSync, readFileSync } from "node:fs";

/** What the build's own steps print when something failed. */
const FAILED: RegExp[] = [
  /^(CLIPCHECK|GEARCHECK) .*"pass": false/, // a clip, or a look on a body
  /^CLIPCHECK fail/,
  /^MORPH \S+ failed/, // a morph's job exited non-zero (build_bodies.sh step 3)
  /FLOATING/, // a rigid look resting on nothing (fit_gear.py)
  /^✗ /, // a clip's foot-down markers (markers.ts)
  /^run\.sh: .* exited \d+/, // a Blender script that failed, even silently (SystemExit, sys.exit)
  /Traceback|Error/, // Python's, and Blender's "Error: script failed"
];

export function verdict(log: string, rig: string | undefined, status = 0): string[] {
  const lines = log.split("\n");
  const out = [...new Set(lines.filter((l) => FAILED.some((re) => re.test(l))).map((l) => l.slice(0, 160)))];
  if (!lines.some((l) => /^CLIPCHECK (pass|fail)\b/.test(l))) out.push("CLIPCHECK never finished: no CLIPCHECK pass or fail line");
  // Its last line names the morphs: the base and each has its looks checked, a GEARCHECKS line.
  const built = lines.find((l) => l.startsWith("BUILT base.glb with morphs "));
  if (!built) out.push("the build never wrote the game's files: no BUILT line");
  else {
    const bodies = 1 + built.slice("BUILT base.glb with morphs ".length).split(" ")[0].replace(/,$/, "").split(",").filter(Boolean).length;
    const checked = lines.filter((l) => l.startsWith("GEARCHECKS ")).length;
    if (checked !== bodies) out.push(`GEARCHECK finished on ${checked} bodies of ${bodies}: a body's looks went unchecked`);
  }
  if (rig === undefined) out.push("RIGCHECK missing: the base's rig check wrote no report");
  else {
    const r = JSON.parse(rig) as { file: string; pass: boolean; checks: Record<string, boolean> };
    if (!r.pass)
      out.push(`RIGCHECK ${r.file} fails: ${Object.entries(r.checks).filter(([, ok]) => !ok).map(([c]) => c).join(", ")}`);
  }
  if (status !== 0) out.push(`the build stopped before its end (exit ${status}): read the log's last lines`);
  return out;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [logPath, rigPath, status] = process.argv.slice(2);
  if (!logPath || !rigPath) {
    console.log("build_verdict.ts BUILD.log RIG-CHECK.json [STATUS]");
    process.exit(2);
  }
  const failed = verdict(readFileSync(logPath, "utf8"), existsSync(rigPath) ? readFileSync(rigPath, "utf8") : undefined, Number(status ?? 0));
  console.log(failed.length ? `BUILD fail\n${failed.join("\n")}` : "BUILD pass");
  process.exit(failed.length ? 1 : 0);
}
