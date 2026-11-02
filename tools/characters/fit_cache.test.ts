import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { Cache, closure, key } from "./fit_cache.ts";

/** A scratch repository: tools/characters with scripts that reach each other, and a data file. */
function scratch() {
  const root = mkdtempSync(join(tmpdir(), "fit-cache-"));
  const chars = join(root, "tools", "characters");
  mkdirSync(chars, { recursive: true });
  const w = (f: string, s: string) => writeFileSync(join(chars, f), s);
  w("maker.sh", 'here=tools/characters\n$blender $here/fit_one.py "$item"\nnode $here/list.ts one "$item"\n');
  w("fit_one.py", "import json\nimport look\nfrom carry import carried\n");
  w("look.py", "X = 1\n");
  w("carry.py", "def carried(): pass\n");
  w("list.ts", 'import { a } from "./util.ts";\nconst step = ["step.py", a];\n');
  w("step.py", "import json\n");
  w("util.ts", "export const a = 1;\n");
  w("lone.ts", "export const b = 2;\n");
  writeFileSync(join(root, "data.json"), '{"turn": -90}\n');
  return { root, chars };
}

test("a stage's key covers every script its own reaches", () => {
  const { root, chars } = scratch();
  const names = closure(join(chars, "maker.sh"), root).map((f) => f.slice(chars.length + 1));
  assert.deepEqual(names, ["carry.py", "fit_one.py", "list.ts", "look.py", "maker.sh", "step.py", "util.ts"]);
  rmSync(root, { recursive: true });
});

test("a key changes with any input's bytes, and with nothing else", () => {
  const { root, chars } = scratch();
  const inputs = () => [...closure(join(chars, "maker.sh"), root), join(root, "data.json")];
  const k = key("make", root, inputs());
  assert.equal(key("make", root, inputs()), k, "nothing changed");
  // Touched, its bytes the same: the same key.
  utimesSync(join(chars, "util.ts"), new Date(), new Date(Date.now() + 5000));
  assert.equal(key("make", root, inputs()), k, "touched only");
  writeFileSync(join(chars, "lone.ts"), "export const b = 3;\n");
  assert.equal(key("make", root, inputs()), k, "a script it does not reach");
  writeFileSync(join(chars, "carry.py"), "def carried(): return 1\n");
  const k2 = key("make", root, inputs());
  assert.notEqual(k2, k, "a script two imports down");
  writeFileSync(join(root, "data.json"), '{"turn": 90}\n');
  assert.notEqual(key("make", root, inputs()), k2, "the item's data");
  assert.notEqual(key("moves", root, inputs()), key("make", root, inputs()), "another stage");
  assert.notEqual(key("make", root, inputs(), ["--template", "male"]), key("make", root, inputs()), "its values");
  rmSync(root, { recursive: true });
});

test("a kept stage puts back the files it made, by their bytes", () => {
  const { root } = scratch();
  const cache = new Cache(join(root, "cache"));
  const look = join(root, "look.glb");
  writeFileSync(look, "made once");
  assert.equal(cache.get("k1", root), undefined, "nothing kept yet");
  cache.put("k1", root, { ok: true, line: "made", data: [1, 2] }, [look]);
  writeFileSync(look, "made from other inputs");
  const e = cache.get("k1", root)!;
  assert.deepEqual([e.ok, e.data], [true, [1, 2]]);
  assert.equal(readFileSync(look, "utf8"), "made once");
  rmSync(look);
  cache.get("k1", root);
  assert.equal(readFileSync(look, "utf8"), "made once", "put back when gone");
  rmSync(join(root, "cache", "blobs"), { recursive: true });
  writeFileSync(look, "other");
  assert.equal(cache.get("k1", root), undefined, "its bytes lost: run again");
  rmSync(root, { recursive: true });
});
