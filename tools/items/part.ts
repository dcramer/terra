// The part checker (D565 step 2, D566): one part of an assembled item against its type's template
// (assets/data/parts/types.json), its points, its size for every slot it fits, its budgets and
// material roles, and a clean join with every partner its kinds allow: no gap, nothing buried in
// its partner, no two faces in one plane. A model is measured; a part made by code is checked
// from its numbers, its joins measured on the models it meets.
//
//   node tools/items/part.ts <part>           assets/data/parts/<part>.json
//   node tools/items/part.ts --write <part>   first writes its volume_m3 (a trim its mass_kg, in its
//                                             density_kg_m3, and cover_m), centre_m and bounds_m as
//                                             measured on its model, so nobody types them
//
// A line a stage (D388); a failure names the field and the fix (D556).

import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Q, V3 } from "../characters/glb.ts";
import { KINDS, kindNames, PARTS, readJson, ROOT } from "./kind.ts";
import { bounds, buried, distance, gap, type Mesh, moved, normal, ray, readMesh, sharedPlanes, slice, solid, triangles } from "./mesh.ts";
import { validate } from "./schema.ts";

export type Ctx = {
  types: Record<string, any>;
  rules: Record<string, any>;
  materials: Record<string, any>;
  kinds: Record<string, any>;
  attach: Record<string, any>;
  parts: Record<string, any>;
  mesh: (part: any) => Mesh;
  /** How the part sits with each partner among the parts: the assembler's, never placed here. */
  pairings: (id: string, parts: any[]) => Pairing[];
  /** The kinds taking a part of a type, and those it is made for: the assembler's, never worked out here. */
  kindsOf: (type: string, named?: string[]) => Kinds;
  /** Why the assembler refuses `kind` of these parts in `material` among `parts`, or undefined when it makes it whole. */
  refusal: (kind: string, ids: string[], material: string, parts: any[]) => string | undefined;
};
export type Kinds = { taking: string[]; made_for: string[] };
export type Stage = { stage: string; faults: string[]; ok: string };

const RAYS = 16;
const mm = (m: number) => `${(m * 1000).toFixed(1)} mm`;
const v = (x: any) => x?.value;
const pt = (part: any, name: string) => part.points?.find((p: any) => p.name === name);
const hole = (size: any) => (Array.isArray(size) ? size : [size, size]); // [across Z, across X]

