#!/usr/bin/env node
// The iso experiment's art pipeline: concept, model, rig, animate, export and check, one command
// a stage, each recorded in art/iso/NAME/asset.json with its credits and minutes. Everything
// Tripo does goes through tools/tripo/tripo.ts. See README.md.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, openSync, readFileSync, writeFileSync, closeSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { stylePrompt } from "../props/props.ts";
import { check, headingsDisagree, measure, printResults } from "./check.ts";
import { compact, readGlb, writeGlb } from "./gltf.ts";
import { type Body, GAME_CLIPS, KINDS, type Kind, SPECS, UNARMED_ATTACK, budget, clipsFor, isoPart } from "./kinds.ts";
import { type Asset, type Stage, type StageRecord, allAssets, assetDir, readAsset, readmeCost, statusTable, writeAsset } from "./ledger.ts";
import { centre, cut, inPlace, loop, place, renameClips } from "./normalize.ts";
import { DIR, assetSheet, clipSheet } from "./sheet.ts";
import { blenderCut, splice } from "./decimate.ts";
import { baseColour, trim } from "./trim.ts";

export const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const TRIPO = join(ROOT, "tools", "tripo", "tripo.ts");
/** Where the game's copy of each asset goes: Bevy's asset root, `models/iso/NAME.glb`. */
export const assetFile = (root: string, name: string) => join(root, "assets", "models", "iso", `${name}.glb`);
/** Drafts: Tripo's own files and each stage's log, never committed. */
const draftDir = (root: string, name: string) => join(root, "target", "iso-art", name);

/** Runs the Tripo CLI: its one line of JSON, and the minutes it took. Its progress goes to the log. */
export type Tripo = (args: string[], log: string) => { out: any; minutes: number };

/** The CLI's answer: one line of JSON from most commands, pretty JSON from `show`, a table from
 * `list` (kept as `text`). */
export function parseOutput(stdout: string): any {
  for (const text of [stdout.trim(), stdout.trim().split("\n").at(-1) ?? ""]) {
    try {
      return JSON.parse(text);
    } catch {}
  }
  return { text: stdout };
}

/** Minutes a Tripo command may take before it is stopped: its image call sometimes hangs at
 * "running" for good (iso-forest-art j3, 2026-11-08), where one takes 0.2 min. */
export const TRIPO_LIMITS: Record<string, number> = { image: 5 };

export const runTripo: Tripo = (args, log) => runTripoWithin(args, log, TRIPO_LIMITS[args[0]]);

export function runTripoWithin(args: string[], log: string, limitMinutes: number | undefined): ReturnType<Tripo> {
  mkdirSync(dirname(log), { recursive: true });
  const fd = openSync(log, "a");
  const start = Date.now();
  const timeout = limitMinutes === undefined ? undefined : Math.max(1, Math.round(limitMinutes * 60000));
  const r = spawnSync(process.execPath, [TRIPO, ...args], { cwd: ROOT, stdio: ["ignore", "pipe", fd], encoding: "utf8", maxBuffer: 1 << 26, timeout });
  closeSync(fd);
  const minutes = (Date.now() - start) / 60000;
  if (r.error && (r.error as NodeJS.ErrnoException).code === "ETIMEDOUT") {
    throw new Error(`tripo.ts ${args[0]} ran past ${limitMinutes} min and was stopped (Tripo's task hangs at "running" at times; one takes well under that): run the same command again. Its log: ${relative(ROOT, log)}`);
  }
  if (r.status !== 0) {
    const tail = readFileSync(log, "utf8").trim().split("\n").slice(-3).join("\n");
    throw new Error(`tripo.ts ${args[0]} failed (exit ${r.status}):\n${tail}\nfull log: ${relative(ROOT, log)}`);
  }
  return { out: parseOutput(r.stdout), minutes };
};

export type Ctx = { root: string; tripo: Tripo; readme: string; guide: string; print: (s: string) => void };

