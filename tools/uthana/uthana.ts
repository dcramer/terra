#!/usr/bin/env node
// Uthana's text-to-motion from the command line (D532): our body uploaded once as a character,
// then a prompt made into a clip retargeted onto its skeleton and saved as a motion-only GLB,
// for tools/characters' checks to measure. Paid credits, so the clips are ours (uthana.com/terms).
// API: https://uthana.com/docs/api (GraphQL, Basic auth with the key as the user).
//
// Node 22.18 or later; no dependencies.

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { Glb, posed } from "../characters/glb.ts";
import { env, sleep } from "../tripo/studio.ts";

const API = "https://uthana.com";
export const BODY = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "assets", "models", "character", "base.glb");
// Characters uploaded, by the body file's hash, so a changed body is uploaded again.
const CACHE = join(homedir(), ".cache", "terra", "uthana.json");
// Dollars a generated second, by model (uthana.com/pricing, 2026-10-26).
export const MODELS: Record<string, { id: string; rate: number; job: boolean }> = {
  "1": { id: "text-to-motion", rate: 0.02, job: false },
  "2": { id: "text-to-motion-bucmd", rate: 0.03, job: false },
  "3": { id: "text-to-motion-3.0", rate: 0.1, job: true },
};

export class Failed extends Error {}

const USAGE = `node tools/uthana/uthana.ts <command>
  org                                  the account and what is left this month
  character [body.glb]                 upload a rigged body (default assets/models/character/base.glb); prints its id
  motion "<prompt>" -o clip.glb        make a clip on our body and save it, motion only
      --model 1|2|3 (2)  --length s (5)  --seed n  --fps 24|30|60 (30)  --in-place  --foot-ik
      --character id (default: our base body uploaded)  --loop s (its end eased into its start over s seconds)
      --from id (loop a motion already made instead of making one)
      --scale k (the translations' scale; by default what puts the feet on the ground)
  schema                               the arguments the live API takes for each mutation`;

function auth(): string {
  let key: string;
  try {
    key = env("UTHANA_API_KEY");
  } catch {
    throw new Failed("UTHANA_API_KEY is not set: put it in the environment or in .env (see tools/uthana/README.md)");
  }
  return `Basic ${Buffer.from(`${key}:`).toString("base64")}`;
}

async function gql<T>(query: string, variables: Record<string, unknown> = {}, form?: FormData): Promise<T> {
  const body = form ?? JSON.stringify({ query, variables });
  const headers: Record<string, string> = { authorization: auth() };
  if (!form) headers["content-type"] = "application/json";
  const res = await fetch(`${API}/graphql`, { method: "POST", headers, body });
  const text = await res.text();
  if (res.status === 401 || res.status === 403) throw new Failed(`Uthana refused the key (${res.status}): check UTHANA_API_KEY`);
  let out: { data?: T; errors?: { message: string }[] };
  try {
    out = JSON.parse(text);
  } catch {
    throw new Failed(`Uthana answered ${res.status}: ${text.slice(0, 200)}`);
  }
  if (out.errors?.length) throw new Failed(out.errors.map((e) => e.message).join("; "));
  return out.data as T;
}

function cache(): Record<string, string> {
  return existsSync(CACHE) ? JSON.parse(readFileSync(CACHE, "utf8")) : {};
}

export async function character(file: string = BODY): Promise<string> {
  if (!existsSync(file)) throw new Failed(`no body at ${file}`);
  const bytes = readFileSync(file);
  const hash = createHash("sha256").update(bytes).digest("hex").slice(0, 16);
  const known = cache();
  if (known[hash]) return known[hash];
  const query = "mutation ($file: Upload!, $name: String!) { create_character(file: $file, name: $name, auto_rig: false) { character { id name } } }";
  const form = new FormData();
  form.append("operations", JSON.stringify({ query, variables: { file: null, name: `terra-${basename(file, ".glb")}-${hash}` } }));
  form.append("map", JSON.stringify({ 0: ["variables.file"] }));
  form.append("0", new Blob([bytes]), basename(file));
  const data = await gql<{ create_character: { character: { id: string } } }>(query, {}, form);
  const id = data.create_character.character.id;
  mkdirSync(dirname(CACHE), { recursive: true });
  writeFileSync(CACHE, JSON.stringify({ ...known, [hash]: id }, null, 2));
  return id;
}