export function checkPart(part: any, ctx: Ctx): Stage[] {
  const stages: Stage[] = [];
  const stage = (name: string, ok: string, faults: string[]) => (stages.push({ stage: name, faults, ok }), faults.length === 0);
  const type = ctx.types[part?.type];
  const data = validate(readJson(join(PARTS, "part.schema.json")), part).map((f) => `${f.at || "/"} ${f.says}`);
  if (part && !type) data.push(`/type ${JSON.stringify(part.type)} is not a part type; use one of types.json's: ${Object.keys(ctx.types).join(", ")}`);
  // A working part is made in its recipe's material, so it gives its volume; a trim its own mass.
  const [weight, not] = type?.slot === "working" ? ["volume_m3", "mass_kg"] : ["mass_kg", "volume_m3"];
  if (type?.made_by === "model")
    for (const k of ["model", weight, "centre_m", "bounds_m", "points"])
      if (!(k in part)) data.push(`/${k} is missing: a modelled ${type.slot} part gives it (part.schema.json)${k === "model" || k === "points" ? "" : `; node tools/items/part.ts --write ${part.part} measures it on the model`}`);
  if (type?.made_by === "model" && type.slot === "trim" && !("density_kg_m3" in part))
    data.push(`/density_kg_m3 is missing: a trim keeps its own material, so it gives that material's density with its source, and node tools/items/part.ts --write ${part.part} weighs its model in it`);
  if (type?.made_by === "model" && not in part)
    data.push(`/${not} is not a ${type.slot} part's: ${type.slot === "working" ? "its mass is its volume_m3 in the recipe's material (materials.json)" : "a trim keeps its own material, so it gives mass_kg"}; remove it`);
  if (type?.made_by === "code") for (const k of type.numbers) if (!(k in part)) data.push(`/${k} is missing: a ${part.type} is made from it (types.json)`);
  const kinds = type && ctx.kindsOf(part.type, part.kinds);
  if (kinds) data.push(...kindFaults(part, kinds));
  if (!stage("data", `a ${part?.type}, made by ${type?.made_by}`, data)) return stages;
  const join_m = v(ctx.rules.join_m);
  if (type.made_by === "code") {
    stage("numbers", "positive, its grip on it", codeNumbers(part));
    if (part.skin) {
      // Its look: one material named Wood (a handle's role), its maps within the part budget.
      const skin = ctx.mesh({ ...part, model: part.skin });
      const names = [...new Set(skin.materials.map((m) => m.split(".")[0]))];
      const faults = budget({ triangles: "small_triangles" }, { ...skin, indices: [] }, ctx);
      if (names.length !== 1 || names[0] !== "Wood") faults.push(`skin ${part.skin} has materials ${JSON.stringify(names)}: a handle's skin is one material named Wood; make it with tools/items/haft.py skin`);
      if (skin.textures.length === 0) faults.push(`skin ${part.skin} has no pictures: it would draw the handle flat; make it with tools/items/haft.py skin`);
      stage("skin", `${part.skin}: ${skin.textures.map((t) => `${t.name} ${t.px} px`).join(", ")}`, faults);
    }
  } else {
    const mesh = ctx.mesh(part);
    stage("points", `${type.points.length} of ${part.type}'s, ${type.origin} at the origin`, points(part, type, mesh, ctx));
    stage("scale", "none: placed by a shift or one rigid turn", mesh.scaled.map((n) => `node ${n} carries a scale: parts are never scaled (D566); apply it in Blender (Object > Apply > Scale) and export again, or make a bigger part`));
    stage("openings", "each within 1 mm of its declared size round the line", openings(part, mesh, join_m));
    stage("budget", budgetNote(part, type, mesh, ctx), budget(type, mesh, ctx));
    stage("roles", [...new Set(mesh.materials.map((m) => m.split(".")[0]))].join(", "), roles(part, type, mesh));
    const weighed = type.slot === "working" ? "volume_m3, " : type.slot === "trim" ? `mass_kg, ${type.place === "seat" ? "" : "cover_m, "}` : "";
    stage("measured", `${weighed}centre_m and bounds_m as measured on its model`, measuredFaults(part, type, mesh, join_m));
  }
  stage("slots", kinds.made_for.join(", ") || "no kind fits it", slots(part, kinds.made_for, ctx));
  const [lacks, makes] = whole(part, kinds.made_for, ctx);
  stage("whole", makes.join("; ") || "no partner to make it with yet", lacks);
  const [faults, notes] = joins(part, ctx);
  stage("joins", notes.join("; ") || "none to make", faults);
  return stages;
}

function codeNumbers(part: any): string[] {
  const out: string[] = [];
  for (const [k, x] of Object.entries(part)) if (k.endsWith("_m") || k.endsWith("_m3")) if (typeof v(x) === "number" && k !== "second_from_grip_m" && !(v(x) > 0)) out.push(`/${k} is ${v(x)}: it must be above 0`);
  const [len, grip, second] = [v(part.length_m), v(part.grip_from_end_m), v(part.second_from_grip_m)];
  if (len !== undefined && grip !== undefined && grip >= len) out.push(`/grip_from_end_m ${grip} is not on a handle ${len} m long; put the grip on it`);
  if (len !== undefined && grip !== undefined && second !== undefined && (grip + second < 0 || grip + second > len))
    out.push(`/second_from_grip_m ${second} puts the second grip ${(grip + second).toFixed(3)} m from the butt, off a handle ${len} m long`);
  return out;
}

