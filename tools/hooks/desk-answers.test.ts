// The desk-answers hook speaks only to a session that uses the desk or whose prompt mentions it.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { reminder } from "./desk-answers.ts";

const URL = "https://claude.ai/code/artifact/0000-desk";

test("a session that posted to the desk is reminded on every prompt", () => {
  const line = reminder(URL, "how we looking", `{"input":{"url":"${URL}"}}`);
  assert.match(line ?? "", /status "answered"/);
  assert.ok(line?.includes(URL));
});

test("a prompt that mentions the desk is reminded in any session", () => {
  for (const prompt of ["i think i answered items in the desk", "answered your questions in the deck"]) {
    assert.ok(reminder(URL, prompt, ""), prompt);
  }
});

test("other sessions, and machines with no desk, hear nothing", () => {
  assert.equal(reminder(URL, "fix the icons test", "nothing about it"), undefined);
  assert.equal(reminder("", "answered on the desk", URL), undefined);
  assert.equal(reminder(URL, "the desktop build", ""), undefined);
});

test("run as a hook it prints the context JSON, or nothing", () => {
  const home = mkdtempSync(join(tmpdir(), "desk-hook-"));
  const config = join(home, ".config/terra");
  spawnSync("mkdir", ["-p", config]);
  writeFileSync(join(config, "desk.json"), JSON.stringify({ url: URL }));
  const run = (prompt: string) =>
    spawnSync("node", [join(import.meta.dirname, "desk-answers.ts")], {
      input: JSON.stringify({ prompt, transcript_path: join(home, "none.jsonl") }),
      env: { ...process.env, HOME: home },
      encoding: "utf8",
    });
  const said = run("did you see the desk?");
  assert.equal(said.status, 0);
  assert.equal(JSON.parse(said.stdout).hookSpecificOutput.hookEventName, "UserPromptSubmit");
  assert.equal(run("fix the icons test").stdout, "");
});
