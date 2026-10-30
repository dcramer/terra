#!/usr/bin/env node
// Uthana's text-to-motion from the command line (D532): our body uploaded once as a character,
// then a prompt made into a clip retargeted onto its skeleton and saved as a GLB on it, carried
// and checked by carry.ts (D533), for tools/characters' checks to measure. Paid credits, so the
// clips are ours (uthana.com/terms).
// API: https://uthana.com/docs/api (GraphQL, Basic auth with the key as the user).
//
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { Glb } from "../characters/glb.ts";
import { env, sleep } from "../tripo/studio.ts";
import { against, bare, faults, lift, lifted, Refused, type Stand, withSkin } from "./carry.ts";

const API = "https://uthana.com";
export const BODY = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "assets", "models", "character", "base.glb");
// Uthana's stock character ("Manny Fingers", UE5's mannequin): every take is checked against
// the same motion on it.
export const REFERENCE = "c7uJKSavZ5aC";
// Characters uploaded, by the hash of what was uploaded, so a changed body is uploaded again.
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
  motion "<prompt>" -o clip.glb        make a clip on our body and save it on its skeleton, checked
      --model 1|2|3 (2)  --length s (5)  --seed n  --fps 24|30|60 (30)  --in-place  --foot-ik
      --body file.glb (default the base body)  --loop s (its end eased into its start over s seconds)
      --from id (a motion already made, looped by --loop or else downloaded again, free)
  video clip.mp4 -o clip.glb           a clip from a video of one person (2-60 s, $0.05 a second sent), checked
      --playback x (1; 2 for a video slowed twice)  --fps  --in-place  --loop s  --from id  --body  as for motion
      --most $ (0.5: refuses a longer video)
  loco -o clip.glb                     a walk or run by Uthana's locomotion ($0.02 a second), looping, checked
      --speed m/s (1.2)  --strides 1-5 (4)  --style id (neutral_male_a)  --fps  --in-place  --body
  styles                               the styles loco takes
  stitch <id[:from-to]> <id[:from-to]> -o clip.glb   two motions joined by Uthana's transition ($0.01 a second), checked
      --over s (0.5)  --fps  --in-place  --body
  schema                              the arguments the live API takes for each mutation`;

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

/** Our body's character on Uthana: the body as `bare` gives it, uploaded once. */
export async function character(file: string = BODY): Promise<string> {
  if (!existsSync(file)) throw new Failed(`no body at ${file}`);
  const bytes = bare(readFileSync(file));
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
    return await finished(made.create_text_to_motion_job.job.id);
  }
  const data = await gql<{ create_text_to_motion: { motion: { id: string } } }>(
    "mutation ($p: String!, $m: String, $c: String, $l: Float, $s: Int, $f: Boolean) { create_text_to_motion(prompt: $p, model: $m, character_id: $c, length: $l, seed: $s, foot_ik: $f) { motion { id name } } }",
    { p: prompt, m: model.id, c: o.character, l: o.length, s: o.seed, f: o.footIk },
  );
  return data.create_text_to_motion.motion.id;
}

/** A job's motion once it has finished, polled every 5 s as the docs ask. */
async function finished(jobId: string): Promise<string> {
  const started = Date.now();
  for (;;) {
    await sleep(5000);
    const { job } = await gql<{ job: { status: string; result?: { result?: { id: string } } } }>("query ($j: String) { job(job_id: $j) { id status result } }", { j: jobId });
    if (job.status === "FINISHED" && job.result?.result?.id) return job.result.result.id;
    if (job.status === "FAILED") throw new Failed(`the job failed: ${JSON.stringify(job.result).slice(0, 200)}`);
    if (Date.now() - started > 600_000) throw new Failed(`job ${jobId} took over 10 minutes: poll it again later, it is not charged twice (a file sent again within a day returns the same job)`);
  }
}

// Video-to-motion's input limits (docs/api/capabilities/video-to-motion, 2026-10-30): charged
// by the second of video sent ($0.05), so a video is trimmed to what is wanted first.
const VIDEO = { types: /\.(mp4|mov|avi)$/i, seconds: [2, 60] };
export const VIDEO_RATE = 0.05;

/** Why a video of `seconds` is not sent at a cap of `most` dollars, or null: Uthana takes 2-60 s,
 * and charges for every second sent, so a long one is refused before it costs anything. */
export function videoRefused(file: string, seconds: number, most: number): string | null {
  if (seconds < VIDEO.seconds[0] || seconds > VIDEO.seconds[1]) return `${file} runs ${seconds.toFixed(2)} s; Uthana takes 2-60 s: trim it with ffmpeg (-ss, -t)`;
  const dollars = seconds * VIDEO_RATE;
  if (dollars > most) return `${file} runs ${seconds.toFixed(2)} s, $${dollars.toFixed(2)} at $${VIDEO_RATE} a second, over --most ${most}: trim it to the stretch wanted, or raise --most`;
  return null;
}

/** A video's length in seconds, by ffprobe. */
function videoSeconds(file: string): number {
  const probe = spawnSync("ffprobe", ["-v", "error", "-show_entries", "format=duration", "-of", "csv=p=0", file], { encoding: "utf8" });
  const seconds = Number(probe.stdout.trim());
  if (probe.status !== 0 || !Number.isFinite(seconds)) throw new Failed(`ffprobe could not read ${file}: ${probe.stderr.slice(0, 200)} (install ffmpeg)`);
  return seconds;
}

/** A motion from a video of one person, whole body in frame, from a still camera
 * (create_video_to_motion, model video-to-motion-2.0), retargeted onto `character`. */
export async function video(file: string, o: { character: string; name: string }): Promise<string> {
  if (!VIDEO.types.test(file)) throw new Failed(`${file}: Uthana takes .mp4, .mov or .avi; convert it with ffmpeg first`);
  const query = "mutation ($file: Upload!, $n: String!, $c: String) { create_video_to_motion(file: $file, motion_name: $n, character_id: $c) { job { id status } } }";
  const form = new FormData();
  form.append("operations", JSON.stringify({ query, variables: { file: null, n: o.name, c: o.character } }));
  form.append("map", JSON.stringify({ 0: ["variables.file"] }));
  form.append("0", new Blob([readFileSync(file)]), basename(file));
  const made = await gql<{ create_video_to_motion: { job: { id: string } } }>(query, {}, form);
  return await finished(made.create_video_to_motion.job.id);
}

/** A walk or run made by Uthana's locomotion 1.0 ($0.02 a second, uthana.com/pricing): `strides`
 * left-and-right pairs (1 to 5) at `speed` m/s straight ahead, in a style from `styles()`, its
 * last frame its first (docs/api/capabilities/locomotion). Returns at once, no job. */
export async function locomotion(characterId: string, o: { speed: number; strides: number; style: string }): Promise<string> {
  if (!(o.speed >= 0.25 && o.speed <= 8)) throw new Failed(`locomotion runs at 0.25 to 8 m/s, not ${o.speed}`);
  if (!(Number.isInteger(o.strides) && o.strides >= 1 && o.strides <= 5)) throw new Failed(`locomotion makes 1 to 5 stride pairs, not ${o.strides}`);
  const data = await gql<{ create_locomotion: { motion: { id: string } } }>(
    "mutation ($c: String!, $n: Int, $v: Float, $s: String, $a: Float) { create_locomotion(character_id: $c, strides: $n, move_speed: $v, style_id: $s, travel_angle: $a) { motion { id name } } }",
    { c: characterId, n: o.strides, v: o.speed, s: o.style, a: 0 },
  );
  return data.create_locomotion.motion.id;
}

/** The styles Uthana's locomotion walks and runs in (docs/api/capabilities/locomotion). */
export async function styles(): Promise<string[]> {
  const data = await gql<{ locomotion_styles: unknown }>("query { locomotion_styles }");
  const list = data.locomotion_styles;
  return Array.isArray(list) ? list.map((s) => (typeof s === "string" ? s : JSON.stringify(s))) : [JSON.stringify(list)];
}

/** What is left of the account's pay-as-you-go credit, dollars: a call's cost is the fall in it. */
export async function balance(): Promise<number> {
  const data = await gql<{ org: { payg_total_usd: string } }>("{ org { payg_total_usd } }");
  return Number(data.org.payg_total_usd);
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

/** A stretch of a motion for `stitch`: `id`, or `id:from-to` in seconds, either end left open. */
export function stretch(spec: string): { motion_id: string; range?: { start_seconds?: number; end_seconds?: number } } {
  const [motion_id, span] = spec.split(":");
  if (!span) return { motion_id };
  const [from, to] = span.split("-");
  if (!/^[\d.]*$/.test(from) || !/^[\d.]*$/.test(to ?? "")) throw new Failed(`${spec}: a stretch is motion:from-to in seconds, either end left open`);
  return { motion_id, range: { ...(from ? { start_seconds: Number(from) } : {}), ...(to ? { end_seconds: Number(to) } : {}) } };
}

/** The three steps Uthana's `stitch` takes (docs/api/capabilities/stitch-loop-motions): a
 * stretch, a transition of 0.1-3 s its model makes (quantized to 20 Hz), a stretch. */
export function stitchSteps(first: string, second: string, over: number): Record<string, unknown>[] {
  if (!(over >= 0.1 && over <= 3)) throw new Failed(`a transition runs 0.1 to 3 s, not ${over}`);
  return [{ type: "motion", ...stretch(first) }, { type: "transition", duration_seconds: over }, { type: "motion", ...stretch(second) }];
}

/** Two motions joined by Uthana's transition model, $0.01 a second of what comes out. */
export async function stitched(characterId: string, first: string, second: string, over: number): Promise<string> {
  const data = await gql<{ stitch: { motion: { id: string } } }>("mutation ($i: StitchInput!) { stitch(input: $i) { motion { id name } } }", {
    i: { character_id: characterId, steps: stitchSteps(first, second, over) },
  });
  return data.stitch.motion.id;
}

// `playback` is Uthana's playback_speed (0.1-2): a video filmed slowed is downloaded at its
// real speed, 2 for one shot at 50 a second and shown at 25.
async function fetched(characterId: string, motionId: string, fps: string, inPlace: boolean, playback = 1): Promise<Buffer> {
  const query = new URLSearchParams({ fps, ...(inPlace ? { in_place: "true" } : {}), ...(playback !== 1 ? { playback_speed: String(playback) } : {}) });
  const res = await fetch(`${API}/motion/animation/motion_viewer/${characterId}/${motionId}/glb/clip.glb?${query}`, { headers: { authorization: auth() } });
  if (!res.ok) throw new Failed(`download failed (${res.status}): ${(await res.text()).slice(0, 200)}`);
  return Buffer.from(await res.arrayBuffer());
}

/** A motion on our body saved to `out` as a GLB on the body's skeleton, ready for
 * made_clips.py: the body's skin added, its feet put on our ground (`lift`), and checked against
 * the same motion on Uthana's reference character (`against`; downloads are free). A take that
 * does not stand as the reference does is saved beside `out` as `.refused.glb` and refused. */
export async function download(motionId: string, out: string, o: { fps: string; inPlace: boolean; body?: string; playback?: number }): Promise<{ lift: number; stand: Stand }> {
  const body = o.body ?? BODY;
  const ours = await fetched(await character(body), motionId, o.fps, o.inPlace, o.playback);
  const dir = mkdtempSync(join(tmpdir(), "uthana-"));
  const refFile = join(dir, "reference.glb");
  writeFileSync(refFile, await fetched(REFERENCE, motionId, o.fps, o.inPlace, o.playback));
  const figure = new Glb(body);
  const reference = new Glb(refFile);
  const by = lift(figure, reference);
  const take = join(dir, "take.glb");
  writeFileSync(take, lifted(withSkin(ours, readFileSync(body)), by));
  const clip = (g: Glb) => g.clip(g.gltf.animations?.[0]?.name as string);
  const made = new Glb(take);
  const [mine, theirs] = [clip(made), clip(reference)];
  if (!mine || !theirs) throw new Failed(`motion ${motionId} came with no animation`);
  const stand = against(figure, mine, reference, theirs);
  const wrong = faults(stand);
  mkdirSync(dirname(out), { recursive: true });
  if (wrong.length) {
    const refused = out.replace(/\.glb$/, ".refused.glb");
    copyFileSync(take, refused);
    throw new Refused(`motion ${motionId} does not stand on our body as on Uthana's reference: ${wrong.join("; ")} (kept at ${refused})`);
  }
  copyFileSync(take, out);
  return { lift: by, stand };
}