function points(part: any, type: any, mesh: Mesh, ctx: Ctx): string[] {
  const out: string[] = [];
  const join_m = v(ctx.rules.join_m);
  const want = new Map<string, string>(type.points);
  for (const [name, t] of want) {
    const p = pt(part, name);
    if (!p) out.push(`/points has no ${name}: a ${part.type} declares ${[...want.keys()].join(", ")} (docs/assembled-items.md, the part-type table)`);
    else if (p.type !== t) out.push(`/points ${name} is a ${p.type}; on a ${part.type} it is a ${t}`);
  }
  for (const p of part.points) {
    if (!want.has(p.name)) out.push(`/points ${p.name} is not a ${part.type}'s point; remove it or name it as types.json does`);
    const len = Math.hypot(...p.dir);
    if (Math.abs(len - 1) > 1e-3) out.push(`/points ${p.name}.dir has length ${len.toFixed(3)}; make it a unit direction`);
    if (["eye", "socket", "tang", "bore"].includes(p.type)) {
      if (Math.hypot(p.at[0], p.at[2]) > join_m) out.push(`/points ${p.name}.at is ${mm(Math.hypot(p.at[0], p.at[2]))} off the line (x = z = 0): a join along a handle lies on it (D566)`);
      if (Math.abs(p.dir[1] - 1) > 1e-3) out.push(`/points ${p.name}.dir is [${p.dir}]: a join on the line faces +Y (D566)`);
      if (!(Math.min(...hole(p.size ?? 0)) > 0)) out.push(`/points ${p.name}.size is missing or not above 0: a join on the line has its diameter (or [width, thickness] for a tang)`);
    }
    if (p.type === "seat" || p.type === "nock") {
      if (!p.up || Math.abs(Math.hypot(...p.up) - 1) > 1e-3 || Math.abs(p.up.reduce((s: number, x: number, i: number) => s + x * p.dir[i], 0)) > 1e-3)
        out.push(`/points ${p.name}.up must be a unit direction square to its dir: a full point has its roll (D566)`);
      const off = distance(mesh, p.at);
      if (off > join_m) out.push(`/points ${p.name} is ${mm(off)} off the model's surface; put it on the surface its partner meets`);
    }
  }
  const origin = type.origin === "grip" ? undefined : pt(part, type.origin);
  if (origin && Math.hypot(...origin.at) > join_m) out.push(`/points ${type.origin}.at is [${origin.at}]: a ${part.type}'s frame has its origin at ${type.origin} (D566); move the model, not the point`);
  const pair = (lo: string, hi: string) => {
    const [a, b] = [pt(part, lo), pt(part, hi)];
    if (a && b && !(a.at[1] < b.at[1])) out.push(`/points ${hi} must be above ${lo} along +Y`);
  };
  pair("edge.lower", "edge.upper");
  pair("eye.lower", "eye.upper");
  pair("socket.mouth", "socket.bottom");
  pair("tang.end", "shoulder");
  if (part.cover_m && part.cover_m[1] - part.cover_m[0] < v(ctx.rules.trim_cover_m)) out.push(`/cover_m spans ${mm(part.cover_m[1] - part.cover_m[0])}: a trim covers a join by at least ${mm(v(ctx.rules.trim_cover_m))} (D566)`);
  if (type.slot === "trim" && type.place !== "seat" && !part.cover_m) out.push(`/cover_m is missing: a trim on the line says how far it covers, [below, above] its bore; node tools/items/part.ts --write ${part.part} measures it on the model`);
  return out;
}

