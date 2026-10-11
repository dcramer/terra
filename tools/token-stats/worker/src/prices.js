// What tokens would cost at Anthropic's API list prices, in dollars per million tokens,
// from https://platform.claude.com/docs/en/about-claude/pricing (2026-10-10).
//
// Most of us pay by subscription, not by token, so this is what the work would cost
// on the API, not what anyone paid. Two approximations: cache writes are priced at
// the 1-hour rate, which is the only kind Claude Code writes; and Haiku 5.5 is priced
// at its rate for prompts up to 100,000 tokens, since totals do not keep each prompt's length.

const PRICES = [
  // model id prefix, input, cache write (1h), cache read, output
  ["claude-fable-5-1", 10, 20, 0.25, 50],
  ["claude-mythos-5-1", 10, 20, 0.25, 50],
  ["claude-fable-5", 10, 20, 1, 50],
  ["claude-mythos-5", 10, 20, 1, 50],
  ["claude-opus-5-5", 4, 8, 0.2, 20],
  ["claude-opus-5", 5, 10, 0.5, 25],
  ["claude-opus-4-8", 5, 10, 0.5, 25],
  ["claude-opus-4-7", 5, 10, 0.5, 25],
  ["claude-opus-4-6", 5, 10, 0.5, 25],
  ["claude-opus-4-5", 5, 10, 0.5, 25],
  ["claude-opus-4", 15, 30, 1.5, 75],
  ["claude-sonnet-5-5", 2, 4, 0.1, 10],
  ["claude-sonnet-5", 2, 4, 0.2, 10],
  ["claude-sonnet-4", 3, 6, 0.3, 15],
  ["claude-haiku-5-5", 0.1, 0.2, 0.01, 0.5],
  ["claude-haiku-4-5", 1, 2, 0.1, 5],
  ["claude-3-5-haiku", 0.8, 1.6, 0.08, 4],
];

// The price row for a model id, matching the longest prefix, or null if unknown.
function priceOf(model) {
  let best = null;
  for (const row of PRICES) if (model.startsWith(row[0]) && (!best || row[0].length > best[0].length)) best = row;
  return best;
}

// Dollars for one row of counts of a model; 0 for a model with no known price.
export function cost(model, t) {
  const p = priceOf(model);
  if (!p) return 0;
  const [, input, write, read, output] = p;
  return (t.input * input + t.cache_write * write + t.cache_read * read + t.output * output) / 1e6;
}
