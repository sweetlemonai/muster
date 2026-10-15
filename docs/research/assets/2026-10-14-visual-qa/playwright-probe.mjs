// mu-130 probe: the walkthrough overlay mechanics in headless Playwright WebKit, on synthetic pages.
// Checks that an addInitScript overlay comes back after a navigation, that its buttons reach the
// runner through exposeBinding, that a masked screenshot is stable while a clock ticks, and what
// the trace and the video files are. Headless: no window opens on the screen.
//
// Setup (outside the repo; Playwright isn't a Muster dependency):
//   mkdir -p "$TMPDIR/pw" && cd "$TMPDIR/pw" && echo '{"type":"module"}' > package.json
//   DO_NOT_TRACK=1 npm install --no-audit --no-fund playwright@1.64.1
//   npx playwright install webkit
// Run:  cd "$TMPDIR/pw" && node <path to this file> <out-dir>
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, statSync, writeFileSync, readFileSync, readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

// Playwright resolves from the current folder (the setup above), not from this file's folder.
const { webkit } = createRequire(join(process.cwd(), 'noop.js'))('playwright');

const out = resolve(process.argv[2] ?? 'pw-probe-out');
mkdirSync(out, { recursive: true });

const page = (title, next) => `<!doctype html><meta charset="utf-8"><title>${title}</title>
<body style="font: 16px -apple-system, sans-serif; margin: 40px">
<h1>${title}</h1><p>Synthetic fixture page.</p>
<p>Now: <span class="clock"></span></p>
${next ? `<a id="next" href="${next}">Next screen</a>` : ''}
<script>setInterval(() => { document.querySelector('.clock').textContent = new Date().toISOString() }, 50)</script>
</body>`;
writeFileSync(join(out, 'b.html'), page('Screen B'));
writeFileSync(join(out, 'a.html'), page('Screen A', 'b.html'));

// The walkthrough overlay: one step card with pass, fail and comment, injected into every page.
const overlay = () => {
  const mount = () => {
    if (document.getElementById('qa-overlay')) return;
    const card = document.createElement('div');
    card.id = 'qa-overlay';
    card.style.cssText = 'position:fixed;right:16px;bottom:16px;padding:12px;background:#222;color:#fff;border-radius:8px;z-index:2147483647';
    const step = document.createElement('div');
    step.textContent = 'Step: check the heading';
    card.appendChild(step);
    for (const verdict of ['pass', 'fail']) {
      const button = document.createElement('button');
      button.id = `qa-${verdict}`;
      button.textContent = verdict;
      button.onclick = () => window.qaVerdict({ verdict, heading: document.querySelector('h1').textContent });
      card.appendChild(button);
    }
    document.body.appendChild(card);
  };
  if (document.body) mount();
  else document.addEventListener('DOMContentLoaded', mount);
};

const browser = await webkit.launch({ headless: true });
const context = await browser.newContext({
  viewport: { width: 800, height: 500 },
  recordVideo: { dir: join(out, 'video'), size: { width: 800, height: 500 } },
});
await context.tracing.start({ screenshots: true, snapshots: true });
const verdicts = [];
await context.exposeBinding('qaVerdict', (source, v) => verdicts.push({ url: source.page.url().split('/').pop(), ...v }));
await context.addInitScript(overlay);

const tab = await context.newPage();
await tab.goto(pathToFileURL(join(out, 'a.html')).href);
await tab.click('#qa-pass');
await tab.click('#next');
await tab.waitForURL(/b\.html$/);
const overlayAfterNavigation = await tab.locator('#qa-overlay').count();
await tab.click('#qa-fail');
await tab.waitForFunction(() => true);
console.log('overlay after navigation:', overlayAfterNavigation === 1 ? 'present' : 'missing');
console.log('verdicts received by the runner:', JSON.stringify(verdicts));

const hash = (b) => createHash('sha256').update(b).digest('hex').slice(0, 12);
const masked = { mask: [tab.locator('.clock')], maskColor: '#FF00FF', animations: 'disabled', caret: 'hide' };
const m1 = await tab.screenshot({ path: join(out, 'masked-1.png'), ...masked });
await tab.waitForTimeout(300);
const m2 = await tab.screenshot({ path: join(out, 'masked-2.png'), ...masked });
const u1 = await tab.screenshot({ path: join(out, 'unmasked-1.png') });
await tab.waitForTimeout(300);
const u2 = await tab.screenshot({ path: join(out, 'unmasked-2.png') });
console.log('masked screenshots identical:', hash(m1) === hash(m2), '| unmasked identical:', hash(u1) === hash(u2));

// The alternative to a mask: freeze the page's clock, so the live text stops changing.
const frozen = await context.newPage();
await frozen.clock.setFixedTime(new Date('2026-01-01T09:41:00Z'));
await frozen.goto(pathToFileURL(join(out, 'b.html')).href);
await frozen.waitForTimeout(200);
const f1 = await frozen.screenshot({ path: join(out, 'frozen-1.png') });
await frozen.waitForTimeout(300);
const f2 = await frozen.screenshot({ path: join(out, 'frozen-2.png') });
console.log('frozen-clock screenshots identical:', hash(f1) === hash(f2));

await context.tracing.stop({ path: join(out, 'trace.zip') });
console.log('trace.zip bytes:', statSync(join(out, 'trace.zip')).size);
console.log('private recorder API on 1.64.1:', ['_enableRecorder', '_startRecording', '_stopRecording', '_showRecorder']
  .map((name) => `${name}=${typeof context[name]}`).join(' '));
await context.close();
await browser.close();
const videos = readdirSync(join(out, 'video'));
console.log('video files:', videos.join(', '), '| first bytes:', readFileSync(join(out, 'video', videos[0])).subarray(0, 4).toString('hex'));
// Codec, size and frame rate of the recording (ffprobe from Homebrew).
console.log(execFileSync('ffprobe', ['-v', 'error', '-select_streams', 'v:0',
  '-show_entries', 'stream=codec_name,width,height,r_frame_rate,avg_frame_rate', '-of', 'default=noprint_wrappers=1',
  join(out, 'video', videos[0])], { encoding: 'utf8' }).trim().replace(/\n/g, ' '));