/** Refuses a stage that would spend more than the account holds, saying what to do instead. */
export function ensureCredits(ctx: Ctx, stage: string, cost: number, log: string): void {
  if (cost <= 0) return;
  const { out } = ctx.tripo(["balance"], log);
  if (typeof out.credits !== "number") throw new Error(`could not read Tripo's balance (${JSON.stringify(out)}); run \`node tools/tripo/tripo.ts balance\` and fix the login first (tools/tripo/README.md, Setup)`);
  if (out.credits < cost) {
    throw new Error(`${stage} costs ${cost} credits (tools/tripo/README.md's table) and the account has ${out.credits}: nothing was spent. Ask the developer for credits, or take an existing project in with \`adopt NAME PROJECT\` (\`node tools/tripo/tripo.ts list\`), whose rig and preset clips are free`);
  }
}

function record(ctx: Ctx, asset: Asset, stage: Stage, rec: Omit<StageRecord, "at">): void {
  asset.stages[stage] = { at: new Date().toISOString(), ...rec };
  writeAsset(ctx.root, asset);
  const cr = rec.credits === null ? "?" : rec.credits;
  const min = rec.minutes === null ? "?" : rec.minutes.toFixed(1);
  ctx.print(`✓ ${asset.name} ${stage}: ${cr} credits, ${min} min${rec.note ? `, ${rec.note}` : ""}`);
}

const needProject = (a: Asset, stage: string) => {
  if (!a.project) throw new Error(`${a.name} has no Tripo project yet: run \`model ${a.name}\` (or \`adopt ${a.name} PROJECT\`) before ${stage}`);
  return a.project;
};
const needRig = (a: Asset, stage: string) => {
  if (!a.body) throw new Error(`${a.name} is a ${a.kind}, which is not rigged or animated; go on to \`export ${a.name}\``);
  if (!a.rig) throw new Error(`${a.name} is not rigged yet: run \`rig ${a.name}\` before ${stage}`);
  return a.rig;
};

/** The concept's prompt: the subject and its theme, the iso part, the backdrop, then the style
 * guide's paragraph word for word, last, as docs/art-style.md asks. The theme is the game the
 * subject is made for, never a place it stands in: "its setting: a temperate oak forest" drew a
 * forest behind the treant of 2026-11-08, which Tripo's model would read as part of it. */
export function conceptPrompt(description: string, theme: string, kind: Kind, body: Body | undefined, guide: string): string {
  const subject = description.trim().replace(/\.$/, "");
  return `${subject}, made for a game set in ${theme}. ${isoPart(kind, body)} ${BACKDROP} ${stylePrompt(guide)}`;
}
/** What the concept stands on and before: a studio's, the one thing in the picture. */
export const BACKDROP = "Shown alone, the only thing in the picture, before a plain, even, neutral grey studio backdrop.";

export function concept(ctx: Ctx, name: string, opts: { kind?: string; theme?: string; body?: string; height?: string }, description?: string): void {
  const prior = existsSync(join(assetDir(ctx.root, name), "asset.json")) ? readAsset(ctx.root, name) : undefined;
  const kind = (opts.kind ?? prior?.kind) as Kind;
  if (!KINDS.includes(kind)) throw new Error(`concept needs --kind, one of ${KINDS.join(", ")}`);
  const theme = opts.theme ?? prior?.theme;
  const desc = description ?? prior?.description;
  if (!theme || !desc) throw new Error(`concept needs --theme TEXT and a "DESCRIPTION" of the subject (what it is, what it is made of), nothing about style: the style guide's paragraph is added`);
  const body = !SPECS[kind].body ? undefined : ((opts.body ?? prior?.body ?? SPECS[kind].body) as Body);
  if (body && body !== "biped" && body !== "other") throw new Error(`--body is biped (Tripo's humanoid rig) or other (a creature rigged as its shape reads)`);
  const height = opts.height ? Number(opts.height) : (prior?.height ?? SPECS[kind].defaultHeight);
  if (!height) throw new Error(`a ${kind} needs --height METRES, its real height, so export can make it life size (D110)`);
  const asset: Asset = { ...(prior ?? { stages: {} }), name, kind, theme, description: desc, body, height } as Asset;
  const prompt = conceptPrompt(desc, theme, kind, body, ctx.guide);
  const out = join(assetDir(ctx.root, name), "concept.png");
  mkdirSync(dirname(out), { recursive: true });
  const r = ctx.tripo(["image", prompt, ...(body === "biped" ? ["--t-pose"] : []), "-o", out], join(draftDir(ctx.root, name), "concept.log"));
  writeFileSync(join(assetDir(ctx.root, name), "prompt.txt"), prompt + "\n");
  const files = (r.out.files ?? [out]).map((f: string) => relative(ctx.root, f));
  record(ctx, asset, "concept", { credits: r.out.credits ?? 0, minutes: r.minutes, files, note: `look at ${files[0]}` });
}

