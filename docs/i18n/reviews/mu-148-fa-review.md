# mu-148 Persian (fa) translation review

- **Who:** an independent reviewer agent (Claude) that didn't write these translations.
- **When:** 2026-10-15, on the uncommitted working tree of
  `task/mu-148-task-pane-forms-comments-review-and-the-help` (base `a8e1c42`; `main` is at `935f8d0`, with no locale changes since).
- **What:** every key mu-148 added or changed in `app/src/i18n/locales/fa/{pane,help,common,board,shell}.json`
  (462 strings, all of them new: 288 in `pane.json`, 142 in `help.json`, 16 in `board.json`, 15 in
  `common.json` and 1 in `shell.json`), against `en/` and `docs/i18n/glossary-fa.md`.
- **Method:** a script flattened both locales at `main` and in the working tree, and listed every
  added or changed key side by side. It also checked for missing or extra keys, `{{placeholder}}` and
  tag sets, `_one`/`_other` pairs, Arabic ي/ك, ASCII quotes, Latin `? , ;` after Persian letters,
  می/نمی without ZWNJ, a space before ها, الان, گیت, a suffix after a closing », and ⌘ ⌥ ⇧ ⌃ ↩
  outside LRI…PDI. Every string was then read against its English source. Where the context
  mattered, I read the component that renders it: what fills `{{stage}}` (`stageLabel`, a bare
  label), `{{ago}}` (`ago()`, a bare span), the comment hint's commands (`Comments.tsx:143`), the
  Help page's hit line (`Help.tsx:108`), the Feature grid's arrow keys in RTL
  (`featureGridData.ts:301`) and the splitter's (`splitter.ts:33`). Strings that are fine aren't listed.

**Summary:** no blocking findings. 12 should-fix, 13 nits. The mechanical checks all pass: no missing
or extra keys in any namespace, identical placeholders and tags in every string, every plural pair
present, Persian letters, quotes and punctuation, ZWNJ where it belongs, and every ⌘ ⌥ ⇧ ⌃ ↩ isolated.
Three of the should-fix findings are bidi problems the mechanical checks don't cover: a leading `@`
or `+` before a Latin run, and a mirrored chevron.

## Findings

### Glossary and consistency

**R1. Stage names after a preposition: `pane:pane.track.cur`, `pane.track.put`, `pane.track.putReleases`,
`pane.undo`, `pane.moved`, `pane.movedReleased`, `pane.did.passed`, `pane.did.putBack`, `pane.sentBack.to`** (should)
- Current: `اکنون در {{stage}}`, `گذاشتن در {{stage}}`, `واگرد: بازگشت به {{stage}}`, `در {{stage}} گذاشته شد.`,
  `به {{stage}} فرستاده شد.`, `به {{stage}} برگردانده شد.` …
- Problem: `{{stage}}` is the bare stage label (`stageLabel()` in `paneData.ts:137`,
  `useTaskActions.ts:82,102,124`, `StageTrack.tsx:35`). Three of the seven are phrases, so these read
  «اکنون در در حال ساخت», «به در حال تست فرستاده شد», «واگرد: بازگشت به در حال ساخت». The
  established strings quote the stage after a preposition (`board:words.pass.to` «فرستادن به «{{stage}}»»,
  `common:reach.noStageSession` «در «{{stage}}»»), and so does this task's own `side.next.holder`.
- Proposed: `اکنون در «{{stage}}»`, `گذاشتن در «{{stage}}»`, `گذاشتن در «{{stage}}» (تسک برداشته‌شده‌ی {{who}} رها می‌شود)`,
  `واگرد: بازگشت به «{{stage}}»`, `در «{{stage}}» گذاشته شد.`,
  `در «{{stage}}» گذاشته شد. تسک برداشته‌شده‌ی {{who}} در «{{from}}» رها شد.`, `به «{{stage}}» فرستاده شد.`,
  `به «{{stage}}» برگردانده شد.` (both `did.putBack` and `sentBack.to`)

