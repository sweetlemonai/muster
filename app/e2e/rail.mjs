#!/usr/bin/env node
// Headless checks and screenshots of the sidebar hidden to its rail (mu-167), on the same footing
// as shot.mjs: headless WebKit, app/dist on 127.0.0.1, Tauri's IPC mocked (mock.mjs), and a
// synthetic many-project state, so nothing reads or writes a real queue.
//
//   cd app && pnpm build && node e2e/rail.mjs [--out <folder>]
//
// It prints each check and the screenshots' paths, and exits 1 when a check fails.
import { mkdirSync, readFileSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, join, normalize, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseArgs } from "node:util";
import { mockScript } from "./mock.mjs";

const here = fileURLToPath(new URL(".", import.meta.url));
const { values: args } = parseArgs({ options: { out: { type: "string", default: join(tmpdir(), "muster-rail-shots") } } });
const PLAYWRIGHT = process.env.PLAYWRIGHT ?? "/Users/imassih/playground/braindump/main/node_modules/playwright";
const DIST = resolve(here, "../dist");
const { webkit } = await import(pathToFileURL(join(PLAYWRIGHT, "index.mjs")).href);

// Twelve made-up projects under three headings; "atlas" has 128 tasks waiting on the owner.
const NOW = Math.floor(Date.now() / 1000);
const COLOURS = ["pink", "lime", "orchid", "tan", "mint", ""];
const PROJECTS = ["atlas", "birch", "cedar", "delta", "ember", "fjord", "grove", "harbor", "iris", "juniper", "kestrel", "lumen"];
const task = (queue, n, over = {}) => ({
  queue, id: String(n).padStart(4, "0"), title: `Task ${n}`, kind: "task", priority: "normal", status: "todo", stage: "ready", blocked: false, stageHistory: [], folder: "todo",
  path: `todo/${n}.md`, claimedBy: "", heldBy: "", stageSince: "", checkins: [], heartbeat: "", waitingOn: "", depends: [], forWho: "ai", commit: "", captured: "", capturedAt: "", doneAt: "",
  blockHistory: [], questions: [], openQuestions: 0, live: false, tokensUsed: null, tokensShared: false, ownerCriteria: [], waitingForOwner: false, tags: [], estTime: "", estTestTime: "",
  estTokens: "", estMinutes: 0, estTestMinutes: 0, estTokenCount: 0, claimedOn: "", doneOn: "", links: [], comments: [], attachments: [], logTail: [], modified: 0, level: "task", parent: "",
  due: "", children: [], progress: null, shownStage: over.stage ?? "ready", search: "", ...over,
});
const tasks = PROJECTS.flatMap((p, i) => {
  const waiting = p === "atlas" ? 128 : i % 3;
  return [
    ...Array.from({ length: waiting }, (_, n) => task(p, n + 1, { stage: "building", waitingForOwner: true })),
    task(p, 900, { stage: "ready" }),
    ...(i % 4 === 1 ? [task(p, 901, { stage: "building", live: true })] : []),
  ];
});
const counts = (p) => {
  const c = {};
  for (const t of tasks.filter((t) => t.queue === p)) c[t.stage] = (c[t.stage] ?? 0) + 1;
  return c;
};
const STATE = {
  queues: PROJECTS.map((p, i) => ({
    name: p, path: `/synthetic/${p}/tasks`, prefix: p.slice(0, 2), counts: counts(p), yourCheck: 0, yourCheckLimit: 5, label: p[0].toUpperCase() + p.slice(1),
    category: ["Apps", "Tools", "Sites"][i % 3], colour: COLOURS[i % COLOURS.length], rank: i, categoryRank: i % 3, categoryFolded: false, lessonsWaiting: 0, running: false,
  })),
  tasks,
  backlog: [],
  problems: [],
  activity: [],
  now: NOW,
  settings: { leaseMinutes: 720, liveMinutes: 15, oldPausesOffered: true, claudeAccounts: [] },
};

