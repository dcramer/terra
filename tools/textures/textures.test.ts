import assert from "node:assert/strict";
import { test } from "node:test";
import { credit, period } from "./textures.ts";

test("an image of rows repeats where its rows do", () => {
  // Eight wide and 40 high: a light row every 10 pixels.
  const width = 8;
  const height = 40;
  const data = Buffer.alloc(width * height);
  for (let y = 0; y < height; y++) data.fill(y % 10 === 0 ? 220 : 60, y * width, (y + 1) * width);
  assert.equal(period(data, width, height, "y"), 20);
});

test("a material's credit goes in name order and replaces what it said before", () => {
  const readme = "# Materials\n\n- `a`: one.\n- `c`: three.\n";
  const added = credit(readme, "b", "two.");
  assert.equal(added, "# Materials\n\n- `a`: one.\n- `b`: two.\n- `c`: three.\n");
  assert.equal(credit(added, "b", "again."), "# Materials\n\n- `a`: one.\n- `b`: again.\n- `c`: three.\n");
  assert.equal(credit(readme, "d", "four."), "# Materials\n\n- `a`: one.\n- `c`: three.\n- `d`: four.\n");
});

test("the first credit is set apart from the README's introduction", () => {
  assert.equal(credit("# Materials\n\nWhere.\n", "a", "one."), "# Materials\n\nWhere.\n\n- `a`: one.\n");
});