export function model(ctx: Ctx, name: string, opts: { standard?: boolean }): void {
  const a = readAsset(ctx.root, name);
  const image = join(assetDir(ctx.root, name), "concept.png");
  if (!existsSync(image)) throw new Error(`${name} has no concept.png: run \`concept ${name}\` and look at it first`);
  if (a.project) throw new Error(`${name} already has project ${a.project}; a new model would spend again. To remake it, delete "project" and the model stage from ${relative(ctx.root, join(assetDir(ctx.root, name), "asset.json"))} first`);
  const log = join(draftDir(ctx.root, name), "model.log");
  const cost = opts.standard ? 25 : readmeCost(ctx.readme, "model");
  ensureCredits(ctx, "model", cost, log);
  const flags = opts.standard ? ["--geometry", "standard", "--texture-quality", "standard", "--no-pbr"] : [];
  const r = ctx.tripo(["model", image, ...flags, "-o", join(draftDir(ctx.root, name), "model.glb")], log);
  a.project = r.out.project;
  record(ctx, a, "model", { credits: r.out.credits ?? null, minutes: r.minutes, project: r.out.project });
}

export function rig(ctx: Ctx, name: string): void {
  const a = readAsset(ctx.root, name);
  const pid = needProject(a, "rig");
  if (!a.body) throw new Error(`${name} is a ${a.kind}, which is not rigged; go on to \`export ${name}\``);
  const log = join(draftDir(ctx.root, name), "rig.log");
  ensureCredits(ctx, "rig", readmeCost(ctx.readme, "rig"), log);
  const r = ctx.tripo(["rig", pid, "--type", a.body === "biped" ? "humanoid" : "other", "-o", join(draftDir(ctx.root, name), "rig.glb")], log);
  const shown = ctx.tripo(["show", pid], log);
  a.rig = shown.out.rig?.type;
  record(ctx, a, "rig", { credits: r.out.credits ?? null, minutes: r.minutes + shown.minutes, note: `rigged as ${a.rig}` });
}

export function animate(ctx: Ctx, name: string): void {
  const a = readAsset(ctx.root, name);
  const pid = needProject(a, "animate");
  const specs = clipsFor(needRig(a, "animate"));
  const log = join(draftDir(ctx.root, name), "animate.log");
  ensureCredits(ctx, "animate", readmeCost(ctx.readme, "animate"), log);
  const r = ctx.tripo(["animate", pid, ...specs.map((c) => c.preset), "--in-place", "-o", join(draftDir(ctx.root, name), "animate.glb")], log);
  record(ctx, a, "animate", { credits: r.out.credits ?? 0, minutes: r.minutes, clips: specs.map((c) => `${c.name}=${c.preset}`), note: specs.map((c) => c.name).join(" ") });
}

