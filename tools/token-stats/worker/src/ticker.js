// The embeddable ticker at `/ticker`: the running cost, then a line for each person
// with theirs, for an iframe or a stream overlay. At `/ticker/<user>` it counts one
// person's alone, with the tokens behind it. It scales to fill whatever box it is
// given, keeping its shape, over a transparent background. `?theme=light` or
// `?theme=dark` fixes the colours; without it they follow the embedding page.

import { ICON, compact, esc, money } from "./page.js";

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
  <span class="people">${summary.users.map(row).join("")}</span>`;
  return `<!doctype html>
<html lang="en"${fixed} data-user="${user ?? ""}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Terra Cost Ticker</title>
<style>${CSS}</style>
</head>
<body>
<a class="ticker" href="/" target="_blank" rel="noopener" title="${whose} at API list prices">
  ${ICON}
  ${body}
</a>
<script>${SCRIPT}</script>
</body>
</html>`;
}

function row(u) {
  return `<span class="person" data-user="${esc(u.user)}"><span class="name">${esc(u.user)}</span><span class="amount" data-n="${u.cost}">${money(u.cost)}</span></span>`;
}

// Tokens written and read fresh, leaving out cache reads as the page does: the same
// context read again each turn, most of the count and little of the cost.
function tokens(t) {
  return t.input + t.cache_write + t.output;
}

const CSS = `
:root { --surface: #ffffff; --ink: #111111; --muted: #6e6e6e; --glow: #b0b0b0; }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --surface: #111111; --ink: #f2f2f2; --muted: #9a9a9a; --glow: #5a5a5a; }
}
:root[data-theme="dark"] { --surface: #111111; --ink: #f2f2f2; --muted: #9a9a9a; --glow: #5a5a5a; }
* { box-sizing: border-box; }
/* No color-scheme: one that differs from the embedding page's, or a dark one in a
   stream overlay, makes the browser paint an opaque backdrop behind the page. */
html, body { margin: 0; height: 100%; overflow: hidden; background: transparent; }
body { display: flex; align-items: center; justify-content: center;
  font: 42px/1.2 ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif; }
/* The icon sits beside the first line; everyone's lines run under the cost. */
.ticker { flex: none; display: inline-grid; grid-template-columns: auto auto; align-items: center;
  column-gap: 27px; padding: 21px 36px 21px 27px;
  background: var(--surface); color: var(--ink); text-decoration: none; }
.ticker svg { width: 78px; height: 78px; flex: none; }
.ticker svg circle { fill: var(--muted); }
.ticker svg path { fill: var(--ink); }
.text { display: flex; flex-direction: column; }
.cost, .tokens, .amount { display: inline-block; font-variant-numeric: tabular-nums; }
.cost { font-size: 54px; font-weight: 600; }
.label { font-size: 33px; color: var(--muted); white-space: nowrap; font-variant-numeric: tabular-nums; }
.people { grid-column: 2; display: grid; grid-template-columns: auto auto; column-gap: 33px;
  margin-top: 6px; font-size: 33px; white-space: nowrap; }
.people:empty { display: none; }
.person { display: contents; }
.name { color: var(--muted); }
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
const user = document.documentElement.dataset.user;
const cost = document.querySelector(".cost"), used = document.querySelector(".tokens");
const people = document.querySelector(".people");
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

// One line for each person, in the order the summary gives. When someone new reports
// or the order changes the lines are made again, each starting from what it showed.
function rows(users, ms) {
  const shown = [...people.children];
  if (shown.map((r) => r.dataset.user).join("\\n") !== users.map((u) => u.user).join("\\n")) {
    const was = new Map(shown.map((r) => [r.dataset.user, +r.lastChild.dataset.n]));
    people.replaceChildren(...users.map((u) => {
      const r = document.createElement("span"), name = document.createElement("span"), amount = document.createElement("span");
      r.className = "person";
      r.dataset.user = name.textContent = u.user;
      name.className = "name";
      amount.className = "amount";
      amount.dataset.n = was.get(u.user) ?? 0;
      r.append(name, amount);
      return r;
    }));
  }
  users.forEach((u, i) => {
    const amount = people.children[i].lastChild;
    roll(amount, +amount.dataset.n, u.cost, ms);
  });
}

// Count up from zero on load.
roll(cost, 0, +cost.dataset.n, 1600);
if (used) roll(used, 0, +used.dataset.n, 1600);
if (people) for (const amount of people.querySelectorAll(".amount")) roll(amount, 0, +amount.dataset.n, 1600);

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
    if (people) rows(summary.users, 1500);
  } catch {} finally { busy = false; }
}
// An embed can sit on many pages at once, so it rests while its page is hidden.
setInterval(refresh, 5000);
document.addEventListener("visibilitychange", refresh);
`;
