import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { test } from "node:test";
import { guarded } from "./guarded.ts";

const origin = ["git@github.com:gricha/terra.git", "https://github.com/gricha/terra"];

test("a push to origin is checked, by its name or either spelling of its URL", () => {
  assert.ok(guarded("origin", "git@github.com:gricha/terra.git", origin));
  assert.ok(guarded("git@github.com:gricha/terra", "git@github.com:gricha/terra", origin));
  assert.ok(guarded("https://github.com/gricha/terra.git/", "https://github.com/gricha/terra.git/", origin));
});

test("a push to the render worker, or any other remote, is not checked", () => {
  const worker = "ssh://workstation/home/gricha/terra-render/repo";
  assert.ok(!guarded(worker, worker, origin));
  assert.ok(!guarded("workstation", worker, origin));
  assert.ok(!guarded("", "", []));
});

const git = (cwd: string, ...args: string[]) => spawnSync("git", args, { cwd, encoding: "utf8" });

test("the hook runs its checks on a push to origin and none on a job push to a worker", () => {
  // A repo with origin and a worker as bare repos, and the real pre-push hook with its checks
  // stood in for by a script that records it ran.
  const root = mkdtempSync(join(tmpdir(), "guarded-"));
  const repo = join(root, "repo");
  for (const bare of ["origin.git", "worker.git"]) git(root, "init", "-q", "--bare", bare);
  git(root, "init", "-q", "-b", "main", repo);
  git(repo, "remote", "add", "origin", join(root, "origin.git"));
  mkdirSync(join(repo, ".githooks"));
  mkdirSync(join(repo, "tools/git"), { recursive: true });
  const here = resolve(import.meta.dirname);
  writeFileSync(join(repo, "tools/git/guarded.ts"), readFileSync(join(here, "guarded.ts")));
  writeFileSync(join(repo, "tools/git/undone.ts"), "");
  // remote-check.ts stands in for both the remote and the local check: TERRA_REMOTE_CHECKS=1.
  writeFileSync(join(repo, "tools/git/remote-check.ts"), `import { appendFileSync, readFileSync } from "node:fs";\nif (!process.argv.includes("--enabled")) appendFileSync(${JSON.stringify(join(root, "ran"))}, readFileSync(0, "utf8"));\n`);
  const hook = join(repo, ".githooks/pre-push");
  writeFileSync(hook, readFileSync(resolve(here, "../../.githooks/pre-push")));
  chmodSync(hook, 0o755);
  git(repo, "config", "core.hooksPath", ".githooks");
  git(repo, "add", "-A");
  git(repo, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", "one");
  const env = { ...process.env, TERRA_REMOTE_CHECKS: "1" };
  const push = (...args: string[]) => spawnSync("git", ["push", "-q", ...args], { cwd: repo, encoding: "utf8", env });
  const ran = () => {
    try {
      return readFileSync(join(root, "ran"), "utf8");
    } catch {
      return "";
    }
  };
  assert.equal(push(join(root, "worker.git"), "HEAD:refs/jobs/20261024-000000-abcd").status, 0);
  assert.equal(ran(), "", "a job push to the worker ran the checks");
  assert.equal(push("origin", "HEAD:main").status, 0);
  assert.match(ran(), /refs\/heads\/main/, "a push to origin skipped the checks");
});