export function exportAsset(ctx: Ctx, name: string): void {
  const a = readAsset(ctx.root, name);
  const pid = needProject(a, "export");
  const specs = a.body ? clipsFor(needRig(a, "export")) : [];
  const raw = join(draftDir(ctx.root, name), "tripo.glb");
  const log = join(draftDir(ctx.root, name), "export.log");
  const anim = specs.length ? ["--animations", ...specs.map((c) => c.preset), "--in-place"] : [];
  const r = ctx.tripo(["export", pid, "--format", "glb", ...anim, "--texture-size", String(SPECS[a.kind].texture), "-o", raw], log);
  const glb = readGlb(readFileSync(r.out.files?.[0] ?? raw));
  const missing = renameClips(glb, specs);
  if (missing.length) throw new Error(`Tripo's export has no clip for ${missing.join(", ")} (it has ${(glb.doc.animations ?? []).map((x: any) => x.name).join(", ") || "none"}): run \`animate ${name}\` first`);
  const still = inPlace(glb, specs);
  const cuts = cut(glb, specs);
  const loops = loop(glb, specs).filter((l) => l.from > 0 || l.gap > 0.01);
  const centred = centre(glb, specs).filter((c) => c.off > 0.005);
  const measured = specs.some((c) => c.name === "walk") ? measure(glb) : undefined;
  const disagree = measured && headingsDisagree(name, measured);
  if (disagree) throw new Error(disagree);
  const walk = measured?.clips.walk?.motion;
  const placed = place(glb, name, a.height, walk?.heading ?? 0);
  const out = assetFile(ctx.root, name);
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, writeGlb(glb));
  const did = [`scaled ${placed.scale.toFixed(3)}`, `turned ${Math.round(placed.turn)} deg`, still.length && `in place ${still.join(" ")}`, cuts.length && `cut ${cuts.join(" ")}`, ...loops.map((l) => `looped ${l.name} on ${l.from.toFixed(2)}-${l.to.toFixed(2)} s (ends ${(l.gap * 100).toFixed(1)}% apart before closing)`), ...centred.map((c) => `centred ${c.name} (body ${(c.off * 100).toFixed(0)}% of height off)`)].filter(Boolean).join(", ");
  record(ctx, a, "export", { credits: r.out.credits ?? 0, minutes: r.minutes, files: [relative(ctx.root, out)], scale: Number(placed.scale.toFixed(4)), turn: Math.round(placed.turn), note: did });
}

/** Takes a biped converted onto the game's skeleton in, in place of `animate` and `export`: `file`
 * is tools/characters/finish_character.sh's NAME-game.glb, made on the render worker from
 * art/iso/NAME/rigged.glb (the iso-art skill, asset.md). Keeps the game's clips iso plays, named
 * as iso asks for them (kinds.ts GAME_CLIPS), drops the rest and what only they read, and puts
 * the scene under its root node life size, facing +Z, on y=0, as `export` does. */
export function convert(ctx: Ctx, name: string, file: string | undefined, opts: { unarmed?: boolean }): void {
  const a = readAsset(ctx.root, name);
  if (a.body !== "biped") throw new Error(`${name} is not a biped: only a humanoid is converted onto the game's skeleton; a creature keeps Tripo's rig (\`animate\`, \`export\`)`);
  if (!file || !existsSync(file)) throw new Error(`convert needs FILE, the game's export of ${name}: on the render worker \`cargo xtask remote-render --cpu --collect tools/characters/out/${name}-game.glb -- tools/characters/finish_character.sh ${name} ${a.height ?? "HEIGHT"} art/iso/${name}/rigged.glb\`, then the collected file`);
  const start = Date.now();
  const glb = readGlb(readFileSync(file));
  const attack = opts.unarmed ? UNARMED_ATTACK : ((a.stages.convert?.attack as string | undefined) ?? GAME_CLIPS.attack);
  const want = Object.entries({ ...GAME_CLIPS, attack });
  const anims: any[] = glb.doc.animations ?? [];
  const missing = want.filter(([, g]) => !anims.some((x) => x.name === g)).map(([, g]) => g);
  if (missing.length) throw new Error(`${relative(ctx.root, file)} has no clip ${missing.join(", ")} (it has ${anims.length}): it is not the game's export, or art/animation/clips.txt lost them; run finish_character.sh again`);
  glb.doc.animations = want.map(([ours, g]) => ({ ...anims.find((x) => x.name === g), name: ours }));
  compact(glb);
  // The game's walk and jog lean their body a few cm ahead of the feet (4-5% of the height on
  // the scavenger, 2026-11-08): set over the entity's place, as export does Tripo's.
  centre(glb, clipsFor("biped"));
  const placed = place(glb, name, a.height, 0);
  const out = assetFile(ctx.root, name);
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, writeGlb(glb));
  a.rig = "biped";
  record(ctx, a, "convert", {
    credits: 0,
    minutes: (Date.now() - start) / 60000,
    files: [relative(ctx.root, out)],
    clips: want.map(([ours, g]) => `${ours}=${g}`),
    attack,
    scale: Number(placed.scale.toFixed(4)),
    note: `the game's skeleton and clips: ${want.map(([ours, g]) => `${ours}=${g}`).join(" ")}, ${anims.length - want.length} others dropped, scaled ${placed.scale.toFixed(3)}`,
  });
}

