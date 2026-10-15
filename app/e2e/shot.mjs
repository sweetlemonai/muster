#!/usr/bin/env node
// Headless screenshots and checks of Muster's window (mu-143; mu-133 and mu-136 build on it).
//
// Dependency-free beyond Playwright, which Muster doesn't depend on: it's loaded from another
// checkout on this Mac (braindump's node_modules) or from $PLAYWRIGHT, a folder with Playwright's
// index.mjs. Nothing is added to Muster's package.json.
//
// Never touches the owner's screen: headless WebKit, a static server on 127.0.0.1 for app/dist,
// and Tauri's IPC mocked from a read-only snapshot (mock.mjs), so no check can write to a queue.
//
//   cd app && pnpm build && node e2e/shot.mjs [--snapshot state.json] [--queue muster]
//     [--label Muster] [--out <folder>]
//
// Without --snapshot it runs `muster list --json --queue <queue>` for one. It prints each check
// and the screenshots' paths, and exits 1 when a check fails.
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, statSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { extname, join, normalize, resolve, sep } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { parseArgs } from "node:util";
import { mockScript, stateFrom } from "./mock.mjs";

const here = fileURLToPath(new URL(".", import.meta.url));
const { values: args } = parseArgs({
  options: {
    snapshot: { type: "string" },
    queue: { type: "string", default: "muster" },
    label: { type: "string", default: "Muster" },
    out: { type: "string", default: join(tmpdir(), "muster-shots") },
  },
});
const PLAYWRIGHT = process.env.PLAYWRIGHT ?? "/Users/imassih/playground/braindump/main/node_modules/playwright";
const DIST = resolve(here, "../dist");
const { webkit } = await import(pathToFileURL(join(PLAYWRIGHT, "index.mjs")).href);

// The snapshot: read-only, from a file or from `muster list` (arguments as an array, no shell).
const snap = JSON.parse(args.snapshot ? readFileSync(args.snapshot, "utf8") : execFileSync("muster", ["list", "--json", "--queue", args.queue], { encoding: "utf8", maxBuffer: 64 << 20 }));
const base = { queue: args.queue, label: args.label };

// app/dist on 127.0.0.1, on a port the system picks; only files inside dist are served.
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

/** A fresh page on the board, with `storage` seeded and the project picked in the sidebar. */
async function open({ scheme = "light", dir = "ltr", layout = "grid", groupBy = "feature", state = stateFrom(snap, base) } = {}) {
  // The app's default window (src-tauri/tauri.conf.json): what the owner sees first (review R1).
  const context = await browser.newContext({ viewport: { width: 1400, height: 880 }, colorScheme: scheme, reducedMotion: "reduce" });
  // Rows by feature, as the owner's mockup shows it (the board's Group by, kept on this Mac).
  // Right-to-left is the window in Persian (mu-146): the language sets the page's direction.
  const storage = { [`muster.layout.${args.queue}`]: layout, "muster.layout.all": layout, "muster.groupBy": groupBy, ...(dir === "rtl" ? { "muster.locale": "fa" } : {}) };
  await context.addInitScript(mockScript(state, storage, { dir }));
  const page = await context.newPage();
  page.on("pageerror", (e) => check("no page errors", false, String(e)));
  await page.goto(url);
  await page.locator(".side-row").getByText(args.label, { exact: true }).first().click();
  await page.waitForSelector(layout === "grid" ? ".fg-grid" : "main.board, main.bands");
  return { page, context };
}
const shoot = async (page, name) => {
  const file = join(args.out, `${name}.png`);
  await page.screenshot({ path: file });
  shots.push(file);
};

