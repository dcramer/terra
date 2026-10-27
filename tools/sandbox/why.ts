// What changed this? (D548): `sandbox.why`'s answer as a few lines, each change's place in
// the source with the function it is in, which for a system is the system's name.
// Node 22.18 or later; no dependencies.

import { existsSync, readdirSync, readFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";

export type Place = { file: string; line: number } | null;
export type When = { frame: number | null; ago: number | null };
export type Part = { name: string; type: string; by: Place; changed: When | null; added: When | null; immutable: boolean };
export type Why = { entity: number; name: string | null; tracked: boolean; frame: number | null; kept: number; spawned: { by: Place; when: When }; components: Part[] };

const ROOT = resolve(import.meta.dirname, "../..");

/// The function a line of Rust is in: the nearest `fn` above it indented less than it is, so
/// a closure's line is told by the function that holds it.
export function enclosingFn(source: string, line: number): string | undefined {
  const lines = source.split("\n");
  const indent = (text: string) => text.length - text.trimStart().length;
  let within = indent(lines[line - 1] ?? "");
  for (let i = line - 2; i >= 0; i--) {
    const text = lines[i];
    if (!text.trim()) continue;
    const found = /^\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:const|async|unsafe|extern\s+"[^"]*")\s+)*fn\s+([A-Za-z_]\w*)/.exec(text);
    if (found && indent(text) < within) return found[1];
    within = Math.min(within, indent(text) + 1);
  }
  return undefined;
}

/// Where a file the game names is on this machine: the workspace's from this checkout, a
/// crate's from Cargo's registry.
function sourceOf(file: string): string | undefined {
  const here = resolve(ROOT, file);
  if (existsSync(here)) return here;
  const registry = join(process.env.CARGO_HOME ?? join(homedir(), ".cargo"), "registry", "src");
  if (!existsSync(registry)) return undefined;
  for (const index of readdirSafe(registry)) {
    const there = join(registry, index, file);
    if (existsSync(there)) return there;
  }
  return undefined;
}

function readdirSafe(dir: string): string[] {
  try {
    return readdirSync(dir);
  } catch {
    return [];
  }
}

const sources = new Map<string, string | undefined>();

/// A place as `file:line in fn`, the function found when the file is at hand.
export function where(place: Place, read: (file: string) => string | undefined = readSource): string {
  if (!place) return "?";
  const source = read(place.file);
  const inFn = source === undefined ? undefined : enclosingFn(source, place.line);
  return `${place.file}:${place.line}${inFn ? ` in ${inFn}` : ""}`;
}

function readSource(file: string): string | undefined {
  if (!sources.has(file)) {
    const path = sourceOf(file);
    sources.set(file, path ? readFileSync(path, "utf8") : undefined);
  }
  return sources.get(file);
}

const when = (at: When | null, kept: number) =>
  !at ? "" : at.frame === null ? `over ${kept} frames ago` : at.ago === 0 ? `frame ${at.frame}, this frame` : `frame ${at.frame}, ${at.ago} frame${at.ago === 1 ? "" : "s"} ago`;

/// The answer as lines: the entity and who spawned it, then a line a component, the latest
/// change first.
export function describeWhy(answer: Why, read?: (file: string) => string | undefined): string[] {
  const place = (p: Place) => (answer.tracked ? where(p, read) : "");
  const name = answer.name ? `${answer.name} (${answer.entity})` : `${answer.entity}`;
  const lines = [`${name} at frame ${answer.frame}: spawned ${place(answer.spawned.by)}${answer.tracked ? ", " : ""}${when(answer.spawned.when, answer.kept)}`];
  const width = Math.max(...answer.components.map((part) => part.name.length), 0);
  for (const part of answer.components) {
    const by = !answer.tracked ? "" : part.immutable ? "(set when inserted; its line is not readable) " : `${where(part.by, read)}, `;
    lines.push(`  ${part.name.padEnd(width)}  ${by}${when(part.changed, answer.kept)}`);
  }
  if (!answer.tracked) lines.push("(this build does not keep lines of code: it was built without track_location, as release builds are; `r start` without --release keeps them)");
  return lines;
}
