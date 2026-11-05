#!/usr/bin/env node
// Vendoring their pieces (D599, D611): each named glTF piece of Quaternius's CC0 packs copied byte
// for byte from the private repository (~/src/terra-private or TERRA_PRIVATE) into
// art/characters/quaternius/<pack>/, with the .bin and textures it names and, with --colourways,
// its outfit's other BaseColor maps (T_<Outfit>_<n>_BaseColor.png); their SHA256SUMS lines added.
// Never the Bestiary or any pack not CC0 (vendored.ts's PACKS). Then `wardrobe.ts --build`.
//
//   node tools/characters/vendor.ts --list [PREFIX]       the pieces their packs ship, by name, * vendored
//   node tools/characters/vendor.ts NAME|PREFIX… [--colourways] [--dry]
//
// A name is a glTF's file name without .gltf (Male_Knight_Arms) or the start of some
// (Male_Knight_). Of the copies a pack ships of one name, the glTF exports' are taken
// (`Modular Parts`, `Rigged to Head Bone`, then any other glTF folder); two of one name with
// different bytes are refused, since the vendored folder is flat. A line a stage (D388).

import { createHash } from "node:crypto";
import { copyFileSync, existsSync, readFileSync, statSync, utimesSync, writeFileSync } from "node:fs";
import { basename, dirname, join, posix } from "node:path";
import { fileURLToPath } from "node:url";
import { manifest, PACKS, privateRoot, VENDORED } from "./vendored.ts";