**R2. `help:shortcuts.heading-fold.what`** (should)
- Current: `جمع کردن سرفصل تا نام و شمارش‌هایش …`
- Problem: the glossary sets "count" (the number shown) to «شمار»; «شمارش» is the act of counting.
- Proposed: `جمع کردن سرفصل تا نام و شمارهایش (فلش کوچکش هم همین کار را می‌کند)؛ پروژه‌ی انتخاب‌شده و آن‌هایی که به شما نیاز دارند همچنان رویش دیده می‌شوند`

**R3. `help:manual.commandsError`, `help:manual.loadingCommands`** (should)
- Current: `فرمان‌ها بار نشدند: {{error}}` / `در حال بار کردن فرمان‌ها…`
- Problem: every other "load" in the app is «بارگذاری» (`common:loading`, `pane:review.cantLoad`,
  `pane:vision.loadingBrief`, `board:note.briefFailed`, `shell:about.loading`). «بار کردن» also reads
  as loading cargo.
- Proposed: `فرمان‌ها بارگذاری نشدند: {{error}}` / `در حال بارگذاری فرمان‌ها…`

**R4. `pane:helper.placeholder`** (should)
- Current: `ایده‌ها، باگ‌ها، تسک‌ها… تصویر صفحه را اینجا بچسبانید (⁦⌘↩⁩ برای ارسال)`
- Problem: `board:newTask.bodyPlaceholder` says «اسکرین‌شات‌ها» for the same word, and the English is
  plural.
- Proposed: `ایده‌ها، باگ‌ها، تسک‌ها… اسکرین‌شات‌ها را اینجا بچسبانید (⁦⌘↩⁩ برای ارسال)`

**R5. `pane:pane.unreached.building.note`, `pane.unreached.testing.note`, `help:shortcuts.release-cancel.where`** (should)
- Current: `نشستی برای رسیدن به این تسک برداشته‌شده نیست: …` / `نشستی برای رسیدن به دارنده‌ی این تسک نیست: …` /
  `… روی تسک برداشته‌شده‌ای که به هیچ نشستی نمی‌توان رسید`
- Problem: the same English ("no session to reach") is already «نشستی برای دسترسی نیست» in
  `common:reach.noSession` and `noSessionHeld`, which show next to these in the pane. The third string
  doesn't parse: the relative clause has no link back to the claim ("a claim that one can reach no
  session").
- Proposed: `نشستی برای دسترسی به این تسک برداشته‌شده نیست: «رها کردن» آن را به «آماده» برمی‌گرداند و worktree و شاخه‌اش نگه داشته می‌شوند.` /
  `نشستی برای دسترسی به دارنده‌ی این تسک نیست: «رها کردن» آن را آزاد می‌کند و در «در حال تست» برای تستر بعدی می‌ماند.` /
  `تأیید «رها کردن»، در «گام بعدی» پنل تسک، روی تسک برداشته‌شده‌ای که نشستی برای دسترسی به آن نیست`

**R6. `common:answer.pickTitleWords`** (nit)
- Current: `پاسخ به {{q}} با ({{letter}})، و پس از آن متن داخل کادر`
- Problem: every other "box" in this task is «جعبه» (جعبه‌ی پاسخ، جعبه‌ی دیدگاه، جعبه‌ی جستجو).
- Proposed: `پاسخ به {{q}} با ({{letter}})، و پس از آن متن داخل جعبه`

**R7. `help:slash.what.block`** (nit)
- Current: `مسدود کردن آن، و دلیلش: تسک‌ها، شما، یک اجازه، یک پرسش یا بیرون`
- Problem: it lists the block kinds, and the kind's label is «بیرونی» (`common:block.kind.outside.label`).
- Proposed: `مسدود کردن آن، و دلیلش: تسک‌ها، شما، یک اجازه، یک پرسش یا بیرونی`

**R8. Spelling variants: `pane:pane.note.placeholder`, `pane:pane.track.hint`, `pane:side.next.blockedUnnamed`** (nit)
- Current: `… این‌جا بچسبانید …` / `… تا به آن‌جا برود` / `به خاطر چیزی که …`
- Problem: the rest of the app writes «اینجا» (18 strings) and «آنجا»; this task's other block strings
  write «به‌خاطر» (`pane.block.by`, `did.blockedTasks`, `block.depends`). («به خاطر سپرده می‌شود»,
  "is remembered", is a different phrase and keeps its space.)
