#!/usr/bin/env node
// Make, rig and animate 3D models with Tripo, on our Studio subscription's credits. See README.md.
// Node 22.18 or later, which runs TypeScript directly; no dependencies.

import { readFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, extname, join } from "node:path";
import { parseArgs } from "node:util";
import { recordSpend, recordedSpends, spendId } from "../token-stats/report.mjs";
import { type ImageRef, Studio, StudioError, log, sleep } from "./studio.ts";

// Generation models, by the names the web app shows.
const MODELS: Record<string, string> = {
  "h3.1": "v3.1-20260211",
  "h3.0": "v3.0-20250812",
  "h2.5": "v2.5-20250123",
  p1: "Nexus-v1.0-20260214",
  p2: "Nexus-v2.0-20260801",
};
const IMAGE_MODELS: Record<string, string> = {
  "gpt-image-2.5": "gpt_image_2.5_sunburst",
  "gpt-image-2": "gpt_image_2",
  "nano-banana-pro": "gemini_3_pro_image_preview",
  "nano-banana-2": "gemini_3.1_flash_image_preview",
  "nano-banana": "gemini_2.5_flash_image_preview",
  midjourney: "midjourney",
};
const VIEWS = ["front", "left", "back", "right"]; // the order Studio uses everywhere
// Rig skeletons. All but "tripo" are the V3 rig the app's Auto Rig sends, its bones named for
// that target; "tripo" is Tripo's own older V1 rig (BoneRoot, Hip, L_Thigh…), which the app
// no longer offers but the server still runs. All come with fingers. "unreal" is the default:
// checked on our base body (2026-10-12), it is the only one whose twist bones carry skin
// (upper arm, forearm, thigh, calf). V1 and "actorcore" have twist and share bones that move
// nothing, and "mixamo" and "unity" have none on the limbs. Our character converter takes V1.
const SKELETONS = ["mixamo", "actorcore", "unreal", "unity", "vrm", "tripo"];
const RIG_V1 = "v1.0-20240301";
const RIG_V3 = "v3.0-20260909";
const EXPORT_FORMATS: Record<string, string> = { glb: "gltf", fbx: "fbx", obj: "obj", stl: "stl", usdz: "usdz", "3mf": "3mf" };
// Commands that cannot spend credits, so need not note any.
const FREE = ["help", "balance", "list", "show", "report", "rename", "cancel", "unanimate"];

type Opt = { type: "string" | "boolean"; short?: string; multiple?: boolean; default?: string | boolean; choices?: string[]; help: string };
type Args = { values: Record<string, any>; positionals: string[] };
type Command = { usage: string; help: string; options?: Record<string, Opt>; run: (client: Studio, a: Args) => Promise<void> };

const out: Record<string, Opt> = { out: { type: "string", short: "o", help: "output file" } };

