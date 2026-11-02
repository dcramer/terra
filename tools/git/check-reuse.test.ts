import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { changesCheck, manifestDeps, reusable, save } from "./check-reuse.ts";

// terra_game depends on terra_gen; terra_land is apart from both.
const deps = new Map([
  ["terra_gen", []],
  ["terra_game", ["terra_gen"]],
  ["terra_land", []],
]);
const change = (gained: string[], pushed: string[], included: string[] = []) => changesCheck({ gained, pushed, deps, included: new Set(included) });

test("main's new commits leave the check as it was only when they touch nothing it read", () => {
  const pushed = ["crates/terra_land/src/lib.rs", "docs/land.md"];
  // Docs, research, tools, assets and an unrelated crate: the check reads none of them.
  assert.equal(change(["docs/other.md", "research/x.md", "tools/sandbox/run.ts", "assets/data/a.json", "crates/terra_game/src/a.rs"], pushed), undefined);
  assert.equal(change([], pushed), undefined);
  // The same file, the same crate, a crate it depends on or one depending on it.
  assert.match(change(["docs/land.md"], pushed)!, /main changed docs\/land.md too/);
  assert.match(change(["crates/terra_land/src/other.rs"], pushed)!, /in terra_land, which the push changes/);
  assert.match(change(["crates/terra_gen/src/hash.rs"], ["crates/terra_game/src/a.rs"])!, /in terra_gen, which terra_game depends on/);
  assert.match(change(["crates/terra_game/src/a.rs"], ["crates/terra_gen/src/hash.rs"])!, /in terra_game, which depends on terra_gen/);
  // What every check reads, on either side.
  for (const f of ["Cargo.lock", "Cargo.toml", "xtask/src/check.rs", "patches/bevy_pbr/src/lib.rs", ".cargo/config.toml", "rust-toolchain.toml"]) {
    assert.match(change([f], pushed)!, /which every check reads/, f);
  }
  assert.match(change(["crates/terra_game/src/a.rs"], ["Cargo.lock"])!, /lints every crate/);
  assert.equal(change(["docs/x.md"], ["Cargo.lock"]), undefined, "docs only: nothing to lint again");
  // A file a crate's code includes.
  assert.match(change(["tools/sandbox/places.ts"], pushed, ["tools/sandbox/places.ts"])!, /which a crate's code includes/);
});

test("a manifest's workspace dependencies, of every kind and form", () => {
  const names = new Set(["terra_gen", "terra_core", "terra_land", "terra_bake"]);
  const text = `[package]\nname = "terra_x"\nversion.workspace = true\n\n[dependencies]\nbevy = { workspace = true }\nterra_gen.workspace = true\nterra_core = { path = "../terra_core" }\n\n[dev-dependencies]\nterra_land.workspace = true\n\n[target.'cfg(unix)'.dependencies.terra_bake]\npath = "../terra_bake"\n\n[features]\nterra_gen = []\n`;
  assert.deepEqual(manifestDeps(text, names).sort(), ["terra_bake", "terra_core", "terra_gen", "terra_land"]);
  assert.deepEqual(manifestDeps(`[features]\nterra_gen = []\n`, names), []);
});

// ---- A clone with main moving on under a pushed commit ----

function repo() {
  const root = mkdtempSync(join(tmpdir(), "check-reuse-"));
  const run = (...args: string[]) => {
    const r = spawnSync("git", ["-c", "user.name=t", "-c", "user.email=t@t", ...args], { cwd: root, encoding: "utf8" });
    assert.equal(r.status, 0, `git ${args.join(" ")}: ${r.stderr}`);
    return r.stdout.trim();
  };
  const put = (file: string, text: string) => {
    mkdirSync(dirname(join(root, file)), { recursive: true });
    writeFileSync(join(root, file), text);
  };
  const commit = (files: Record<string, string>, message: string) => {
    for (const [f, t] of Object.entries(files)) put(f, t);
    run("add", "-A");
    run("commit", "-q", "-m", message);
    return run("rev-parse", "HEAD");
  };
  run("init", "-q", "-b", "main");
  const manifest = (name: string, dep = "") => `[package]\nname = "${name}"\n\n[dependencies]\n${dep}`;
  const base = commit(
    {
      "crates/terra_gen/Cargo.toml": manifest("terra_gen"),
      "crates/terra_gen/src/lib.rs": "pub fn a() {}\n",
      "crates/terra_game/Cargo.toml": manifest("terra_game", "terra_gen.workspace = true\n"),
      "crates/terra_game/src/lib.rs": "pub fn g() {}\n",
      "crates/terra_land/Cargo.toml": manifest("terra_land"),
      "crates/terra_land/src/lib.rs": 'const P: &str = include_str!("../../../tools/places.ts");\n',
      "tools/places.ts": "export {};\n",
      "docs/a.md": "a\n",
    },
    "base",
  );
  return { root, run, commit, base };
}

/// The agent's commit on `from`, checked and passed; then main gains `gained` and the agent
/// rebases onto it: what the rebased push may reuse.
function race(gained: Record<string, string>, mine: Record<string, string> = { "crates/terra_land/src/lib.rs": 'const P: &str = include_str!("../../../tools/places.ts");\npub fn l() {}\n' }) {
  const r = repo();
  r.run("checkout", "-q", "-b", "agent");
  const first = r.commit(mine, "mine");
  save(r.root, first, r.base);
  r.run("checkout", "-q", "main");
  const main = r.commit(gained, "theirs");
  r.run("checkout", "-q", "agent");
  r.run("rebase", "-q", "main");
  const rebased = r.run("rev-parse", "HEAD");
  assert.notEqual(rebased, first);
  return { ...r, first, main, rebased, reuse: reusable(r.root, rebased, main) };
}

test("a pass is reused after a clean rebase over commits that touch nothing it read", () => {
  const { reuse, first } = race({ "docs/b.md": "b\n", "crates/terra_game/src/lib.rs": "pub fn g2() {}\n" });
  assert.ok("passed" in reuse, JSON.stringify(reuse));
  assert.equal(reuse.passed.sha, first);
  assert.equal(reuse.gained, 1);
});

test("a pass is not reused when main touched what it read, or the patches changed", () => {
  const overlap = race({ "crates/terra_land/src/other.rs": "pub fn o() {}\n" });
  assert.match("why" in overlap.reuse ? overlap.reuse.why : "", /in terra_land, which the push changes/);
  const dep = race({ "crates/terra_gen/src/lib.rs": "pub fn b() {}\n" }, { "crates/terra_game/src/lib.rs": "pub fn g() { }\n" });
  assert.match("why" in dep.reuse ? dep.reuse.why : "", /terra_gen, which terra_game depends on/);
  const included = race({ "tools/places.ts": "export const x = 1;\n" });
  assert.match("why" in included.reuse ? included.reuse.why : "", /which a crate's code includes/);
  const lock = race({ "Cargo.lock": "# changed\n" });
  assert.match("why" in lock.reuse ? lock.reuse.why : "", /every check reads/);
  // Amended after its pass: other patches, no pass of them.
  const r = race({ "docs/b.md": "b\n" });
  r.commit({ "crates/terra_land/src/lib.rs": "pub fn amended() {}\n" }, "more");
  const head = r.run("rev-parse", "HEAD");
  assert.deepEqual(reusable(r.root, head, r.main), { why: "no earlier pass of these patches" });
});

test("a main that was rewritten under the pass, or a merge among the pushed commits, checks again", () => {
  const r = race({ "docs/b.md": "b\n" });
  // Main rewritten: the pass's base is no longer under it.
  r.run("checkout", "-q", "-b", "rewritten", r.base);
  r.run("commit", "-q", "--amend", "-m", "base, rewritten");
  const other = r.run("rev-parse", "HEAD");
  r.run("checkout", "-q", "-b", "replayed", r.first);
  r.run("rebase", "-q", "--onto", other, r.base);
  const replayed = r.run("rev-parse", "HEAD");
  assert.match("why" in reusable(r.root, replayed, other) ? (reusable(r.root, replayed, other) as { why: string }).why : "", /does not hold/);
  // A merge among the pushed commits.
  r.run("checkout", "-q", "-b", "merged", r.main);
  r.run("merge", "-q", "--no-ff", "-m", "merge", r.rebased);
  assert.deepEqual(reusable(r.root, r.run("rev-parse", "HEAD"), r.base), { why: "a merge or an empty commit among the pushed ones" });
});