try {
  // Feature grid, light: the checks, then the screenshot as the grid first draws.
  {
    const { page, context } = await open();
    check("the grid is an ARIA grid with 8 column heads", (await page.locator('[role="grid"] [role="columnheader"]').count()) === 8);
    check("each row has a row head", (await page.locator('[role="rowheader"]').count()) === (await page.locator('[role="grid"] [role="row"]').count()) - 1);
    const done = (await page.locator(".fg-colhead.col-done").innerText()).replace(/\s+/g, " ").trim();
    // One line: "Done" and "· N today" start on the same line, and the words aren't cut (review R8).
    const oneLine = await page.locator(".fg-colhead.col-done").evaluate((h) => {
      const b = h.querySelector("b").getBoundingClientRect();
      const n = h.querySelector(".fg-n").getBoundingClientRect();
      return Math.abs(b.top - n.top) < 4 && h.scrollWidth <= h.clientWidth + 1;
    });
    check("the Done head is one line", /^Done · \d+ today$/.test(done) && oneLine, done);
    check("no cell says None", !(await page.locator(".fg-cell").allInnerTexts()).some((t) => /\bNone\b/.test(t)));
    await shoot(page, "grid-light");

    // Past the edge (review R1): at the default window, with Recently finished docked, the columns
    // past the frame's end are named by the cue over it, with what needs you; its click brings them
    // in, and the whole Your check head shows.
    const frameEnd = async () => page.locator(".fg-frame").evaluate((f) => f.getBoundingClientRect().left + f.clientLeft + f.clientWidth);
    const headIn = async (col) => (await page.locator(`[data-col="${col}"]`).evaluate((h) => h.getBoundingClientRect().right)) <= (await frameEnd()) + 1;
    const cue = page.locator(".fg-over .col-cue");
    if (await headIn("done")) check("every column fits the default window, so there's no cue", (await cue.count()) === 0);
    else if (!(await cue.count())) check("a cue over the grid's end names the columns past the edge, and what needs you there", false, "Done is past the edge and nothing says so");
    else {
      const words = (await cue.innerText()).replace(/\s+/g, " ").trim();
      check("a cue over the grid's end names the columns past the edge, and what needs you there", /^\+\d+ more( · \d+ needs? you)?$/.test(words) && /past the edge/.test((await cue.getAttribute("aria-label")) ?? ""), words);
      for (let i = 0; i < 3 && (await cue.count()) && !(await headIn("your-check")); i++) {
        await cue.click();
        await page.waitForTimeout(150);
      }
      check("the cue's click brings Your check into view, the focus in its column", (await headIn("your-check")) && (await page.evaluate(() => !!document.activeElement?.closest("[data-cell]"))));
      await shoot(page, "grid-light-end");
      await page.locator(".fg-frame").evaluate((f) => f.scrollTo({ left: 0 }));
    }

    // The keys never leave the focus under the sticky names or header row (review R2): End to
    // Done, then ← twice, and ↓ through the rows.
    const clear = () =>
      page.evaluate(() => {
        const el = document.activeElement;
        const names = document.querySelector(".fg-corner").getBoundingClientRect();
        const head = document.querySelector(".fg-headrow").getBoundingClientRect();
        const r = el.getBoundingClientRect();
        const rtl = getComputedStyle(el).direction === "rtl";
        const side = rtl ? r.right <= names.left + 1 : r.left >= names.right - 1;
        return side && r.top >= head.bottom - 1;
      });
    await page.locator('[data-cell="0:0"]').focus();
    await page.keyboard.press("End");
    await page.keyboard.press("ArrowLeft");
    const afterLeft = await clear();
    await page.keyboard.press("ArrowLeft");
    const afterLeft2 = await clear();
    await page.keyboard.press("Home");
    const afterHome = await clear();
    let downs = true;
    for (let i = 0; i < 4; i++) {
      await page.keyboard.press("ArrowDown");
      downs = downs && (await clear());
    }
    let ups = true;
    for (let i = 0; i < 4; i++) {
      await page.keyboard.press("ArrowUp");
      ups = ups && (await clear());
    }
    check("the focused cell is never under the sticky names or header row", afterLeft && afterLeft2 && afterHome && downs && ups, `${afterLeft}, ${afterLeft2}, ${afterHome}, ${downs}, ${ups}`);
    await page.locator(".fg-frame").evaluate((f) => f.scrollTo({ left: 0, top: 0 }));

    // Only what needs me dims every card but the ones waiting on you, and dims nothing once off
    // (review R4).
    await page.locator(".fg-mine-pick").click();
    const dim = await page.locator(".fg-card.fg-dim").count();
    const youDim = await page.locator(".fg-card.fg-you.fg-dim").count();
    const cards = await page.locator(".fg-card").count();
    const you = await page.locator(".fg-card.fg-you").count();
    check("Only what needs me dims every card but the ones waiting on you", dim > 0 && youDim === 0 && dim === cards - you, `${dim} of ${cards} dimmed, ${you} waiting on you`);
    await shoot(page, "grid-light-mine");
    await page.locator(".fg-mine-pick").click();
    check("off again, it dims nothing", (await page.locator(".fg-card.fg-dim").count()) === 0);

    // Arrow keys move between cells.
    await page.locator('[data-cell="0:0"]').focus();
    await page.keyboard.press("ArrowRight");
    const right = await page.evaluate(() => document.activeElement?.getAttribute("data-cell"));
    await page.keyboard.press("ArrowDown");
    const down = await page.evaluate(() => document.activeElement?.getAttribute("data-cell"));
    check("→ then ↓ move the focus between cells", right === "0:1" && down === "1:1", `${right}, ${down}`);

    // The keyboard: ↩ into a cell, ↓ to its "+N more", ↩ expands it with the focus kept, Esc back
    // to the cell, Tab out of the grid in one stop, and ← from Backlog to the feature's name.
    const moreAt = await page.locator(".fg-more").first().evaluate((b) => b.closest("[data-cell]").getAttribute("data-cell"));
    await page.locator(`[data-cell="${moreAt}"]`).focus();
    await page.keyboard.press("Enter");
    await page.keyboard.press("ArrowDown");
    await page.keyboard.press("ArrowDown");
    const onMore = await page.evaluate(() => document.activeElement?.classList.contains("fg-more") ?? false);
    await page.keyboard.press("Enter");
    const label = await page.evaluate(() => document.activeElement?.textContent ?? "");
    await page.keyboard.press("Enter");
    await page.keyboard.press("Escape");
    const backAt = await page.evaluate(() => document.activeElement?.getAttribute("data-cell"));
    check("↩ ↓ ↓ ↩ expands a cell from the keys, the focus staying on Show fewer; Esc back to the cell", onMore && label === "Show fewer" && backAt === moreAt, `${onMore}, ${label}, ${backAt}`);
    await page.keyboard.press("Tab");
    check("Tab leaves the grid in one stop", await page.evaluate(() => !document.activeElement?.closest(".fg-grid")));
    await page.locator('[data-cell="0:0"]').focus();
    await page.keyboard.press("ArrowLeft");
    check("← from Backlog goes to the feature's name", await page.evaluate(() => !!document.activeElement?.closest('[data-rowhead="0"]')));

    // "+N more" expands the cell in place.
    const more = page.locator(".fg-more", { hasText: /^\+\d+ more$/ }).first();
    if (await more.count()) {
      const n = Number((await more.innerText()).match(/\d+/)[0]);
      // The cell by its place: the "+N more" button is gone once it's clicked.
      const at = await more.evaluate((b) => b.closest("[data-cell]").getAttribute("data-cell"));
      const cell = page.locator(`[data-cell="${at}"]`);
      const before = await cell.locator(".fg-card").count();
      await more.click();
      const after = await cell.locator(".fg-card").count();
      check("+N more shows the rest in the same cell", after === before + n && (await cell.locator(".fg-more", { hasText: "Show fewer" }).count()) === 1, `${before} + ${n} → ${after}`);
      await shoot(page, "grid-light-expanded");
      await cell.locator(".fg-more", { hasText: "Show fewer" }).click();
    } else check("+N more shows the rest in the same cell", false, "no cell has more than 2 cards");

    // A card opens the task pane beside the board.
    const card = page.locator(".fg-card:not(.fg-note)").first();
    const title = (await card.locator(".fg-t").innerText()).trim();
    await card.click();
    const pane = page.locator("aside.pane");
    const opened = await pane.getByText(title, { exact: true }).first().waitFor({ timeout: 10000 }).then(() => true, () => false);
    check("a card opens its task in the pane beside the grid", opened, title);
    check("the grid stays in view beside the pane", await page.locator(".fg-grid").isVisible());
    await shoot(page, "grid-light-pane");

    // The layout is kept per project: back to Columns from the Layout chips, then a reload.
    await page.locator(".layout-pick .ui-pill", { hasText: "Columns" }).click();
    check("Columns from the Layout chips draws the columns", (await page.locator("main.board, main.bands").count()) === 1 && (await page.locator(".fg").count()) === 0);
    const kept = await page.evaluate((q) => localStorage.getItem(`muster.layout.${q}`), args.queue);
    check("the choice is kept for the project", kept === "columns", kept);
    await context.close();
  }

  // Feature grid, dark.
  {
    const { page, context } = await open({ scheme: "dark" });
    await shoot(page, "grid-dark");
    await context.close();
  }

  // Right-to-left: features on the right, ← moves forward.
  {
    const { page, context } = await open({ dir: "rtl" });
    const head = await page.locator('[role="rowheader"]').first().boundingBox();
    const cell = await page.locator('[data-cell="0:0"]').boundingBox();
    check("right-to-left puts the features on the right", head.x > cell.x, `row head at ${Math.round(head.x)}, Backlog at ${Math.round(cell.x)}`);
    await shoot(page, "grid-rtl");
    await page.locator('[data-cell="0:0"]').focus();
    await page.keyboard.press("ArrowLeft");
    check("in right-to-left, ← moves to the next stage", (await page.evaluate(() => document.activeElement?.getAttribute("data-cell"))) === "0:1");
    await context.close();
  }

  // Columns, the default, unchanged: the plain board, and grouped by feature.
  for (const groupBy of ["none", "feature"]) {
    const { page, context } = await open({ layout: "columns", groupBy });
    check(`Columns is today's board (Group by ${groupBy})`, (await page.locator(groupBy === "none" ? "main.board" : "main.bands").count()) === 1 && (await page.locator(".fg").count()) === 0);
    await shoot(page, groupBy === "none" ? "columns-light" : "columns-light-by-feature");
    await context.close();
  }

  // Your check full: synthetic, the snapshot's first two Testing tasks moved to Your check with a
  // limit of 2, to show the alert line and the column head (the real queue may have room).
  {
    const state = stateFrom(snap, { ...base, limit: 2 });
    let moved = 0;
    for (const t of state.tasks) {
      if (moved < 2 && t.stage === "testing" && t.level === "task") {
        Object.assign(t, { stage: "your-check", shownStage: "your-check", heldBy: "owner", waitingForOwner: true });
        moved++;
      }
    }
    state.queues[0].yourCheck = state.tasks.filter((t) => t.stage === "your-check" && t.forWho !== "human").length;
    const { page, context } = await open({ state });
    const alert = (await page.locator(".fg-alert").innerText()).trim();
    check("a full Your check says so in one line over the grid", /^Your check is (full|over its limit) \(\d+ of 2\)/.test(alert), alert);
    // The whole Your check head in the shot (review R1): scrolled to the grid's end.
    await page.locator(".fg-frame").evaluate((f) => f.scrollTo({ left: f.scrollWidth }));
    await page.waitForTimeout(150);
    const head = page.locator(".fg-colhead.col-your-check");
    const headText = (await head.innerText()).replace(/\s+/g, " ").trim();
    check("the full Your check head says so in the warning colour", (await head.getAttribute("class")).includes("fg-full") && /\d+ of 2, full$/.test(headText), headText);
    await shoot(page, "grid-light-full-synthetic");
    await context.close();
  }
} finally {
  await browser.close();
  server.close();
}

for (const s of shots) console.log(`shot ${s}`);
const failed = results.filter((ok) => !ok).length;
console.log(failed ? `e2e: RED (${failed} of ${results.length} checks failed)` : `e2e: green (${results.length} checks)`);
process.exit(failed ? 1 : 0);