const COMMANDS: Record<string, Command> = {
  balance: { usage: "balance", help: "credits left on the subscription", run: balance },
  list: {
    usage: "list [-n 20]",
    help: "your projects, newest first",
    options: { n: { type: "string", short: "n", default: "20", help: "how many" } },
    run: list,
  },
  show: { usage: "show PROJECT", help: "a project's state: what has been done to it, its rig and animations, and its steps, newest first", run: show },
  image: {
    usage: 'image "PROMPT" [--ref IMAGE …] [-o out.png]',
    help: "generate images from a prompt, optionally like up to ten reference images",
    options: {
      ref: { type: "string", multiple: true, help: "a reference image (repeatable, up to 10)" },
      model: { type: "string", default: "nano-banana-2", choices: Object.keys(IMAGE_MODELS), help: "the image model (the app's default)" },
      aspect: { type: "string", choices: ["1:1", "16:9", "9:16", "4:3", "3:4"], help: "aspect ratio (default 1:1, or the reference's own)" },
      count: { type: "string", default: "1", choices: ["1", "2", "3", "4"], help: "how many images" },
      "4k": { type: "boolean", help: "4K instead of 1K (costs extra)" },
      "t-pose": { type: "boolean", help: "draw a figure in a T-pose, ready to rig" },
      ...out,
    },
    run: image,
  },
  views: {
    usage: "views IMAGE [-o PREFIX]",
    help: "four views (front, left, back, right) of the subject of one image; writes PREFIX-front.jpeg and so on",
    options: out,
    run: views,
  },
  model: {
    usage: "model IMAGE | --text PROMPT | --views FRONT LEFT BACK RIGHT  [options]",
    help: "make a model from an image, a prompt or several views ('-' for a missing view)",
    options: {
      text: { type: "string", help: "a prompt instead of an image" },
      views: { type: "boolean", help: "the images given are front, left, back and right views" },
      model: { type: "string", default: "h3.1", help: `${Object.keys(MODELS).join(", ")} or a dated version` },
      faces: { type: "string", help: "face limit (default 10000, or 5000 for p1 and p2 as the app has it)" },
      quad: { type: "boolean", help: "quads instead of triangles (default for p2, as the app has it; --no-quad for triangles)" },
      variations: { type: "string", default: "1", choices: ["1", "2", "4"], help: "p2: several models from one input, each its own project" },
      texture: { type: "boolean", default: true, help: "texture the model (--no-texture for geometry only)" },
      geometry: { type: "string", default: "detailed", choices: ["standard", "detailed"], help: "h3.1's geometry; standard is 15 credits less" },
      "texture-quality": { type: "string", default: "detailed", choices: ["standard", "detailed", "extreme"], help: "2K, 4K or 8K; 4K is 10 credits more than 2K" },
      pbr: { type: "boolean", default: true, help: "PBR maps as well as colour, 5 credits (H models; --no-pbr for colour only)" },
      parts: { type: "string", choices: ["simple", "balanced", "detailed"], help: "generate in separate parts (H models)" },
      "smart-poly": { type: "boolean", help: "low-poly topology (H models)" },
      "t-pose": { type: "boolean", help: "with --text: model a figure in a T-pose" },
      public: { type: "boolean", help: "share the project instead of keeping it private" },
      ...out,
    },
    run: model,
  },
  import: {
    usage: "import FILE [-o out.glb]",
    help: "bring in a model of our own (GLB, FBX, OBJ or STL) to texture, rig or animate it",
    options: out,
    run: importModel,
  },
  texture: {
    usage: "texture PROJECT --image IMAGE | --views FRONT LEFT BACK RIGHT | --prompt TEXT  [options]",
    help: "texture a model from an image, views or a prompt",
    options: {
      image: { type: "string", help: "an image to texture from" },
      views: { type: "boolean", help: "texture from the views given after the project" },
      prompt: { type: "string", help: "describe the texture instead" },
      quality: { type: "string", default: "detailed", choices: ["standard", "detailed", "extreme"], help: "2K, 4K or 8K (4K is the app's default)" },
      align: { type: "string", default: "original_image", choices: ["original_image", "geometry"], help: "follow the image or the shape" },
      delight: { type: "boolean", help: "remove the lighting baked into the image" },
      ...out,
    },
    run: texture,
  },
  upscale: {
    usage: "upscale PROJECT [--8k]",
    help: "raise a textured model's texture to 4K or 8K",
    options: { "8k": { type: "boolean", help: "8K instead of 4K" }, ...out },
    run: upscale,
  },
  pbr: { usage: "pbr PROJECT", help: "add normal, roughness and metallic maps to a textured model", options: out, run: pbr },
  retopo: {
    usage: "retopo PROJECT [--faces 10000] [--no-quad] [--smart-poly]",
    help: "rebuild a model's mesh at a face budget",
    options: {
      faces: { type: "string", default: "10000", help: "face limit" },
      quad: { type: "boolean", default: true, help: "quads, as the app's default (--no-quad for triangles, 5 credits less)" },
      "smart-poly": { type: "boolean", help: "hand-made-looking low-poly topology (costs extra)" },
      ...out,
    },
    run: retopo,
  },
  segment: {
    usage: "segment PROJECT [--detail balanced]",
    help: "split a model into parts",
    options: { detail: { type: "string", default: "balanced", choices: ["simple", "balanced", "detailed"], help: "how finely" }, ...out },
    run: segment,
  },
  fill: {
    usage: "fill PROJECT [--ai]",
    help: "close the holes left by segmenting, or with --ai rebuild the hidden parts",
    options: { ai: { type: "boolean", help: "AI completion instead of a quick cap" }, ...out },
    run: fill,
  },
  uv: { usage: "uv PROJECT", help: "unwrap a model's UVs again (Smart UV)", options: out, run: uv },
  rig: {
    usage: "rig PROJECT [--skeleton unreal] [--type humanoid|other]",
    help: "rig a model with Tripo's auto-rig",
    options: {
      skeleton: { type: "string", default: "unreal", choices: SKELETONS, help: "a humanoid's bone names; tripo is the older V1 rig our character converter takes" },
      type: { type: "string", default: "humanoid", choices: ["humanoid", "other"], help: "other rigs an animal or creature as Tripo reads its shape" },
      anyway: { type: "boolean", help: "rig a humanoid even when Tripo's check says it is not ready" },
      ...out,
    },
    run: rig,
  },
  animate: {
    usage: 'animate PROJECT [MOTION …] [--prompt "TEXT" …] [-o out.glb]',
    help: "put preset animations (walk, run, idle, jump…) or ones made from a description on a rigged model, and export them in one file",
    options: {
      prompt: { type: "string", multiple: true, help: "a motion made from a description (repeatable; 20 credits each)" },
      seconds: { type: "string", default: "5", help: "length of a --prompt motion, 1 to 10" },
      format: { type: "string", default: "glb", choices: ["glb", "fbx"], help: "file format" },
      "in-place": { type: "boolean", help: "keep the root still instead of moving through the world" },
      ...out,
    },
    run: animate,
  },
  export: {
    usage: "export PROJECT [--format glb] [--animations [NAME …]]",
    help: "export a model in another format, with its animations if asked",
    options: {
      format: { type: "string", default: "glb", choices: Object.keys(EXPORT_FORMATS), help: "file format; OBJ comes zipped with its textures" },
      "texture-size": { type: "string", choices: ["512", "1024", "2048", "4096", "8192"], help: "texture size (default the model's own: 2048, or 4096 or 8192 when upscaled)" },
      "fbx-for": { type: "string", default: "blender", choices: ["blender", "3dsmax", "mixamo"], help: "the FBX flavour" },
      animations: { type: "boolean", help: "include the animations named after the project, or all of them" },
      "in-place": { type: "boolean", help: "keep the root still" },
      ...out,
    },
    run: exportModel,
  },
  download: {
    usage: "download PROJECT [-o out.glb]",
    help: "download a project's model as Studio stores it (meshopt-compressed GLB)",
    options: out,
    run: download,
  },
  edit: {
    usage: "edit PROJECT --box X0,Y0,Z0,X1,Y1,Z1 [--faces N]",
    help: "remake the part of an untextured P2 Smart Mesh inside a box (Mesh Edit)",
    options: {
      box: { type: "string", help: "the box's corners in the model's coordinates as Studio stores it, each -1 to 1 (required)" },
      faces: { type: "string", help: "face limit (default the model's own, at most 50,000, as the app does)" },
      ...out,
    },
    run: edit,
  },
  unanimate: {
    usage: "unanimate PROJECT MOTION …",
    help: "remove animations from a rigged model",
    run: unanimate,
  },
  cancel: { usage: "cancel PROJECT", help: "cancel the operation running on a project", run: cancel },
  restore: {
    usage: "restore PROJECT OPERATOR",
    help: "roll a project back to an earlier step (an operator id from `show`'s steps)",
    options: out,
    run: restore,
  },
  fork: {
    usage: "fork PROJECT [OPERATOR]",
    help: "copy a project, at its latest step or an earlier one, into a new project",
    run: fork,
  },
  rename: { usage: 'rename PROJECT "NAME"', help: "rename a project", run: rename },
  report: {
    usage: "report [--all]",
    help: "note this account's Tripo spending for the token stats (every command does this too)",
    options: { all: { type: "boolean", help: "the whole history, not only the latest 100 charges" } },
    run: async (client, a) => noteSpending(client, a.values.all, false),
  },
};