// The model's own openings, measured: an eye or socket from its lower point to its upper, a trim's
// bore over its cover, a tang's flat across Z and X. The handle is made to the declared sizes, so a
// wall further out is a gap round it and one further in buries it.
function openings(part: any, mesh: Mesh, join_m: number): string[] {
  const out: string[] = [];
  const spans: [any, any][] = [];
  for (const [lo, hi] of [["eye.lower", "eye.upper"], ["socket.mouth", "socket.bottom"], ["tang.end", "shoulder"]]) if (pt(part, lo) && pt(part, hi)) spans.push([pt(part, lo), pt(part, hi)]);
  const bore = pt(part, "bore");
  if (bore && part.cover_m) spans.push([{ ...bore, at: [0, part.cover_m[0], 0] }, { ...bore, at: [0, part.cover_m[1], 0] }]);
  for (const [a, b] of spans) {
    const tang = a.type === "tang";
    for (const s of [0, 0.25, 0.5, 0.75, 1]) {
      const y = a.at[1] + s * (b.at[1] - a.at[1]) + (s === 0 ? 5e-4 : s === 1 ? -5e-4 : 0);
      const [wa, wb] = [hole(a.size), hole(b.size)];
      const want = [wa[0] + s * (wb[0] - wa[0]), wa[1] + s * (wb[1] - wa[1])].map((d) => d / 2);
      const segs = slice(mesh, y);
      const where = `${a.name.split(".")[0]} at y ${y.toFixed(4)}`;
      if (tang) {
        const reach = (dx: number, dz: number) => Math.max(0, ...segs.flatMap((g) => g.map(([x, z]) => x * dx + z * dz)));
        for (const [dx, dz, w, across] of [[0, 1, want[0], "Z"], [0, -1, want[0], "Z"], [1, 0, want[1], "X"], [-1, 0, want[1], "X"]] as const) {
          const r = reach(dx, dz);
          if (Math.abs(r - w) > join_m) out.push(`${where}: the tang reaches ${mm(r)} across ${across}, its points say ${mm(w)}: the handle round it would ${r > w ? "bury it" : "stand off it"}; fix the tang's size or the model`);
        }
        continue;
      }
      const rs = [...Array(RAYS).keys()].map((i) => ray(segs, Math.cos((2 * Math.PI * i) / RAYS), Math.sin((2 * Math.PI * i) / RAYS)));
      if (rs.some((r) => !Number.isFinite(r))) {
        out.push(`${where}: no wall all round the line: the opening is not where ${a.name} and ${b.name} say; move the points onto it, or the model so its opening is on +Y`);
        break;
      }
      const lo = Math.min(...rs), hi = Math.max(...rs), r = want[0];
      if (hi - r > join_m) out.push(`${where}: the opening reaches ${mm(hi)} from the line where its points say ${mm(r)}: a ${mm(hi - r)} gap round the handle; fix ${a.name}'s or ${b.name}'s size, or the model`);
      if (r - lo > join_m) out.push(`${where}: the wall comes ${mm(lo)} from the line where its points say ${mm(r)}: the handle would be buried ${mm(r - lo)} in it; fix the size or the model`);
    }
  }
  return out;
}

const budgetOf = (type: any, ctx: Ctx) => v(ctx.rules[type.triangles]);
const budgetNote = (part: any, type: any, mesh: Mesh, ctx: Ctx) => `${mesh.indices.length / 3} triangles (at most ${budgetOf(type, ctx)[1]}), ${mesh.textures.length} textures`;

function budget(type: any, mesh: Mesh, ctx: Ctx): string[] {
  const out: string[] = [];
  const [tris, [, most]] = [mesh.indices.length / 3, budgetOf(type, ctx)];
  if (tris > most) out.push(`${tris} triangles, over the ${most} of its budget (${ctx.rules[type.triangles].source}); cut it in the clean-up (the models skill)`);
  if (mesh.textures.length > v(ctx.rules.textures)) out.push(`${mesh.textures.length} textures, over ${v(ctx.rules.textures)}: ${ctx.rules.textures.source}`);
  for (const t of mesh.textures)
    if (t.px === undefined) out.push(`texture ${t.name} is not a PNG in the glb, so its size cannot be read; export it as PNG`);
    else if (t.px > v(ctx.rules.texture_px)) out.push(`texture ${t.name} is ${t.px} px, over ${v(ctx.rules.texture_px)} (${ctx.rules.texture_px.source})`);
  return out;
}

function roles(part: any, type: any, mesh: Mesh): string[] {
  return mesh.materials
    .map((m) => m.split(".")[0])
    .filter((r) => !type.roles.includes(r))
    .map((r) => `material ${JSON.stringify(r)} is not a role a ${part.type} takes (${type.roles.join(", ")}; types.json): name each material by its role`);
}

