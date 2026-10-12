// The embeddable ticker at `/ticker`: the running cost, then a line for each person
// with theirs and, after a gap, a line for each service it went on (Claude, Tripo), each
// with its picture, for an iframe or a stream overlay. At `/ticker/<user>` it counts one
// person's alone, with the tokens behind it. It fills whatever box it is given with a
// solid background, the card scaled to fit and keeping its shape. `?theme=light` or
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
    ? `${icon({ key: user, x: user, kind: "person" })}
  <span class="text">
    <span class="cost" data-n="${cost}">${money(cost)}</span>
    <span class="label">${user} · <span class="tokens" data-n="${used}">${compact(used)}</span> tokens</span>
  </span>`
    : `<span class="cost" data-n="${cost}">${money(cost)}</span>
  <span class="people">${people(summary).map(row).join("")}</span>
  <span class="sources">${sources(summary.totals).map(row).join("")}</span>`;
  return `<!doctype html>
<html lang="en"${fixed} data-user="${user ?? ""}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Terra Cost Ticker</title>
<style>${CSS}</style>
</head>
<body>
<a class="ticker${user ? " one" : ""}" href="/" target="_blank" rel="noopener" title="${whose} at list prices">
  ${body}
</a>
<script>${SCRIPT}</script>
</body>
</html>`;
}

// The lines under the cost, as `{ key, x, kind, cost }`: `key` is the name shown, `x`
// the X account whose picture goes beside it, and `kind` the line's class. People
// report under their X handle, and come in the summary's order, most spent first.
function people(s) {
  return s.users.map((u) => ({ key: u.user, x: u.user, kind: "person", cost: u.cost }));
}

// What the money went on, in a fixed order so the lines never swap. Tripo shows once
// it has been used, as on the page.
function sources(t) {
  const out = [{ key: "Claude", x: "claudeai", kind: "source", cost: t.claude_cost }];
  if (t.tripo_cost > 0 || t.tripo_credits > 0) out.push({ key: "Tripo", x: "tripoai", kind: "source", cost: t.tripo_cost });
  return out;
}

// An X account's picture, through unavatar.io, which the browser caches for weeks. It
// is the name's first letter until the picture loads, and if it never does.
function avatar(x) {
  return "https://unavatar.io/x/" + encodeURIComponent(x) + "?fallback=false";
}

function icon(l) {
  return `<span class="icon" aria-hidden="true">${esc(l.key[0].toUpperCase())}<img src="${avatar(l.x)}" alt="" onerror="this.remove()"></span>`;
}

function row(l) {
  return `<span class="${l.kind}" data-key="${esc(l.key)}">${icon(l)}<span class="name">${esc(l.key)}</span><span class="amount" data-n="${l.cost}">${money(l.cost)}</span></span>`;
}

// Tokens written and read fresh, leaving out cache reads as the page does: the same
// context read again each turn, most of the count and little of the cost.
function tokens(t) {
  return t.input + t.cache_write + t.output;
}

const CSS = `
:root { --surface: #ffffff; --ink: #111111; --muted: #6e6e6e; --glow: #b0b0b0;
  --tile: #ececec; --ring: rgba(0, 0, 0, 0.08); }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --surface: #111111; --ink: #f2f2f2; --muted: #9a9a9a; --glow: #5a5a5a;
    --tile: #2a2a2a; --ring: rgba(255, 255, 255, 0.12); }
}
:root[data-theme="dark"] { --surface: #111111; --ink: #f2f2f2; --muted: #9a9a9a; --glow: #5a5a5a;
  --tile: #2a2a2a; --ring: rgba(255, 255, 255, 0.12); }
* { box-sizing: border-box; }
/* The page is the ticker: its colour fills the whole box, edge to edge, with no border. */
html, body { margin: 0; height: 100%; overflow: hidden; background: var(--surface); }
body { display: flex; align-items: center; justify-content: center;
  font: 33px/1.2 ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif; }
/* The cost on top, then one grid of lines, so every picture, name and amount lines up;
   half a line between the people and the services keeps the card short in a corner. */
.ticker { flex: none; display: inline-grid; grid-template-columns: auto auto auto; align-items: center;
  row-gap: 8px; padding: 24px 32px; color: var(--ink); text-decoration: none; white-space: nowrap; }
.cost, .tokens, .amount { display: inline-block; font-variant-numeric: tabular-nums; }
.cost { grid-column: 1 / -1; margin-bottom: 6px; font-size: 60px; line-height: 1.1; font-weight: 650;
  letter-spacing: -0.02em; }
.people, .sources, .person, .source { display: contents; }
.people:not(:empty) + .sources > :first-child > * { margin-top: 16px; }
.name { margin-left: 14px; color: var(--muted); }
.amount { margin-left: 36px; text-align: right; }
/* People are round, services are app tiles. */
.icon { position: relative; display: grid; place-items: center; width: 40px; height: 40px; overflow: hidden;
  border-radius: 50%; background: var(--tile); color: var(--muted); font-size: 20px; font-weight: 600; }
.source .icon { border-radius: 10px; }
.icon img { position: absolute; inset: 0; width: 100%; height: 100%; object-fit: cover; }
.icon::after { content: ""; position: absolute; inset: 0; border-radius: inherit; box-shadow: inset 0 0 0 1px var(--ring); }
/* One person's: their picture beside their cost and tokens. */
.one { grid-template-columns: auto auto; column-gap: 24px; }
.one .icon { width: 96px; height: 96px; font-size: 44px; }
.text { display: flex; flex-direction: column; }
.label { color: var(--muted); font-variant-numeric: tabular-nums; }
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
${avatar}
const user = document.documentElement.dataset.user;
const cost = document.querySelector(".cost"), used = document.querySelector(".tokens");
const listed = document.querySelector(".people"), services = document.querySelector(".sources");
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
      const r = document.createElement("span"), pic = document.createElement("span"), img = document.createElement("img");
      const name = document.createElement("span"), amount = document.createElement("span");
      r.className = l.kind;
      r.dataset.key = name.textContent = l.key;
      pic.className = "icon";
      pic.setAttribute("aria-hidden", "true");
      pic.textContent = l.key[0].toUpperCase();
      img.alt = "";
      img.onerror = () => img.remove();
      img.src = avatar(l.x);
      pic.append(img);
      name.className = "name";
      amount.className = "amount";
      amount.dataset.n = was.get(l.key) ?? 0;
      r.append(pic, name, amount);
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
for (const amount of document.querySelectorAll(".amount")) hold(amount, +amount.dataset.n);

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
    if (listed) {
      rows(listed, people(summary), 1500);
      rows(services, sources(summary.totals), 1500);
    }
  } catch {} finally { busy = false; }
}
// An embed can sit on many pages at once, so it rests while its page is hidden.
setInterval(refresh, 5000);
document.addEventListener("visibilitychange", refresh);
`;
