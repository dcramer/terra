// Reading a job's cookie and saying what of a job was left, on made-up input (the stopping of real
// processes is in flow.test.ts).
import assert from "node:assert/strict";
import { test } from "node:test";
import { cookie, reaped } from "./reap.ts";

test("a process's job is the TERRA_RENDER_JOB of its environment, none without one", () => {
  assert.equal(cookie("HOME=/h\0TERRA_RENDER_JOB=20261029-083350-269b-patch\0PATH=/bin\0"), "20261029-083350-269b-patch");
  assert.equal(cookie("HOME=/h\0MY_TERRA_RENDER_JOB=x\0"), undefined);
  assert.equal(cookie("TERRA_RENDER_JOB=\0"), undefined);
  assert.equal(cookie(""), undefined);
});

test("the job log says nothing is left, what was stopped, or what survived and what to do", () => {
  const p = { pid: 7, job: "j", cmd: "sh -c while :; do cargo clippy; done" };
  assert.equal(reaped("j", { found: [], left: [] }, "cancelled"), "cancelled: nothing of the job is left running (no process carries TERRA_RENDER_JOB=j)");
  assert.match(reaped("j", { found: [p], left: [] }, "cancelled"), /^cancelled: stopped 1 process of the job still running \(pid 7 `sh -c while/);
  assert.match(reaped("j", { found: [p], left: [] }, "cancelled"), /nothing of the job is left\./);
  const stuck = reaped("j", { found: [p], left: [p] }, "timed out");
  assert.match(stuck, /^✗ timed out: 1 of the job's processes still run after SIGKILL: pid 7/);
  assert.match(stuck, /kill -9 <pid>/);
});