async function main(argv: string[]): Promise<void> {
  const name = argv[0];
  const command = COMMANDS[name];
  if (!command || name === "help" || argv.includes("--help") || argv.includes("-h")) {
    console.log(command ? commandHelp(name, command) : overview());
    if (!command && name && !["help", "--help", "-h"].includes(name)) process.exitCode = 2;
    return;
  }
  let a: Args;
  try {
    a = parse(command, argv.slice(1));
  } catch (err) {
    console.error(`tripo ${name}: ${(err as Error).message}\n\n${commandHelp(name, command)}`);
    process.exitCode = 2;
    return;
  }
  let client: Studio | undefined;
  try {
    client = new Studio();
    await command.run(client, a);
  } catch (err) {
    // Tripo's answers are StudioErrors; anything else is a bug here, so keep its stack.
    console.error(err instanceof StudioError ? `tripo: ${err.message}` : err);
    process.exitCode = 1;
  } finally {
    if (client && !FREE.includes(name)) await noteSpending(client, false, true);
  }
}

function parse(command: Command, argv: string[]): Args {
  const options = command.options ?? {};
  const defined = (o: object) => Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined));
  const spec = Object.fromEntries(Object.entries(options).map(([k, o]) => [k, defined({ type: o.type, short: o.short, multiple: o.multiple, default: o.default })]));
  const { values, positionals } = parseArgs({ args: argv, options: spec as any, allowPositionals: true, allowNegative: true, strict: true });
  for (const [k, o] of Object.entries(options)) {
    const v = values[k];
    if (o.choices && typeof v === "string" && !o.choices.includes(v)) throw new Error(`--${k} must be one of ${o.choices.join(", ")}`);
  }
  return { values, positionals };
}

function overview(): string {
  const width = Math.max(...Object.keys(COMMANDS).map((n) => n.length));
  const lines = Object.entries(COMMANDS).map(([n, c]) => `  ${n.padEnd(width)}  ${c.help}`);
  return `Make, rig and animate 3D models with Tripo, on our Studio subscription's credits.

  node tools/tripo/tripo.ts <command> [options]

${lines.join("\n")}

Every command that makes or changes a model prints one line of JSON with the project it worked
on, so the next can take it. PROJECT is that id or the project's Studio URL. See README.md, and
\`node tools/tripo/tripo.ts <command> --help\` for a command's options.`;
}

function commandHelp(name: string, c: Command): string {
  const opts = Object.entries(c.options ?? {}).map(([k, o]) => {
    const flag = `${o.short ? `-${o.short}, ` : ""}--${k}${o.type === "string" ? " VALUE" : ""}`;
    const extra = [o.choices ? `one of ${o.choices.join(", ")}` : "", o.default !== undefined && o.type === "string" ? `default ${o.default}` : ""].filter(Boolean);
    return `  ${flag.padEnd(26)} ${o.help}${extra.length ? ` (${extra.join("; ")})` : ""}`;
  });
  return `${c.help}\n\n  node tools/tripo/tripo.ts ${c.usage}${opts.length ? `\n\n${opts.join("\n")}` : ""}`;
}

// Commands.

async function balance(client: Studio): Promise<void> {
  report({ credits: await client.credits() });
}

async function list(client: Studio, a: Args): Promise<void> {
  const n = int(a.values.n, "-n");
  const data = await client.call("GET", "/v2/studio/assets/v2", undefined, { asset_type: "mine", type: "all", offset: 0, size: n, locale: "en" });
  for (const p of (data.projects ?? []).slice(0, n)) console.log(`${p.id}  ${String(p.type ?? "").padEnd(10)} ${p.project_name ?? ""}`);
}

async function show(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const d = await client.project(pid);
  const history = (await client.call("GET", `/v2/studio/project/history/${pid}`)).history ?? [];
  const op = d.operator ?? {};
  const rig = op.rigging ?? {};
  const flags = Object.entries(op)
    .filter(([k, v]) => k.startsWith("is_") && v === true && k !== "is_owner" && k !== "is_latest_operator")
    .map(([k]) => k.slice(3));
  console.log(
    JSON.stringify(
      {
        project: d.id,
        name: d.project_name,
        head: op.operator_id,
        made_with: op.model_version,
        flags,
        running: d.running_operator?.type,
        rig: rig.type ? { type: rig.type, status: rig.status } : undefined,
        animations: (op.retarget ?? []).map((r: any) => ({ name: r.name.split(":").at(-1), status: r.status })),
        steps: history.map((h: any) => ({ operator: h.operator_id, type: h.type, at: new Date(h.created_at * 1000).toISOString() })),
      },
      null,
      2,
    ),
  );
}

