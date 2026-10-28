// The character build's steps as data (D71): what build_bodies.sh and build_item.sh run, each
// step with what it reads and writes, so build.ts can skip a step whose inputs are unchanged and
// rerun only what a change reaches, as make and ninja do. Read from the bodies (bodies.json) and
// each worn item's data (art/gear/<item>/fit.json, gear_list.ts), so adding a body or an item
// is adding its data, never editing a step.
//
// A step's inputs are files or folders (a folder is every file under it); the scripts named in
// them bring the local modules they import and the data those modules read (MODULE_DATA), so a
// change to look.py reaches every Blender step and a change to fit.json only that item's steps.

import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { type GearData, garmentLine, garmentOrder, gearData, rigidPieces } from "./gear_list.ts";

const ROOT = join(import.meta.dirname, "..", "..");
const HERE = "tools/characters";
const OUT = `${HERE}/out`;

export type Step = {
  name: string;
  /** Its part of build.log: 0 the clips, 1 the base, 2 its gear, 3 the morphs, 4 the skins, 5 the game's files. */
  section: number;
  /** The steps whose outputs it reads. */
  needs: string[];
  /** Steps it waits for when they run, reading nothing of theirs (ninja's order-only): those that read a file before it changes it in place. */
  after?: string[];
  /** Files and folders it reads beyond its needs' outputs, scripts included (their imports are followed). */
  inputs: string[];
  /** Files it writes: removed before it runs, so one it did not write is caught. */
  outputs: string[];
  /** Files it changes where they are (the mannequin's clips): its outputs, kept when it runs. */
  inPlace?: string[];
  /** Its in-place outputs are committed, and rewriting them is not idempotent (Blender's glTF
   * round trip moves the mannequin's floats a little each time): while git has them and every
   * input as committed, they are taken as built from those inputs and the step runs with
   * ADOPT=1, checking without rewriting. */
  committed?: boolean;
  /** Bash, run at the repository's root with $blender, $here and $out set; set -euo pipefail. */
  run: string;
};

export const SECTIONS = ["== 0. the clips", "== 1. the base", "== 2. its gear", "== 3. the morphs, side by side", "== 4. the skins", "== 5. the game's files"];

/** What a module reads besides its imports, followed with them (look.py's is read by every Blender script). */
export const MODULE_DATA: Record<string, string[]> = {
  "tools/characters/look.py": ["assets/models/character/skinning.json", "art/animation/clips.txt", "tools/blender/run.sh", "tools/blender/version", "assets/models/character/mannequin.glb"],
  "tools/characters/clips.py": ["art/animation/library", "art/animation/made.json"],
  "tools/characters/made_clips.py": ["art/animation/made", "art/animation/made.json", "tools/characters/glb_clips.ts"],
  "tools/characters/export_character.py": ["art/animation/made.json"],
  "tools/characters/hold.py": ["art/animation/shield_targets.json"],
  "tools/characters/rig_check.py": ["art/characters/rig-baseline.json"],
  "tools/characters/fit_gear.py": ["art/gear/harnesses.json"],
  "tools/characters/mirror.ts": ["art/animation/mirrors.json"],
  "tools/characters/clear.ts": ["art/animation/made.json", "art/animation/sets/fight.json", "art/animation/mirrors.json"],
};

export type Bodies = {
  base: { name: string; height: number };
  designed: string[];
  rigged: { morph: string; body: string; height: number }[];
  skins: { morph: string; front: string }[];
  /** A designed morph's model where it is not art/characters/<base>/morphs/<name>/model.glb (make_morph.sh --design); not in bodies.json. */
  designs?: Record<string, string>;
};

export function bodiesData(root = ROOT): Bodies {
  return JSON.parse(readFileSync(join(root, HERE, "bodies.json"), "utf8"));
}

/** Every morph's name, in the order the game's files carry them. */
export function morphNames(b: Bodies): string[] {
  return [...b.designed, ...b.rigged.map((r) => r.morph)].sort();
}

const blender = (script: string, args: string, keep: string) => `$blender $here/${script} ${args} | grep -E "${keep}" || true`;