/** Cuts the exported GLB's mesh to its budget (decimate.ts): by Blender here, or from `file`, a
 * cut tools/blender/decimate.py made elsewhere of this same GLB. Run after every `export`. */
export function decimateAsset(ctx: Ctx, name: string, file?: string): void {
  const a = readAsset(ctx.root, name);
  const path = assetFile(ctx.root, name);
  if (!existsSync(path)) throw new Error(`${relative(ctx.root, path)} is missing: run \`export ${name}\` first`);
  const start = Date.now();
  const glb = readGlb(readFileSync(path));
  const most = budget(a.kind, measure(glb));
  const tris = measure(glb).triangles;
  if (tris <= most && !file) {
    ctx.print(`✓ ${name} is ${tris} triangles, within its ${most}: nothing to cut`);
    return;
  }
  const cutFile = file ?? join(ctx.root, "target", "iso-art", name, "decimated.glb");
  if (!file) {
    mkdirSync(dirname(cutFile), { recursive: true });
    blenderCut(ctx.root, path, cutFile, most);
  }
  const s = splice(glb, readGlb(readFileSync(cutFile)));
  writeFileSync(path, writeGlb(glb));
  record(ctx, a, "decimate", { credits: 0, minutes: (Date.now() - start) / 60000, triangles: s.after, note: `${s.before} to ${s.after} triangles (budget ${most}), Blender's collapse on the mesh alone, skeleton and clips kept; bounds within ${(s.drift * 100).toFixed(2)}%` });
}

/** Drops a tree creature's leaf triangles but a `keep` share of its tufts (trim.ts), so its
 * trunk and limbs show from the ARPG camera. Run after `decimate` (export and decimate write the
 * mesh anew). */
export function trimAsset(ctx: Ctx, name: string, keep: number): void {
  const a = readAsset(ctx.root, name);
  const path = assetFile(ctx.root, name);
  if (!existsSync(path)) throw new Error(`${relative(ctx.root, path)} is missing: run \`export ${name}\` first`);
  if (!(keep >= 0 && keep <= 1)) throw new Error(`--keep is the share of leaf tufts kept, 0 to 1 (0.25 keeps a quarter)`);
  if (a.stages.trim && (a.stages.trim.at ?? "") > (a.stages.decimate?.at ?? a.stages.export?.at ?? "")) throw new Error(`${name} was trimmed already since its last export or decimate (${a.stages.trim.note}): run \`export ${name}\` and \`decimate ${name}\` first, so the trim is of the whole crown`);
  const start = Date.now();
  const glb = readGlb(readFileSync(path));
  const s = trim(glb, baseColour(glb), keep);
  writeFileSync(path, writeGlb(glb));
  record(ctx, a, "trim", { credits: 0, minutes: (Date.now() - start) / 60000, triangles: s.after, note: `${s.leaf} of ${s.before} triangles leaf in ${s.tufts} tufts; ${s.kept} tufts kept (--keep ${keep}), ${s.before} to ${s.after} triangles` });
}