async function image(client: Studio, a: Args): Promise<void> {
  const prompt = one(a, "a prompt");
  const v = a.values;
  const refs: string[] = v.ref ?? [];
  if (refs.length > 10) throw new StudioError("at most 10 reference images");
  if (refs.length && v.model === "midjourney") throw new StudioError("Midjourney takes no reference images");
  const body: Record<string, unknown> = {
    if_upload: true,
    sketch_to_render: false,
    t_pose: Boolean(v["t-pose"]),
    amount: int(v.count, "--count"),
    model_version: IMAGE_MODELS[v.model],
    prompt,
    resolution: v["4k"] ? "4K" : "1K",
    // As the app does: with references and no ratio chosen, the result keeps theirs.
    scale: v.aspect ?? (refs.length ? undefined : "1:1"),
  };
  const uploaded = [];
  for (const path of refs) uploaded.push(await client.uploadImage(path));
  if (uploaded.length === 1) body.image = uploaded[0];
  else if (uploaded.length) body.images = uploaded;
  const before = await client.credits();
  const started = await client.call("POST", "/v2/studio/image/gen_image_v2", body);
  log(`image ${started.asset_id}, ${started.remaining_free_count} free images left this month`);
  const asset = await client.waitImage(started.asset_id, "image");
  const outputs = asset.output.data;
  const base = v.out ?? `${slug(prompt)}.png`;
  const files = [];
  for (const [i, item] of outputs.entries()) files.push(await client.download(item.url, outputs.length === 1 ? base : numbered(base, i + 1)));
  report({ asset: started.asset_id, credits: before - (await client.credits()), files });
}

async function views(client: Studio, a: Args): Promise<void> {
  const path = one(a, "an image");
  const source = await client.uploadImage(path);
  const before = await client.credits();
  const started = await client.call("POST", "/v2/studio/image/gen_multiview", { if_upload: true, image: source });
  const asset = await client.waitImage(started.asset_id, "views");
  const prefix = a.values.out ?? path.slice(0, path.length - extname(path).length);
  const files = [];
  for (const [i, item] of asset.output.data.slice(0, 4).entries()) {
    files.push(await client.download(item.url, `${prefix}-${VIEWS[i]}${extname(item.key) || ".jpeg"}`));
  }
  report({ asset: started.asset_id, credits: before - (await client.credits()), files });
}

async function model(client: Studio, a: Args): Promise<void> {
  const v = a.values;
  const version = MODELS[String(v.model).toLowerCase()] ?? v.model;
  const smartMesh = version.startsWith("Nexus");
  const textured = Boolean(v.texture);
  const variations = int(v.variations, "--variations");
  if (variations > 1 && version !== MODELS.p2) throw new StudioError("--variations needs --model p2");
  const body: Record<string, any> = {
    model_version: version,
    face_limit: int(v.faces ?? (smartMesh ? 5000 : 10000), "--faces"),
    quad: v.quad ?? version === MODELS.p2,
    visibility: v.public ? "shareable" : "private",
  };
  if (!smartMesh) {
    // Smart Mesh refuses every one of these, even when false.
    Object.assign(body, { generate_parts: Boolean(v.parts), smart_poly: Boolean(v["smart-poly"]), texture: textured });
    if (v.parts) body.segmentation_granularity = v.parts;
    if (textured) {
      Object.assign(body, { delight: false, pbr: Boolean(v.pbr), texture_alignment: "original_image", texture_quality: v["texture-quality"] });
    }
    if (version === MODELS["h3.1"]) body.geometry_quality = v.geometry;
  }

  let endpoint: string;
  let name: string;
  let textureFrom: Record<string, unknown>;
  if (v.text) {
    if (a.positionals.length) throw new StudioError("give a prompt with --text or an image, not both");
    endpoint = "text_to_model";
    name = slug(v.text);
    Object.assign(body, { prompt: v.text, gen_image_model_version: "flux.1_dev", sketch_to_render: false, t_pose: Boolean(v["t-pose"]) });
    textureFrom = { prompt_text: v.text };
  } else if (v.views) {
    endpoint = "multiview_to_model";
    name = stem(a.positionals.find((p) => p !== "-") ?? "views");
    body.image = await uploadViews(client, a.positionals);
    textureFrom = { images: body.image };
  } else {
    const path = one(a, "an image, --text or --views");
    endpoint = "image_to_model";
    name = stem(path);
    body.image = await client.uploadImage(path);
    if (!smartMesh) body.enable_image_autofix = false;
    textureFrom = { image: body.image };
  }
  if (version === MODELS.p2) {
    // Whether the subject is symmetric, from the main view; the app takes a failed check as no.
    const front = Array.isArray(body.image) ? body.image[0] : body.image;
    body.symmetry = false;
    if (front) {
      try {
        const check = await client.call("POST", "/v2/studio/operation/symmetry_check", { image: { bucket: front.bucket, key: front.key } });
        body.symmetry = Boolean(check.symmetry);
      } catch (err) {
        log(`symmetry check failed, so none assumed: ${(err as Error).message}`);
      }
    }
  }
  if (variations > 1) {
    body.variations = Array.from({ length: variations }, () => ({ face_limit: body.face_limit }));
    delete body.face_limit;
  }

  let before = await client.credits();
  const reply = await client.call("POST", `/v2/studio/operation/${endpoint}`, body);
  const receipts: any[] = reply.variations ? reply.variations.filter((r: any) => r.accepted !== false) : [Array.isArray(reply) ? reply[0] : reply];
  for (const [i, receipt] of receipts.entries()) {
    log(`project ${receipt.project_id}`);
    await client.wait(receipt.operator_id, "model");
    let op = receipt.operator_id;
    if (smartMesh && textured) {
      // Smart Mesh comes untextured; the app textures it in a second step, as we do, with
      // the app's settings for it.
      log("Smart Mesh comes untextured; texturing it");
      op = await textureModel(client, receipt.project_id, textureFrom, v["texture-quality"], "original_image", true);
    }
    const path = v.out ?? `${name}.glb`;
    await finish(client, receipt.project_id, op, receipts.length > 1 ? numbered(path, i + 1) : path, before);
    before = await client.credits();
  }
}

