#!/usr/bin/env node
// Makes a construction material in one command (D290, docs/proposals/construction-002.md):
// its reference pack's texture set from a source, checked and credited, its pack file and
// its material file, which is all the hammer's picker and the kit sheet need to show it.
// See README.md. Node 22.18 or later, which runs TypeScript directly; no dependencies but
// ImageMagick (`convert`), and Blender for a model's bake.

import { execFileSync } from "node:child_process";
import { copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { release, tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { command as previewCommand } from "./preview.ts";

const ROOT = resolve(dirname(new URL(import.meta.url).pathname), "../..");
const ASSETS = join(ROOT, "assets");
const MATERIALS = join(ASSETS, "materials");
const KITS = join(ASSETS, "data", "kits");
const PACKS = join(ASSETS, "data", "packs");
const README = join(MATERIALS, "README.md");
/** The texture budget a set is made at (crates/terra_game/src/build/arrays, `SIDE`). */
export const SIDE = 1024;

/** What each pattern's material is, unless told otherwise: the texture slots its pack fills,
 * its cost, how heavy it reads where kinds meet, the roof over it, and the reference pack it
 * is laid over (`under`), which gives it everything its pattern needs but the face it is
 * made for: a wood the daub, sawn ends and modelled log of the pine logs' pack, a covering
 * the battens, rafters and ridge of the shakes'. */
export const PATTERNS: Record<string, { slots: string[]; cost: string; steps: number; weight: number; roof?: string; under?: string }> = {
  block: { slots: ["Masonry", "Floor"], cost: "Stone 1/4", steps: 1, weight: 3, roof: "slate" },
  frame: { slots: ["Masonry", "Floor"], cost: "Wood 1/64, Fibre 1/64", steps: 4, weight: 1, roof: "thatch" },
  coursed: { slots: ["Log"], cost: "Wood 1/16", steps: 2, weight: 2, roof: "shakes", under: "pine_logs" },
  roof: { slots: ["Covering"], cost: "Wood 1/32", steps: 4, weight: 0, under: "shakes" },
};

/** The pack a material is laid over: the one asked for, else its pattern's; none if told
 * `--under none`. */
export function underFor(pattern: string, asked?: string): string | undefined {
  if (asked === "none") return undefined;
  return asked ?? PATTERNS[pattern]?.under;
}

/** Runs `work`, and if it fails puts every one of `paths` back as it was before (a file's
 * text, a folder's files) or takes it away if it was not there, so a failed run leaves
 * nothing behind to break the checks after it. */
export async function leavingNothingIfItFails<T>(paths: string[], work: () => Promise<T>): Promise<T> {
  const keep = mkdtempSync(join(tmpdir(), "pack-"));
  const was = paths.map((path, i) => {
    if (!existsSync(path)) return { path, kept: undefined };
    const kept = join(keep, String(i));
    cpSync(path, kept, { recursive: true });
    return { path, kept };
  });
  try {
    return await work();
  } catch (error) {
    for (const { path, kept } of was) {
      rmSync(path, { recursive: true, force: true });
      if (kept) cpSync(kept, path, { recursive: true });
    }
    throw error;
  } finally {
    rmSync(keep, { recursive: true, force: true });
  }
}

/** A bill as the material file writes it: `Stone 1/4, Wood 1/64` to `[("Stone", 1, 4), …]`. */
export function bill(text: string): string {
  const items = text.split(",").map((part) => {
    const match = part.trim().match(/^([A-Za-z]+)\s+(\d+)\/(\d+)$/);
    if (!match) throw new Error(`a cost is items and fractions like "Stone 1/4, Wood 1/64", not "${part.trim()}"`);
    return `("${match[1]}", ${match[2]}, ${match[3]})`;
  });
  return `[${items.join(", ")}]`;
}

/** The next free id for a material: walls under 2048, roofs from it (D241). */
export function nextId(ids: number[], roof: boolean): number {
  const range = ids.filter((id) => (roof ? id >= 2048 : id < 2048));
  return range.length ? Math.max(...range) + 1 : roof ? 2048 : 1;
}

/** The ids the material files hold, and each file's id by its name. */
function materialIds(): Map<string, number> {
  const out = new Map<string, number>();
  for (const file of readdirSync(KITS).filter((f) => f.endsWith(".ron"))) {
    const id = readFileSync(join(KITS, file), "utf8").match(/^\s*id:\s*(\d+)/m);
    if (id) out.set(file.slice(0, -4), Number(id[1]));
  }
  return out;
}

const fmt = (n: number) => (Math.round(n * 1000) / 1000).toString().replace(/^(-?\d+)$/, "$1.0");
const triple = (c: number[]) => `(${c.map(fmt).join(", ")})`;

/** The pack's file, as RON. */
export function packFile(o: { name: string; source: string; pattern: string; metres: number; depth: number; mean: number[]; under?: string; rows?: number }): string {
  const set = (slot: string, depth: boolean) =>
    `        ${slot}: (path: "materials/${o.name}", metres: (${fmt(o.metres)}, ${fmt(o.metres)})${depth && o.depth > 0 ? `, depth: ${fmt(o.depth)}` : ""}),`;
  const slots = PATTERNS[o.pattern].slots;
  const lines = [
    `// ${o.name}'s reference pack (D290), made by \`cargo xtask pack\` from ${o.source}.`,
    "(",
    ...(o.under ? [`    pack: "${o.under}",`] : []),
    "    textures: {",
    ...slots.map((slot, i) => set(slot, i === 0)),
    "    },",
  ];
  if (o.pattern === "roof") lines.push(`    roof: (rows: ${fmt(o.rows ?? 6)}, phase: 0.0),`);
  // Its dust's colour as it comes down; and a coursed wood's daub, which only courses have.
  if (o.pattern === "coursed") {
    const daub = o.mean.map((c) => Math.min(1, c * 1.15));
    lines.push(`    colours: (light: ${triple(o.mean)}, chinking: ${triple(daub)}),`);
  } else if (o.pattern !== "roof") {
    lines.push(`    colours: (light: ${triple(o.mean)}),`);
  }
  lines.push(")", "");
  return lines.join("\n");
}

/** The material's file, as RON: a few lines over its pattern and its pack. */
export function materialFile(o: { name: string; title: string; source: string; id: number; pattern: string; cost: string; steps: number; weight: number; roof?: number }): string {
  return [
    `// ${o.title}: made by \`cargo xtask pack\` from ${o.source} (D290).`,
    "(",
    `    id: ${o.id},`,
    `    pattern: "${o.pattern}",`,
    `    pack: "${o.name}",`,
    `    cost: (steps: ${o.steps}, step: ${bill(o.cost)}),`,
    `    weight: ${o.weight},`,
    ...(o.roof !== undefined ? [`    roof_kind: Some(${o.roof}),`] : []),
    ")",
    "",
  ].join("\n");
}

function credit(name: string, line: string) {
  const prefix = `- \`${name}\`: `;
  const lines = (existsSync(README) ? readFileSync(README, "utf8") : "# Materials\n").split("\n").filter((l) => !l.startsWith(prefix));
  while (lines.length && lines[lines.length - 1] === "") lines.pop();
  lines.push(prefix + line, "");
  writeFileSync(README, lines.join("\n"));
}

function textures(...args: string[]): string {
  return execFileSync("node", [join(ROOT, "tools/textures/textures.ts"), ...args], { encoding: "utf8" });
}

/** Its texture set, from its source, in `assets/materials/<name>`; and its size in metres
 * where the source says it. */
async function make(name: string, source: string, values: Record<string, any>): Promise<{ metres?: number }> {
  const out = join(MATERIALS, name);
  const [kind, ...rest] = source.split(":");
  const what = rest.join(":");
  if (kind === "polyhaven") {
    textures("fetch", what, "--name", name, "--size", "1k");
    const info = await (await fetch(`https://api.polyhaven.com/info/${what}`)).json();
    return { metres: info.dimensions ? info.dimensions[0] / 1000 : undefined };
  }
  if (kind === "ambientcg") {
    textures("fetch", `ambientcg:${what}`, "--name", name, "--size", "1k");
    return {};
  }
  if (kind === "image") {
    textures("material", name, resolve(what), "--size", String(SIDE), ...(values.tile ? ["--tile", values.tile] : []));
    return {};
  }
  if (kind === "model") {
    // A hand-made reference (a Tripo model of a patch of the material, cleaned up), baked
    // onto a square of it seen from above: its colour, normal, occlusion-roughness and height.
    // A member's reference (a log) is baked round it instead, `--round diameter,length`
    // giving the size the kit lays it at.
    mkdirSync(out, { recursive: true });
    const round = values.round ? ["round", ...values.round.split(",")] : [];
    execFileSync(join(ROOT, "tools/blender/run.sh"), [join(ROOT, "tools/kits/bake.py"), resolve(what), out, String(values.metres ?? 2), String(SIDE), ...round], { stdio: "inherit" });
    credit(name, `baked ${round.length ? "round " : ""}by \`tools/kits/bake.py\` from the reference model \`${what}\`${round.length ? `, ${values.metres ?? 2} m along` : ""}.`);
    return { metres: Number(values.metres ?? 2) };
  }
  if (kind === "pack") {
    // A variant of a texture set already made: the generator's way to many materials from
    // one scan, its colour multiplied by `--tint`.
    const from = join(MATERIALS, what);
    if (!existsSync(join(from, "color.jpg"))) throw new Error(`no texture set materials/${what}`);
    mkdirSync(out, { recursive: true });
    for (const file of readdirSync(from).filter((f) => f.endsWith(".jpg"))) copyFileSync(join(from, file), join(out, file));
    credit(name, `a variant of \`${what}\` by \`tools/kits/pack.ts\`${values.tint ? `, tinted ${values.tint}` : ""}.`);
    // Its size the set's own, where a pack of that name says it.
    const pack = join(PACKS, `${what}.ron`);
    const metres = existsSync(pack) ? readFileSync(pack, "utf8").match(/metres: \(([\d.]+)/) : null;
    return { metres: metres ? Number(metres[1]) : undefined };
  }
  throw new Error(`a source is polyhaven:<id>, ambientcg:<Id>, image:<file>, model:<file.glb> or pack:<texture set>, not ${source}`);
}

/** Every map at the budget at most, a tint baked into its colour; and its mean colour. */
function fit(name: string, tint?: number[]): number[] {
  const out = join(MATERIALS, name);
  for (const file of ["color.jpg", "normal.jpg", "arm.jpg", "height.jpg"]) {
    const path = join(out, file);
    if (!existsSync(path)) continue;
    const [w, h] = execFileSync("identify", ["-format", "%w %h", path], { encoding: "utf8" }).split(" ").map(Number);
    if (w > SIDE || h > SIDE) execFileSync("convert", [path, "-resize", `${SIDE}x${SIDE}>`, "-quality", "92", path]);
  }
  const colour = join(out, "color.jpg");
  if (tint) execFileSync("convert", [colour, "-color-matrix", `${tint[0]} 0 0 0 ${tint[1]} 0 0 0 ${tint[2]}`, "-quality", "92", colour]);
  return execFileSync("convert", [colour, "-resize", "1x1!", "-format", "%[fx:r] %[fx:g] %[fx:b]", "info:"], { encoding: "utf8" }).split(" ").map(Number);
}

/** Puts right a height map the kit checker refused (D349): flipped, where its joints were
 * bright (`invert`), and stretched to the full range, where it was weak (`normalise`). */
export function fixHeight(path: string, o: { invert?: boolean; normalise?: boolean }) {
  if (!o.invert && !o.normalise) return;
  if (!existsSync(path)) throw new Error(`--invert-height and --normalise-height need a height map, and there is no ${path}`);
  execFileSync("convert", [path, ...(o.invert ? ["-negate"] : []), ...(o.normalise ? ["-auto-level"] : []), "-quality", "92", path]);
}

/** What makes a texture set usable: its three maps there, a square, its colour not black. */
export function problems(name: string, mean: number[]): string[] {
  const out = join(MATERIALS, name);
  const found: string[] = [];
  for (const file of ["color.jpg", "normal.jpg", "arm.jpg"]) if (!existsSync(join(out, file))) found.push(`no ${file}`);
  if (Math.max(...mean) < 0.03) found.push("its colour is black");
  return found;
}

async function main() {
  const { positionals, values } = parseArgs({
    allowPositionals: true,
    options: {
      from: { type: "string" },
      pattern: { type: "string", default: "block" },
      title: { type: "string" },
      cost: { type: "string" },
      steps: { type: "string" },
      weight: { type: "string" },
      roof: { type: "string" },
      under: { type: "string" },
      metres: { type: "string" },
      depth: { type: "string" },
      rows: { type: "string" },
      tint: { type: "string" },
      round: { type: "string" },
      tile: { type: "string" },
      "invert-height": { type: "boolean", default: false },
      "normalise-height": { type: "boolean", default: false },
      preview: { type: "boolean" },
      "no-check": { type: "boolean", default: false },
      help: { type: "boolean", short: "h" },
    },
  });
  const [name] = positionals;
  const pattern = PATTERNS[values.pattern!];
  if (!name || !values.from || !pattern || values.help) {
    console.log(`usage: cargo xtask pack <name> --from <source> [--pattern block|frame|coursed|roof] [options]
  sources: polyhaven:<id>, ambientcg:<Id> (CC0 scans), image:<file> (a generated picture),
           model:<file.glb> (a reference model, baked), pack:<texture set> (a variant, with --tint)
  --title T          what it is, for its file's first line
  --cost "Stone 1/4" what a step takes; --steps N steps a cube (the pattern's by default)
  --weight N         how heavy it reads where kinds meet; --roof <material> the roof over it
  --under <pack>     the pack it is laid over, for all its pattern needs but its own face:
                     a coursed wood the pine_logs pack's, a roof covering the shakes' by
                     default; none for none
  --metres M         how many metres one copy covers (a scan's own size where it says)
  --depth D          its parallax depth (0.015 where it has a height map)
  --rows N           a roof covering's courses a copy; --tint r,g,b its colour multiplied
  --round D,L        a model: source that is a member (a log), baked round it, laid D thick
                     and L long; its texture once round it by --metres along
  --invert-height    flip its height map, where the checker finds it inverted (joints bright)
  --normalise-height stretch its height map to the full range, where the checker finds it weak
  --preview          make its preview sheet (tools/kits/preview.ts) even with no GPU here;
                     made by default where a GPU draws it, else its command printed
  --no-check        skip the kit checker (for many at once; run cargo xtask kits after)`);
    process.exit(values.help ? 0 : 2);
  }
  const started = Date.now();
  const under = underFor(values.pattern!, values.under);
  // Everything it writes, put back as it was if any step fails.
  const touched = [join(MATERIALS, name), join(PACKS, `${name}.ron`), join(KITS, `${name}.ron`), README];
  await leavingNothingIfItFails(touched, async () => {
    const made = await make(name, values.from!, values);
    const tint = values.tint?.split(",").map(Number);
    fixHeight(join(MATERIALS, name, "height.jpg"), { invert: values["invert-height"], normalise: values["normalise-height"] });
    const mean = fit(name, tint);
    const wrong = problems(name, mean);
    if (wrong.length) throw new Error(`${name}: ${wrong.join("; ")}`);
    const metres = Number(values.metres ?? made.metres ?? 2);
    const height = existsSync(join(MATERIALS, name, "height.jpg"));
    const depth = Number(values.depth ?? (height ? 0.015 : 0));
    mkdirSync(PACKS, { recursive: true });
    writeFileSync(join(PACKS, `${name}.ron`), packFile({ name, source: values.from!, pattern: values.pattern!, metres, depth, mean, under, rows: values.rows ? Number(values.rows) : undefined }));
    const ids = materialIds();
    const existing = ids.get(name);
    ids.delete(name);
    const id = existing ?? nextId([...ids.values()], values.pattern === "roof");
    const roofName = values.roof ?? pattern.roof;
    const roof = roofName ? ids.get(roofName) : undefined;
    if (roofName && roof === undefined) throw new Error(`no roof material ${roofName}`);
    writeFileSync(
      join(KITS, `${name}.ron`),
      materialFile({
        name,
        title: values.title ?? name.replaceAll("_", " "),
        source: values.from!,
        id,
        pattern: values.pattern!,
        cost: values.cost ?? pattern.cost,
        steps: Number(values.steps ?? pattern.steps),
        weight: Number(values.weight ?? pattern.weight),
        roof,
      }),
    );
    if (!values["no-check"]) {
      // The kit checker over every material, this one among them (cargo xtask kits).
      // It exits failing when any material fails; what it says is the same.
      let report: string;
      try {
        report = execFileSync("cargo", ["run", "-q", "-p", "terra_build", "--example", "kits"], { cwd: ROOT, encoding: "utf8" });
      } catch (error: any) {
        report = String(error.stdout ?? "");
      }
      const lines = report.split("\n");
      const at = lines.findIndex((l) => l.startsWith(`${name}.ron:`));
      // Its line and the problems under it.
      const own: string[] = [];
      if (at >= 0) {
        own.push(lines[at]);
        for (const line of lines.slice(at + 1)) {
          if (!line.startsWith("  ")) break;
          own.push(line);
        }
      }
      console.log(own.join("\n"));
      if (!own[0]?.includes(": ok")) throw new Error(`${name} does not pass the kit checker; nothing of it is kept`);
    }
    console.log(`${name}: material ${id}, ${values.pattern} pattern${under ? `, over ${under}` : ""}, ${fmt(metres)} m a copy, in ${((Date.now() - started) / 1000).toFixed(0)} s. Files: assets/data/kits/${name}.ron, assets/data/packs/${name}.ron, assets/materials/${name}/.`);
  });
  // Its preview sheet (D349), for a developer to look over once: made here where a GPU
  // draws it, else the command that makes it.
  if (values.preview ?? drawsOnGpu()) {
    execFileSync("node", [join(ROOT, "tools/kits/preview.ts"), name], { stdio: "inherit" });
  } else {
    console.log(`${name}: its preview sheet, every role and shape of its pattern at 10 m and 2 m, noon and evening: ${previewCommand(name)} (shots/preview/${name}.jpg)`);
  }
}

/** Whether this machine draws the sandbox on a GPU: a Mac or Windows, or Linux with a GPU
 * device and not under WSL, whose Linux builds draw on the CPU (slowly: the preview's
 * command is printed there instead). */
export function drawsOnGpu(): boolean {
  if (process.platform === "darwin" || process.platform === "win32") return true;
  const wsl = /microsoft/i.test(release()) || existsSync("/proc/sys/fs/binfmt_misc/WSLInterop");
  return process.platform === "linux" && !wsl && existsSync("/dev/dri");
}

if (import.meta.main) {
  try {
    await main();
  } catch (error: any) {
    console.error(`pack: ${error?.message ?? error}`);
    process.exit(1);
  }
}