export function checkAsset(ctx: Ctx, name: string, file?: string): boolean {
  const a = readAsset(ctx.root, name);
  const path = file ?? assetFile(ctx.root, name);
  if (!existsSync(path)) throw new Error(`${relative(ctx.root, path)} is missing: run \`export ${name}\` first`);
  const start = Date.now();
  const m = measure(readGlb(readFileSync(path)));
  const results = check(m, a.kind, a.rig, a.height);
  ctx.print(printResults(name, results));
  const failed = results.filter((x) => !x.ok).map((x) => x.check);
  // How fast the game moves the body while each gait plays, so its feet do not slide.
  const speeds = Object.fromEntries(["walk", "run"].filter((k) => m.clips[k]?.motion).map((k) => [k, Number(m.clips[k].motion!.speed.toFixed(2))]));
  record(ctx, a, "check", {
    credits: 0,
    minutes: (Date.now() - start) / 60000,
    passed: results.length - failed.length,
    of: results.length,
    failed,
    clips: Object.fromEntries(Object.entries(m.clips).map(([k, c]) => [k, Number(c.seconds.toFixed(2))])),
    speeds,
    triangles: m.triangles,
    height: Number(m.height.toFixed(3)),
    note: `passed ${results.length - failed.length} of ${results.length}`,
  });
  return failed.length === 0;
}

const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;

export function adopt(ctx: Ctx, name: string, project: string, opts: { kind?: string; theme?: string; body?: string; height?: string }, description?: string): void {
  const kind = opts.kind as Kind;
  if (!KINDS.includes(kind)) throw new Error(`adopt needs --kind, one of ${KINDS.join(", ")}`);
  const prior = existsSync(join(assetDir(ctx.root, name), "asset.json")) ? readAsset(ctx.root, name) : undefined;
  const height = opts.height ? Number(opts.height) : (prior?.height ?? SPECS[kind].defaultHeight);
  if (!height) throw new Error(`a ${kind} needs --height METRES, its real height, so export can make it life size (D110)`);
  const log = join(draftDir(ctx.root, name), "adopt.log");
  let pid = project.split("/").filter(Boolean).at(-1)!;
  if (!UUID.test(pid)) {
    // A short id, as `list` prints its first part: found among the account's projects.
    const listing = ctx.tripo(["list", "-n", "200"], log);
    const ids = JSON.stringify(listing.out).match(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g) ?? [];
    const found = [...new Set(ids.filter((id) => id.startsWith(pid)))];
    if (found.length !== 1) throw new Error(`"${project}" matches ${found.length} projects: give the whole id from \`node tools/tripo/tripo.ts list\``);
    pid = found[0];
  }
  const shown = ctx.tripo(["show", pid], log).out;
  const rigType: string | undefined = shown.rig?.status === "success" ? shown.rig.type : undefined;
  const body = !SPECS[kind].body ? undefined : rigType ? (rigType === "biped" ? "biped" : "other") : ((opts.body ?? SPECS[kind].body) as Body);
  const a: Asset = {
    ...(prior ?? { stages: {} }),
    name, kind, body, height,
    theme: opts.theme ?? prior?.theme ?? "",
    description: description ?? prior?.description ?? shown.name ?? "",
    project: pid,
    rig: rigType,
  } as Asset;
  const adopted = { credits: null, minutes: null, adopted: true };
  if ((shown.flags ?? []).includes("textured") || shown.made_with) a.stages.model ??= { at: shown.steps?.at(-1)?.at ?? "", ...adopted, project: pid, note: shown.made_with };
  if (rigType) a.stages.rig ??= { at: shown.steps?.find((s: any) => s.type === "rigging")?.at ?? "", ...adopted, note: `rigged as ${rigType}` };
  const done = (shown.animations ?? []).filter((x: any) => x.status === "success").map((x: any) => x.name);
  writeAsset(ctx.root, a);
  const next = !a.stages.model ? "model" : !a.body ? "export" : !rigType ? "rig" : "animate";
  ctx.print(`✓ ${name} adopted ${pid.slice(0, 8)} (${shown.name}): ${[a.stages.model && "modelled", rigType && `rigged as ${rigType}`, done.length && `clips ${done.join(" ")}`].filter(Boolean).join(", ")}; next \`${next} ${name}\``);
}