async function importModel(client: Studio, a: Args): Promise<void> {
  const path = one(a, "a model file");
  const format = extname(path).slice(1).toLowerCase();
  if (!["glb", "fbx", "obj", "stl"].includes(format)) throw new StudioError("Studio imports GLB, FBX, OBJ or STL");
  const place = await client.put(path, format);
  const identity = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]; // placed as it is, column-major
  const before = await client.credits();
  const receipt = await client.operate("import_user_model", { format, model: place, name: basename(path), transform_matrix: identity, use_original_uv: hasUVs(path, format) });
  await client.wait(receipt.operator_id, "import");
  await finish(client, receipt.project_id, receipt.operator_id, a.values.out ?? `${stem(path)}-imported.glb`, before);
}

async function texture(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const v = a.values;
  const given = [v.image, v.views, v.prompt].filter(Boolean).length;
  if (given !== 1) throw new StudioError("give one of --image, --views or --prompt");
  const source = v.image
    ? { image: await client.uploadImage(v.image) }
    : v.views
      ? { images: await uploadViews(client, a.positionals.slice(1)) }
      : { prompt_text: v.prompt };
  const before = await client.credits();
  const op = await textureModel(client, pid, source, v.quality, v.align, Boolean(v.delight));
  await finish(client, pid, op, output(a, pid, "textured"), before);
}

async function upscale(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  await operation(client, a, pid, "texture_upscaler", { model_version: "v3.0-20250812", project_id: pid, texture_quality: a.values["8k"] ? "extreme" : "detailed" }, "upscaled");
}

async function pbr(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  await operation(client, a, pid, "pbr_generate", { model_version: "v3.0-20250812", project_id: pid }, "pbr");
}

async function retopo(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const body = {
    bake: true,
    face_limit: int(a.values.faces, "--faces"),
    model_version: "default",
    part_name_list: await partNames(client, pid),
    project_id: pid,
    quad: Boolean(a.values.quad),
    smart_poly: Boolean(a.values["smart-poly"]),
  };
  await operation(client, a, pid, "remesh", body, "retopo");
}

async function segment(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  await operation(client, a, pid, "ai_segmentation", { model_version: "v2.0-20260430", project_id: pid, segmentation_granularity: a.values.detail }, "segmented");
}

async function fill(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const names = await partNames(client, pid);
  if (a.values.ai) await operation(client, a, pid, "ai_completion", { model_version: "v1.0-20250506", part_names: names, project_id: pid }, "completed");
  else await operation(client, a, pid, "mesh_fill", { model_version: "default", part_names: names, project_id: pid }, "filled");
}

// Smart UV makes a candidate unwrap beside the model, which then replaces the model's own.
async function uv(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  await ready(client, pid);
  const head = (await client.project(pid)).operator.operator_id;
  const current = { project_id: pid, current_operator_id: head };
  const before = await client.credits();
  let context = await client.call("POST", "/v2/studio/operation/uv_edit/context", current);
  if (context.running_task?.operator_id) {
    log("waiting for the unwrap already running on this project");
    await client.wait(context.running_task.operator_id, "uv");
    context = await client.call("POST", "/v2/studio/operation/uv_edit/context", current);
  }
  const started = await client.call("POST", "/v2/studio/operation/uv_edit/generate", { ...current, action: context.next_action || "generate" });
  await client.wait(started.operator_id, "uv");
  await client.call("POST", "/v2/studio/operation/uv_edit/apply", { ...current, candidate_operator_id: started.operator_id });
  await finish(client, pid, undefined, output(a, pid, "uv"), before);
}

// As the app's Auto Rig: a humanoid is rigged as a biped on the skeleton chosen; anything
// else as the free pre-check reads it (quadruped, avian, serpentine…), with no choice of bones.
async function rig(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const { skeleton, type } = a.values;
  const version = skeleton === "tripo" ? RIG_V1 : RIG_V3;
  await ready(client, pid);
  const check = await client.call("POST", "/v2/studio/operation/pre_rig_check", { model_version: version, project_id: pid });
  if (type === "humanoid" && !check.riggable && !a.values.anyway) {
    throw new StudioError(`Tripo reads this as not ready to rig as a humanoid (${JSON.stringify(check)}). A figure rigs best in a T-pose or A-pose; --anyway tries regardless, as the app offers.`);
  }
  const rigType = type === "humanoid" ? "biped" : check.rig_type || "others";
  const body: Record<string, unknown> = { model_version: version, project_id: pid, rig_type: rigType };
  if (version === RIG_V3 && type === "humanoid") body.spec = skeleton;
  log(`rigging a ${rigType}`);
  await operation(client, a, pid, "rigging_model", body, "rigged");
}