const VOLUME_WITHIN = 0.005; // a share of the measured volume: its mass is good to half a percent

/** What a modelled part's data says of its model, measured on it: `--write` writes these. */
export function measured(part: any, type: any, mesh: Mesh): Record<string, any> {
  const { volume, centre, open } = solid(mesh);
  if (open) throw new Error(`${part.part}: its model ${part.model} is not a closed surface facing out (${open} edges met by one face, or by two running the same way), so its volume and centre cannot be measured; close it in Blender (Select > Select All by Trait > Non Manifold, fill or flip) and export again`);
  const r = (p: V3) => p.map((x) => Number(x.toFixed(5)) || 0); // to 10 µm, never -0
  const out: Record<string, any> = { centre_m: r(centre), bounds_m: bounds(mesh).map(r) };
  if (type.slot === "working") out.volume_m3 = { value: Number(volume.toPrecision(5)), source: `measured on ${part.model} by tools/items/part.ts --write` };
  if (type.slot === "trim") {
    // A trim keeps its own material: its mass is its closed volume in that density, the
    // density's source carried; one turned on the line covers from end to end of its bore.
    const d = part.density_kg_m3;
    if (!d) throw new Error(`${part.part}: a trim is weighed in its own material; give its /density_kg_m3 with the source`);
    out.mass_kg = { value: Number((volume * v(d)).toFixed(4)), source: `${(volume * 1e6).toFixed(2)} cm^3 measured on ${part.model} by tools/items/part.ts --write, at ${v(d)} kg/m^3: ${d.source}` };
    if (type.place !== "seat") out.cover_m = [out.bounds_m[0][1], out.bounds_m[1][1]];
  }
  return out;
}

function measuredFaults(part: any, type: any, mesh: Mesh, join_m: number): string[] {
  const write = `run node tools/items/part.ts --write ${part.part}, never type them by hand`;
  let m: Record<string, any>;
  try {
    m = measured(part, type, mesh);
  } catch (e) {
    return [(e as Error).message];
  }
  const out: string[] = [];
  const off = Math.max(...[0, 1].flatMap((c) => [0, 1, 2].map((k) => Math.abs(m.bounds_m[c][k] - part.bounds_m[c][k]))));
  if (off > join_m) out.push(`/bounds_m is ${mm(off)} off the model's: ${write}`);
  const from = Math.hypot(...[0, 1, 2].map((k) => m.centre_m[k] - part.centre_m[k]));
  if (from > join_m) out.push(`/centre_m is ${mm(from)} from the model's centre of mass: ${write}`);
  if (m.volume_m3 && Math.abs(v(part.volume_m3) / m.volume_m3.value - 1) > VOLUME_WITHIN)
    out.push(`/volume_m3 is ${v(part.volume_m3)}, the model's ${m.volume_m3.value}: ${write}`);
  if (m.mass_kg && Math.abs(v(part.mass_kg) / m.mass_kg.value - 1) > VOLUME_WITHIN)
    out.push(`/mass_kg is ${v(part.mass_kg)} kg, the model's ${m.mass_kg.value} kg in ${v(part.density_kg_m3)} kg/m^3: ${write}`);
  const cover = m.cover_m && Math.max(...[0, 1].map((k) => Math.abs(m.cover_m[k] - (part.cover_m?.[k] ?? Infinity))));
  if (cover > join_m) out.push(`/cover_m is ${Number.isFinite(cover) ? `${mm(cover)} off` : "not"} the model's span along the line: ${write}`);
  return out;
}

function kindFaults(part: any, { taking }: Kinds): string[] {
  if (!part.kinds) return taking.length > 1 ? [`/kinds is missing: a ${part.type} fits ${taking.join(", ")}; name the kinds it is made for`] : [];
  return part.kinds.filter((k: string) => !taking.includes(k)).map((k: string) => `/kinds names ${k}, whose slots take no ${part.type} (assets/data/kinds/${k}.json)`);
}