const TYPES = { ".html": "text/html", ".json": "application/json", ".js": "text/javascript", ".css": "text/css", ".svg": "image/svg+xml", ".png": "image/png", ".woff2": "font/woff2", ".md": "text/markdown" };
const server = createServer((req, res) => {
  const path = decodeURIComponent(new URL(req.url ?? "/", "http://x").pathname);
  const file = normalize(join(DIST, path === "/" ? "index.html" : path));
  if (!file.startsWith(DIST + sep) || !statSync(file, { throwIfNoEntry: false })?.isFile()) {
    res.writeHead(404).end();
    return;
  }
  res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" }).end(readFileSync(file));
});
await new Promise((ok) => server.listen(0, "127.0.0.1", ok));
const url = `http://127.0.0.1:${server.address().port}/`;
mkdirSync(args.out, { recursive: true });

const results = [];
const check = (name, ok, detail = "") => {
  results.push(ok);
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail ? ` (${detail})` : ""}`);
};
const shots = [];
const browser = await webkit.launch({ headless: true });

async function open({ scheme = "light", dir = "ltr", mode = "adhd", storage = {} } = {}) {
  const context = await browser.newContext({ viewport: { width: 1400, height: 880 }, colorScheme: scheme, reducedMotion: "reduce" });
  const seed = { "muster.displayMode": mode, "muster.layout.all": "columns", ...(dir === "rtl" ? { "muster.locale": "fa" } : {}), ...storage };
  await context.addInitScript(mockScript(STATE, seed, { dir }));
  const page = await context.newPage();
  page.on("pageerror", (e) => check("no page errors", false, String(e)));
  await page.goto(url);
  await page.waitForSelector(".sidebar .side-item, .rail .rail-item");
  await page.waitForTimeout(300);
  return { page, context };
}
const shoot = async (page, name) => {
  const file = join(args.out, `${name}.png`);
  await page.screenshot({ path: file });
  shots.push(file);
};
const width = (page, sel) => page.locator(sel).first().evaluate((e) => e.getBoundingClientRect().width);
const focused = (page) => page.evaluate(() => document.activeElement?.className ?? "");
const said = (page) => page.locator('p.sr-only[role="status"]').last().innerText();

/** The rail's checks: every project there with its badge whole, inside the rail. */
async function railChecks(page, tag) {
  const items = await page.locator(".rail .rail-item").count();
  check(`${tag}: the rail has All and every project`, items === PROJECTS.length + 1, `${items}`);
  const whole = await page.evaluate(() => {
    const rail = document.querySelector(".sidebar").getBoundingClientRect();
    return [...document.querySelectorAll(".rail-needs")].every((b) => {
      const r = b.getBoundingClientRect();
      return b.scrollWidth <= b.clientWidth + 1 && r.left >= rail.left - 0.5 && r.right <= rail.right + 0.5;
    });
  });
  check(`${tag}: every needs-you badge is whole inside the rail`, whole);
  const atlas = await page.locator(".rail .rail-item", { hasText: /128|۱۲۸/ }).count();
  check(`${tag}: the 3-digit count shows`, atlas >= 1);
}

try {
  for (const scheme of ["light", "dark"]) {
    for (const mode of ["adhd", "focus"]) {
      const tag = `${mode} ${scheme}`;
      const { page, context } = await open({ scheme, mode });
      const side = await width(page, ".sidebar");
      const main = await width(page, ".main-col");
      await shoot(page, `${mode}-${scheme}-shown`);
      // Keyboard: Tab to a project row, then Hide from the keyboard... the row first, so the
      // focus is inside the sidebar when it hides.
      await page.locator(".sidebar .side-item").nth(1).focus();
      await page.evaluate(() => window.__MOCK_EMIT__("view-toggle", "sidebar"));
      await page.waitForSelector(".rail");
      check(`${tag}: the menu's event hides it once`, (await page.locator(".rail").count()) === 1);
      check(`${tag}: the rail is 56px`, Math.round(await width(page, ".sidebar")) === 56);
      const gained = Math.round((await width(page, ".main-col")) - main);
      // The difference, and the splitter's own few pixels, since it isn't drawn beside the rail.
      check(`${tag}: the board is wider by the difference`, gained >= Math.round(side - 56) && gained <= Math.round(side - 56) + 8, `${gained} vs ${Math.round(side - 56)}`);
      check(`${tag}: no splitter beside the rail`, (await page.locator('.ui-splitter[aria-label="Projects width"]').count()) === 0);
      check(`${tag}: the focus moved to Show`, (await focused(page)).includes("side-toggle") && (await page.evaluate(() => document.activeElement?.textContent)) === "Show");
      check(`${tag}: it says "Sidebar hidden"`, (await said(page)) === "Sidebar hidden");
      const menu = await page.evaluate(() => window.__MOCK_ARGS__.filter((c) => c.cmd === "set_view_menu").at(-1)?.args);
      check(`${tag}: the menu is told it's hidden`, menu?.shown?.sidebar === false, JSON.stringify(menu));
      await railChecks(page, tag);
      await shoot(page, `${mode}-${scheme}-hidden-focus`);
      // Kept across a reload, in this mode.
      await page.reload();
      await page.waitForSelector(".rail .rail-item");
      check(`${tag}: still hidden after a reload`, (await page.locator(".rail").count()) === 1);
      // Show from the keyboard: Enter on the rail's Show, the focus back on Hide, the width back.
      await page.locator(".rail .side-toggle").focus();
      await page.keyboard.press("Enter");
      await page.waitForSelector(".sidebar .side-item");
      check(`${tag}: shown again at its kept width`, Math.round(await width(page, ".sidebar")) === Math.round(side));
      check(`${tag}: the focus is on Hide`, (await page.evaluate(() => document.activeElement?.textContent)) === "Hide");
      check(`${tag}: it says "Sidebar shown"`, (await said(page)) === "Sidebar shown");
      await context.close();
    }
  }
  // The pick is per mode: hidden in ADHD leaves Standard shown.
  {
    const { page, context } = await open({ mode: "standard", storage: { "muster.toggles": JSON.stringify({ adhd: { sidebar: "hidden" } }) } });
    check("standard: ADHD's hide leaves Standard's sidebar shown", (await page.locator(".rail").count()) === 0);
    await context.close();
  }
  // ⌘. into a mode that keeps the sidebar hidden: the focus on a project row lands on Show, and
  // back out of it on Hide, never on the page's body (review a11y-1).
  {
    const { page, context } = await open({ storage: { "muster.toggles": JSON.stringify({ focus: { sidebar: "hidden" } }) } });
    await page.locator(".sidebar .side-item").nth(1).focus();
    await page.keyboard.press("Meta+.");
    await page.waitForSelector(".rail");
    check("⌘. into Focus, where it's hidden: the focus moves to Show", (await page.evaluate(() => document.activeElement?.textContent)) === "Show");
    await page.keyboard.press("Meta+.");
    await page.waitForSelector(".sidebar .side-item");
    check("⌘. back: the focus moves to Hide", (await page.evaluate(() => document.activeElement?.textContent)) === "Hide");
    await context.close();
  }
  // Right to left: the rail on the right, Show at its top, in Persian.
  {
    const { page, context } = await open({ dir: "rtl", storage: { "muster.toggles": JSON.stringify({ adhd: { sidebar: "hidden" } }) } });
    const sides = await page.evaluate(() => ({ rail: document.querySelector(".sidebar").getBoundingClientRect().left, main: document.querySelector(".main-col").getBoundingClientRect().left }));
    check("fa: the rail is on the right", sides.rail > sides.main, JSON.stringify(sides));
    check("fa: Show reads نمایش", (await page.locator(".rail .side-toggle").innerText()) === "نمایش");
    await railChecks(page, "fa");
    await shoot(page, "fa-rtl-hidden");
    await page.locator(".rail .side-toggle").click();
    await page.waitForSelector(".sidebar .side-item");
    await shoot(page, "fa-rtl-shown");
    await context.close();
  }
} finally {
  await browser.close();
  server.close();
}
for (const s of shots) console.log(s);
const failed = results.filter((r) => !r).length;
console.log(failed ? `${failed} of ${results.length} checks failed` : `all ${results.length} checks passed`);
process.exit(failed ? 1 : 0);
