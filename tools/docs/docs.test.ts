import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import {
  AREAS,
  anchors,
  brokenLinks,
  decisionIds,
  gist,
  GUIDES,
  missingPaths,
  missingTests,
  nextDecision,
  readDecisions,
  renderIndex,
  renderMatrix,
  renderPages,
  rendered,
  show,
  strayRows,
  unindexed,
  unknownDecisions,
  unknownTasks,
  xtaskTasks,
} from "./docs.ts";

test("headings anchor as GitHub makes them, repeats numbered", () => {
  const a = anchors("# Worlds: the realms\n## Size\n## Size\n## What `it` asks, (D4)\n```\n# not a heading\n```\n");
  assert.deepEqual([...a], ["worlds-the-realms", "size", "size-1", "what-it-asks-d4"]);
});

test("a link to a missing file or heading is a fault; the web and code are not looked at", () => {
  const files: Record<string, string> = { "docs/a.md": "## Water\n", "docs/b.md": "" };
  const read = (p: string) => files[p];
  const text = "[ok](a.md#water) [gone](c.md) [no](a.md#size) [web](https://x.y/z) `[code](nope.md)` [self](#here)\n## Here\n";
  assert.deepEqual(brokenLinks("docs/b.md", text, (p) => (p === "docs/b.md" ? text : read(p))), [
    "docs/b.md: links to c.md, which does not exist",
    "docs/b.md: links to a.md#size, which has no such heading",
  ]);
});

const file = (id: string, meta: string, body = "Two.\n\n## Why\n\nWhy.\n", topic = "Team") => ({
  name: `${id}.md`,
  text: `# ${id}: ${topic}\n\n${meta}\n\n${body}`,
});

test("a decision file needs its shape, its number for a name, a known area, owner, status and who decided", () => {
  const files = [
    file("D1", "Area: Building · Owner: Design · By: Developers · Status: Decided"),
    file("D2", "Area: Engine and code · Owner: Tech · By: Agent · Status: Leaning", "Bevy.\n"),
    file("D3", "Area: — · Owner: Design · By: — · Status: Superseded by D1, D2"),
    file("D4", "Area: Building · Owner: Design · By: Agent · Status: Open"),
    file("D5", "Area: Art · Owner: Art · By: Someone · Status: Done"),
    { name: "D6.md", text: "# D7: Misnamed\n\nArea: Building · Owner: Tech · By: Agent · Status: Decided\n\nX.\n" },
    { name: "D8.md", text: "| D8 | Old | Tech | Agent | Decided | A row. | Why. |\n" },
    { name: "notes.md", text: "" },
  ];
  const { decisions, faults } = readDecisions(files);
  assert.deepEqual(
    decisions.map((d) => [d.id, d.decision, d.why]),
    [["D1", "Two.", "Why."], ["D2", "Bevy.", ""], ["D3", "Two.", "Why."], ["D4", "Two.", "Why."], ["D5", "Two.", "Why."], ["D7", "X.", ""]],
  );
  assert.deepEqual(faults, [
    'docs/decisions/D4.md: has By "Agent": Developers or Agent, or — when Open, Rejected or Superseded',
    'docs/decisions/D5.md: has Owner "Art", not Design or Tech',
    'docs/decisions/D5.md: has Status "Done": Decided, Leaning, Open, Rejected or Superseded by Dn',
    'docs/decisions/D5.md: has By "Someone": Developers or Agent, or — when Open, Rejected or Superseded',
    "docs/decisions/D5.md: has Area \"Art\": one of docs/decisions.md's headings, or — when Rejected or Superseded",
    "docs/decisions/D6.md: is headed D7; a decision's file is named for its number",
    "docs/decisions/D8.md: is not `# Dnnn: Topic`, a blank line, `Area: … · Owner: … · By: … · Status: …`, a blank line, the decision, then optionally `## Why` and the reason",
    "docs/decisions/notes.md: is not a decision; the folder holds only Dnnn.md files",
  ]);
});

test("a gist is the first sentence, past one that only points elsewhere, cut at a clause or a word", () => {
  assert.equal(gist("Two developers, both of them busy. Every asset is free."), "Two developers, both of them busy.");
  assert.equal(gist("None. Setbacks only. A third sentence that does not fit at all."), "None. Setbacks only.");
  assert.equal(gist("Amends D270 and D286. Glass is lit by the sky. More."), "Glass is lit by the sky.");
  assert.equal(gist("See [tech.md](tech.md) for how it works. Then that."), "See tech.md for how it works.");
  assert.equal(gist("The world, its ground and water, (and what grows there) rests on rock below it all.", 40), "The world, its ground and water…");
  assert.equal(gist("One `a, b, c, d, e` and `f g h i j k l m n o p` thing", 30), "One `a, b, c, d, e` and `f g h`…");
  assert.equal(gist("Lights are *baked into every single texture* we ship today", 30), "Lights are *baked into every*…");
});

