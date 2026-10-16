#!/usr/bin/env node
// Makes the props things are built as, from a description to the file the game loads: a concept
// image (free), a Tripo model of it with its colour, normal and roughness maps (Studio credits), then
// a free clean-up in Blender that keeps them at the prop's texture budget (refine.py, D14). Each prop is a folder in art/props holding
// its prop.json, and the concept and model it was made from. See README.md.
// Node 22.18 or later, which runs TypeScript directly; no dependencies.

import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const HERE = join(ROOT, "tools", "props");
const OUT = join(HERE, "out");
const ART = join(ROOT, "art", "props");
const TRIPO = join(ROOT, "tools", "tripo", "tripo.ts");
const BLENDER = join(ROOT, "tools", "blender", "run.sh");
// The style guide, whose concept paragraph ends every concept's prompt (D14).
const GUIDE = join(ROOT, "docs", "art-style.md");
// An approved image in the style, given as a reference once there is one (docs/art-style.md,
// "Where the pipeline stands"). Until then the paragraph alone sets the style.
const REFERENCE = join(ROOT, "art", "style", "reference.png");

/** The style guide's concept paragraph: the quoted block under "The concept prompt", which
 * every concept ends with word for word, so that all are drawn alike. */
export function stylePrompt(guide: string): string {
  const section = guide.split(/^## The concept prompt$/m)[1]?.split(/^## /m)[0] ?? "";
  const quote = section.split("\n").find((line) => line.startsWith("> "));
  if (!quote) throw new Error("docs/art-style.md has no quoted paragraph under \"The concept prompt\"");
  return quote.slice(2).trim();
}

/** A concept's prompt: what the thing is, from its prop.json, then the guide's paragraph.
 * The subject says what it is and is made of, never how it is drawn. */
export function conceptPrompt(subject: string, guide: string): string {
  return `A single ${subject}. ${stylePrompt(guide)}`;
}

const USAGE = `usage: node tools/props/props.ts <step> [PROP …] [--again]

  concept   draws each prop's concept from its prop.json (free); look at it before a model
  model     a Tripo model of each concept: four views (free), then a low-poly P1 model
            textured at 2K with its normal, roughness and metal maps, about 50 Studio
            credits each
  refine    the model made ours in Blender (free): welded, cut to its triangle budget, its
            textures kept at its budget (a palette prop's faces each one palette colour), at
            its size; writes assets/models/props/<file>.glb (or the folder its "into"
            names) and a sheet in tools/props/out/<prop>.png
  all       the three in turn, each skipped where art/props already has its result
  fronts    every model in assets/models/props, tools and village seen from straight in
            front, on one sheet, tools/props/out/fronts.png: each should face the viewer,
            stand level and rest on the ground line (free)

  PROP …    the props, by folder name in art/props; every prop if none are named
  --again   make a concept or model even if art/props already has one`;

/** A prop's prop.json: what it is, and how its model is made ours. */
export type Prop = {
  // What the concept shows, finishing the brief's "A single …".
  subject: string;
  // Its file in assets/models/props, without .glb, as builds.ron names it.
  file: string;
  // The folder in assets/models it goes in, if not props: "tools" for a tool's source, which
  // tools/characters/fit_held.py makes the held look from.
  into?: string;
  // How wide, high and deep it is, in metres, along glTF's X, Y (up) and Z (its front), as
  // its catalogue entry gives it: refine makes the model exactly this size (the model
  // contract, docs/art-style.md).
  dims?: [number, number, number];
  // Where its origin goes (the model contract): "base", the middle of its base, for
  // anything stood on a floor or the ground (the default); "hinge", the foot of its hinge
  // edge in the middle of its thickness, the leaf running along +X, for a door's or a
  // shutter's leaf.
  anchor?: "base" | "hinge";
  // For a leaf whose model has its hinges on its right seen from its front: mirrored, so
  // every leaf is hinged on its left, at its origin.
  hinge?: "left" | "right";
  // The points code attaches to, by name, in metres in the finished model's own space
  // (glTF's axes, from its origin): written into its file as empties called socket.<name>.
  sockets?: Record<string, [number, number, number]>;
  // A palette model's longest side, in metres, for those not yet given dims.
  size?: number;
  // Triangles the finished model may have.
  budget: number;
  // A palette prop's colours, which its faces may take (tools/characters/look.py). A textured
  // prop keeps its own and needs none, but for a part that glows: its faces nearest Fire,
  // among the roles given, glow in the game, such as a forge's coals.
  roles?: string[];
  // For a role whose colour in the texture is not its own, the sRGB colour that means it
  // there: coals painted dull red, say, mean Fire, which is far brighter in the game.
  match?: Record<string, [number, number, number]>;
  // Degrees about the up axis that turn its front toward the viewer; 0 if not given.
  turn?: number;
  // Tripo's face limit for the generated model, before it is cut to the budget.
  faces?: number;
  // A building kit's piece laid along its length, such as a log or a board, which the game
  // stretches to fit: its length along X, its thinnest side up, its middle on the origin.
  along?: boolean;
  // Painted in palette roles, a colour on each face, rather than keeping the textures Tripo
  // made, as every prop does by default (D14, docs/art-style.md): for a tool, whose held look
  // fit_held.py colours by its Wood and Metal, and for the older flat models. Needs roles.
  palette?: boolean;
  // Kept for older prop.json files: textured is the default.
  textured?: boolean;
  // Tripo's generator: "p1", the low-poly Smart Mesh, by default; "h3.1", its most detailed,
  // for a piece seen close and often, such as a building kit's, textured in detail so its
  // colours are matched to roles more truly.
  generator?: string;
  // It lies flat, as a rug does: laid on the plane it best fits and pressed thin, since a
  // generator models such things tilted or curled.
  flat?: boolean;
  // A block, such as a section of stone wall: squared to its box however far it leans,
  // so it sits flush against its neighbours, and each of its sides' colour evened out to
  // the same, so it reads alike turned any way.
  block?: boolean;
  // Another prop whose colour this one's texture is matched to when refined, its mean and
  // spread on the surface, so a kit's pieces share one grade. That one is refined first.
  grade?: string;
  // Another prop whose concept it is drawn after, so the pieces of a kit share one stone,
  // one wood and one grade (docs/art-style.md, "One kit, one grade").
  reference?: string;
  // Copies of it with its one strongly coloured cloth dyed another colour (sRGB), each its
  // own file in its folder, made after refine by dye.py: a stall's awning in other dyes, so a
  // market is not one stall over and over. Its own file may be one, dyed its own colour.
  dyes?: Record<string, [number, number, number]>;
  // With dyes, how much the rest is greyed and darkened (0.75 a quarter darker), for wood
  // painted fresh that stands out in the weather.
  weather?: number;
};

export function readProp(text: string, name: string): Prop {
  const prop = JSON.parse(text);
  for (const key of ["subject", "file"]) {
    if (typeof prop[key] !== "string") throw new Error(`${name}: prop.json needs "${key}"`);
  }
  if (typeof prop.budget !== "number" || prop.budget <= 0) throw new Error(`${name}: prop.json needs a positive "budget"`);
  const dims = prop.dims;
  const sized = Array.isArray(dims) ? dims.length === 3 && dims.every((d: unknown) => typeof d === "number" && d > 0) : false;
  if (!sized && !(prop.palette && typeof prop.size === "number" && prop.size > 0)) {
    throw new Error(`${name}: prop.json needs "dims", its width, height and depth in metres`);
  }
  if (prop.anchor === "hinge" && !(prop.sockets?.hinge && prop.sockets?.handle)) {
    throw new Error(`${name}: a hinged leaf needs its "hinge" and "handle" sockets`);
  }
  if (prop.palette && (!Array.isArray(prop.roles) || prop.roles.length === 0)) {
    throw new Error(`${name}: prop.json needs "roles" for a palette prop`);
  }
  return prop;
}

/** The arguments refine.py takes for a prop. */
export function refineArgs(prop: Prop, model: string, out: string, sheet: string): string[] {
  const roles = (prop.roles ?? []).map((role) => (prop.match?.[role] ? `${role}=${prop.match[role].join(",")}` : role));
  return [
    model,
    out,
    sheet,
    String(prop.dims ? Math.max(...prop.dims) : prop.size),
    String(prop.budget),
    String(prop.turn ?? 0),
    ...roles,
    ...(prop.flat ? ["--flat"] : []),
    ...(prop.along ? ["--along"] : []),
    ...(prop.palette ? [] : ["--textured"]),
    ...(prop.block ? ["--block"] : []),
    ...(prop.dims ? [`--dims=${prop.dims.join(",")}`] : []),
    ...(prop.anchor && prop.anchor !== "base" ? [`--anchor=${prop.anchor}`] : []),
    ...(prop.hinge === "right" ? ["--mirror"] : []),
    ...Object.entries(prop.sockets ?? {}).map(([socket, at]) => `--socket=${socket}:${at.join(",")}`),
  ];
}

/** Runs a command that prints one line of JSON last, as the Tripo CLI does, and reads it. */
function runJson(command: string, args: string[]): { project: string } {
  console.log(`> ${[command, ...args].map((a) => relative(ROOT, a) || a).join(" ")}`);
  const result = spawnSync(command, args, { cwd: ROOT, stdio: ["inherit", "pipe", "inherit"], encoding: "utf8" });
  process.stdout.write(result.stdout ?? "");
  if (result.status !== 0) throw new Error(`${command} failed`);
  const last = (result.stdout ?? "").trim().split("\n").at(-1) ?? "";
  return JSON.parse(last);
}

function run(command: string, args: string[]) {
  console.log(`> ${[command, ...args].map((a) => relative(ROOT, a) || a).join(" ")}`);
  const result = spawnSync(command, args, { cwd: ROOT, stdio: "inherit" });
  if (result.status !== 0) throw new Error(`${command} failed`);
}

function concept(name: string, prop: Prop, again: boolean) {
  const image = join(ART, name, "concept.png");
  if (existsSync(image) && !again) return console.log(`${name}: has a concept`);
  const prompt = conceptPrompt(prop.subject, readFileSync(GUIDE, "utf8"));
  const own = prop.reference ? join(ART, prop.reference, "concept.png") : undefined;
  if (own && !existsSync(own)) throw new Error(`${name}: its reference ${prop.reference} has no concept`);
  const ref = own ?? (existsSync(REFERENCE) ? REFERENCE : undefined);
  const reference = ref ? ["--ref", ref] : [];
  run("node", [TRIPO, "image", prompt, ...reference, "--aspect", "1:1", "-o", image]);
  writeFileSync(join(ART, name, "concept.prompt.txt"), `${prompt}\n`);
}

function model(name: string, prop: Prop, again: boolean) {
  const image = join(ART, name, "concept.png");
  const glb = join(ART, name, "model.glb");
  if (existsSync(glb) && !again) return console.log(`${name}: has a model`);
  if (!existsSync(image)) throw new Error(`${name}: no concept yet; make one with the concept step`);
  // The views are free and remade at will, so they stay in out/.
  const views = join(OUT, `${name}-views`);
  mkdirSync(OUT, { recursive: true });
  run("node", [TRIPO, "views", image, "-o", views]);
  const sides = ["front", "left", "back", "right"].map((side) => `${views}-${side}.jpeg`);
  const generator = prop.generator ?? "p1";
  const quality = generator === "p1" ? "standard" : "detailed";
  const made = runJson("node", [TRIPO, "model", "--views", ...sides, "--model", generator, "--faces", String(prop.faces ?? 5000), "--texture-quality", quality, "-o", glb]);
  // A P1 model is textured in colour only; its normal, roughness and metal maps are a
  // further step (5 credits), which a textured prop needs (docs/art-style.md).
  if (!prop.palette && !hasPbr(readFileSync(glb))) run("node", [TRIPO, "pbr", made.project, "-o", glb]);
}

/** Whether a GLB's materials have a normal map, as Tripo's PBR step gives them. */
export function hasPbr(glb: Buffer): boolean {
  const json = JSON.parse(glb.subarray(20, 20 + glb.readUInt32LE(12)).toString("utf8"));
  return (json.materials ?? []).some((m: { normalTexture?: unknown }) => m.normalTexture);
}

function refine(name: string, prop: Prop) {
  const glb = join(ART, name, "model.glb");
  if (!existsSync(glb)) throw new Error(`${name}: no model yet; make one with the model step`);
  mkdirSync(OUT, { recursive: true });
  const out = join(ROOT, "assets", "models", prop.into ?? "props", `${prop.file}.glb`);
  // Relative paths, which a Windows Blender under WSL reads as they are (tools/blender).
  const here = (path: string) => relative(ROOT, path);
  const args = refineArgs(prop, here(glb), here(out), here(join(OUT, `${name}.png`)));
  if (prop.grade) {
    const report = join(OUT, `${prop.grade}.json`);
    const colour = existsSync(report) ? JSON.parse(readFileSync(report, "utf8")).colour : undefined;
    if (!colour) throw new Error(`${name}: refine ${prop.grade} first, whose grade it matches`);
    args.push(`--grade=${colour.join(",")}`);
  }
  run(BLENDER, [join(HERE, "refine.py"), ...args]);
  const folder = join(ROOT, "assets", "models", prop.into ?? "props");
  const refined = join(folder, `${prop.file}.glb`);
  const kept = join(OUT, `${name}-undyed.glb`);
  if (prop.dyes) copyFileSync(refined, kept);
  for (const [file, colour] of Object.entries(prop.dyes ?? {})) {
    const args = [join(HERE, "dye.py"), kept, join(folder, `${file}.glb`), colour.join(",")];
    run(BLENDER, prop.weather ? [...args, `${prop.weather}`] : args);
  }
}

/** Every prop, tool and village model from the front, labelled, on one sheet. */
function fronts() {
  const folders = ["props", "tools", "openings"].map((folder) => join(ROOT, "assets", "models", folder));
  const models = folders.flatMap((folder) => readdirSync(folder).filter((f) => f.endsWith(".glb")).sort().map((f) => join(folder, f)));
  const dir = join(OUT, "fronts");
  mkdirSync(dir, { recursive: true });
  run(BLENDER, [join(HERE, "fronts.py"), relative(ROOT, dir), ...models.map((m) => relative(ROOT, m))]);
  const pictures = models.map((m) => join(dir, m.split("/").pop()!.replace(/\.glb$/, ".png")));
  const sheet = join(OUT, "fronts.png");
  run("montage", ["-label", "%t", "-pointsize", "16", ...pictures, "-tile", "6x", "-geometry", "+3+3", sheet]);
  console.log(`Look at ${relative(ROOT, sheet)}: every model should face you, stand level and rest on the ground.`);
}

function main() {
  const { values, positionals } = parseArgs({ allowPositionals: true, options: { again: { type: "boolean" }, help: { type: "boolean", short: "h" } } });
  const [step, ...named] = positionals;
  if (values.help || !step || !["concept", "model", "refine", "all", "fronts"].includes(step)) {
    console.log(USAGE);
    process.exit(values.help ? 0 : 2);
  }
  if (step === "fronts") return fronts();
  const names = named.length ? named : readdirSync(ART).filter((n) => existsSync(join(ART, n, "prop.json"))).sort();
  for (const name of names) {
    const file = join(ART, name, "prop.json");
    if (!existsSync(file)) throw new Error(`${name}: there is no art/props/${name}/prop.json`);
    const prop = readProp(readFileSync(file, "utf8"), name);
    const again = values.again ?? false;
    if (step === "concept" || step === "all") concept(name, prop, again);
    if (step === "model" || step === "all") model(name, prop, again);
    if (step === "refine" || step === "all") refine(name, prop);
  }
}

if (import.meta.main) main();