const USAGE = `usage: node tools/iso-art/iso-art.ts <stage> NAME …

  concept NAME --kind hero|creature|npc|prop --theme TEXT [--body biped|other] [--height M] "DESCRIPTION"
                         a concept image in the art style with the iso part (free, 100 a month)
  model NAME [--standard] a Tripo model of the concept (55 credits; 25 standard)
  rig NAME               Tripo's auto-rig, humanoid or as its shape reads (20 credits)
  animate NAME           the kind's preset clips (free; a creature's, as a biped is converted)
  export NAME            the GLB with its clips, life size, facing +Z, on y=0, to assets/models/iso/NAME.glb (free)
  convert NAME FILE [--unarmed]
                         a biped on the game's skeleton and clips: FILE is finish_character.sh's
                         NAME-game.glb from the worker; its idle, walk, run, attack (Sword_Attack, or
                         Punch_Cross --unarmed), hit and death to assets/models/iso/NAME.glb (free)
  decimate NAME [FILE]   the GLB's mesh cut to its budget by Blender (or FILE, tools/blender/decimate.py's
                         cut of it made elsewhere), skeleton and clips kept; after every export (free)
  trim NAME [--keep F]   a tree creature's leaf triangles dropped but a share F of their tufts
                         (default 0), so its trunk and limbs read from above; after decimate (free)
  check NAME [FILE]      what can be checked without a GPU; exits 1 if any check fails
  adopt NAME PROJECT --kind K [--height M] [--theme T] ["DESCRIPTION"]
                         takes an existing Tripo project in at the step it has reached
  status                 every asset's stages and costs
  sheet [--dir DIR]      DIR/sheet.png and DIR/clips.png from the asset view's DIR/views
                         (default ~/.cache/terra-review/iso-art; sheet.ts)

Credits are tools/tripo/README.md's; a spending stage checks the balance first.`;

export function main(argv: string[], ctx: Ctx): number {
  const { values: v, positionals: p } = parseArgs({
    args: argv,
    allowPositionals: true,
    options: { kind: { type: "string" }, theme: { type: "string" }, body: { type: "string" }, height: { type: "string" }, standard: { type: "boolean" }, unarmed: { type: "boolean" }, keep: { type: "string" }, dir: { type: "string" }, help: { type: "boolean", short: "h" } },
  });
  const [stage, name, third] = p;
  if (!stage || v.help) {
    ctx.print(USAGE);
    return stage ? 0 : 2;
  }
  if (stage === "sheet") {
    const dir = v.dir ?? DIR;
    ctx.print(`✓ ${assetSheet(ctx.root, dir, join(dir, "sheet.png"))}`);
    ctx.print(`✓ ${clipSheet(ctx.root, dir, join(dir, "clips.png"))}`);
    return 0;
  }
  if (stage === "status") {
    ctx.print(statusTable(allAssets(ctx.root)));
    return 0;
  }
  if (!name || !/^[a-z][a-z0-9-]*$/.test(name)) throw new Error(`${stage} needs a NAME in lower case, letters, digits and hyphens (it names art/iso/NAME and the GLB)`);
  switch (stage) {
    case "concept": concept(ctx, name, v, third); return 0;
    case "model": model(ctx, name, v); return 0;
    case "rig": rig(ctx, name); return 0;
    case "animate": animate(ctx, name); return 0;
    case "export": exportAsset(ctx, name); return 0;
    case "convert": convert(ctx, name, third, v); return 0;
    case "decimate": decimateAsset(ctx, name, third); return 0;
    case "trim": trimAsset(ctx, name, Number(v.keep ?? 0)); return 0;
    case "check": return checkAsset(ctx, name, third) ? 0 : 1;
    case "adopt":
      if (!third) throw new Error(`adopt needs the Tripo PROJECT (its id, a prefix \`node tools/tripo/tripo.ts list\` shows, or its Studio URL)`);
      adopt(ctx, name, third, v, p[3]); return 0;
    default: throw new Error(`no stage "${stage}"\n${USAGE}`);
  }
}

if (import.meta.main) {
  const ctx: Ctx = {
    root: ROOT,
    tripo: runTripo,
    readme: readFileSync(join(ROOT, "tools", "tripo", "README.md"), "utf8"),
    guide: readFileSync(join(ROOT, "docs", "art-style.md"), "utf8"),
    print: (s) => console.log(s),
  };
  try {
    process.exitCode = main(process.argv.slice(2), ctx);
  } catch (e) {
    console.error(`✗ ${(e as Error).message}`);
    process.exitCode = 2;
  }
}