export async function motion(prompt: string, o: { model: string; length: number; seed?: number; footIk: boolean; character: string }): Promise<string> {
  const model = MODELS[o.model];
  if (!model) throw new Failed(`--model is 1, 2 or 3, not ${o.model}`);
  if (model.job) {
    const made = await gql<{ create_text_to_motion_job: { job: { id: string } } }>(
      "mutation ($p: String!, $m: String!, $c: String, $l: Float) { create_text_to_motion_job(prompt: $p, model: $m, character_id: $c, length: $l) { job { id status } } }",
      { p: prompt, m: model.id, c: o.character, l: Math.round(o.length) },
    );
    const started = Date.now();
    for (;;) {
      await sleep(5000);
      const { job } = await gql<{ job: { status: string; result?: { result?: { id: string } } } }>("query ($j: String) { job(job_id: $j) { id status result } }", { j: made.create_text_to_motion_job.job.id });
      if (job.status === "FINISHED" && job.result?.result?.id) return job.result.result.id;
      if (job.status === "FAILED") throw new Failed(`the job failed: ${JSON.stringify(job.result).slice(0, 200)}`);
      if (Date.now() - started > 300_000) throw new Failed("the job took over 5 minutes");
    }
  }
  const data = await gql<{ create_text_to_motion: { motion: { id: string } } }>(
    "mutation ($p: String!, $m: String, $c: String, $l: Float, $s: Int, $f: Boolean) { create_text_to_motion(prompt: $p, model: $m, character_id: $c, length: $l, seed: $s, foot_ik: $f) { motion { id name } } }",
    { p: prompt, m: model.id, c: o.character, l: o.length, s: o.seed, f: o.footIk },
  );
  return data.create_text_to_motion.motion.id;
}

/** What is left of the account's pay-as-you-go credit, dollars: a call's cost is the fall in it. */
export async function balance(): Promise<number> {
  const data = await gql<{ org: { payg_total_usd: string } }>("{ org { payg_total_usd } }");
  return Number(data.org.payg_total_usd);
}

type Gltf = { nodes: { name?: string }[]; animations?: { channels: { sampler: number; target: { path: string } }[]; samplers: { output: number }[] }[]; skins?: { joints: number[]; inverseBindMatrices?: number; skeleton?: number }[]; accessors: { bufferView?: number; byteOffset?: number; count: number }[]; bufferViews: { buffer: number; byteOffset?: number; byteLength: number; byteStride?: number }[]; buffers: { byteLength: number }[] };

function chunks(glb: Buffer): { json: Gltf; bin: Buffer } {
  const jsonLen = glb.readUInt32LE(12);
  const binLen = glb.readUInt32LE(20 + jsonLen);
  return { json: JSON.parse(glb.subarray(20, 20 + jsonLen).toString()), bin: glb.subarray(28 + jsonLen, 28 + jsonLen + binLen) };
}

/** A GLB from its JSON and binary chunk, each padded to four bytes (the JSON with spaces). */
export function glbOf(json: object, bin: Buffer): Buffer {
  const text = Buffer.from(JSON.stringify(json));
  const j = Buffer.concat([text, Buffer.alloc((4 - (text.length % 4)) % 4, 0x20)]);
  const b = Buffer.concat([bin, Buffer.alloc((4 - (bin.length % 4)) % 4)]);
  const head = Buffer.alloc(12);
  head.writeUInt32LE(0x46546c67, 0);
  head.writeUInt32LE(2, 4);
  head.writeUInt32LE(12 + 8 + j.length + 8 + b.length, 8);
  const chunk = (data: Buffer, type: number) => {
    const h = Buffer.alloc(8);
    h.writeUInt32LE(data.length, 0);
    h.writeUInt32LE(type, 4);
    return Buffer.concat([h, data]);
  };
  return Buffer.concat([head, chunk(j, 0x4e4f534a), chunk(b, 0x004e4942)]);
}

/** A downloaded clip with `body`'s skin added: Uthana's GLB is our skeleton's nodes and the
 * motion, with no skin, so an importer reads it as loose nodes (Blender as empties); with the
 * body's joints (matched by name) and inverse bind matrices it reads as the body's skeleton,
 * as a Kimodo take's BVH does. A clip that has a skin is left as it is. */