// Its size for each slot it fits: a working part's mass within its kind's head-mass envelope, a
// handle's length within its kind's and its grip where the kind's mount puts it (D546).
function slots(part: any, made_for: string[], ctx: Ctx): string[] {
  const out: string[] = [];
  for (const name of made_for) {
    const kind = ctx.kinds[name];
    const slot = kind.slots.find((s: any) => s.fits.includes(part.type));
    const head = v(kind.envelope.head_mass_kg), len = v(kind.envelope.length_m);
    if (slot.slot === "working" && Array.isArray(head) && part.volume_m3) {
      const kg = Object.entries(ctx.materials).map(([m, d]: [string, any]) => [m, v(part.volume_m3) * v(d.density_kg_m3)] as const);
      if (!kg.some(([, k]) => k >= head[0] && k <= head[1]))
        out.push(`${name}: in no material is its head inside the ${name}'s head mass [${head}] kg (${kg.map(([m, k]) => `${m} ${k.toFixed(3)} kg`).join(", ")}; assets/data/kinds/${name}.json envelope, materials.json): another part, or another kind`);
    }
    if (slot.slot !== "handle" || part.length_m === undefined) continue;
    if (Array.isArray(len) && v(part.length_m) > len[1]) out.push(`${name}: ${v(part.length_m)} m long, past the ${name}'s longest item ${len[1]} m (its envelope's length_m)`);
    const mount = ctx.attach[kind.attach_kind] ?? {};
    const [g, l] = [v(part.grip_from_end_m), v(part.length_m)];
    if (mount.grip_from_end_m && (g < mount.grip_from_end_m[0] || g > mount.grip_from_end_m[1]))
      out.push(`${name}: its grip ${g} m from the butt is outside a ${kind.attach_kind} look's [${mount.grip_from_end_m}] (tools/characters/attach_kinds.json, D546)`);
    if (mount.grip_share && (g / l < mount.grip_share[0] || g / l > mount.grip_share[1]))
      out.push(`${name}: its grip at ${(g / l).toFixed(2)} of its length is outside a ${kind.attach_kind} look's [${mount.grip_share}] (tools/characters/attach_kinds.json, D546)`);
  }
  return out;
}

// A whole item with each partner of its kind (assembled-items.md, Checking a part): a handle with
// each working part made for the kind, a working part with each handle, the least recipe (no
// trim), in any material. The kind's envelope is on the whole item (D459), and the head sets how
// far past the haft it reaches and what it weighs, so the assembler, the one rule, is asked
// rather than a haft's own range kept beside it. A part no partner makes whole is refused; one
// some partners refuse is not, the envelope refusing that recipe when it is made.
function whole(part: any, made_for: string[], ctx: Ctx): [string[], string[]] {
  const faults: string[] = [], notes: string[] = [];
  const others = Object.values(ctx.parts);
  for (const name of made_for) {
    const kind = ctx.kinds[name];
    const mine = kind.slots.find((s: any) => s.fits.includes(part.type))?.slot;
    const want = mine === "handle" ? "working" : mine === "working" ? "handle" : undefined;
    const slot = kind.slots.find((s: any) => s.slot === want);
    if (!slot) {
      notes.push(`a ${mine}: the ${name} is whole without it`);
      continue;
    }
    const partners = others.filter((p) => p.part !== part.part && slot.fits.includes(p.type) && (!p.kinds || p.kinds.includes(name)));
    if (partners.length === 0) continue;
    const ok: string[] = [], refused: string[] = [];
    for (const p of partners) {
      const ids = mine === "handle" ? [p.part, part.part] : [part.part, p.part];
      const why = Object.keys(ctx.materials).map((m) => ctx.refusal(name, ids, m, others));
      if (why.some((w) => w === undefined)) ok.push(p.part);
      else refused.push(`${p.part} (${why[0]})`);
    }
    if (ok.length) notes.push(`a whole ${name} with ${ok.join(", ")}${refused.length ? `; not with ${refused.join(", ")}` : ""}`);
    else
      faults.push(
        `${name}: no ${slot.name} of the ${name}'s makes it a whole ${name}: ${refused.join("; ")}. ${mine === "handle" ? "Change its length_m (or grip) until one does" : "Change its size until one does"}: the kind's envelope (assets/data/kinds/${name}.json, D459) is on the whole item`,
      );
  }
  return [faults, notes];
}