- Proposed: `اینجا`, `آنجا`, `به‌خاطر چیزی که نامش را نمی‌برد مسدود است. …`

**R9. `common:answer.answered`** (nit)
- Current: `پاسخ داده شد. ایجنتی که پرسیده پس از گام بعدی‌اش آن را دریافت می‌کند.`
- Problem: «گام بعدی» is now the task pane's Next step section, a few lines away. The step meant here is
  a tool call, which `pane.paused.askedOn` and `board:ping.cardTitle` already say in full.
- Proposed: `پاسخ داده شد. ایجنتی که پرسیده پس از فراخوانی ابزار بعدی‌اش آن را دریافت می‌کند.`

### Bidi

**R10. `pane:comments.hint`** (should)
- Current: `برای انجام کار، با {{accept}}، {{sendback}} <{{why}}>، {{block}} <{{why}}> یا {{ask}} <{{question}}> شروع کنید.`
- Problem: the placeholders are filled with `@accept`, `@sendback`, `@block`, `@ask` (`Comments.tsx:143`)
  in a plain `<p>`. The `@` is neutral and sits between Persian and a Latin word, so it resolves
  right to left and shows after the word: «accept@». The glossary's rule for a Latin run that starts
  with punctuation applies.
- Proposed: put U+200E before each command: `برای انجام کار، با ‎{{accept}}، ‎{{sendback}} <{{why}}>، ‎{{block}} <{{why}}> یا ‎{{ask}} <{{question}}> شروع کنید.`

**R11. `help:manual.hit`** (should)
- Current: `{{title}} ‹`
- Problem: ‹ and › (U+2039, U+203A) are Bidi_Mirrored, so in right-to-left text the renderer already
  draws › as ‹. Swapping the character in the string flips it twice: the breadcrumb between the page
  and its section (`Help.tsx:108`) points back at the page. ▶/◀ in `manual.next`/`prev` and ▸/◂ in
  mu-147 aren't mirrored, so swapping those was right.
- Proposed: `{{title}} ›` (the same as en)

**R12. `help:shortcuts.grid-enter.what`, `grid-leave.where`, `grid-move.what`** (should)
- Current: `«+N دیگر»` in all three
- Problem: the leading + is neutral between Persian and Latin N and lands on N's right: «N+ دیگر».
  mu-147 isolated the same thing in `board:cue.more` and `lane.more*` (`⁦+{{count}}⁩`), and this is the
  label the board shows.
- Proposed: `«⁦+N⁩ دیگر»` in all three

**R13. `help:manual.copied`** (nit)
- Current: `کپی شد: {{text}}`
- Problem: `{{text}}` is a copied command from the manual (`Help.tsx:82`), often `! muster …` or `/mu …`,
  shown as plain text. The leading `!` or `/` lands on the wrong side.
- Proposed: `کپی شد: ⁦{{text}}⁩`

**R14. `pane:pane.gate.placeholder`** (nit)
- Current: `خط نتیجه‌ی دروازه (gate full: green …)`
- Problem: the ellipsis after the Latin run resolves right to left and shows before it: «(… gate full: green)».
- Proposed: `خط نتیجه‌ی دروازه (⁦gate full: green …⁩)`

### Meaning

**R15. `pane:helper.loginTitle`** (should)
- Current: `حساب {{account}} به‌تنهایی وارد نشده است.`
- Problem: «به‌تنهایی» means "alone, unaided", which doesn't say anything. The English means the
  account has no login of its own (it only works with a token), as the body explains.
- Proposed: `حساب {{account}} با ورود خودش وارد نشده است.`

**R16. `pane:helper.loginBody`** (should)
- Current: `… حسابی را که وارد شده انتخاب کنید، یا این یکی را وارد کنید (…).`
- Problem: «این یکی را وارد کنید» reads "enter/import this one", not "log this one in", and «حسابی که
  وارد شده» reads "an account that has been imported".