export function withSkin(clip: Buffer, body: Buffer): Buffer {
  const t = chunks(clip);
  const s = chunks(body);
  if (t.json.skins?.length) return clip;
  const skin = s.json.skins?.[0];
  if (!skin || skin.inverseBindMatrices === undefined) throw new Failed("the body has no skin to give the clip");
  const at = new Map(t.json.nodes.map((n, i) => [n.name, i]));
  const node = (i: number) => {
    const name = s.json.nodes[i].name;
    const found = at.get(name);
    if (found === undefined) throw new Failed(`the clip has no node ${name}: it is not on this body's skeleton`);
    return found;
  };
  const acc = s.json.accessors[skin.inverseBindMatrices];
  const view = s.json.bufferViews[acc.bufferView!];
  const from = (view.byteOffset ?? 0) + (acc.byteOffset ?? 0);
  const matrices = s.bin.subarray(from, from + acc.count * 64);
  const offset = t.bin.length + ((4 - (t.bin.length % 4)) % 4);
  const bin = Buffer.concat([t.bin, Buffer.alloc(offset - t.bin.length), matrices]);
  t.json.bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: matrices.length });
  t.json.accessors.push({ bufferView: t.json.bufferViews.length - 1, componentType: 5126, count: acc.count, type: "MAT4" } as Gltf["accessors"][number]);
  t.json.skins = [{ joints: skin.joints.map(node), inverseBindMatrices: t.json.accessors.length - 1, ...(skin.skeleton !== undefined ? { skeleton: node(skin.skeleton) } : {}) }];
  t.json.buffers[0].byteLength = bin.length;
  return glbOf(t.json, bin);
}

/** A clip with every translation keyed in it (Uthana keys the root's and the pelvis's)
 * multiplied by `by`. */
export function scaled(clip: Buffer, by: number): Buffer {
  const { json, bin } = chunks(clip);
  const out = Buffer.from(bin);
  const done = new Set<number>();
  for (const a of json.animations ?? []) {
    for (const c of a.channels) {
      const at = a.samplers[c.sampler].output;
      if (c.target.path !== "translation" || done.has(at)) continue;
      done.add(at);
      const acc = json.accessors[at];
      const view = json.bufferViews[acc.bufferView!];
      const start = (view.byteOffset ?? 0) + (acc.byteOffset ?? 0);
      for (let i = 0; i < acc.count; i++) {
        for (let j = 0; j < 3; j++) {
          const k = start + i * (view.byteStride ?? 12) + j * 4;
          out.writeFloatLE(out.readFloatLE(k) * by, k);
        }
      }
    }
  }
  return glbOf(json, out);
}

const FEET = ["ball_l", "ball_r", "foot_l", "foot_r"];

/** How much a clip's translations must be multiplied by for its feet to reach the ground: the
 * lowest of the balls and feet, at its lowest over the clip, where the body's rest pose has it.
 * Uthana's clips on our body come with the root's and pelvis's translations about 13 times too
 * small (2026-10-26: the pelvis 6.4 cm off the ground standing, while the same motion on
 * Uthana's Manny stands at full height); the feet put the hips back at
 * their height. The translations place the hips linearly, so the fit is exact for a clip whose
 * feet touch the ground; one that never does (a swim) needs the scale another clip found. */
export function groundScale(clip: string, body: string = BODY): number {
  const figure = new Glb(body);
  const file = new Glb(clip);
  const motion = file.clip(file.gltf.animations?.[0]?.name as string);
  if (!motion) throw new Failed(`${clip} has no animation`);
  const feet = FEET.map((f) => figure.named(f));
  const pelvis = figure.named("pelvis");
  const rest = posed(figure, undefined, 0);
  const ground = Math.min(...feet.map((f) => rest[f].t[1]));
  let most = 0;
  for (let t = 0; t <= motion.end + 1e-6; t += 1 / 30) {
    const pose = posed(figure, motion, t);
    // The hips' height times the scale, the feet's drop below them unchanged.
    const hip = pose[pelvis].t[1];
    const drop = Math.min(...feet.map((f) => pose[f].t[1])) - hip;
    most = Math.max(most, (ground - drop) / hip);
  }
  return most;
}