test("the index has every area's heading and a row per decision by number, the gone ones last, pipes kept", () => {
  const { decisions } = readDecisions([
    file("D9", "Area: Building · Owner: Tech · By: Agent · Status: Decided", "`a\\|b` or c, whichever is nearer. More.\n", "Pipes | too"),
    file("D3", "Area: Building · Owner: Design · By: — · Status: Superseded by D9"),
    file("D2", "Area: Building · Owner: Design · By: Developers · Status: Decided"),
  ]);
  const index = renderIndex(decisions).split("\n");
  assert.deepEqual(
    index.filter((line) => line.startsWith("## ")),
    [...AREAS.map((a) => `## ${a.title}`), "## Superseded and rejected"],
  );
  assert.deepEqual(
    index.filter((line) => line.startsWith("| [")),
    [
      "| [D2](decisions/D2.md) | Team | Design | Developers | Decided | Two. |",
      "| [D9](decisions/D9.md) | Pipes \\| too | Tech | Agent | Decided | `a\\\\|b` or c, whichever is nearer. |",
      "| [D3](decisions/D3.md) | Team | Design | — | Superseded by D9 | Two. |",
    ],
  );
});

test("an index edited by hand, or a table written into the page, names the files to edit", () => {
  const begin = "<!-- rendered from docs/decisions/ by `node tools/docs/docs.ts --render`: edit that, not this -->";
  const page = `# Decisions\n\n${begin}\n\n| D1 | Team | Design | Developers | Decided | Two. | Why. |\n<!-- end of rendered -->\n`;
  const read = (path: string) => (path === "docs/decisions.md" ? page : undefined);
  const faults = renderPages(read).filter((f) => f.startsWith("docs/decisions.md"));
  assert.deepEqual(faults, ["docs/decisions.md: decisions live in docs/decisions/Dnnn.md; edit that and run `node tools/docs/docs.ts --render`"]);
  assert.deepEqual(strayRows(page), []);
  assert.deepEqual(strayRows(`# Decisions\n| D1 | x |\n${begin}\n<!-- end of rendered -->\n`), [
    "docs/decisions.md: has a table outside its index; decisions live in docs/decisions/Dnnn.md; edit that and run `node tools/docs/docs.ts --render`",
  ]);
});

test("the next free decision number is one past the highest, gaps left alone, from files or index rows", () => {
  assert.equal(nextDecision(["D1", "D12", "D3"]), 13);
  assert.equal(nextDecision([]), 1);
  assert.deepEqual(decisionIds("D1.md\nD12.md\n| [D14](decisions/D14.md) | x |\n| D15 | old row |\nD2 is cited"), ["D1", "D12", "D14", "D15", "D2"]);
});

test("decisions are shown in full with the log's lines that name them, ranges included", () => {
  const log = "# Log\n\n- D1 to D8 added.\n- D12 changed.\n- D3 and D21 added.\n";
  const read = (id: string) => (id === "D3" ? "# D3: Old\n\nText.\n" : undefined);
  assert.equal(
    show(["d3", "4"], read, log),
    "# D3: Old\n\nText.\n\n## Log\n\n- D1 to D8 added.\n- D3 and D21 added.\n\nD4: there is no docs/decisions/D4.md\n",
  );
});

test("citing a decision that has no file is a fault, once per file", () => {
  assert.deepEqual(unknownDecisions("x.md", "D1, D9 and D9 again; not DD9 or D1234", new Set(["D1"])), [
    "x.md: cites D9, which has no docs/decisions/D9.md",
  ]);
});

test("an index must name every entry of its folder", () => {
  assert.deepEqual(unindexed("r/README.md", "[a](a.md)", ["a.md", "b.md"]), ["r/README.md: does not list b.md"]);
});

test("a contract page naming a test no crate defines, or one cut short, is a fault", () => {
  const sources = "    #[test]\n    fn a_wall_is_closed_to_its_roof() {}\n";
  const page =
    "| row | `runs`: `a_wall_is_closed_to_its_roof`, `a_door_is_centred_in_its_frame`, `a_wall_is_closed…` | `roof_walls` `runs::under_roofs` |\n";
  assert.deepEqual(missingTests("docs/v.md", page, sources), [
    "docs/v.md: names the test a_door_is_centred_in_its_frame, which no crate defines",
    "docs/v.md: names a test cut short, a_wall_is_closed…",
  ]);
});

