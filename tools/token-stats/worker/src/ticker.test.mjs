import assert from "node:assert/strict";
import { test } from "node:test";
import { ticker } from "./ticker.js";

const counts = (cost, input, tripo = 0) => ({
  input, output: 0, cache_read: 1e9, cache_write: 0, messages: 1,
  cost, claude_cost: cost - tripo, tripo_cost: tripo, tripo_credits: tripo * 100,
});
const summary = {
  totals: counts(30, 3000, 4),
  users: [
    { user: "ana", ...counts(10, 1000) },
    { user: "ben", ...counts(20, 2000) },
  ],
};

test("counts everyone without a user", () => {
  const html = ticker(summary, null);
  assert.match(html, /class="cost" data-n="30"/);
  assert.match(html, /data-user=""/);
  assert.doesNotMatch(html, /class="tokens"/);
});

test("gives everyone a line with their cost", () => {
  const html = ticker(summary, null);
  assert.match(html, /class="person" data-key="ana"><span class="name">ana<\/span><span class="amount" data-n="10">\$10\.00</);
  assert.match(html, /class="person" data-key="ben"><span class="name">ben<\/span><span class="amount" data-n="20">\$20\.00</);
});

test("gives each service a line after the people", () => {
  const html = ticker(summary, null);
  assert.match(html, /class="source claude" data-key="Claude">.*data-n="26">\$26\.00</);
  assert.match(html, /class="source tripo" data-key="Tripo">.*data-n="4">\$4\.00</);
  assert.ok(html.indexOf('data-key="ben"') < html.indexOf('data-key="Claude"'));
});

test("leaves Tripo out until it has been used", () => {
  const html = ticker({ ...summary, totals: counts(30, 3000) }, null);
  assert.match(html, /data-key="Claude"/);
  assert.doesNotMatch(html, /data-key="Tripo"/);
});

test("counts one person's alone with a user", () => {
  const html = ticker(summary, "dark", "ben");
  assert.match(html, /class="cost" data-n="20"/);
  assert.match(html, /class="tokens" data-n="2000"/);
  assert.match(html, /data-user="ben"/);
  assert.match(html, /ben · <span class="tokens"/);
  assert.match(html, /data-theme="dark"/);
  assert.doesNotMatch(html, /class="person"/);
  assert.doesNotMatch(html, /class="source/);
});

test("is null for someone who has reported nothing", () => {
  assert.equal(ticker(summary, null, "cara"), null);
});