const round = (x: number) => Math.round(x * 1000) / 1000;

export async function main(argv: string[]): Promise<void> {
  const { values: v, positionals: [command, arg, second] } = parseArgs({
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
      body: { type: "string" },
      loop: { type: "string" },
      from: { type: "string" },
      over: { type: "string", default: "0.5" },
      playback: { type: "string", default: "1" },
      most: { type: "string", default: "0.5" },
      speed: { type: "string", default: "1.2" },
      strides: { type: "string", default: "4" },
      style: { type: "string", default: "neutral_male_a" },
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
    case "styles":
      return console.log((await styles()).join("\n"));
    case "schema": {
      const data = await gql<{ __schema: { mutationType: { fields: { name: string; args: { name: string }[] }[] } } }>(
        "{ __schema { mutationType { fields { name args { name } } } } }",
      );
      for (const f of data.__schema.mutationType.fields) if (/motion|character|stitch/.test(f.name)) console.log(`${f.name}(${f.args.map((a) => a.name).join(", ")})`);
      return;
    }
    case "stitch": {
      if (!arg || !second || !v.out) throw new Failed("stitch needs two motions and -o clip.glb: node tools/uthana/uthana.ts stitch mGuard:0-0.6 mBlow:0.5- --over 0.4 -o blow.glb");
      const body = v.body ?? BODY;
      const characterId = await character(body);
      const before = await balance();
      const motionId = await stitched(characterId, arg, second, Number(v.over));
      const dollars = Math.round((before - (await balance())) * 10000) / 10000;
      console.log(JSON.stringify({ motion: motionId, character: characterId, dollars }));
      const { lift, stand } = await download(motionId, v.out, { fps: v.fps!, inPlace: v["in-place"]!, body });
      return console.log(JSON.stringify({ lift: round(lift), stand: Object.fromEntries(Object.entries(stand).map(([k, x]) => [k, round(x)])), files: [v.out] }));
    }
    case "motion": {
      if ((!arg && !v.from) || !v.out) throw new Failed('motion needs a prompt and -o clip.glb: node tools/uthana/uthana.ts motion "a person idles, sword in hand" -o idle.glb');
      const length = Number(v.length);
      const body = v.body ?? BODY;
      const characterId = await character(body);
      const before = await balance();
      const made = v.from ?? (await motion(arg, { model: v.model!, length, seed: v.seed ? Number(v.seed) : undefined, footIk: v["foot-ik"]!, character: characterId }));
      const motionId = v.loop ? await looped(characterId, made, Number(v.loop)) : made;
      // What it cost, by the account's credit (generating and looping are both charged).
      const dollars = Math.round((before - (await balance())) * 10000) / 10000;
      console.log(JSON.stringify({ motion: motionId, character: characterId, dollars }));
      const { lift, stand } = await download(motionId, v.out, { fps: v.fps!, inPlace: v["in-place"]!, body });
      return console.log(JSON.stringify({ lift: round(lift), stand: Object.fromEntries(Object.entries(stand).map(([k, x]) => [k, round(x)])), files: [v.out] }));
    }
    case "video": {
      if ((!arg && !v.from) || !v.out) throw new Failed("video needs a video and -o clip.glb: node tools/uthana/uthana.ts video sprint.mp4 -o sprint.glb");
      const body = v.body ?? BODY;
      const characterId = await character(body);
      let made = v.from;
      const before = await balance();
      if (!made) {
        const refused = videoRefused(arg, videoSeconds(arg), Number(v.most));
        if (refused) throw new Failed(refused);
        made = await video(arg, { character: characterId, name: basename(arg).replace(/\.[^.]+$/, "") });
      }
      const motionId = v.loop ? await looped(characterId, made, Number(v.loop)) : made;
      const dollars = Math.round((before - (await balance())) * 10000) / 10000;
      console.log(JSON.stringify({ motion: motionId, made, character: characterId, dollars }));
      const { lift, stand } = await download(motionId, v.out, { fps: v.fps!, inPlace: v["in-place"]!, body, playback: Number(v.playback) });
      return console.log(JSON.stringify({ lift: round(lift), stand: Object.fromEntries(Object.entries(stand).map(([k, x]) => [k, round(x)])), files: [v.out] }));
    }
    case "loco": {
      if (!v.out) throw new Failed("loco needs -o clip.glb: node tools/uthana/uthana.ts loco --speed 3.7 --style neutral_male_a -o jog.glb");
      const body = v.body ?? BODY;
      const characterId = await character(body);
      const before = await balance();
      const motionId = await locomotion(characterId, { speed: Number(v.speed), strides: Number(v.strides), style: v.style! });
      const dollars = Math.round((before - (await balance())) * 10000) / 10000;
      console.log(JSON.stringify({ motion: motionId, character: characterId, dollars }));
      const { lift, stand } = await download(motionId, v.out, { fps: v.fps!, inPlace: v["in-place"]!, body });
      return console.log(JSON.stringify({ lift: round(lift), stand: Object.fromEntries(Object.entries(stand).map(([k, x]) => [k, round(x)])), files: [v.out] }));
    }
    default:
      throw new Failed(`no command ${command}\n${USAGE}`);
  }
}

if (import.meta.main) {
  try {
    await main(process.argv.slice(2));
  } catch (e) {
    if (!(e instanceof Failed || e instanceof Refused)) throw e;
    console.error(`✗ ${e.message}`);
    process.exit(1);
  }
}
