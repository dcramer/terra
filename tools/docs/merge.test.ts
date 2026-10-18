import assert from "node:assert/strict";
import { test } from "node:test";
import { ORDER, merge } from "./merge.ts";
import { renderNotes } from "./docs.ts";

const page = (sections: Record<string, string[]>, intro = "Intro.") =>
  [
    "# Decisions",
    "",
    intro,
    "",
    "<!-- rendered from docs/decisions/ by `node tools/docs/docs.ts --render`: edit that, not this -->",
    "",
    ...Object.entries(sections).flatMap(([title, rows]) => [`## ${title}`, "", "| # | Topic |", "|---|---|", ...rows, ""]),
    "<!-- end of rendered -->",
    "",
  ].join("\n");
const row = (n: number, topic = "T") => `| [D${n}](decisions/D${n}.md) | ${topic} |`;
const byNumber = ORDER["docs/decisions.md"];

test("two sides adding rows to one table both keep theirs, in the page's order", () => {
  const base = page({ Building: [row(1)], Tools: [row(2)] });
  const ours = page({ Building: [row(1), row(10)], Tools: [row(2)] });
  const theirs = page({ Building: [row(1), row(9)], Tools: [row(2), row(11)] });
  assert.equal(merge(base, ours, theirs, byNumber), page({ Building: [row(1), row(9), row(10)], Tools: [row(2), row(11)] }));
});

test("a row changed or moved on one side and added beside on the other merges, with no stale copy", () => {
  const base = page({ Building: [row(1), row(2)], Gone: [] });
  const ours = page({ Building: [row(1)], Gone: [row(2, "Superseded")] });
  const theirs = page({ Building: [row(1), row(2), row(3)], Gone: [] });
  assert.equal(merge(base, ours, theirs, byNumber), page({ Building: [row(1), row(3)], Gone: [row(2, "Superseded")] }));
});

test("one row changed two ways, or the page round the index changed on both sides, is a conflict", () => {
  const base = page({ Building: [row(1)] });
  assert.equal(merge(base, page({ Building: [row(1, "A")] }), page({ Building: [row(1, "B")] }), byNumber), undefined);
  assert.equal(merge(base, page({ Building: [row(1)] }, "Ours."), page({ Building: [row(1)] }, "Theirs."), byNumber), undefined);
  // One side's change round the index is kept.
  assert.equal(merge(base, page({ Building: [row(1)] }, "Ours."), page({ Building: [row(1), row(2)] }), byNumber), page({ Building: [row(1), row(2)] }, "Ours."));
});

test("the research index is each note's title and date, newest first, and a note with no date is a fault", () => {
  const notes = [
    { name: "old.md", text: "# Old | piped\n\n2026-10-05. Question: why?\n" },
    { name: "2026-10-17-new.md", text: "# New\n\nQuestion: what?\n" },
    { name: "same-day.md", text: "# Also new\n\n2026-10-17. Question: how?\n" },
  ];
  assert.equal(
    renderNotes(notes),
    [
      "| Note | Dated |",
      "|---|---|",
      "| [New](2026-10-17-new.md) | 2026-10-17 |",
      "| [Also new](same-day.md) | 2026-10-17 |",
      "| [Old \\| piped](old.md) | 2026-10-05 |",
      "",
    ].join("\n"),
  );
  assert.throws(() => renderNotes([{ name: "undated.md", text: "# Undated\n\nQuestion: when?\n" }]), /undated\.md/);
});
