// Nothing is guessed (D654): a research note grades its sources, and a design doc carries a Prior art
// section of sources read. Part of the docs check (`node tools/docs/docs.ts`); it reads text only.
//
// It cannot tell whether a source was opened, or whether a claim with no link is true: it holds the
// shape (a grade on every source, memory marked as memory, two primary sources under a design), so a
// guess has to be written down as one.

import { posix } from "node:path";

/** A source's grade: primary read now, secondary, community, or from memory (D654). */
const GRADE = /\[[PSCM]\]/;
const PRIMARY = /\[P\]/;
const LINK = /https?:\/\//;
const MEMORY = /\bfrom memory\b|\brecalled\b/i;
const RULE = "[P] primary read now (the maker's source, docs, paper or talk), [S] secondary, [C] community, [M] from memory";

/** Research notes dated from here grade their sources; earlier ones are as they were written. */
export const SINCE = "2026-11-09";

/** Notes of that day written before the rule, held as they are; regrade one and take it off. */
export const HELD_NOTES: string[] = ["research/2026-11-09-holding-items-programmatically.md", "research/2026-11-09-lighting-system-review.md"];

/**
 * Top-level docs from before the rule, with no Prior art section that passes, pages that design no
 * system among them. A doc leaves the list when its system is next rebuilt and it gains the section;
 * none is ever added.
 */
// prettier-ignore
export const BEFORE_D654: string[] = [
  "docs/ambient-life.md",
  "docs/animation.md",
  "docs/arc.md",
  "docs/art-style.md",
  "docs/assembled-items.md",
  "docs/attachment.md",
  "docs/audio.md",
  "docs/biomes.md",
  "docs/building-drawing.md",
  "docs/building-kits.md",
  "docs/building-plan.md",
  "docs/building-tech.md",
  "docs/building-voxels.md",
  "docs/building.md",
  "docs/caves.md",
  "docs/characters.md",
  "docs/ci-agent.md",
  "docs/climate.md",
  "docs/combat.md",
  "docs/controller.md",
  "docs/crafting.md",
  "docs/creature-rigs.md",
  "docs/creatures.md",
  "docs/demos.md",
  "docs/dungeons.md",
  "docs/engine.md",
  "docs/fire-module.md",
  "docs/fire.md",
  "docs/gameplay.md",
  "docs/holding.md",
  "docs/instances.md",
  "docs/iso.md",
  "docs/items.md",
  "docs/keyed-moves.md",
  "docs/launcher.md",
  "docs/lore.md",
  "docs/magic.md",
  "docs/movement.md",
  "docs/multiplayer.md",
  "docs/naming.md",
  "docs/netcode.md",
  "docs/people.md",
  "docs/planet-sim.md",
  "docs/player.md",
  "docs/progression.md",
  "docs/prototypes.md",
  "docs/realm-two.md",
  "docs/render-worker.md",
  "docs/scene-rules.md",
  "docs/settings.md",
  "docs/sky.md",
  "docs/survival.md",
  "docs/tech.md",
  "docs/terrain.md",
  "docs/trailer.md",
  "docs/tribe.md",
  "docs/troll-fight.md",
  "docs/vision.md",
  "docs/weather.md",
  "docs/worlds.md",
];

/** Top-level docs that design no system (an index, a status page, a plan, a guide), each with what it is. */
export const NOT_A_SYSTEM: Record<string, string> = {};

/** Pages under docs/ that are never a system's design, by name. */
const NEVER_A_SYSTEM = /^docs\/(README|decisions|decisions-log|development|v0-[^/]+)\.md$/;

function lines(markdown: string): string[] {
  return markdown
    .replace(/^```[\s\S]*?^```/gm, "")
    .replace(/`[^`\n]*`/g, "")
    .split("\n");
}

/** The body of the first `Prior art` heading, to the next heading of its level or above. */
export function priorArt(markdown: string): string | undefined {
  const all = markdown.replace(/^```[\s\S]*?^```/gm, "").split("\n");
  const start = all.findIndex((l) => /^#{2,4} Prior art\b/i.test(l));
  if (start < 0) return undefined;
  const level = all[start].match(/^#+/)![0].length;
  const rest = all.slice(start + 1);
  const end = rest.findIndex((l) => new RegExp(`^#{1,${level}} `).test(l));
  return rest.slice(0, end < 0 ? undefined : end).join("\n");
}

/** Lines naming a source with no grade, and lines from memory not marked [M]. */
function ungraded(file: string, markdown: string): string[] {
  const faults: string[] = [];
  for (const line of lines(markdown)) {
    const quote = line.trim().slice(0, 70);
    if (LINK.test(line) && !GRADE.test(line)) {
      faults.push(
        `${file}: a source with no grade, at "${quote}…" (D654: nothing rests on a guess, so a reader must see what was read): ` +
          `mark the line ${RULE}; open the source before marking it [P], do not mark it [P] from what you remember of it`,
      );
    } else if (MEMORY.test(line) && !/\[M\]/.test(line)) {
      faults.push(
        `${file}: a claim from memory not marked [M], at "${quote}…" (D654: memory is a lead, never a ground): ` +
          `look it up and cite the source with its grade, or mark the line [M]; nothing is built, decided or gated on an [M] line`,
      );
    }
  }
  return faults;
}

/** How many lines carry a primary source: a link graded [P]. */
function primaries(markdown: string): number {
  return lines(markdown).filter((l) => LINK.test(l) && PRIMARY.test(l)).length;
}

/** The faults of a doc's sources against D654. `read` gives a tracked file's text. */
export function sourceFaults(
  file: string,
  markdown: string,
  read: (path: string) => string | undefined,
  held = { notes: HELD_NOTES, docs: BEFORE_D654, not: NOT_A_SYSTEM },
): string[] {
  const note = file.match(/^research\/(\d{4}-\d{2}-\d{2})-[^/]+\.md$/);
  if (note) return note[1] >= SINCE && !held.notes.includes(file) ? ungraded(file, markdown) : [];

  if (!/^docs\/[^/]+\.md$/.test(file) || NEVER_A_SYSTEM.test(file) || file in held.not) return [];
  const section = priorArt(markdown);
  const faults: string[] = [];
  if (section === undefined) {
    faults.push(
      `${file}: a design doc with no Prior art section (D654: a system starts from sources read, not from a guess): ` +
        `read at least two shipped implementations now and add \`## Prior art\` with each linked and graded (${RULE}), the architecture they share, ` +
        `the one we port and where we depart (docs/development.md, "Building a system without guessing"); ` +
        `if the page designs no system, add it to NOT_A_SYSTEM in tools/docs/sources.ts with what it is; do not add it to BEFORE_D654`,
    );
  } else {
    faults.push(...ungraded(file, section));
    const notes = [...section.matchAll(/\]\(([^)#\s]*research\/[^)#\s]+\.md)/g)].map((m) => posix.normalize(posix.join(posix.dirname(file), m[1])));
    const best = Math.max(primaries(section), ...notes.map((n) => primaries(read(n) ?? "")));
    if (best < 2) {
      faults.push(
        `${file}: its Prior art has ${primaries(section)} primary source(s)${notes.length ? `, and no research note it links has two (${notes.join(", ")})` : ""} ` +
          `(D654: at least two shipped implementations, read and linked): read a second one and add it graded [P] with its link, here or in the note; ` +
          `a source not opened is [M] and does not count`,
      );
    }
  }
  if (!held.docs.includes(file)) return faults;
  return faults.length ? [] : [`${file}: its Prior art passes now: take it off BEFORE_D654 in tools/docs/sources.ts (D654)`];
}