// Every pairing its kinds allow, placed by the assembler (terra_items::assembly's `meet`, the one
// placing rule) and measured here. A part made by code meets its partners at their declared
// sizes, so those joins are the partners' openings, measured when they are checked; here what
// this part meets is measured: a handle's top in a head's top face, a trim against its working
// part, a seated trim on its board.
function joins(part: any, ctx: Ctx): [string[], string[]] {
  const faults: string[] = [], notes: string[] = [];
  const plane_m = v(ctx.rules.plane_m), join_m = v(ctx.rules.join_m);
  for (const { kind, partner, meet } of ctx.pairings(part.part, Object.values(ctx.parts))) {
    const said = (s: string) => faults.push(`${kind}, with ${partner}: ${s}`);
    if (meet.meet === "handle_top") {
      const head = ctx.parts[meet.head];
      const top = ctx.types[head.type].handle_top;
      const n = sharedDisc(ctx.mesh(head), meet.y, meet.d / 2, plane_m);
      if (n) said(`${n} faces of the head lie in the handle's top plane (y ${meet.y.toFixed(4)}): the handle's top sits there by D566, so the head's top face must not; move ${top} to the opening's real top`);
    } else {
      const [a, b] = [moved(ctx.mesh(ctx.parts[meet.part]), meet.turn, meet.at), ctx.mesh(ctx.parts[meet.onto])];
      const g = gap(a, b);
      if (g > join_m) said(`a ${mm(g)} gap where they meet (the bar ${mm(join_m)}, D566): the points are not where the surfaces are; fix the point or the model`);
      const deep = Math.max(buried(a, b), buried(b, a));
      if (deep > join_m) said(`one is buried ${mm(deep)} in the other (the bar ${mm(join_m)}): fix the point or the model`);
      const n = sharedPlanes(a, b, plane_m);
      if (n) said(`${n} pairs of faces lie in one plane facing the same way, which flickers: offset one by more than ${mm(plane_m)}`);
    }
    notes.push(`${kind} with ${partner}`);
  }
  return [faults, notes];
}

/** How two parts sit, as the assembler says (crates/terra_items/src/assembly/meet.rs). */
export type Meet = { meet: "handle_top"; head: string; y: number; d: number } | { meet: "placed"; part: string; onto: string; at: V3; turn: Q };
export type Pairing = { kind: string; partner: string; meet: Meet };

/** The assembler's answer to `assemble <args>` (crates/terra_items/src/bin/assemble.rs), `input` on stdin. */
export function assembler(args: string[], input?: string): any {
  const run = spawnSync("cargo", ["run", "-q", "-p", "terra_items", "--bin", "assemble", "--", ...args], { cwd: ROOT, input, encoding: "utf8" });
  if (run.status !== 0) throw new Error(`the assembler refused \`assemble ${args.join(" ")}\`: ${run.stderr.trim() || run.error?.message}`);
  return JSON.parse(run.stdout);
}

/** The assembler's pairings of part `id` among `parts`: `assemble joins`, the parts on stdin. */
export const assemblerPairings = (id: string, parts: any[]): Pairing[] => assembler(["joins", id, "--parts", "-"], JSON.stringify(parts));