async function animate(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const motions = a.positionals.slice(1);
  const prompts: string[] = a.values.prompt ?? [];
  if (!motions.length && !prompts.length) throw new StudioError("name at least one motion (walk, run, idle…) or give --prompt");
  await ready(client, pid);
  const rigging = (await client.project(pid)).operator.rigging ?? {};
  if (rigging.status !== "success") throw new StudioError("the model is not rigged yet: run `tripo.ts rig` first");
  if (prompts.length && rigging.type !== "biped") throw new StudioError(`motions made from a description need a humanoid rig; this one is ${rigging.type}`);
  const before = await client.credits();
  const ids = [];
  for (const m of motions) ids.push(await retarget(client, pid, rigging.type, { animation: `preset:${rigging.type}:${m}` }));
  for (const p of prompts) ids.push(await retarget(client, pid, rigging.type, { motion: await aiMotion(client, p, int(a.values.seconds, "--seconds")) }));
  const format = a.values.format;
  const size = textureSize((await client.project(pid)).operator);
  const path = await exportFile(client, pid, output(a, pid, "animated", `.${format}`), format, ids, Boolean(a.values["in-place"]), size);
  report({ project: pid, credits: before - (await client.credits()), files: [path] });
}

async function exportModel(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const v = a.values;
  let ids: string[] = [];
  if (v.animations) {
    const wanted = a.positionals.slice(1);
    const done = ((await client.project(pid)).operator.retarget ?? []).filter((r: any) => r.status === "success");
    const short = (r: any) => r.name.split(":").at(-1);
    ids = done.filter((r: any) => !wanted.length || wanted.includes(short(r))).map((r: any) => r.operator_id);
    if (wanted.length && ids.length < wanted.length) throw new StudioError(`animations ${wanted.join(", ")} asked for; the project has ${done.map(short).join(", ") || "none"}`);
  }
  const ext = v.format === "obj" ? ".zip" : `.${v.format}`; // OBJ comes zipped with its textures
  const before = await client.credits();
  const size = v["texture-size"] ? int(v["texture-size"], "--texture-size") : textureSize((await client.project(pid)).operator);
  const path = await exportFile(client, pid, output(a, pid, "export", ext), v.format, ids, Boolean(v["in-place"]), size, v["fbx-for"]);
  report({ project: pid, credits: before - (await client.credits()), files: [path] });
}

// Mesh Edit: the app lets a box be drawn on an untextured P2 model that is the project's
// latest step, and remakes what is inside it.
async function edit(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const box = String(a.values.box ?? "").split(",").map(Number);
  if (box.length !== 6 || box.some((n) => !Number.isFinite(n))) throw new StudioError("give the box as --box X0,Y0,Z0,X1,Y1,Z1");
  const clamp = (n: number) => Math.min(1, Math.max(-1, n));
  const lo = box.slice(0, 3).map((n, i) => clamp(Math.min(n, box[i + 3])));
  const hi = box.slice(0, 3).map((n, i) => clamp(Math.max(n, box[i + 3])));
  await ready(client, pid);
  const op = (await client.project(pid)).operator;
  if (!op.is_nexus_mesh || op.is_textured || op.is_rigged || op.is_segmented || op.is_pbr) {
    throw new StudioError("Mesh Edit works on an untextured P2 Smart Mesh only (model --model p2 --no-texture)");
  }
  const faces = a.values.faces ? int(a.values.faces, "--faces") : Math.min(triangles(await modelGltf(client, pid)), 50000);
  await operation(client, a, pid, "local_edit", { bbox: [lo, hi], face_limit: faces, project_id: pid, source_operator_id: op.operator_id }, "edited");
}

async function unanimate(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const wanted = a.positionals.slice(1);
  if (!wanted.length) throw new StudioError("name the animations to remove");
  await ready(client, pid);
  const detail = await client.project(pid);
  const on = (detail.operator.retarget ?? []).filter((r: any) => r.status === "success");
  const short = (r: any) => r.name.split(":").at(-1);
  const missing = wanted.filter((w) => !on.some((r: any) => short(r) === w));
  if (missing.length) throw new StudioError(`${missing.join(", ")} not on the model; it has ${on.map(short).join(", ") || "none"}`);
  for (const r of on.filter((r: any) => wanted.includes(short(r)))) {
    await client.call("POST", "/v2/studio/operation/remove_retarget", { model_operator_id: detail.operator.operator_id, project_id: pid, retarget_operator_id: r.operator_id });
  }
  report({ project: pid, removed: wanted });
}

async function cancel(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const running = (await client.project(pid)).running_operator;
  if (!running?.operator_id) throw new StudioError("nothing is running on this project");
  await client.call("POST", "/v2/studio/operation/cancel", { operation_type: running.type, operator_id: running.operator_id, scene: "workspace" });
  report({ project: pid, cancelled: running.type ?? running.operator_id });
}

async function restore(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const op = a.positionals[1];
  if (!op) throw new StudioError("give the operator id of the step to go back to");
  await ready(client, pid);
  await client.call("POST", "/v2/studio/operation/restore", { operator_id: op, project_id: pid });
  await finish(client, pid, undefined, output(a, pid, "restored"), await client.credits());
}

async function fork(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const op = a.positionals[1] ?? (await client.project(pid)).operator.operator_id;
  const made = await client.call("POST", "/v2/studio/operation/save_as_new", { operator_id: op, project_id: pid });
  report({ project: made.project_id, from: pid, operator: op });
}

async function rename(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const name = a.positionals[1];
  if (!name || a.positionals.length > 2) throw new StudioError('give the new name, quoted: rename PROJECT "NAME"');
  await client.call("POST", "/v2/studio/operation/update_name", { name, project_id: pid });
  report({ project: pid, name });
}

async function download(client: Studio, a: Args): Promise<void> {
  const pid = projectId(a);
  const detail = await client.project(pid);
  const path = await client.download(detail.model_url, output(a, pid, "model"));
  report({ project: pid, operator: detail.operator.operator_id, files: [path] });
}

// The steps commands share.

