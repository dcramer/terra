// The public page at `/`: what building Terra has cost per person at list prices, in the look of docs/art.md:
// flat colour, distance as pastel layers, warm light and cool shadow.

export function page(summary) {
  const { totals, users, days } = summary;
  const peak = Math.max(0.01, ...users.flatMap((u) => u.daily.map((d) => d.cost)));
  const cards = users.map((u) => card(u, peak, days)).join("");
  const models = merge(users.flatMap((u) => u.models));
  const empty = users.length === 0 ? `<p class="empty">Nothing reported yet.</p>` : "";

  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Terra Token Stats</title>
<meta name="description" content="What the Claude tokens and Tripo credits spent building Terra cost at list prices, per person.">
<link rel="icon" href="data:image/svg+xml,${encodeURIComponent(ICON)}">
<style>${CSS}</style>
</head>
<body>
<header>
  ${SCENE}
  <div class="title">
    <h1>Terra</h1>
    <p>What building it has cost</p>
  </div>
</header>
<main data-shape="${esc(shape(summary))}">
  <section class="hero" aria-label="Everyone">
    <div class="big"${num(totals.cost, "cost", "usd")}>${money(totals.cost)}</div>
    <div class="label">at list prices, across ${users.length} ${users.length === 1 ? "person" : "people"}</div>
    ${ledger(totals, models)}
  </section>
  ${empty}
  <section class="people" aria-label="Per person">${cards}</section>
</main>
<footer>
  Counted from Claude Code transcripts of sessions in the repository, each message once, and from the Tripo credits our scripts record, each task once.
  Updated <span id="updated">${esc(summary.generated_at.slice(0, 16).replace("T", " "))}</span> UTC. <a href="/json">JSON</a>
</footer>
<div id="tip" role="tooltip" hidden></div>
<script>${SCRIPT}</script>
</body>
</html>`;
}

function card(u, peak, days) {
  const bars = u.daily.map((d) => `<div class="day"${tip(d)}>${stack(d, peak)}</div>`).join("");
  return `<article class="person" data-user="${esc(u.user)}">
  <div class="head">
    <h2>${esc(u.user)}</h2>
    <div class="total"${num(u.cost, "cost", "usd")}>${money(u.cost)}</div>
  </div>
  ${ledger(u, u.models)}
  <figure>
    <div class="chart" role="img" aria-label="Cost per day for ${esc(u.user)} over the last ${days} days">${bars}</div>
    <figcaption><span>${u.daily[0].day.slice(5)}</span><span>cost a day, last ${days} days</span><span>${u.daily.at(-1).day.slice(5)}</span></figcaption>
  </figure>
</article>`;
}

// A day's bar: Claude's cost with Tripo's stacked on it, as a share of the busiest day.
// The top segment gets the rounded corners.
export function stack(d, peak) {
  const h = (c) => (c <= 0 ? 0 : Math.max(2, (c / peak) * 100)).toFixed(1);
  const top = d.tripo_cost > 0 ? "" : " top";
  return `<i class="claude${top}" style="height:${h(d.claude_cost)}%"></i><i class="tripo top" style="height:${h(d.tripo_cost)}%"></i>`;
}

// What the tooltip over a day shows.
function tip(d) {
  return ` data-day="${d.day}" data-cost="${money(d.cost)}" data-claude="${money(d.claude_cost)}" data-tripo="${money(d.tripo_cost)}" data-credits="${full(d.tripo_credits)}" data-output="${full(d.output)}" data-messages="${d.messages}"`;
}

// A total broken down by service, one line each: what it cost, what it was spent in,
// and the models or plans behind it. Claude's tokens leave out cache reads: the same
// context read again each turn, which is most of the count and little of the cost.
// A service nobody used is left out.
export function ledger(t, models) {
  const claude = models.filter((m) => m.credits === undefined);
  const tripo = models.filter((m) => m.credits !== undefined);
  const n = (key, label) => `<span><b${num(value(t, key), key)}>${compact(value(t, key))}</b> ${label}</span>`;
  const rows = [
    line("claude", "Claude", t.claude_cost, claude, [n("output", "output tokens"), n("new_input", "input"), n("messages", "messages")]),
  ];
  if (hasTripo(t)) rows.push(line("tripo", "Tripo", t.tripo_cost, tripo, [n("tripo_credits", "credits")]));
  return `<div class="ledger">${rows.join("")}</div>`;
}

function line(cls, name, cost, models, facts) {
  const via = models.map((m) => `<span title="${spent(m)}">${esc(m.model.replace(/^(claude|tripo)-/, ""))}</span>`).join(", ");
  return `<div class="line ${cls}">
    <div class="name">${name}${via ? ` <small>${via}</small>` : ""}</div>
    <div class="cost"><b${num(cost, `${cls}_cost`, "usd")}>${money(cost)}</b></div>
    <div class="detail">${facts.join(" · ")}</div>
  </div>`;
}

// What a model's tokens or a service's credits cost.
function spent(m) {
  return m.credits === undefined ? `${money(m.cost)} at API prices` : `${full(m.credits)} credits, ${money(m.cost)}`;
}

// Everyone's models as one list, adding up what each cost.
function merge(models) {
  const by = new Map();
  for (const m of models) {
    const was = by.get(m.model);
    by.set(m.model, was ? { ...was, cost: was.cost + m.cost, credits: m.credits === undefined ? undefined : was.credits + m.credits } : m);
  }
  return [...by.values()].sort((a, b) => b.cost - a.cost);
}

function hasTripo(t) {
  return t.tripo_cost > 0 || t.tripo_credits > 0;
}

// What decides the page's layout: who is on it, who has a Tripo line, and the first
// day charted. When a check of /json finds this changed, the page is drawn again.
function shape(s) {
  const people = s.users.map((u) => `${u.user}${hasTripo(u) ? "+" : ""}`).sort();
  return [hasTripo(s.totals) ? "+" : "", s.users[0]?.daily[0]?.day ?? "", ...people].join(" ");
}

// A field of a /json row; `new_input` is input not read from the cache.
function value(t, key) {
  return key === "new_input" ? t.input + t.cache_write : t[key];
}

// A counter the script animates: its exact value, which field of /json it shows,
// and whether it is dollars.
function num(n, key, fmt) {
  const title = fmt === "usd" ? "at list prices" : full(n);
  return ` data-n="${n}" data-key="${key}"${fmt ? ` data-fmt="${fmt}"` : ""} title="${title}"`;
}

export function compact(n) {
  if (n < 1000) return String(n);
  const units = [
    [1e12, "T"],
    [1e9, "B"],
    [1e6, "M"],
    [1e3, "K"],
  ];
  const [size, unit] = units.find(([s]) => n >= s);
  const v = n / size;
  return `${v >= 100 ? Math.round(v) : v.toFixed(1).replace(/\.0$/, "")}${unit}`;
}

export function money(n) {
  return "$" + n.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function full(n) {
  return n.toLocaleString("en-US");
}

export function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

// Ridges as layers of many small facets, fading toward the sky with distance.
function ridge(seed, base, amp, step, cls) {
  let x = -20;
  let s = seed;
  const rand = () => {
    s = (s * 16807) % 2147483647;
    return s / 2147483647;
  };
  const pts = [];
  while (x <= 1220) {
    const y = base - amp * (0.5 + 0.5 * Math.sin(x / 170 + seed)) - amp * 0.35 * rand();
    pts.push(`${x.toFixed(0)},${y.toFixed(1)}`);
    x += step * (0.6 + 0.8 * rand());
  }
  return `<polygon class="${cls}" points="-20,330 ${pts.join(" ")} 1220,330"/>`;
}

function pines(seed, count, base) {
  let s = seed;
  const rand = () => {
    s = (s * 16807) % 2147483647;
    return s / 2147483647;
  };
  let out = "";
  for (let i = 0; i < count; i++) {
    const x = 40 + rand() * 1120;
    const h = 18 + rand() * 26;
    const w = h * 0.42;
    const y = base + rand() * 26;
    out += `<polygon class="pine" points="${x - w / 2},${y} ${x},${y - h} ${x},${y}"/><polygon class="pine-lit" points="${x},${y - h} ${x + w / 2},${y} ${x},${y}"/>`;
  }
  return out;
}

const SCENE = `<svg class="scene" viewBox="0 0 1200 320" preserveAspectRatio="xMidYMax slice" aria-hidden="true">
  <defs>
    <linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" class="sky-top"/><stop offset="1" class="sky-low"/>
    </linearGradient>
    <radialGradient id="glow"><stop offset="0" class="glow"/><stop offset="1" class="glow" stop-opacity="0"/></radialGradient>
  </defs>
  <rect width="1200" height="320" fill="url(#sky)"/>
  <circle cx="860" cy="120" r="140" fill="url(#glow)"/>
  <circle class="sun" cx="860" cy="120" r="34"/>
  ${ridge(7, 200, 90, 60, "l1")}
  ${ridge(13, 235, 70, 46, "l2")}
  ${ridge(29, 270, 50, 38, "l3")}
  ${ridge(41, 300, 34, 30, "l4")}
  ${pines(5, 26, 282)}
  ${ridge(53, 334, 22, 26, "l5")}
</svg>`;

export const ICON = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 32 32"><circle cx="16" cy="16" r="15" fill="#e9a38a"/><path d="M1 18 L8 13 L13 16 L19 10 L25 14 L31 12 L31 20 A15 15 0 0 1 1 20 Z" fill="#1a8c63"/></svg>`;

const CSS = `
:root {
  --bg: #f6efe8; --surface: #fbf7f2; --ink: #2b2540; --muted: #6b6480; --line: #e6dcd3;
  --bar: #1a8c63; --bar-hover: #11704e; --tripo: #6f5fc4; --tripo-hover: #574aa6;
  --sky-top: #f3d9c9; --sky-low: #efe3ea; --sun: #fff4e0; --glow: #ffd9b8;
  --l1: #d9cbe6; --l2: #bcb6de; --l3: #9fb0da; --l4: #e3a78f; --l5: #d98d74;
  --pine: #3f6f63; --pine-lit: #5e8f72; --count: #c0674c;
  color-scheme: light;
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #1b1a2b; --surface: #24223a; --ink: #f1eaf5; --muted: #a9a2be; --line: #34314f;
    --bar: #2fa577; --bar-hover: #4cc493; --tripo: #9a8ce6; --tripo-hover: #b6abf0;
    --sky-top: #1d1c33; --sky-low: #4a3a5e; --sun: #ffe3c4; --glow: #c47a6a;
    --l1: #3b3658; --l2: #332f52; --l3: #2b2a49; --l4: #4b3550; --l5: #3a2a42;
    --pine: #1e3b3a; --pine-lit: #2c5049; --count: #f0a98c;
    color-scheme: dark;
  }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--ink);
  font: 16px/1.45 ui-sans-serif, system-ui, -apple-system, "Segoe UI", sans-serif; }
header { position: relative; height: clamp(180px, 32vw, 300px); overflow: hidden; }
.scene { position: absolute; inset: 0; width: 100%; height: 100%; display: block; }
.sky-top { stop-color: var(--sky-top); } .sky-low { stop-color: var(--sky-low); }
.glow { stop-color: var(--glow); } .sun { fill: var(--sun); }
.l1 { fill: var(--l1); } .l2 { fill: var(--l2); } .l3 { fill: var(--l3); }
.l4 { fill: var(--l4); } .l5 { fill: var(--l5); }
.pine { fill: var(--pine); } .pine-lit { fill: var(--pine-lit); }
.title { position: absolute; left: 0; right: 0; top: clamp(20px, 6vw, 56px); max-width: 960px;
  margin: 0 auto; padding: 0 16px; }
h1 { margin: 0; font-size: clamp(36px, 7vw, 64px); font-weight: 600; letter-spacing: 0.02em; }
.title p { margin: 0; color: var(--muted); font-size: clamp(14px, 2vw, 18px); }
main { max-width: 960px; margin: 0 auto; padding: 24px 16px 8px; }
.hero { text-align: center; padding: 8px 0 24px; }
.big { font-size: clamp(48px, 10vw, 84px); font-weight: 600; line-height: 1;
  font-variant-numeric: tabular-nums; }
.label { color: var(--muted); margin-top: 6px; }
.people { display: grid; grid-template-columns: repeat(auto-fill, minmax(min(100%, 420px), 1fr)); gap: 16px; }
.person { display: flex; flex-direction: column; background: var(--surface); border: 1px solid var(--line);
  border-radius: 14px; padding: 18px 20px 16px; }
.head { display: flex; justify-content: space-between; align-items: baseline; gap: 12px; padding-bottom: 6px; border-bottom: 1px solid var(--line); }
h2 { margin: 0; font-size: 20px; font-weight: 600; overflow-wrap: anywhere; }
.total { font-size: 28px; font-weight: 600; font-variant-numeric: tabular-nums; }
figure { margin: auto 0 0; padding-top: 16px; }
.chart { display: flex; align-items: flex-end; gap: 2px; height: 84px; border-bottom: 1px solid var(--line); }
.day { flex: 1; height: 100%; display: flex; align-items: flex-end; cursor: default; }
.day { flex-direction: column-reverse; align-items: stretch; justify-content: flex-start; }
.day i { display: block; width: 100%; background: var(--bar); }
.day i.top { border-radius: 4px 4px 0 0; }
.day i.tripo { background: var(--tripo); }
.day:hover i.claude, .day.on i.claude { background: var(--bar-hover); }
.day:hover i.tripo, .day.on i.tripo { background: var(--tripo-hover); }
.ledger { margin: 0; }
.hero .ledger { max-width: 480px; margin: 28px auto 0; text-align: left; }
.line { display: grid; grid-template-columns: 1fr auto; column-gap: 12px; padding: 10px 0; }
.line + .line { border-top: 1px solid var(--line); }
.name { font-weight: 600; }
.name::before { content: ""; display: inline-block; width: 10px; height: 10px; border-radius: 3px; margin-right: 8px; }
.claude .name::before { background: var(--bar); }
.tripo .name::before { background: var(--tripo); }
.name small { color: var(--muted); font-size: 12px; font-weight: 400; margin-left: 4px; }
.line .cost { font-weight: 600; text-align: right; }
.detail { grid-column: 1 / -1; padding-left: 18px; color: var(--muted); font-size: 13px; }
.detail b { color: var(--ink); font-weight: 600; }
.detail > span { white-space: nowrap; }
.line b { position: relative; display: inline-block; font-variant-numeric: tabular-nums; }
figcaption { display: flex; justify-content: space-between; color: var(--muted); font-size: 12px; margin-top: 4px; }
.empty { text-align: center; color: var(--muted); }
footer { max-width: 960px; margin: 0 auto; padding: 24px 16px 40px; color: var(--muted); font-size: 13px; text-align: center; }
footer a { color: inherit; }
.big, .total { position: relative; display: inline-block; }
.counting { color: var(--count); }
.landed { animation: land 0.6s cubic-bezier(0.2, 0.8, 0.2, 1); }
@keyframes land {
  0% { transform: scale(1); text-shadow: 0 0 0 transparent; }
  30% { transform: scale(1.08); text-shadow: 0 0 18px var(--glow); }
  100% { transform: scale(1); text-shadow: 0 0 0 transparent; }
}
.gain { position: absolute; left: 50%; bottom: 100%; transform: translateX(-50%); white-space: nowrap;
  font-size: 13px; font-weight: 600; color: var(--bar); pointer-events: none;
  animation: rise 1.8s ease-out forwards; }
@keyframes rise {
  0% { opacity: 0; transform: translate(-50%, 6px); }
  15% { opacity: 1; }
  100% { opacity: 0; transform: translate(-50%, -22px); }
}
.day i { transform-origin: bottom; transition: height 0.8s cubic-bezier(0.2, 0.8, 0.2, 1); }
.grow .day i { animation: grow 0.9s cubic-bezier(0.2, 0.8, 0.2, 1) backwards; }
@keyframes grow { from { transform: scaleY(0); } }
@media (prefers-reduced-motion: reduce) {
  .landed, .gain, .grow .day i { animation: none; }
  .gain { display: none; }
  .day i { transition: none; }
}
#tip { position: fixed; pointer-events: none; background: var(--ink); color: var(--bg); font-size: 12px;
  padding: 6px 9px; border-radius: 8px; line-height: 1.35; white-space: nowrap; z-index: 10; }
`;

const SCRIPT = `
const tip = document.getElementById("tip");
let on = null;
function show(day, x, y) {
  if (on !== day) { on?.classList.remove("on"); on = day; day.classList.add("on"); }
  const d = day.dataset;
  tip.innerHTML = "<b>" + d.day + "</b> · " + d.cost + " at list prices<br>Claude " + d.claude + " · " + d.output + " output tokens · " + d.messages + " messages"
    + (d.credits === "0" ? "" : "<br>Tripo " + d.tripo + " · " + d.credits + " credits");
  tip.hidden = false;
  const w = tip.offsetWidth;
  tip.style.left = Math.min(window.innerWidth - w - 8, Math.max(8, x - w / 2)) + "px";
  tip.style.top = (y - tip.offsetHeight - 12) + "px";
}
function hide() { tip.hidden = true; on?.classList.remove("on"); on = null; }
document.addEventListener("pointermove", (e) => {
  const day = e.target.closest?.(".day");
  if (day) show(day, e.clientX, e.clientY); else hide();
});
document.addEventListener("pointerleave", hide);

// Counters count up from zero the first time they are seen, and the page checks
// /json every few seconds, rolling any counter that grew to its new value.
${compact}
${money}
${value}
${hasTripo}
${shape}
const still = matchMedia("(prefers-reduced-motion: reduce)").matches;
const fmt = (n) => n.toLocaleString("en-US");
const ease = (t) => (t === 1 ? 1 : 1 - Math.pow(2, -10 * t));
// Write only the text node, so a rising "+gain" beside it survives.
const put = (el, n) => { el.firstChild.nodeValue = el.dataset.fmt === "usd" ? money(n) : compact(Math.round(n)); };

function roll(el, from, to, ms, delay) {
  el.dataset.n = to;
  if (el.dataset.fmt !== "usd") el.title = fmt(to);
  // A hidden page draws no animation frames, so it shows the new value at once.
  const me = (el.rolling = {});
  if (still || from === to || document.hidden) { put(el, to); return; }
  const start = performance.now() + (delay || 0);
  el.classList.remove("landed");
  put(el, from);
  const step = (now) => {
    if (el.rolling !== me) return;
    const t = Math.min(1, Math.max(0, (now - start) / ms));
    if (t > 0) el.classList.add("counting");
    put(el, from + (to - from) * ease(t));
    if (t < 1) return requestAnimationFrame(step);
    el.classList.remove("counting");
    void el.offsetWidth;
    el.classList.add("landed");
  };
  requestAnimationFrame(step);
}

function gain(el, by, usd) {
  if (still || by <= 0) return;
  const g = document.createElement("span");
  g.className = "gain";
  g.textContent = "+" + (usd ? money(by) : compact(by));
  el.append(g);
  g.addEventListener("animationend", () => g.remove());
}

// Count up each group when it scrolls into view, largest counter slowest.
const seen = new IntersectionObserver((entries) => {
  for (const e of entries) {
    if (!e.isIntersecting) continue;
    seen.unobserve(e.target);
    e.target.classList.add("grow");
    e.target.querySelectorAll(".day").forEach((day, i) => day.querySelectorAll("i").forEach((b) => { b.style.animationDelay = (i * 18) + "ms"; }));
    e.target.querySelectorAll("[data-n]").forEach((el, i) => {
      const big = el.matches(".big, .total");
      roll(el, 0, +el.dataset.n, big ? 2000 : 1400, big ? 0 : 150 + i * 70);
    });
  }
}, { threshold: 0.2 });
document.querySelectorAll(".hero, .person").forEach((s) => {
  if (!still) s.querySelectorAll("[data-n]").forEach((el) => put(el, 0));
  seen.observe(s);
});

function update(group, data) {
  group.querySelectorAll("[data-n]").forEach((el) => {
    const was = +el.dataset.n, now = value(data, el.dataset.key);
    if (typeof now !== "number" || now === was) return;
    roll(el, was, now, 1500, 0);
    if (el.matches(".big, .total")) gain(el, now - was, el.dataset.fmt === "usd");
  });
}

let busy = false;
async function refresh() {
  if (busy) return;
  busy = true;
  let s;
  try {
    const r = await fetch("/json", { cache: "no-store" });
    if (!r.ok) return;
    s = await r.json();
  } catch { return; } finally { busy = false; }
  // A new person, a new day or someone's first Tripo credits change the layout; draw the page again.
  if (shape(s) !== document.querySelector("main").dataset.shape) return location.reload();
  const cards = [...document.querySelectorAll(".person")];
  document.getElementById("updated").textContent = s.generated_at.slice(0, 16).replace("T", " ");
  update(document.querySelector(".hero"), s.totals);
  const peak = Math.max(0.01, ...s.users.flatMap((u) => u.daily.map((d) => d.cost)));
  for (const u of s.users) {
    const card = cards.find((c) => c.dataset.user === u.user);
    update(card, u);
    card.querySelectorAll(".day").forEach((day, i) => {
      const d = u.daily[i];
      Object.assign(day.dataset, { output: fmt(d.output), messages: d.messages, cost: money(d.cost),
        claude: money(d.claude_cost), tripo: money(d.tripo_cost), credits: fmt(d.tripo_credits) });
      const h = (c) => (c <= 0 ? 0 : Math.max(2, (c / peak) * 100)).toFixed(1) + "%";
      const claude = day.querySelector("i.claude");
      claude.style.height = h(d.claude_cost);
      claude.classList.toggle("top", d.tripo_cost <= 0);
      day.querySelector("i.tripo").style.height = h(d.tripo_cost);
    });
  }
}
// Poll whether or not the page has focus or is in view: a page on a second screen
// should still tick, and a check reads one row.
setInterval(refresh, 5000);
`;
