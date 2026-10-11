import assert from "node:assert/strict";
import { test } from "node:test";
import { page, split, stack } from "./page.js";

const day = (d, claude, tripo) => ({
  day: d, input: 0, output: 10, cache_read: 0, cache_write: 0, messages: 1, total: 10,
  cost: claude + tripo, claude_cost: claude, tripo_cost: tripo, tripo_credits: tripo * 150,
});
const person = (user, claude, tripo) => ({
  user, ...day("2026-10-11", claude, tripo),
  models: [{ model: "claude-opus-5-5", cost: claude }, ...(tripo ? [{ model: "tripo-studio", credits: tripo * 150, cost: tripo }] : [])],
  daily: [day("2026-10-10", 0, 0), day("2026-10-11", claude, tripo)],
});
const summary = {
  generated_at: "2026-10-11T23:00:00.000Z",
  days: 2,
  totals: day("2026-10-11", 30, 2),
  users: [person("ana", 30, 2), person("ben", 5, 0)],
};

test("splits a total between Claude and Tripo", () => {
  const html = split(summary.totals);
  assert.match(html, /data-key="claude_cost"[^>]*>\$30\.00<\/b> Claude/);
  assert.match(html, /data-key="tripo_cost"[^>]*>\$2\.00<\/b> Tripo/);
  assert.match(html, /data-key="tripo_credits"[^>]*>300<\/span> credits/);
});

test("shows the split under the total and on everyone", () => {
  const html = page(summary);
  assert.equal(html.match(/class="split"/g).length, 3);
  assert.match(html, /<span class="tripo" title="300 credits, \$2\.00">tripo-studio<\/span>/);
});

test("stacks Tripo's cost on Claude's in a day's bar, rounding only the top", () => {
  assert.equal(stack(day("d", 3, 1), 4), '<i class="claude" style="height:75.0%"></i><i class="tripo top" style="height:25.0%"></i>');
  assert.equal(stack(day("d", 4, 0), 4), '<i class="claude top" style="height:100.0%"></i><i class="tripo top" style="height:0.0%"></i>');
});