- Proposed: `… حسابی را انتخاب کنید که در آن وارد شده‌اید، یا با این حساب وارد شوید (Terminal را روی صفحه‌ی ورود خود Claude باز می‌کند).`

**R17. `pane:side.saved.forHuman`** (should)
- Current: `اکنون انجامش با شماست: روی بورد هاشورخورده، با روبان «شما».`
- Problem: the same slip mu-147 fixed in `board:newTask.humanHint` (its R9): «بورد هاشورخورده» reads
  "the hatched board", and the half after the colon has no verb.
- Proposed: `اکنون انجامش با شماست: کارتش روی بورد هاشورخورده است و روبان «شما» دارد.`

**R18. `pane:pane.release.ready.does`, `ready.title`, `shaping.does`, `shaping.title`** (should)
- Current: `رها کردن آن را به «آماده» برمی‌گرداند و در لاگ بازپس‌گرفته ثبت می‌شود.` / `رها کردن آن را آزاد می‌کند، …`
- Problem: "Release" names the button; unquoted, «رها کردن آن را آزاد می‌کند» reads "releasing it frees
  it". The help page quotes it («رها کردن»). "logged as reclaimed" names the log word; a bare
  «بازپس‌گرفته» after «در لاگ» reads as an adjective of the log line.
- Proposed: `«رها کردن» آن را به «آماده» برمی‌گرداند و در لاگ با عنوان «بازپس‌گرفته» ثبت می‌شود.` /
  `بازگشت به «آماده»؛ تسک برداشته‌شده رها و در لاگ با عنوان «بازپس‌گرفته» ثبت می‌شود` /
  `«رها کردن» آن را آزاد می‌کند، همچنان در «شکل‌دهی»، و در لاگ با عنوان «بازپس‌گرفته» ثبت می‌شود.` /
  `بی‌دارنده، همچنان در «شکل‌دهی»؛ تسک برداشته‌شده رها و در لاگ با عنوان «بازپس‌گرفته» ثبت می‌شود`

**R19. `pane:stage.work.notTimed`** (nit)
- Current: `زمان‌گیری نشده: در لاگ ورودی به «در حال ساخت» نیست`
- Problem: «ورودی» is the app's word for "input" (`pane.waiting.input` «ورودی در Claude Code»), so
  this reads "there's no input to Building".
- Proposed: `زمان‌گیری نشده: در لاگ جابه‌جایی‌ای به «در حال ساخت» نیست`

**R20. `pane:pane.status.quiet`** (nit)
- Current: `ساکت، {{ago}} ·`
- Problem: `{{ago}}` is a bare span (`ago()`, «۵ دقیقه»), so «ساکت، ۵ دقیقه» doesn't say since when.
- Proposed: `ساکت از {{ago}} پیش ·`

### Wording

**R21. `pane:check.left_one`, `_other`, `pane:check.allTicked`** (nit)
- Current: `برای بررسی شما: {{count}} مورد مانده، سپس پذیرفتن.` / `… برای پایان، بپذیرید.`
- Problem: "Accept" names the button. Unquoted «سپس پذیرفتن» is a dangling verbal noun. mu-147 quoted it
  the same way (its R23).
- Proposed: `برای بررسی شما: {{count}} مورد مانده، سپس «پذیرفتن».` / `همه‌ی موارد شما تیک خورده است. برای پایان، «پذیرفتن» را بزنید.`

**R22. `pane:helper.stepsFiled_one`, `_other`** (nit)
- Current: `{{count}} گام · تسک ثبت شد`
- Problem: "filed tasks" is plural; «تسک ثبت شد» reads "task filed".
- Proposed: `{{count}} گام · تسک‌ها ثبت شد`

**R23. `pane:pane.did.started`** (nit)
- Current: `شروع شد: در «در حال انجام» است.`
- Problem: «در «در حال انجام» است» stacks two در.
- Proposed: `شروع شد: به «در حال انجام» رفت.`

**R24. `pane:pane.paused.resumed`** (nit)
- Current: `ادامه یافت: به ستونش برگشته و همان‌طور که بود ادامه می‌یابد.`
- Problem: «ادامه» twice in one line.
- Proposed: `ادامه یافت: به ستونش برگشته و همان‌طور که بود پیش می‌رود.`