// Waits out an operation already running on a project: Studio refuses another until it ends.
async function ready(client: Studio, pid: string): Promise<void> {
  const running = (await client.project(pid)).running_operator;
  if (running?.operator_id) {
    log(`waiting for the ${running.type ?? "operation"} already running on this project`);
    await client.wait(running.operator_id, running.type);
  }
}

// Runs one operation on a project, waits for it and downloads the result.
async function operation(client: Studio, a: Args, pid: string, name: string, body: unknown, suffix: string): Promise<void> {
  await ready(client, pid);
  const before = await client.credits();
  const receipt = await client.operate(name, body);
  await client.wait(receipt.operator_id, name);
  await finish(client, pid, receipt.operator_id, output(a, pid, suffix), before);
}

// Downloads the model an operation made and reports it.
async function finish(client: Studio, pid: string, op: string | undefined, path: string, before: number): Promise<void> {
  const detail = await client.project(pid, op);
  await client.download(detail.model_url, path);
  report({ project: pid, operator: detail.operator.operator_id, credits: before - (await client.credits()), files: [path] });
}

async function textureModel(client: Studio, pid: string, source: Record<string, unknown>, quality: string, align: string, delight: boolean): Promise<string> {
  await ready(client, pid);
  const body = { ...source, project_id: pid, part_names: await partNames(client, pid), texture_quality: quality, texture_alignment: align, delight };
  const receipt = await client.operate("texture_model", body);
  await client.wait(receipt.operator_id, "texture");
  return receipt.operator_id;
}

// Puts one animation on a rigged model, or reuses it if it is already there; returns its id.
// Studio runs one at a time per project and answers 406 to another while one runs.
async function retarget(client: Studio, pid: string, rigType: string, what: { animation?: string; motion?: string }): Promise<string> {
  const existing = (await client.project(pid)).operator.retarget ?? [];
  const found = existing.find(
    (r: any) => r.status === "success" && ((what.animation && r.name === what.animation) || (what.motion && r.motion_asset_id === what.motion)),
  );
  if (found) {
    log(`${what.animation ?? what.motion}: already on the model`);
    return found.operator_id;
  }
  const body = { model_version: "default", project_id: pid, rig_type: rigType, ...(what.animation ? { animations: [what.animation] } : { motion_asset_id: what.motion }) };
  let [status, reply] = await client.send("POST", "/v2/studio/operation/retarget_model", body);
  for (let i = 0; status === 406 && i < 60; i++) {
    await sleep(5000);
    [status, reply] = await client.send("POST", "/v2/studio/operation/retarget_model", body);
  }
  if (reply.code !== 0) throw new StudioError(`animating ${what.animation ?? what.motion} failed (${status}): ${reply.message || JSON.stringify(reply)}`);
  await client.wait(reply.data.operator_id, what.animation ?? "motion");
  return reply.data.operator_id;
}

// A motion made from a description, ready to put on a rig.
async function aiMotion(client: Studio, prompt: string, seconds: number): Promise<string> {
  const task = await client.call("POST", "/v2/studio/motion/generate", { segments: [{ prompt, duration_seconds: seconds }] });
  log(`motion task ${task.task_id}`);
  for (;;) {
    const state = await client.call("POST", "/v2/studio/motion/get_task", { task_id: task.task_id });
    if (state.status === "success") return state.asset_id;
    if (["failed", "error", "cancelled", "expired"].includes(state.status)) throw new StudioError(`motion "${prompt}" ${state.status}`);
    await sleep(4000);
  }
}

async function exportFile(client: Studio, pid: string, path: string, format: string, animations: string[] = [], inPlace = false, textureSize = 2048, fbxFor = "blender"): Promise<string> {
  const body = {
    animate_in_place: inPlace,
    animations,
    bake_animation_frame: 0,
    enable_bake_animation: false,
    export_orientation: "-y",
    export_vertex_colors: false,
    fbx_preset: fbxFor,
    format: EXPORT_FORMATS[format],
    model_version: "default",
    name: stem(path),
    pack_uv: false,
    project_id: pid,
    // The app's panel always sends zip: a GLB still comes back as one file and an OBJ zipped
    // with its textures, but an FBX as a zip too. Its Blender, Max and Maya bridges send
    // embedded, which gives one FBX, so we do for every other format.
    texture_packaging: format === "glb" || format === "obj" ? "zip" : "embedded",
    texture_size: textureSize,
    with_animation: animations.length > 0,
  };
  const receipt = await client.operate("export", body);
  let url = receipt.model_url;
  if (!url) {
    await client.wait(receipt.operator_id, "export");
    url = (await client.call("POST", "/v2/studio/operation/download_with_name", { file_name: stem(path), operator_id: receipt.operator_id })).model_url;
  }
  return client.download(url, path);
}

