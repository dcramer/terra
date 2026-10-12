// The embeddable ticker at `/ticker`: the running cost, then a line for each person
// with theirs and, after a gap, a line for each service it went on (Claude, Tripo), for
// an iframe or a stream overlay. At `/ticker/<user>` it counts one person's alone, with
// the tokens behind it. It fills whatever box it is given with a solid background, the
// card scaled to fit and keeping its shape. `?theme=light` or
// `?theme=dark` fixes the colours; without it they follow the embedding page.

import { compact, esc, money } from "./page.js";

// `user` picks one person's totals; null when they have reported nothing.
export function ticker(summary, theme, user) {
  const totals = user ? summary.users.find((u) => u.user === user) : summary.totals;
  if (!totals) return null;
  const { cost } = totals;
  const used = tokens(totals);
  // USER in index.js allows only letters, digits and `_.-`, so the name is safe in HTML.
  const whose = user ? `What ${user}'s work on Terra has cost` : "What building Terra has cost";
  const fixed = theme === "light" || theme === "dark" ? ` data-theme="${theme}"` : "";
  const body = user
    ? `<span class="text">
    <span class="cost" data-n="${cost}">${money(cost)}</span>
    <span class="label">${user} · <span class="tokens" data-n="${used}">${compact(used)}</span> tokens</span>
  </span>`
    : `<span class="cost" data-n="${cost}">${money(cost)}</span>
  <span class="lines">
    <span class="people">${people(summary).map(row).join("")}</span>
    <span class="sources">${sources(summary.totals).map(row).join("")}</span>
  </span>`;
  return `<!doctype html>
<html lang="en"${fixed} data-user="${user ?? ""}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Terra Cost Ticker</title>
<style>${CSS}</style>
</head>
<body>
<a class="ticker" href="/" target="_blank" rel="noopener" title="${whose} at list prices">
  ${SPARK}
  ${body}
</a>
<script>${SCRIPT}</script>
</body>
</html>`;
}

// Claude's spark, since Claude is what the ticker counts: tapered rays of uneven length.
const SPARK = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32" aria-hidden="true"><path d="M16.5 16L17.75 3.25A1.75 1.75 0 0 0 14.25 3.25L15.5 16ZM16.43 16.26L22.78 8.12A1.75 1.75 0 0 0 19.78 6.31L15.57 15.74ZM16.23 16.44L27.64 11.79A1.75 1.75 0 0 0 25.99 8.7L15.77 15.56ZM15.97 16.5L25.65 18.26A1.75 1.75 0 0 0 25.83 14.76L16.03 15.5ZM15.74 16.42L25.89 24.24A1.75 1.75 0 0 0 27.74 21.27L16.26 15.58ZM15.56 16.23L19.5 26.31A1.75 1.75 0 0 0 22.59 24.67L16.44 15.77ZM15.5 15.97L13.61 28.14A1.75 1.75 0 0 0 17.11 28.32L16.5 16.03ZM15.58 15.74L9.35 23.34A1.75 1.75 0 0 0 12.32 25.2L16.42 16.26ZM15.76 15.56L4 20.65A1.75 1.75 0 0 0 5.7 23.71L16.24 16.44ZM16 15.5L5.75 14.25A1.75 1.75 0 0 0 5.75 17.75L16 16.5ZM16.25 15.57L6.7 8.61A1.75 1.75 0 0 0 4.95 11.64L15.75 16.43ZM16.43 15.75L12.39 6.25A1.75 1.75 0 0 0 9.36 8L15.57 16.25Z"/></svg>`;

// The lines under the cost, as `{ key, kind, cost }`: `key` is the name shown and
// `kind` the line's class. People come in the summary's order, most spent first.
function people(s) {
  return s.users.map((u) => ({ key: u.user, kind: "person", cost: u.cost }));
}

// What the money went on, in a fixed order so the lines never swap. Tripo shows once
// it has been used, as on the page.
function sources(t) {
  const out = [{ key: "Claude", kind: "source claude", cost: t.claude_cost }];
  if (t.tripo_cost > 0 || t.tripo_credits > 0) out.push({ key: "Tripo", kind: "source tripo", cost: t.tripo_cost });
  return out;
}

function row(l) {
  return `<span class="${l.kind}" data-key="${esc(l.key)}"><span class="name">${esc(l.key)}</span><span class="amount" data-n="${l.cost}">${money(l.cost)}</span></span>`;
}

// Tokens written and read fresh, leaving out cache reads as the page does: the same
// context read again each turn, most of the count and little of the cost.
function tokens(t) {
  return t.input + t.cache_write + t.output;
}