**R25. `pane:side.next.holder`** (nit)
- Current: `… برای جابه‌جا کردنش خودتان، روی یک مرحله در نوار مراحل کلیک کنید.`
- Problem: «جابه‌جا کردنش خودتان» is the English word order.
- Proposed: `{{holder}} آن را در «{{stage}}» در دست دارد، پس فقط او می‌تواند آن را به مرحله‌ی بعد بفرستد. برای اینکه خودتان جابه‌جایش کنید، روی یک مرحله در نوار مراحل کلیک کنید.`

## Proposed glossary rows

New terms the translators used that the glossary doesn't have, with my verdict. All are used the
same way in every namespace this task touched.

| Term | Persian | Verdict | Source |
|---|---|---|---|
| comment / Comments | دیدگاه / دیدگاه‌ها | keep; «ثبت دیدگاه» for the button, «جعبه‌ی دیدگاه»; نظر would collide with opinion in prose | Mozilla, common Persian web usage |
| comment thread | رشته‌ی دیدگاه‌ها | keep | common usage |
| Next step (the pane's section) | گام بعدی | keep; then «گام» shouldn't mean a tool call nearby (R9) | (proposed) |
| finding (a review's) | یافته | keep; «یافته‌ی مالک» for an owner finding | common usage |
| clean (a review) | بدون یافته | keep; says what clean means | (proposed) |
| review round | دور | keep; «دور ۲ از ۳» | common usage |
| re-check | بررسی دوباره | keep | common usage |
| review / vision brief | شرح کار بازبینی / شرح کار چشم‌انداز | keep; built from the glossary's شرح کار | Drover (شرح کار) |
| vision conflict | تعارض | keep; the merge word, and it means the same thing | Pro Git fa (تعارض) |
| write-up (a task's) | متن تسک | keep | (proposed) |
| shortcut | میان‌بر | keep; «میان‌برهای صفحه‌کلید» | Mozilla, Microsoft |
| slash command / route / router | فرمان اسلش / مسیر / مسیریاب | keep | (proposed) |
| reclaimed (a claim, in the log) | بازپس‌گرفته | keep, quoted as the log word (R18) | (proposed) |
| Release (a claim) | رها کردن | already in the glossary; quote it when it names the button | glossary |
| unheld | بی‌دارنده | keep; built from دارنده | (proposed) |
| checklist (yours) | فهرست بررسی | keep | common usage |
| proof / proven | اثبات / اثبات‌شده | keep; matches mu-147's row | glossary (mu-147) |
| roll-up | جمع‌بندی | keep | common usage |
| Raw / Rendered | خام / قالب‌بندی‌شده | keep | common Persian software usage |
| pick (the asker's recommended option) | پیشنهادی | keep; says what the badge means | (proposed) |
| conversation (the helper's) | گفت‌وگو | keep | common usage |
| tab | زبانه | keep | Mozilla |
| cell (the Feature grid's) | خانه | keep | common usage (table cell) |
| caret / focus (keyboard) | مکان‌نما / فوکوس | keep; فوکوس stays apart from the Focus display mode (تمرکز) | Microsoft (مکان‌نما) |
| load | بارگذاری | keep, everywhere (R3) | Mozilla |
| screenshot | اسکرین‌شات | keep, everywhere (R4) | common Persian software usage |
| box (answer, comment, search) | جعبه | keep, everywhere (R6) | common usage |
| the core (Muster's) | هسته‌ی برنامه | keep | common usage |

## Checked and fine

- **Mechanical:** no missing or extra keys in any of the five namespaces, identical placeholders and
  tags (`<code>`, `<who>`) in every string, every `_one` with its `_other`, Persian ی/ک, « » quotes
  (`helper.empty`'s “ ” became « »), Persian ، ؛ ؟ and ٪, ZWNJ on می/نمی/ها/ezafe, no الان, no گیت,
  no suffix after ».
- **Shortcuts and bidi:** every ⌘ ⌥ ↩ run is isolated (`⁦⌘↩⁩`, `⁦⌘S⁩`, `⁦⌘⁩`, `⁦⌥⁩`), and the help
  page isolates Esc, Tab, Home, End, Space and `,` too, which is harmless. Code spans in
  `page.slashIntro` render in `<code>`, which `styles.css:84` isolates. `pane.sentBack.notTold` has an
  LRM before `muster next`. `stage.history.move` turns → into ←, and `side.estimate.tokensTitle` keeps
  `150k`, `1.2M` Latin because they show what the field takes.
- **Arrow keys in RTL:** `grid-move.what` drops the English "(← and → swap in right-to-left)" and states
  the RTL keys directly; `featureGridData.ts:301` confirms ← is forward and → goes from Backlog to the
  feature's name. `resize-pane.what`'s «→ به راست، ← به چپ» is right too: `splitter.ts:33` doesn't swap.
- **Plurals:** every pair reads right for 0, 1 and many; Persian counts take a singular noun.
- **Times:** `pane.header.changed` («{{ago}} پیش تغییر کرد») gets a bare span from `ago()`, so it reads
  right; `changedNow` covers under a minute.
- **The task pane's other areas** read with no other finding: the header and status line, the stage
  track, Send back and Block forms (`pane.block.by` «مسدود به‌خاطر» is a label before the kind chips,
  which reads fine), the gate form, check-ins and log, Paused, Your check's checklist and standings,
  the side column (branch, files, links, For, priority, type, tags, estimates), Review and Vision, and
  the helper apart from R4, R15, R16 and R22.
- **Help:** page names, the shortcuts table's area names and its what/where text apart from R2, R5 and
  R12, the slash command list (who: ایجنت، مالک، سیستم), and the manual's navigation (▶/◀ swapped
  correctly).
- **common, board, shell:** the answer box (`answer.*` apart from R6 and R9), tag editor, review and
  vision badges (`board:badge.*`; «بازبینی بدون یافته · دور {{round}}» is short enough for a card),
  `board:branch.worktree` (Worktree stays Latin per the glossary), and `shell:app.visionUnchecked`.

## Fixes

- **R1** fixed: the stage (and `{{from}}`) is quoted after در and به in all nine strings.
- **R2** fixed: «شمارهایش».
- **R3** fixed: «بارگذاری» in both.
- **R4** fixed: «اسکرین‌شات‌ها».
- **R5** fixed: «نشستی برای دسترسی …» in all three, with «رها کردن» quoted in the two notes.
- **R6** fixed: «جعبه».
- **R7** fixed: «بیرونی».
- **R8** fixed: «اینجا», «آنجا», «به‌خاطر».
- **R9** fixed: «فراخوانی ابزار بعدی‌اش».
- **R10** fixed: an LRM before each @ command; `talkFa.test.tsx` asserts it.
- **R11** fixed: `manual.hit` keeps ›; `Help.tsx` says why at the hit list, and `helpFa.test.tsx` asserts › and no ‹. ◀ ▶ stay swapped.
- **R12** fixed: «⁦+N⁩ دیگر» in all three.
- **R13** fixed: `{{text}}` inside LRI…PDI.
- **R14** fixed: the Latin run inside LRI…PDI.
- **R15** fixed as proposed.
- **R16** fixed as proposed.
- **R17** fixed as proposed.
- **R18** fixed: «رها کردن» quoted where it names the button, «با عنوان «بازپس‌گرفته»» in all four.
- **R19** fixed: «جابه‌جایی‌ای».
- **R20** fixed differently: «{{ago}} ساکت ·». `ago()` returns «اکنون» under a minute, which made the proposal's «ساکت از اکنون پیش».
- **R21** fixed: «پذیرفتن» quoted in all three.
- **R22** fixed: «تسک‌ها ثبت شد».
- **R23** fixed: «به «در حال انجام» رفت».
- **R24** fixed: «پیش می‌رود».
- **R25** fixed as proposed.
- **Glossary:** the mu-148 table notes the quoted stage, the quoted log word and «فراخوانی ابزار بعدی», and gains comment thread, review / vision brief, Release (the button), no session to reach, unheld, checklist, caret / focus, load, screenshot, box and the core. Comment, finding, round, shortcut and the rest were already there.