/** The assembler's refusal of `assemble item <kind> <ids> --material <m>`, said short, or undefined when it assembles. */
export function assemblerRefusal(kind: string, ids: string[], material: string, parts: any[]): string | undefined {
  try {
    assembler(["item", kind, ...ids, "--material", material, "--parts", "-"], JSON.stringify(parts));
    return undefined;
  } catch (e) {
    const said = (e as Error).message;
    const env = said.match(/'s (\w+) ([\d.]+) is outside its envelope (\[[^\]]+\])/);
    return env ? `in ${material} its ${env[1]} ${env[2]} is outside ${env[3]}` : said;
  }
}

/** The assembler's kinds for a part of `type` naming `named` (terra_items::assembly's `kinds_of`). */
export const assemblerKinds = (type: string, named: string[] = []): Kinds => assembler(["kinds", type, ...named]);

function sharedDisc(mesh: Mesh, y: number, r: number, within: number): number {
  return triangles(mesh).filter((t) => normal(t)[1] > Math.cos(Math.PI / 180) && t.every((p) => Math.abs(p[1] - y) < within) && t.some((p) => Math.hypot(p[0], p[2]) < r)).length;
}

export function loadCtx(): Ctx {
  const t = readJson(join(PARTS, "types.json"));
  const kinds = Object.fromEntries(kindNames().map((k) => [k, readJson(join(KINDS, `${k}.json`))]));
  const parts = Object.fromEntries(
    readdirSync(PARTS)
      .filter((f) => f.endsWith(".json") && !["types.json", "materials.json", "part.schema.json"].includes(f))
      .map((f) => [f.slice(0, -5), readJson(join(PARTS, f))]),
  );
  const cache = new Map<string, Mesh>();
  const mesh = (p: any) => {
    const file = join(ROOT, "assets", p.model);
    if (!existsSync(file)) throw new Error(`${p.part}: its model ${p.model} is not under assets/; make it (the models skill) or fix /model`);
    if (!cache.has(file)) cache.set(file, readMesh(file));
    return cache.get(file)!;
  };
  const materials = readJson(join(PARTS, "materials.json")).materials;
  return { types: t.types, rules: t.rules, materials, kinds, attach: readJson(join(ROOT, "tools", "characters", "attach_kinds.json")), parts, mesh, pairings: assemblerPairings, kindsOf: assemblerKinds, refusal: assemblerRefusal };
}

export function report(name: string, stages: Stage[]): string[] {
  return stages.flatMap((s) => (s.faults.length ? s.faults.map((f) => `✗ ${s.stage}: ${f} (assets/data/parts/${name}.json)`) : [`✓ ${s.stage}: ${s.ok}`]));
}

/** The part with what its model measures written over its own: `--write`'s, before it is checked. */
export function withMeasured(part: any, ctx: Ctx): any {
  const type = ctx.types[part.type];
  if (type?.made_by !== "model" || !part.model) throw new Error(`${part.part}: only a modelled part with its /model is measured; a ${part.type} is ${type ? `made by ${type.made_by}` : "no part type"} (types.json)`);
  return { ...part, ...measured(part, type, ctx.mesh(part)) };
}

if (import.meta.main) {
  const write = process.argv[2] === "--write";
  const name = process.argv[write ? 3 : 2];
  const ctx = loadCtx();
  if (!name || !ctx.parts[name]) {
    console.log(`✗ part: name one of assets/data/parts/: ${Object.keys(ctx.parts).join(", ") || "(none yet: a part is assets/data/parts/<part>.json, part.schema.json)"}`);
    process.exit(1);
  }
  if (write) {
    const was = ctx.parts[name];
    try {
      ctx.parts[name] = withMeasured(was, ctx);
    } catch (e) {
      console.log(`✗ write: ${(e as Error).message}`);
      process.exit(1);
    }
    const changed = Object.keys(ctx.parts[name]).filter((k) => JSON.stringify(was[k]) !== JSON.stringify(ctx.parts[name][k]));
    writeFileSync(join(PARTS, `${name}.json`), `${JSON.stringify(ctx.parts[name], null, 2)}\n`);
    console.log(`✓ write: ${changed.join(", ") || "nothing changed"}, as measured on ${ctx.parts[name].model} (assets/data/parts/${name}.json)`);
  }
  const stages = checkPart(ctx.parts[name], ctx);
  for (const line of report(name, stages)) console.log(line);
  const bad = stages.filter((s) => s.faults.length).length;
  console.log(bad ? `✗ ${name}: ${bad} of ${stages.length} stages refused` : `✓ ${name}: fits its type and every join its kinds allow`);
  process.exit(bad ? 1 : 0);
}
