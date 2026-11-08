// An asset's ledger, art/iso/NAME/asset.json: what it is, its Tripo project, and each stage's
// credits (from the Tripo CLI's own output) and minutes. `status` prints them all as a table.

import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Body, Kind } from "./kinds.ts";

export const STAGES = ["concept", "model", "rig", "animate", "export", "convert", "decimate", "check"] as const;
export type Stage = (typeof STAGES)[number];

export type StageRecord = {
  at: string;
  /** Credits Tripo charged, from the CLI's output; null where it was spent before the
   * pipeline took the project (`adopt`). */
  credits: number | null;
  /** Wall-clock minutes the stage took; null where it is not known (`adopt`). */
  minutes: number | null;
  adopted?: boolean;
  files?: string[];
  note?: string;
  [more: string]: unknown;
};

export type Asset = {
  name: string;
  kind: Kind;
  theme: string;
  description: string;
  /** The body it is rigged as; none for a prop. */
  body?: Body;
  /** The height it is made to, in metres. */
  height?: number;
  project?: string;
  /** The rig's type as Tripo reports it: biped, quadruped… */
  rig?: string;
  stages: Partial<Record<Stage, StageRecord>>;
};

export const assetDir = (root: string, name: string) => join(root, "art", "iso", name);
export const ledgerPath = (root: string, name: string) => join(assetDir(root, name), "asset.json");

export function readAsset(root: string, name: string): Asset {
  const path = ledgerPath(root, name);
  if (!existsSync(path)) throw new Error(`no asset "${name}" (${path} missing): start it with \`concept ${name} --kind … --theme … "DESCRIPTION"\` or \`adopt ${name} PROJECT --kind …\``);
  return JSON.parse(readFileSync(path, "utf8"));
}

export function writeAsset(root: string, asset: Asset): void {
  mkdirSync(assetDir(root, asset.name), { recursive: true });
  writeFileSync(ledgerPath(root, asset.name), JSON.stringify(asset, null, 2) + "\n");
}

export function allAssets(root: string): Asset[] {
  const dir = join(root, "art", "iso");
  if (!existsSync(dir)) return [];
  return readdirSync(dir)
    .filter((n) => existsSync(ledgerPath(root, n)))
    .sort()
    .map((n) => readAsset(root, n));
}

function cell(r: StageRecord | undefined): string {
  if (!r) return "-";
  if (r.adopted) return "adopted";
  const cr = r.credits === null ? "?" : `${r.credits}cr`;
  const min = r.minutes === null ? "?" : `${r.minutes.toFixed(1)}m`;
  return r.note === "failed" ? `FAIL ${min}` : `${cr} ${min}`;
}

/** Every asset's stages and costs, one row each, and the totals. */
export function statusTable(assets: Asset[]): string {
  const head = ["asset", "kind", "project", ...STAGES, "credits", "minutes"];
  const rows = assets.map((a) => {
    const rs = STAGES.map((s) => a.stages[s]);
    const credits = rs.reduce((n, r) => n + (r?.credits ?? 0), 0);
    const minutes = rs.reduce((n, r) => n + (r?.minutes ?? 0), 0);
    return [a.name, a.kind, a.project?.slice(0, 8) ?? "-", ...rs.map(cell), String(credits), minutes.toFixed(1)];
  });
  if (!rows.length) return "no assets yet: art/iso/ is empty";
  const widths = head.map((h, i) => Math.max(h.length, ...rows.map((r) => r[i].length)));
  const line = (r: string[]) => r.map((c, i) => c.padEnd(widths[i])).join("  ").trimEnd();
  return [line(head), ...rows.map(line)].join("\n");
}

/** A stage's credits as tools/tripo/README.md's table gives them, the one place they are
 * written: the first number in the command's row, 0 for a free one. `model` is 55 by default. */
export function readmeCost(readme: string, command: string): number {
  const row = readme.split("\n").find((l) => l.startsWith(`| \`${command} `) || l.startsWith(`| \`${command}\``));
  if (!row) throw new Error(`tools/tripo/README.md has no row for \`${command}\`; its credits table changed, so read it and fix readmeCost`);
  const credits = row.split("|").at(-2)!.trim();
  if (/^free|presets free/.test(credits)) return 0;
  const n = credits.match(/\d+/);
  if (!n) throw new Error(`tools/tripo/README.md's row for \`${command}\` gives no credits ("${credits}")`);
  return Number(n[0]);
}