const ROOT = fileURLToPath(new URL("../../", import.meta.url));
/** Folders a piece is taken from, best first. */
const PREFER = [/\/Modular Parts\//, /\/Rigged to Head Bone\/.*\/Male\//, /\/Rigged to Head Bone\//, /glTF|Godot/];

export type Plan = { copy: { from: string; to: string; hash: string }[]; have: string[]; faults: string[]; pieces: string[] };

const rank = (p: string) => {
  const i = PREFER.findIndex((re) => re.test(p));
  return i < 0 ? PREFER.length : i;
};

/** What vendoring `names` would copy: `priv` the private manifest (path to hash), `sums` ours. */
export function plan(names: string[], priv: Map<string, string>, sums: Map<string, string>, read: (path: string) => string, colourways = false): Plan {
  const ours = Object.fromEntries(Object.entries(PACKS).map(([o, t]) => [t, o]));
  const byName = new Map<string, { path: string; hash: string; pack: string }[]>();
  for (const [path, hash] of priv) {
    const pack = ours[path.split("/")[1]];
    if (!pack) continue; // not a CC0 pack: never vendored
    const n = basename(path);
    byName.set(n, [...(byName.get(n) ?? []), { path, hash, pack }]);
  }
  const out: Plan = { copy: [], have: [], faults: [], pieces: [] };
  const take = (file: string, near?: string) => {
    const all = (byName.get(file) ?? []).sort((a, b) => rank(a.path) - rank(b.path));
    if (!all.length) return void out.faults.push(`${file}: no file of that name in the private repository's CC0 packs`);
    // A file a glTF names comes from beside it; a piece from the best folder that has it.
    const beside = near ? all.filter((c) => dirname(c.path) === near) : [];
    const best = beside.length ? beside : all.filter((c) => rank(c.path) === rank(all[0].path));
    const hashes = new Set(best.map((c) => c.hash));
    if (hashes.size > 1) return void out.faults.push(`${file}: ${best.length} different files of that name (${best.map((c) => c.path).join("; ")}); the vendored folder is flat: vendor it by hand from the right one, its line with it`);
    const c = best[0];
    const to = `${VENDORED}/${c.pack}/${file}`;
    const had = sums.get(to);
    if (had === c.hash) return void (out.have.includes(to) || out.have.push(to));
    if (had) return void out.faults.push(`${to} is vendored with other bytes than ${c.path}: a different file of the same name; vendor it by hand under another folder`);
    if (!out.copy.some((x) => x.to === to)) out.copy.push({ from: c.path, to, hash: c.hash });
    return c;
  };
  const gltfs = [...byName.keys()].filter((n) => n.endsWith(".gltf"));
  // A prefix takes pieces alone (a pack's Modular Parts, its rigged hair), never a whole outfit's file.
  const piece = (n: string) => (byName.get(n) ?? []).some((c) => rank(c.path) < PREFER.length - 1);
  for (const name of names) {
    const hits = gltfs.filter((n) => n === `${name}.gltf` || (name.endsWith("_") && n.startsWith(name) && piece(n)));
    if (!hits.length) out.faults.push(`${name}: no glTF piece of that name in the CC0 packs (node tools/characters/vendor.ts --list ${name.slice(0, 6)} names them)`);
    for (const file of hits.sort()) {
      out.pieces.push(file.replace(/\.gltf$/, ""));
      const c = take(file);
      const at = c ? c.path : (byName.get(file) ?? []).sort((a, b) => rank(a.path) - rank(b.path))[0]?.path;
      if (!at) continue;
      // What the glTF names beside it: its buffers and images, by relative uri.
      let gltf: { buffers?: { uri?: string }[]; images?: { uri?: string }[] };
      try {
        gltf = JSON.parse(read(at));
      } catch {
        out.faults.push(`${at}: not readable as glTF JSON`);
        continue;
      }
      for (const uri of [...(gltf.buffers ?? []), ...(gltf.images ?? [])].map((x) => x.uri).filter((u): u is string => !!u && !u.startsWith("data:"))) {
        const want = posix.normalize(posix.join(dirname(at), decodeURI(uri)));
        if (posix.dirname(want) !== dirname(at)) out.faults.push(`${at} names ${uri} outside its folder; the vendored folder is flat`);
        else if (!priv.has(want)) out.faults.push(`${at} names ${uri}, which the private manifest lacks`);
        else take(basename(want), dirname(at));
        const base = /^T_(.+)_BaseColor\.png$/.exec(basename(uri));
        if (colourways && base) for (const n of [...byName.keys()].filter((n) => new RegExp(`^T_${base[1]}_\\d+_BaseColor\\.png$`).test(n))) take(n);
      }
    }
  }
  return out;
}

/** SHA256SUMS with the new files' lines after its own, as written, in path order among themselves. */
export function withLines(text: string, add: { to: string; hash: string }[]): string {
  const m = manifest(text);
  const fresh = add.filter((a) => !m.has(a.to)).sort((a, b) => (a.to < b.to ? -1 : 1));
  const head = text.endsWith("\n") || !text ? text : `${text}\n`;
  return head + fresh.map((a) => `${a.hash}  ${a.to}\n`).join("");
}

function list(priv: Map<string, string>, sums: Map<string, string>, prefix: string): string[] {
  const ours = new Set([...sums.keys()].map((p) => basename(p)));
  const names = new Set<string>();
  const cc0 = new Set(Object.values(PACKS));
  for (const path of priv.keys()) if (cc0.has(path.split("/")[1]) && path.endsWith(".gltf") && rank(path) < PREFER.length - 1 && basename(path).startsWith(prefix)) names.add(basename(path));
  const rows = new Map<string, string[]>();
  for (const n of [...names].sort()) {
    const group = n.split("_").slice(0, /^(Male|Female)_/.test(n) ? 2 : 1).join("_");
    rows.set(group, [...(rows.get(group) ?? []), `${n.replace(/\.gltf$/, "").slice(group.length + 1) || n}${ours.has(n) ? "*" : ""}`]);
  }
  return [...rows.entries()].map(([g, ps]) => `${g}: ${ps.join(" ")}`);
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const at = privateRoot();
  if (!at) {
    console.log("✗ vendor: no private repository on this machine (~/src/terra-private, or set TERRA_PRIVATE): their files come from there alone (D599)");
    process.exit(1);
  }
  const priv = manifest(readFileSync(join(at, "SHA256SUMS"), "utf8"));
  const sumsFile = join(ROOT, VENDORED, "SHA256SUMS");
  const sumsText = readFileSync(sumsFile, "utf8");
  const sums = manifest(sumsText);
  if (args[0] === "--list") {
    for (const l of list(priv, sums, args[1] ?? "")) console.log(l);
    console.log("(* vendored; the CC0 packs alone)");
    process.exit(0);
  }
  const names = args.filter((a) => !a.startsWith("--"));
  if (!names.length) {
    console.log("Usage: node tools/characters/vendor.ts --list [PREFIX] | NAME|PREFIX… [--colourways] [--dry] (see its header)");
    process.exit(1);
  }
  const p = plan(names, priv, sums, (path) => readFileSync(join(at, path), "utf8"), args.includes("--colourways"));
  if (p.faults.length) {
    console.log(`✗ vendor: ${p.faults.length} fault${p.faults.length > 1 ? "s" : ""}, nothing copied`);
    for (const f of p.faults.slice(0, 8)) console.log(`  ${f}`);
    process.exit(1);
  }
  if (args.includes("--dry")) {
    console.log(`✓ vendor --dry: ${p.pieces.length} pieces; would copy ${p.copy.length} files (${p.copy.slice(0, 6).map((c) => basename(c.to)).join(", ") || "none"}${p.copy.length > 6 ? ", …" : ""}), ${p.have.length} already vendored`);
    process.exit(0);
  }
  for (const c of p.copy) {
    const from = join(at, c.from);
    const data = readFileSync(from);
    if (createHash("sha256").update(data).digest("hex") !== c.hash) {
      console.log(`✗ vendor: ${c.from} is not the private manifest's bytes (an LFS pointer? git -C ${at} lfs pull); nothing more copied`);
      process.exit(1);
    }
    copyFileSync(from, join(ROOT, c.to));
    const st = statSync(from);
    utimesSync(join(ROOT, c.to), st.atime, st.mtime); // as cp -p
  }
  writeFileSync(sumsFile, withLines(sumsText, p.copy));
  const packs = [...new Set(p.copy.map((c) => c.to.split("/")[3]))];
  console.log(`✓ vendor: ${p.pieces.length} pieces, ${p.copy.length} files copied byte for byte, ${p.have.length} already there; ${VENDORED}/SHA256SUMS has their lines`);
  if (p.copy.length) console.log(`  licence: ${packs.join(", ")} already ${packs.length > 1 ? "have" : "has"} rows in THIRD_PARTY.md, art/README.md and ${VENDORED}/README.md; name the outfit in that README's "What is here" list. Next: node tools/characters/wardrobe.ts --build`);
  if (!existsSync(join(ROOT, VENDORED, ".gitattributes"))) console.log(`✗ ${VENDORED}/.gitattributes is missing: put back "* -text"`);
}
