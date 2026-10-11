// The embeddable ticker at `/ticker`: the running cost and the tokens behind it, for
// an iframe or a stream overlay. It scales to fill whatever box it is given, keeping
// its shape, over a transparent background. `?theme=light` or `?theme=dark` fixes the colours; without
// it they follow the embedding page.

import { ICON, compact, money } from "./page.js";

export function ticker(summary, theme) {
  const { cost } = summary.totals;
  const used = tokens(summary.totals);
  const fixed = theme === "light" || theme === "dark" ? ` data-theme="${theme}"` : "";
  return `<!doctype html>
<html lang="en"${fixed}>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Terra Cost Ticker</title>
<style>${CSS}</style>
</head>
<body>
<a class="ticker" href="/" target="_blank" rel="noopener" title="What building Terra has cost at API list prices">
  ${ICON}
  <span class="text">
    <span class="cost" data-n="${cost}">${money(cost)}</span>
    <span class="label"><span class="tokens" data-n="${used}">${compact(used)}</span> tokens</span>
  </span>
</a>
<script>${SCRIPT}</script>
</body>
</html>`;
}

// Tokens written and read fresh, leaving out cache reads as the page does: the same
// context read again each turn, most of the count and little of the cost.
function tokens(t) {
  return t.input + t.cache_write + t.output;
}

const CSS = `
:root {
  --surface: #fbf7f2; --ink: #2b2540; --muted: #6b6480; --line: #e6dcd3;
  --count: #c0674c; --glow: #ffd9b8;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    --surface: #24223a; --ink: #f1eaf5; --muted: #a9a2be; --line: #34314f;
    --count: #f0a98c; --glow: #c47a6a;
  }
}
:root[data-theme="dark"] {
  --surface: #24223a; --ink: #f1eaf5; --muted: #a9a2be; --line: #34314f;
  --count: #f0a98c; --glow: #c47a6a;
}
* { box-sizing: border-box; }
/* No color-scheme: one that differs from the embedding page's, or a dark one in a
   stream overlay, makes the browser paint an opaque backdrop behind the page. */
html, body { margin: 0; height: 100%; overflow: hidden; background: transparent; }
body { display: flex; align-items: center; justify-content: center;
  font: 42px/1.2 ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif; }
.ticker { flex: none; display: inline-flex; align-items: center; gap: 27px; padding: 21px 36px 21px 27px;
  background: var(--surface); color: var(--ink); border: 2px solid var(--line); border-radius: 36px;
  text-decoration: none; }
.ticker:hover { border-color: var(--muted); }
.ticker svg { width: 78px; height: 78px; flex: none; }
.text { display: flex; flex-direction: column; }
.cost, .tokens { display: inline-block; }
.cost { font-size: 54px; font-weight: 600; font-variant-numeric: tabular-nums; }
.label { font-size: 33px; color: var(--muted); white-space: nowrap; font-variant-numeric: tabular-nums; }
.counting { color: var(--count); }
.landed { animation: land 0.6s cubic-bezier(0.2, 0.8, 0.2, 1); }
@keyframes land { 30% { text-shadow: 0 0 36px var(--glow); } }
@media (prefers-reduced-motion: reduce) {
  .landed { animation: none; }
}
`;

// Checks /json every few seconds while the embedding page is visible, and rolls
// each number up to any new value.
const SCRIPT = `
${money}
${compact}
${tokens}
const cost = document.querySelector(".cost"), used = document.querySelector(".tokens");
const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
const ease = (t) => (t === 1 ? 1 : 1 - Math.pow(2, -10 * t));
const put = (el, n) => { el.textContent = el === cost ? money(n) : compact(Math.round(n)); };

// Scale the card to fill the page, keeping its shape. Its layout size is fixed while a
// number rolls, so it holds still and only refits when a number gains a digit.
const card = document.querySelector(".ticker");
function fit() {
  const s = Math.min((innerWidth - 4) / card.offsetWidth, (innerHeight - 4) / card.offsetHeight);
  card.style.transform = "scale(" + s + ")";
}
function hold(el, to) {
  el.style.minWidth = "";
  put(el, to);
  el.style.minWidth = el.offsetWidth + "px";
  fit();
}
addEventListener("resize", fit);

function roll(el, from, to, ms) {
  el.dataset.n = to;
  hold(el, to);
  const me = (el.rolling = {});
  if (still || from === to || document.hidden) { put(el, to); return; }
  const start = performance.now();
  el.classList.remove("landed");
  el.classList.add("counting");
  const step = (now) => {
    if (el.rolling !== me) return;
    const t = Math.min(1, (now - start) / ms);
    put(el, from + (to - from) * ease(t));
    if (t < 1) return requestAnimationFrame(step);
    el.classList.remove("counting");
    void el.offsetWidth;
    el.classList.add("landed");
  };
  requestAnimationFrame(step);
}

// Count up from zero on load.
roll(cost, 0, +cost.dataset.n, 1600);
roll(used, 0, +used.dataset.n, 1600);

let busy = false;
async function refresh() {
  if (busy || document.hidden) return;
  busy = true;
  try {
    const r = await fetch("/json", { cache: "no-store" });
    if (!r.ok) return;
    const { totals } = await r.json();
    roll(cost, +cost.dataset.n, totals.cost, 1500);
    roll(used, +used.dataset.n, tokens(totals), 1500);
  } catch {} finally { busy = false; }
}
// An embed can sit on many pages at once, so it rests while its page is hidden.
setInterval(refresh, 5000);
document.addEventListener("visibilitychange", refresh);
`;