test("a contract's matrix is rendered from its rows, tests grouped by module, then cases", () => {
  const row = { junction: "A free end", rule: "Arris up it", code: ["layout::roles"], status: "Closed" };
  const data = {
    sections: [
      {
        title: "Walls",
        rows: [
          { ...row, tests: ["layout::roles::a_b_c_d", "layout::runs::e_f_g_h", "layout::roles::i_j_k_l"], cases: ["free end"], also: "by eye" },
          { ...row, tests: [], cases: [] },
        ],
      },
    ],
  };
  assert.deepEqual(renderMatrix(data).split("\n"), [
    "### Walls",
    "",
    "| Junction | What closes it | Code | Proved by | Status |",
    "|---|---|---|---|---|",
    "| A free end | Arris up it | `layout::roles` | `layout::roles`: `a_b_c_d`, `i_j_k_l`; `layout::runs`: `e_f_g_h`; the checker's *free end*; by eye | Closed |",
    "| A free end | Arris up it | `layout::roles` | — | Closed |",
    "",
  ]);
});

test("a page's rendered part is written again between its markers, the rest kept", () => {
  const begin = "<!-- rendered from x.json by `node tools/docs/docs.ts --render`: edit that, not this -->";
  const page = `# P\n${begin}\nold\n<!-- end of rendered -->\ntail\n`;
  assert.equal(rendered(page, (data) => `from ${data}\n`), `# P\n${begin}\n\nfrom x.json\n\n<!-- end of rendered -->\ntail\n`);
  assert.equal(
    rendered("# no markers\n", () => ""),
    undefined,
  );
});

test("the tasks are read from xtask's table, and help with them", () => {
  const main = 'fn x() { let name: "not";\n}\nconst TASKS: &[Task] = &[\n    Task {\n        name: "play",\n    },\n    Task {\n        name: "paint-icons",\n';
  assert.deepEqual([...xtaskTasks(main)].sort(), ["help", "paint-icons", "play"]);
  const real = xtaskTasks(readFileSync(new URL("../../xtask/src/main.rs", import.meta.url), "utf8"));
  for (const task of ["check", "sizes", "decision", "tools"]) assert.ok(real.has(task), task);
});

test("a page naming a task that is not one is a fault; a placeholder or bare `cargo xtask` is not", () => {
  const tasks = new Set(["check", "help"]);
  const text = "Run `cargo xtask check --quick`, then cargo xtask chek. Any `cargo xtask <task>`; `cargo xtask` alone; cargo xtask check.";
  assert.deepEqual(unknownTasks("docs/a.md", text, tasks), [
    "docs/a.md: names `cargo xtask chek`, which is no task; `cargo xtask tools` lists them (xtask/src/main.rs)",
  ]);
});

test("the guides are the docs, the skills, AGENTS.md and the tools' READMEs", () => {
  const files = ["docs/a.md", ".agents/skills/x/SKILL.md", "AGENTS.md", "tools/docs/README.md", "research/n.md", "README.md", "tools/docs/docs.ts"];
  assert.deepEqual(files.filter(GUIDES), files.slice(0, 4));
});

test("a skill naming a path that is not there is a fault; placeholders, patterns and written outputs are not", () => {
  const there = new Set(["crates/terra_build/src/kit.rs", "tools/props/props.ts", "tools/props", "tools/out-ignored"]);
  const tops = new Set(["crates", "tools", "art"]);
  const text = [
    "See `crates/terra_build/src/kit.rs:40`, `crates/terra_build/src/kits.rs` and [x](`nope`).",
    "Run `node tools/props/props.ts --into tools/prop/x.glb`, then `tools/props/`.",
    "Name it `art/<name>/sheet.png`, `art/NAME/sheet.png`, `art/x/*.png` or `docs/Dnnn.md`; `target/x.rs` is no top here.",
    "It writes `tools/out-ignored/a.glb`; `art/gone.png`, `art/gone.png`.",
    "```\ncrates/fenced/out.rs\n```",
  ].join("\n");
  const exists = (path: string) => there.has(path) || path.startsWith("tools/out-ignored");
  assert.deepEqual(missingPaths(".agents/skills/x/SKILL.md", text, exists, tops), [
    ".agents/skills/x/SKILL.md: names `crates/terra_build/src/kits.rs`, which is not in the repository",
    ".agents/skills/x/SKILL.md: names `tools/prop/x.glb`, which is not in the repository",
    ".agents/skills/x/SKILL.md: names `art/gone.png`, which is not in the repository",
  ]);
});