/** A look carried onto a body and pushed clear where its skin comes through in our clips, in three rounds, then checked. */
function fitRun(base: string, body: string, item: string): string {
  const carry = blender("carry_gear.py", `"$out/${base}-game.glb" "$out/gear/${base}/${item}.glb" "$out/${body}-${item}.glb" "${body}"`, "PUSHED|Error");
  const check = (fix: boolean) => blender("gear_check.py", `"$out/${body}-game.glb" "$out/${body}-${item}.glb"${fix ? " --fix" : ""}`, fix ? "^FIX|Error" : "^GEARCHECK|Error");
  return [`rm -f "$out/${body}-${item}-push.npz" "$out/${body}-${item}-uncover.json"`, carry, "for round in 1 2 3; do", `  ${check(true)}`, `  ${carry}`, "done", check(false)].join("\n");
}

/** The steps of the whole build, in build.log's order; `items` the worn looks' data (gear_list.ts). */
export function buildSteps(b: Bodies, items: Map<string, GearData>): Step[] {
  const base = b.base.name;
  const steps: Step[] = [];
  const py = (...names: string[]) => names.map((n) => `${HERE}/${n}`);
  const morphs = morphNames(b);
  const garments = garmentOrder(items);
  const rigid = rigidPieces(items);
  const looks = [...garments, ...rigid].sort();
  const finish = (name: string, height: number, art: string): Omit<Step, "name" | "section" | "needs"> => ({
    inputs: [...py("finish_character.sh", "convert_rig.py", "lift.py", "rig_check.py", "rig_render.py", "export_character.py"), art],
    outputs: [`${OUT}/${name}.glb`, `${OUT}/rig-check-${name}.json`, `${OUT}/${name}-game.glb`],
    run: `$here/finish_character.sh ${name} ${height.toFixed(2)}`,
  });

  steps.push({
    name: "clips",
    section: 0,
    needs: [],
    inputs: [...py("clips.py", "made_clips.py", "clip_check.py", "mirror.ts")],
    outputs: [],
    inPlace: ["assets/models/character/mannequin.glb"],
    committed: true,
    run: [
      "if [[ -z ${ADOPT:-} ]]; then",
      "  if ls art/animation/library/*.glb >/dev/null 2>&1; then",
      `    ${blender("clips.py", "", "CLIPS|Error|not in")}`,
      "  fi",
      `  ${blender("made_clips.py", "", "^MADE|^CLIPS|Error|no clean stride")}`,
      "  node $here/mirror.ts assets/models/character/mannequin.glb    # a made clip's mirror made again from it",
      "fi",
      `$blender $here/clip_check.py | grep -E "^CLIPCHECK (pass|fail)|\\"pass\\": false|Error" | tee $out/build-clips.log || true`,
    ].join("\n"),
  });
  const baseStep = finish(base, b.base.height, `art/characters/${base}/rigged.glb`);
  steps.push({ name: "base", section: 1, needs: ["clips"], ...baseStep, outputs: [...baseStep.outputs, `${OUT}/lift-mannequin.json`] });

  // The morphs: each body the base registered onto (morph.py) and exported as the game loads it.
  // A rigged design's own body is finished first; it is only the shape her morph is fitted to.
  for (const r of b.rigged) steps.push({ name: `body:${r.body}`, section: 3, needs: ["base"], ...finish(r.body, r.height, `art/characters/${r.body}/rigged.glb`) });
  for (const m of [...b.designed, ...b.rigged.map((r) => r.morph)]) {
    const rigged = b.rigged.find((r) => r.morph === m);
    const model = b.designs?.[m] ?? `art/characters/${base}/morphs/${m}/model.glb`;
    const design = rigged ? `"$out/${rigged.body}-game.glb"` : `"${model}"`;
    steps.push({
      name: `morph:${m}`,
      section: 3,
      needs: rigged ? ["base", `body:${rigged.body}`] : ["base"],
      inputs: [...py("morph.py", "export_character.py"), ...(rigged ? [] : [model])],
      outputs: [`${OUT}/${m}.glb`, `${OUT}/${m}.npz`, `${OUT}/${m}-game.glb`],
      run: [
        blender("morph.py", `"$out/${base}-game.glb" ${design} "${m}"`, "MORPH|EXPORTED|Error"),
        blender("export_character.py", `"$out/${m}.glb" "$out/${m}-game.glb"`, "PATCHES|EXPORTED|Error"),
      ].join("\n"),
    });
  }

  // Each worn look made on the base from its data: a garment after what it is worn over or
  // under, and on the female's shape (her morph) when drawn on her; the rigid pieces together.
  for (const g of garments) {
    const d = items.get(g)!;
    const [, maker, role, coverage, dressed, ...flags] = garmentLine(g, d).split(" ");
    const script = maker === "hung" ? "hung_garment.py" : "shell_garment.py";
    const keep = maker === "hung" ? "^HUNG .*(hides|vertices)|EXPORTED|Error" : "^SHELL .*hides|EXPORTED|Error";
    steps.push({
      name: `garment:${g}`,
      section: 2,
      needs: ["base", ...(d.shape ? [`morph:${d.shape}`] : []), ...[...(d.over ?? []), ...(d.under ?? [])].map((n) => `garment:${n}`)],
      inputs: [...py(script), `art/gear/${g}/fit.json`, dressed],
      outputs: [`${OUT}/gear/${base}/${g}.glb`],
      run: blender(script, [`"$out/${base}-game.glb"`, dressed, g, role, coverage, ...flags].join(" "), keep),
    });
  }
  steps.push({
    name: "rigid",
    section: 2,
    needs: ["base"],
    inputs: [...py("fit_gear.py"), ...rigid.flatMap((r) => [`art/gear/${r}`, ...(items.get(r)!.model ? [`art/gear/${items.get(r)!.model}`] : [])])],
    outputs: rigid.map((r) => `${OUT}/gear/${base}/${r}.glb`),
    run: blender("fit_gear.py", [`"$out/${base}-game.glb"`, ...rigid].join(" "), "^FIT|FITTED|Error"),
  });
  const made = (item: string) => (garments.includes(item) ? `garment:${item}` : "rigid");

  // Every look on every body, pushed clear and checked: the base's in its gear, each morph's with it.
  for (const body of [base, ...b.designed, ...b.rigged.map((r) => r.morph)])
    for (const item of looks)
      steps.push({
        name: `fit:${body}:${item}`,
        section: body === base ? 2 : 3,
        needs: ["base", made(item), ...(body === base ? [] : [`morph:${body}`])],
        inputs: [...py("carry_gear.py", "gear_check.py"), `art/gear/${item}/fit.json`],
        outputs: [`${OUT}/${body}-${item}.glb`, `${OUT}/${body}-${item}-push.npz`, `${OUT}/${body}-${item}-uncover.json`],
        run: fitRun(base, body, item),
      });

  // A template's skin baked onto its body (paint_skin.py `in`): the game's PNG, and the body
  // wearing it to look at (out/M-skinned.glb). The body it reads stays as the morph wrote it, so
  // the skin reaches nothing else: the game loads the PNG by itself. The PNG keeps no dates.
  for (const s of b.skins)
    steps.push({
      name: `skin:${s.morph}`,
      section: 4,
      needs: [`morph:${s.morph}`],
      inputs: [...py("paint_skin.sh", "paint_skin.py"), s.front, `art/characters/${base}/morphs/${s.morph}/skin.glb`],
      outputs: [`${OUT}/${s.morph}-skin.png`, `assets/models/character/skins/${s.morph}.png`, `${OUT}/${s.morph}-skinned.glb`],
      run: [
        "mkdir -p assets/models/character/skins",
        `$here/paint_skin.sh ${s.morph} ${s.front}`,
        `convert "$out/${s.morph}-skin.png" -resize "2048x2048>" -define png:exclude-chunk=date,time,tEXt,zTXt,iTXt "assets/models/character/skins/${s.morph}.png"   # the body budget (D188); no dates, so the same bytes`,
      ].join("\n"),
    });

  // The game's files: the base with every morph, the clips mirrored and their foot markers
  // baked; each look with the same morphs, hiding no patch that opened on any body.
  const all = morphs.join(",");
  steps.push({
    name: "export",
    section: 5,
    needs: ["base", ...morphs.map((m) => `morph:${m}`)],
    inputs: py("export_character.py", "clear.ts", "mirror.ts", "markers.ts"),
    outputs: ["assets/models/character/base.glb"],
    run: [
      blender("export_character.py", `"$out/${base}.glb" assets/models/character/base.glb "--morphs=${all}" --textures=2048`, "MORPH|SOCKETS|EXPORTED|Error"),
      "node $here/clear.ts assets/models/character/base.glb     # a gliding swing foot lifted clear (clear.ts)",
      "node $here/mirror.ts assets/models/character/base.glb    # the mirrored clips (art/animation/mirrors.json)",
      "node $here/markers.ts assets/models/character/base.glb   # each clip's foot-down markers (D455)",
    ].join("\n"),
  });
  // The fight's clips (art/animation/sets/fight.json, D454), which every body plays: its takes
  // made onto a copy of the mannequin beside the game's clips, the base exported with them, each
  // held as its entry says (hold.py, the shield on its targets), then the mirrored clips and the
  // foot markers baked, as clip.ts --into does.
  const fight = "art/animation/sets/fight";
  steps.push({
    name: "fight",
    section: 5,
    needs: ["clips", "base"],
    inputs: [...py("made_clips.py", "export_character.py", "hold.py", "clear.ts", "mirror.ts", "markers.ts"), `${fight}.json`, fight],
    outputs: [`${OUT}/fight-mannequin.glb`, "assets/models/character/fight.glb"],
    run: [
      blender("made_clips.py", `"$out/fight-mannequin.glb" --list=${fight}.json`, "^MADE [0-9]|Error"),
      blender("export_character.py", `"$out/${base}.glb" assets/models/character/fight.glb "--clips=$out/fight-mannequin.glb" --list=${fight}.json --textures=2048`, "^(SHIELD|HOLD|EXPORTED)|Error"),
      "node $here/clear.ts assets/models/character/fight.glb     # a gliding swing foot lifted clear (clear.ts)",
      "node $here/mirror.ts assets/models/character/fight.glb    # the mirrored clips (art/animation/mirrors.json)",
      "node $here/markers.ts assets/models/character/fight.glb   # each clip's foot-down markers (D455)",
    ].join("\n"),
  });
  for (const item of looks)
    steps.push({
      name: `look:${item}`,
      section: 5,
      needs: ["base", made(item), ...morphs.map((m) => `morph:${m}`), ...[base, ...morphs].map((body) => `fit:${body}:${item}`)],
      inputs: [...py("carry_gear.py"), `art/gear/${item}/fit.json`],
      outputs: [`assets/models/gear/${item}.glb`],
      run: ["mkdir -p assets/models/gear", blender("carry_gear.py", `"$out/${base}-game.glb" "$out/gear/${base}/${item}.glb" "assets/models/gear/${item}.glb" "--morphs=${all}" --textures=1024`, "CARRIED|Error")].join("\n"),
    });
  return steps;
}