const CSS = `
:root { --surface: #ffffff; --ink: #111111; --muted: #6e6e6e; --glow: #b0b0b0;
  --claude: #1a8c63; --tripo: #6f5fc4; }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --surface: #111111; --ink: #f2f2f2; --muted: #9a9a9a; --glow: #5a5a5a;
    --claude: #2fa577; --tripo: #9a8ce6; }
}
:root[data-theme="dark"] { --surface: #111111; --ink: #f2f2f2; --muted: #9a9a9a; --glow: #5a5a5a;
  --claude: #2fa577; --tripo: #9a8ce6; }
* { box-sizing: border-box; }
/* The page is the ticker: its colour fills the whole box, edge to edge, with no border. */
html, body { margin: 0; height: 100%; overflow: hidden; background: var(--surface); }
body { display: flex; align-items: center; justify-content: center;
  font: 42px/1.2 ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif; }
/* The icon sits beside the first line; everyone's lines run under the cost. */
.ticker { flex: none; display: inline-grid; grid-template-columns: auto auto; align-items: center;
  column-gap: 27px; padding: 21px 36px 21px 27px;
  color: var(--ink); text-decoration: none; }
.ticker svg { width: 78px; height: 78px; flex: none; }
.ticker svg path { fill: var(--ink); }
.text { display: flex; flex-direction: column; }
.cost, .tokens, .amount { display: inline-block; font-variant-numeric: tabular-nums; }
.cost { font-size: 54px; font-weight: 600; }
.label { font-size: 33px; color: var(--muted); white-space: nowrap; font-variant-numeric: tabular-nums; }
/* People and services share one grid, so every amount lines up; half a line between
   the two groups keeps the card short in a corner of the screen. */
.lines { grid-column: 2; display: grid; grid-template-columns: auto auto; column-gap: 33px;
  margin-top: 6px; font-size: 33px; white-space: nowrap; }
.people, .sources, .person, .source { display: contents; }
.people:not(:empty) + .sources > :first-child > * { margin-top: 18px; }
.name { color: var(--muted); }
/* A swatch in the page's colours marks a service, hanging in the gutter so its name
   lines up with the people's. */
.source .name { position: relative; }
.source .name::before { content: ""; position: absolute; left: -24px; top: 50%; width: 12px; height: 12px;
  margin-top: -6px; border-radius: 3px; }
.claude .name::before { background: var(--claude); }
.tripo .name::before { background: var(--tripo); }
.amount { text-align: right; }
.counting { color: var(--muted); }
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
${people}
${sources}
const user = document.documentElement.dataset.user;
const cost = document.querySelector(".cost"), used = document.querySelector(".tokens");
const lines = document.querySelector(".lines");
const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
const ease = (t) => (t === 1 ? 1 : 1 - Math.pow(2, -10 * t));
const put = (el, n) => { el.textContent = el === used ? compact(Math.round(n)) : money(n); };

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

// The lines in the box, in the order given. When one is added or the order changes the
// lines are made again, each starting from what it showed.
function rows(box, items, ms) {
  const shown = [...box.children];
  if (shown.map((r) => r.dataset.key).join("\\n") !== items.map((l) => l.key).join("\\n")) {
    const was = new Map(shown.map((r) => [r.dataset.key, +r.lastChild.dataset.n]));
    box.replaceChildren(...items.map((l) => {
      const r = document.createElement("span"), name = document.createElement("span"), amount = document.createElement("span");
      r.className = l.kind;
      r.dataset.key = name.textContent = l.key;
      name.className = "name";
      amount.className = "amount";
      amount.dataset.n = was.get(l.key) ?? 0;
      r.append(name, amount);
      return r;
    }));
  }
  items.forEach((l, i) => {
    const amount = box.children[i].lastChild;
    roll(amount, +amount.dataset.n, l.cost, ms);
  });
}

// Show the numbers as they are on load; only later changes roll.
hold(cost, +cost.dataset.n);
if (used) hold(used, +used.dataset.n);
if (lines) for (const amount of lines.querySelectorAll(".amount")) hold(amount, +amount.dataset.n);

let busy = false;
async function refresh() {
  if (busy || document.hidden) return;
  busy = true;
  try {
    const r = await fetch("/json", { cache: "no-store" });
    if (!r.ok) return;
    const summary = await r.json();
    const totals = user ? summary.users.find((u) => u.user === user) : summary.totals;
    if (!totals) return;
    roll(cost, +cost.dataset.n, totals.cost, 1500);
    if (used) roll(used, +used.dataset.n, tokens(totals), 1500);
    if (lines) {
      rows(lines.querySelector(".people"), people(summary), 1500);
      rows(lines.querySelector(".sources"), sources(summary.totals), 1500);
    }
  } catch {} finally { busy = false; }
}
// An embed can sit on many pages at once, so it rests while its page is hidden.
setInterval(refresh, 5000);
document.addEventListener("visibilitychange", refresh);
`;