// A motion made a closed loop: its end eased into its start over the last `zone` seconds, the
// length kept (docs/api/capabilities/stitch-loop-motions).
export async function looped(characterId: string, motionId: string, zone: number): Promise<string> {
  const data = await gql<{ create_looped_motion: { motion: { id: string } } }>(
    "mutation ($c: String!, $m: String!, $z: Float) { create_looped_motion(character_id: $c, motion_id: $m, loop_mode: \"closed\", zone_mode: \"modify\", zone_duration: $z) { motion { id name } } }",
    { c: characterId, m: motionId, z: zone },
  );
  return data.create_looped_motion.motion.id;
}

/** A motion on a character saved to `out` as a GLB with the body's skin (`withSkin`) and its
 * translations at our body's size, by `scale` or else as its feet ask (`groundScale`); returns
 * the scale it was given. */
export async function download(characterId: string, motionId: string, out: string, fps: string, inPlace: boolean, scale?: number): Promise<number> {
  const query = new URLSearchParams({ fps, ...(inPlace ? { in_place: "true" } : {}) });
  const res = await fetch(`${API}/motion/animation/motion_viewer/${characterId}/${motionId}/glb/${basename(out)}?${query}`, { headers: { authorization: auth() } });
  if (!res.ok) throw new Failed(`download failed (${res.status}): ${(await res.text()).slice(0, 200)}`);
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, withSkin(Buffer.from(await res.arrayBuffer()), readFileSync(BODY)));
  const by = scale ?? groundScale(out);
  writeFileSync(out, scaled(readFileSync(out), by));
  return by;
}

export async function main(argv: string[]): Promise<void> {
  const { values: v, positionals: [command, arg] } = parseArgs({
    args: argv,
    allowPositionals: true,
    options: {
      out: { type: "string", short: "o" },
      model: { type: "string", default: "2" },
      length: { type: "string", default: "5" },
      seed: { type: "string" },
      fps: { type: "string", default: "30" },
      "in-place": { type: "boolean", default: false },
      "foot-ik": { type: "boolean", default: false },
      character: { type: "string" },
      loop: { type: "string" },
      from: { type: "string" },
      scale: { type: "string" },
      help: { type: "boolean", short: "h" },
    },
  });
  if (v.help || !command) return console.log(USAGE);
  switch (command) {
    case "org": {
      const data = await gql("{ user { email } org { name payg_total_usd motion_download_secs_per_month_remaining characters_allowed_remaining } }");
      return console.log(JSON.stringify(data));
    }
    case "character":
      return console.log(JSON.stringify({ character: await character(arg ?? BODY) }));
    case "schema": {
      const data = await gql<{ __schema: { mutationType: { fields: { name: string; args: { name: string }[] }[] } } }>(
        "{ __schema { mutationType { fields { name args { name } } } } }",
      );
      for (const f of data.__schema.mutationType.fields) if (/motion|character/.test(f.name)) console.log(`${f.name}(${f.args.map((a) => a.name).join(", ")})`);
      return;
    }
    case "motion": {
      if ((!arg && !v.from) || !v.out) throw new Failed('motion needs a prompt and -o clip.glb: node tools/uthana/uthana.ts motion "a person idles, sword in hand" -o idle.glb');
      const length = Number(v.length);
      const characterId = v.character ?? (await character(BODY));
      const before = await balance();
      const made = v.from ?? (await motion(arg, { model: v.model!, length, seed: v.seed ? Number(v.seed) : undefined, footIk: v["foot-ik"]!, character: characterId }));
      const motionId = v.loop ? await looped(characterId, made, Number(v.loop)) : made;
      const scale = await download(characterId, motionId, v.out, v.fps!, v["in-place"]!, v.scale ? Number(v.scale) : undefined);
      // What it cost, by the account's credit (generating and looping are both charged).
      const dollars = Math.round((before - (await balance())) * 10000) / 10000;
      return console.log(JSON.stringify({ motion: motionId, character: characterId, dollars, scale: Math.round(scale * 1000) / 1000, files: [v.out] }));
    }
    default:
      throw new Failed(`no command ${command}\n${USAGE}`);
  }
}

if (import.meta.main) {
  try {
    await main(process.argv.slice(2));
  } catch (e) {
    if (!(e instanceof Failed)) throw e;
    console.error(`✗ ${e.message}`);
    process.exit(1);
  }
}