/** The last line of a full build's log, which build_verdict.ts reads the bodies and looks from. */
export function builtLine(b: Bodies, items: Map<string, GearData>): string {
  return `BUILT base.glb with morphs ${morphNames(b).join(",")}, ${garmentOrder(items).length + rigidPieces(items).length} looks`;
}

/** Local modules a script imports (Python's import and from, TypeScript's relative from), and their data, followed through.
 * A Python import inside a block is not followed: the script reads it only on that path
 * (export_character.py's hold.py, under --list), and a step whose command takes it names it. */
export function closure(paths: string[], root = ROOT): string[] {
  const seen = new Set<string>();
  const visit = (p: string) => {
    if (seen.has(p)) return;
    seen.add(p);
    for (const d of MODULE_DATA[p] ?? []) visit(d);
    if (!/\.(py|ts)$/.test(p) || !existsSync(join(root, p))) return;
    const dir = p.slice(0, p.lastIndexOf("/"));
    const text = readFileSync(join(root, p), "utf8");
    const found = p.endsWith(".py")
      ? [...text.matchAll(/^(?:from\s+(\w+)\s+import|import\s+([\w, ]+))/gm)].flatMap((m) => (m[1] ?? m[2]).split(",").map((s) => `${dir}/${s.trim().split(/\s+/)[0]}.py`))
      : [...text.matchAll(/from\s+"\.\/([\w.]+\.ts)"/g)].map((m) => `${dir}/${m[1]}`);
    for (const f of found) if (existsSync(join(root, f))) visit(f);
  };
  for (const p of paths) visit(p);
  return [...seen].sort();
}

export { gearData };
