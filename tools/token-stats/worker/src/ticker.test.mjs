import assert from "node:assert/strict";
import { test } from "node:test";
import { ticker } from "./ticker.js";

const counts = (cost, input) => ({ input, output: 0, cache_read: 1e9, cache_write: 0, messages: 1, cost });
const summary = {
  totals: counts(30, 3000),
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
  assert.match(html, /data-user="ana"><span class="name">ana<\/span><span class="amount" data-n="10">\$10\.00</);
  assert.match(html, /data-user="ben"><span class="name">ben<\/span><span class="amount" data-n="20">\$20\.00</);
});

test("counts one person's alone with a user", () => {
  const html = ticker(summary, "dark", "ben");
  assert.match(html, /class="cost" data-n="20"/);
  assert.match(html, /class="tokens" data-n="2000"/);
  assert.match(html, /data-user="ben"/);
  assert.match(html, /ben · <span class="tokens"/);
  assert.match(html, /data-theme="dark"/);
  assert.doesNotMatch(html, /class="person"/);
});

test("is null for someone who has reported nothing", () => {
  assert.equal(ticker(summary, null, "cara"), null);
});
