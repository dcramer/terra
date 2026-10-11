import assert from "node:assert/strict";
import { test } from "node:test";
import { cost, creditCost } from "./prices.js";

const counts = { input: 1e6, output: 1e6, cache_read: 1e6, cache_write: 1e6 };

test("prices a model at its input, output, cache read and 1-hour cache write rates", () => {
  assert.equal(cost("claude-opus-5-5", counts), 4 + 20 + 0.2 + 8);
});

test("matches the longest model prefix", () => {
  assert.equal(cost("claude-opus-5", counts), 5 + 25 + 0.5 + 10);
  assert.equal(cost("claude-opus-4-1", counts), 15 + 75 + 1.5 + 30);
  assert.equal(cost("claude-sonnet-4-6", counts), 3 + 15 + 0.3 + 6);
});

test("an unknown model costs nothing", () => {
  assert.equal(cost("some-other-model", counts), 0);
});

test("prices Tripo's credits by service", () => {
  assert.equal(creditCost("tripo-api", 50), 0.5);
  assert.equal(creditCost("tripo-studio", 3000), 19.9);
  assert.equal(creditCost("tripo-somewhere", 50), 0);
});