// The glTF of a project's latest model: its JSON, without the binary.
async function modelGltf(client: Studio, pid: string): Promise<any> {
  const dir = mkdtempSync(join(tmpdir(), "tripo-"));
  try {
    const glb = gltfOf(readFileSync(await client.download((await client.project(pid)).model_url, join(dir, "model.glb"))));
    if (!glb) throw new StudioError("the project's model is not a GLB; cannot read it");
    return glb;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// A GLB's JSON chunk, or undefined if the bytes are not a GLB; exported for the tests.
export function gltfOf(glb: Buffer): any {
  if (glb.length < 20 || glb.toString("latin1", 0, 4) !== "glTF") return undefined;
  return JSON.parse(glb.toString("utf8", 20, 20 + glb.readUInt32LE(12)));
}

// The names of a model's meshes, which texture, retopology and filling act on.
async function partNames(client: Studio, pid: string): Promise<string[]> {
  const doc = await modelGltf(client, pid);
  const names = (doc.nodes ?? []).filter((n: any) => n.mesh !== undefined && n.name).map((n: any) => n.name);
  if (!names.length) throw new StudioError("the project's model has no named meshes");
  return names;
}

// How many triangles a glTF's meshes have; exported for the tests.
export function triangles(doc: any): number {
  let n = 0;
  for (const mesh of doc.meshes ?? []) {
    for (const p of mesh.primitives ?? []) {
      const counted = p.indices !== undefined ? doc.accessors[p.indices] : doc.accessors[p.attributes?.POSITION];
      if ((p.mode ?? 4) === 4 && counted) n += Math.floor(counted.count / 3);
    }
  }
  return n;
}

// Whether a model file of ours has texture coordinates for Studio to keep on import.
function hasUVs(path: string, format: string): boolean {
  if (format === "stl") return false;
  if (format === "obj") return /^vt\s/m.test(readFileSync(path, "latin1"));
  if (format === "glb") return Boolean(gltfOf(readFileSync(path))?.meshes?.some((m: any) => m.primitives?.some((p: any) => p.attributes?.TEXCOORD_0 !== undefined)));
  return true; // FBX: too costly to read here; Studio copes when there are none
}

// The texture size a model was made at, which the app exports at unless asked otherwise.
export function textureSize(op: any): number {
  return op?.is_ultra_textured ? 8192 : op?.is_hd_textured ? 4096 : 2048;
}

async function uploadViews(client: Studio, paths: string[]): Promise<(ImageRef | null)[]> {
  if (paths.length > 4 || paths[0] === undefined || paths[0] === "-" || paths.filter((p) => p !== "-").length < 2) {
    throw new StudioError("give the front view and at least one more, in the order front left back right ('-' for a missing one)");
  }
  const refs: (ImageRef | null)[] = [];
  for (const p of paths) refs.push(p === "-" ? null : await client.uploadImage(p));
  while (refs.length < 4) refs.push(null);
  return refs;
}

// The token stats (tools/token-stats) count Tripo credits beside Claude tokens.

// Notes the account's charges for the token stats from Tripo's own history, so web-app work
// counts too and amounts are exact; report.mjs sends them. Each operation is one entry by its
// id, so noting it again is harmless. Pending charges wait for the next run; cancelled and
// failed ones cost nothing.
async function noteSpending(client: Studio, everything = false, quiet = false): Promise<void> {
  try {
    const charges = new Map<string, { credits: number; day: string }>();
    for (let page = 1; ; page++) {
      const data = await client.call("GET", "/v2/studio/txn/records", undefined, { page_num: page, page_size: 100 });
      for (const c of chargesIn(data.records ?? [])) {
        // One operation can be billed in parts, as two records.
        const known = charges.get(c.operation);
        charges.set(c.operation, { credits: (known?.credits ?? 0) + c.credits, day: c.day });
      }
      if (!(everything && data.have_more)) break;
    }
    const recorded = recordedSpends();
    let noted = 0;
    for (const [op, c] of charges) {
      if (c.credits <= (recorded.get(spendId("tripo-studio", op)) ?? 0)) continue;
      recordSpend("tripo-studio", op, c.credits, c.day);
      noted++;
    }
    if (!quiet) report({ operations: charges.size, credits: [...charges.values()].reduce((s, c) => s + c.credits, 0), newly_noted: noted });
  } catch (err) {
    // The stats must never stop a model being made.
    if (!quiet) throw new StudioError(`noting spending failed: ${(err as Error).message}`);
    log(`(could not note Tripo spending for the token stats: ${(err as Error).message})`);
  }
}

// The fulfilled charges in a page of Tripo's history, by operation; exported for the tests.
export function chargesIn(records: any[]): { operation: string; credits: number; day: string }[] {
  return records
    .filter((r) => r.txn_type === "charge" && r.status === "fulfilled" && /[1-9a-f]/.test(r.operator_id ?? ""))
    .map((r) => ({ operation: r.operator_id, credits: Number(r.amount) || 0, day: String(r.create_time ?? "").slice(0, 10) }));
}

// Small helpers.

// A project id from the first argument, an id or any Studio URL with one in it.
export function projectIdOf(ref: string | undefined): string {
  if (ref === undefined) throw new StudioError("give a project: its id, or its Studio URL");
  const found = ref.match(/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/g);
  if (!found) throw new StudioError(`${JSON.stringify(ref)} is not a project id or a Studio project URL`);
  return found.at(-1)!;
}

function projectId(a: Args): string {
  return projectIdOf(a.positionals[0]);
}

function one(a: Args, what: string): string {
  if (a.positionals.length !== 1) throw new StudioError(`give ${what}`);
  return a.positionals[0];
}

function int(value: unknown, flag: string): number {
  const n = Number(value);
  if (!Number.isInteger(n) || n <= 0) throw new StudioError(`${flag} must be a whole number`);
  return n;
}

function output(a: Args, pid: string, suffix: string, ext = ".glb"): string {
  return a.values.out ?? `${pid.slice(0, 8)}-${suffix}${ext}`;
}

function report(fields: Record<string, unknown>): void {
  console.log(JSON.stringify(Object.fromEntries(Object.entries(fields).filter(([, v]) => v !== undefined))));
}

export function slug(text: string): string {
  return (text.toLowerCase().match(/[a-z0-9]+/g) ?? []).slice(0, 6).join("-") || "tripo";
}

function stem(path: string): string {
  return basename(path, extname(path));
}

function numbered(path: string, n: number): string {
  const ext = extname(path);
  return `${path.slice(0, path.length - ext.length)}-${n}${ext}`;
}

if (import.meta.main) await main(process.argv.slice(2));
