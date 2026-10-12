#!/usr/bin/env node
// Makes the props things are built as, from a description to the file the game loads: a concept
// image (free), a Tripo model of it (Studio credits), then a free clean-up in Blender that gives
// every face one of our palette colours (refine.py). Each prop is a folder in art/props holding
// its prop.json, and the concept and model it was made from. See README.md.
// Node 22.18 or later, which runs TypeScript directly; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
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
            textured at 2K, about 45 Studio credits each
  refine    the model made ours in Blender (free): welded, cut to its triangle budget, every
            face one palette colour, at its size; writes assets/models/props/<file>.glb (or
            the folder its "into" names) and a sheet in tools/props/out/<prop>.png
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
  // Its longest side, in metres, as builds.ron gives it.
  size: number;
  // Triangles the finished model may have.
  budget: number;
  // The palette colours its faces may take (tools/characters/look.py).
  roles: string[];
  // For a role whose colour in the texture is not its own, the sRGB colour that means it
  // there: coals painted dull red, say, mean Fire, which is far brighter in the game.
  match?: Record<string, [number, number, number]>;
  // Degrees about the up axis that turn its front toward the viewer; 0 if not given.
  turn?: number;
  // Tripo's face limit for the generated model, before it is cut to the budget.
  faces?: number;
  // It lies flat, as a rug does: laid on the plane it best fits and pressed thin, since a
  // generator models such things tilted or curled.
  flat?: boolean;
};

export function readProp(text: string, name: string): Prop {
  const prop = JSON.parse(text);
  for (const key of ["subject", "file"]) {
    if (typeof prop[key] !== "string") throw new Error(`${name}: prop.json needs "${key}"`);
  }
  for (const key of ["size", "budget"]) {
    if (typeof prop[key] !== "number" || prop[key] <= 0) throw new Error(`${name}: prop.json needs a positive "${key}"`);
  }
  if (!Array.isArray(prop.roles) || prop.roles.length === 0) throw new Error(`${name}: prop.json needs "roles"`);
  return prop;
}

/** The arguments refine.py takes for a prop. */
export function refineArgs(prop: Prop, model: string, out: string, sheet: string): string[] {
  const roles = prop.roles.map((role) => (prop.match?.[role] ? `${role}=${prop.match[role].join(",")}` : role));
  return [model, out, sheet, String(prop.size), String(prop.budget), String(prop.turn ?? 0), ...roles, ...(prop.flat ? ["--flat"] : [])];
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
  const reference = existsSync(REFERENCE) ? ["--ref", REFERENCE] : [];
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
  run("node", [TRIPO, "model", "--views", ...sides, "--model", "p1", "--faces", String(prop.faces ?? 5000), "--texture-quality", "standard", "-o", glb]);
}

function refine(name: string, prop: Prop) {
  const glb = join(ART, name, "model.glb");
  if (!existsSync(glb)) throw new Error(`${name}: no model yet; make one with the model step`);
  mkdirSync(OUT, { recursive: true });
  const out = join(ROOT, "assets", "models", prop.into ?? "props", `${prop.file}.glb`);
  // Relative paths, which a Windows Blender under WSL reads as they are (tools/blender).
  const here = (path: string) => relative(ROOT, path);
  run(BLENDER, [join(HERE, "refine.py"), ...refineArgs(prop, here(glb), here(out), here(join(OUT, `${name}.png`)))]);
}

/** Every prop, tool and village model from the front, labelled, on one sheet. */
function fronts() {
  const folders = ["props", "tools", "village"].map((folder) => join(ROOT, "assets", "models", folder));
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
