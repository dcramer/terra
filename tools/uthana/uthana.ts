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
import { parseArgs } from "node:util";
import { env, sleep } from "../tripo/studio.ts";

const API = "https://uthana.com";
const BODY = "assets/models/character/base.glb";
// Characters uploaded, by the body file's hash, so a changed body is uploaded again.
const CACHE = join(homedir(), ".cache", "terra", "uthana.json");
// Dollars a generated second, by model (uthana.com/pricing, 2026-10-26).
const MODELS: Record<string, { id: string; rate: number; job: boolean }> = {
  "1": { id: "text-to-motion", rate: 0.02, job: false },
  "2": { id: "text-to-motion-bucmd", rate: 0.03, job: false },
  "3": { id: "text-to-motion-3.0", rate: 0.1, job: true },
};

class Failed extends Error {}

const USAGE = `node tools/uthana/uthana.ts <command>
  org                                  the account and what is left this month
  character [body.glb]                 upload a rigged body (default ${BODY}); prints its id
  motion "<prompt>" -o clip.glb        make a clip on our body and save it, motion only
      --model 1|2|3 (2)  --length s (5)  --seed n  --fps 24|30|60 (30)  --in-place  --foot-ik
      --character id (default: ${BODY} uploaded)  --loop s (its end eased into its start over s seconds)
      --from id (loop a motion already made instead of making one)
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

async function character(file: string): Promise<string> {
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

async function motion(prompt: string, o: { model: string; length: number; seed?: number; footIk: boolean; character: string }): Promise<string> {
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

// A motion made a closed loop: its end eased into its start over the last `zone` seconds, the
// length kept (docs/api/capabilities/stitch-loop-motions).
async function looped(characterId: string, motionId: string, zone: number): Promise<string> {
  const data = await gql<{ create_looped_motion: { motion: { id: string } } }>(
    "mutation ($c: String!, $m: String!, $z: Float) { create_looped_motion(character_id: $c, motion_id: $m, loop_mode: \"closed\", zone_mode: \"modify\", zone_duration: $z) { motion { id name } } }",
    { c: characterId, m: motionId, z: zone },
  );
  return data.create_looped_motion.motion.id;
}

async function download(characterId: string, motionId: string, out: string, fps: string, inPlace: boolean): Promise<void> {
  const query = new URLSearchParams({ fps, ...(inPlace ? { in_place: "true" } : {}) });
  const res = await fetch(`${API}/motion/animation/motion_viewer/${characterId}/${motionId}/glb/${basename(out)}?${query}`, { headers: { authorization: auth() } });
  if (!res.ok) throw new Failed(`download failed (${res.status}): ${(await res.text()).slice(0, 200)}`);
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, Buffer.from(await res.arrayBuffer()));
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
      help: { type: "boolean", short: "h" },
    },
  });
  if (v.help || !command) return console.log(USAGE);
  switch (command) {
    case "org": {
      const data = await gql("{ user { email } org { name motion_download_secs_per_month_remaining characters_allowed_remaining } }");
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
      const made = v.from ?? (await motion(arg, { model: v.model!, length, seed: v.seed ? Number(v.seed) : undefined, footIk: v["foot-ik"]!, character: characterId }));
      const motionId = v.loop ? await looped(characterId, made, Number(v.loop)) : made;
      await download(characterId, motionId, v.out, v.fps!, v["in-place"]!);
      const dollars = v.from ? 0 : Math.round(length * MODELS[v.model!].rate * 100) / 100;
      return console.log(JSON.stringify({ motion: motionId, character: characterId, dollars, files: [v.out] }));
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
