# mu-147 Persian (fa) translation review

- **Who:** an independent reviewer agent (Claude) that didn't write these translations.
- **When:** 2026-10-15, on the uncommitted working tree of
  `task/mu-147-board-views-backlog-sessions-notifications-and` (base `c494bff`).
- **What:** every key mu-147 added or changed in `app/src/i18n/locales/fa/{board,common,native,shell}.json`
  (about 600 strings, nearly all of them new in `board.json`), against `en/` and
  `docs/i18n/glossary-fa.md`. Two keys outside the diff (`common:kind.chore`, `common:kind.docs`) were
  added because the builders asked about them.
- **Method:** a script flattened both locales at `c494bff` and in the working tree, and listed every
  added or changed key side by side. It also checked for missing or extra keys, `{{placeholder}}` and
  tag sets, Arabic ي/ك, ASCII quotes, Latin `? , ;` after Persian letters, می/نمی without ZWNJ,
  a space before ها, الان, گیت, a suffix after a closing », and ⌘ ⌥ ⇧ ⌃ outside LRI…PDI. Every
  string was then read against its English source. Where the context mattered, I read the component
  that renders it (where it shows, what fills `{{ago}}`/`{{at}}`, whether `_one` can get 0). Strings
  that are fine aren't listed.

**Summary:** no blocking findings. 14 should-fix, 12 nits. The mechanical checks all pass. The two
placeholder differences (`check.step.allProven_one` and `allTicked_one` drop `{{count}}`) are
intentional and safe: the caller only reaches them with one or more criteria (`yourCheck.ts:148`),
so fa's "one" form never gets 0.

## Findings

### Glossary and consistency

**R1. `board:pause.pauseTitle`** (should)
- Current: `فعلاً نه: با همان مرحله و همان برداشتن به لین «متوقف شده» می‌رود، …`
- Problem: "claim" as a noun is «تسک برداشته‌شده» in the glossary, never the bare verbal noun.
  «با همان برداشتن» means "with the same taking" and doesn't parse. What the English means is that
  whoever holds the task still holds it.
- Proposed: `فعلاً نه: با همان مرحله و همان دارنده به لین «متوقف شده» می‌رود، کسی روی آن کار نمی‌کند و از نشستش خواسته می‌شود که دست نگه دارد`

**R2. `board:pause.was`, `board:pause.wasSince` vs `board:views.was`, `board:views.barPaused`** (should)
- Current: `قبلاً {{stage}} · {{ago}} متوقف شد` and `قبلاً {{stage}} · متوقف از {{at}}`, but `پیش‌تر {{stage}}` in views
- Problem: the same English "was {{stage}}" has two words, and «متوقف از» is a truncated participle.
- Proposed: `پیش‌تر {{stage}} · {{ago}} متوقف شد` and `پیش‌تر {{stage}} · متوقف‌شده از {{at}}`

**R3. `board:words.built.needsTester` vs `board:grid.needsTester`** (should)
- Current: `نیازمند تستر` / `نیاز به تستر`
- Problem: same English ("needs a tester"), two forms. «نیاز به …» matches the glossary's «نیاز به شما».
- Proposed: `نیاز به تستر` for both

**R4. `board:lessons.acceptAll_one`, `_other` vs `board:effort.all_one`, `_other`** (should)
- Current: `پذیرفتن هر {{count}} مورد` (lessons) / `پذیرفتن همه ({{count}})` (Your check)
- Problem: same button, two forms. The lessons form also fails at one, which is reachable
  (`Lessons.tsx:144` passes `lessons.length`): «پذیرفتن هر ۱ مورد» reads "accept every 1 item".
- Proposed: `پذیرفتن همه ({{count}})` for both plural forms of both keys

**R5. `board:grid.overLimit`** (should)
- Current: `«بررسی شما» از سقفش گذشته است ({{n}} از {{limit}}): …`
- Problem: the meaning is right, but the glossary fixes "over the limit" as «بیش از سقف», and
  every `check.limit.*` string uses it. The alert should use the same word as the column under it.
- Proposed: `«بررسی شما» بیش از سقف است ({{n}} از {{limit}}): پیش از شروع کارهای بیشتر، چند مورد را بررسی کنید.`

**R6. `board:run.runAgain`, `board:run.runAgainAria`** (should)
- Current: `اجرای مجدد` / `اجرای مجدد {{project}}`
- Problem: the glossary sets restart (a project's run) to «اجرای دوباره».
- Proposed: `اجرای دوباره` / `اجرای دوباره‌ی {{project}}`

**R7. `board:oldPauses.later`** (should)
- Current: `اکنون نه`
- Problem: this calques "not now". The idiom is «فعلاً نه», which `pause.pauseTitle` already uses
  for the same English. The «اکنون» convention replaces الان. It doesn't apply to this idiom.
- Proposed: `فعلاً نه`

### Meaning

**R8. `common:block.waitingOnCapture`, `common:block.detailWaitingOnCapture`** (should)
- Current: `در انتظار capture` / `{{detail}}، در انتظار capture`
- Problem: "capture" is the old name of the session that answers questions (now the lead role, see
  `docs/ADOPTION.md`). It isn't a product name, so in Persian text it's an unexplained English word.
  The glossary word for that role is «سرپرست».
- Proposed: `در انتظار سرپرست` / `{{detail}}، در انتظار سرپرست`
- Note: the English could say "waiting on the lead" too. That's for the en owner.

**R9. `board:newTask.humanHint`** (should)
- Current: `برای انجام به دست خود شما: روی بورد هاشورخورده، با روبان «شما»، و ایجنت‌ها هرگز آن را برنمی‌دارند`
- Problem: «روی بورد هاشورخورده» reads as "on the hatched board": the hatching attaches to the board,
  not the card. The sentence also has no verb for its middle part.
- Proposed: `برای انجام به دست خود شما: روی بورد هاشورخورده و با روبان «شما» نشان داده می‌شود و ایجنت‌ها هرگز آن را برنمی‌دارند`

**R10. `board:sessions.none`** (should)
- Current: `هیچ`
- Problem: this is the whole paragraph in an empty session group (`Sessions.tsx:157`), not a value
  after a heading. Bare «هیچ» is an unfinished word (the glossary's own note on "none").
- Proposed: `نشستی نیست`

**R11. `board:ping.cardTitle`** (should)
- Current: `… و کارت تا وقتی گزارش وضعیت بدهد نشان می‌دهد کی درخواست کرده‌اید`
- Problem: «کی» for "when" is colloquial. mu-146 fixed the same word (its finding 18).
- Proposed: `در صندوق ورودی نشستش می‌نویسد: درخواست را پس از فراخوانی ابزار بعدی‌اش دریافت می‌کند، و کارت تا وقتی گزارش وضعیت بدهد نشان می‌دهد چه زمانی درخواست کرده‌اید`

**R12. `common:kind.chore`** (should; not changed in this task, asked about by the builders)
- Current: `کار جانبی`
- Problem: «کار جانبی» means a side job or moonlighting. A chore task is upkeep: dependency bumps,
  cleanup, config. It's also two words on a card's top row.
- Proposed: `نگهداری` (alternative: `خرده‌کار`)
- `common:kind.docs` «مستندات» is the standard word and only a little wider than "docs" at the card's
  font size. Keep it. The singular «مستند» is shorter, but it reads as "documentary".

### Length (chips, pills, column lines)

**R13. `board:newTask.ai`** (should)
- Current: `هوش مصنوعی`
- Problem: it's one of the two chips in the For pair (`NewTask.tsx:164`), five times the width of
  "AI" and twice the width of its partner «انسان». Its hint already says "an agent's".
- Proposed: `ایجنت`

**R14. `board:recap.kind.pass`** (should)
- Current: `فرستادن به مرحله‌ی بعد`
- Problem: it's the kind label at the start of each history line in a session's recap
  (`SessionRecap.tsx:242`), and 5.5 times the width of "pass". The other kinds are one or two words.
- Proposed: `فرستادن به بعد` (alternative: `فرستادن`, which matches `words.pass.to`'s «فرستادن به «…»»)

**R15. `board:grid.canStart_one`, `_other`, `board:grid.noneCanStart`** (nit)
- Current: `{{count}} مورد می‌تواند شروع شود` / `هیچ‌کدام نمی‌تواند شروع شود`
- Problem: this is Ready's second line in the feature grid's column head (`featureGridData.ts:222`),
  at 1.7–1.9 times the English length.
- Proposed: `{{count}} مورد آماده‌ی شروع` / `هیچ‌کدام آماده‌ی شروع نیست`

**R16. `board:graph.hideDone`** (nit)
- Current: `پنهان کردن انجام‌شده‌ها`
- Problem: it's a toggle pill (`GraphView.tsx:46`), 2.6 times the width of "Hide done".
- Proposed: `بدون انجام‌شده‌ها`

**R17. `board:check.limit.pile*`, `board:check.limit.title*`** (nit)
- Current: `… بیش از سقف {{limit}} که تعیین کرده‌اید`
- Problem: the meaning is right. The relative clause just makes long tooltips longer.
- Proposed: `… بیش از سقف شما ({{limit}})` (e.g. `{{count}} تسک در «بررسی شما» منتظر است، بیش از سقف شما ({{limit}})`)

### Wording

**R18. `board:grid.mine`** (nit)
- Current: `فقط آنچه به من نیاز دارد`
- Problem: the label is first person, but its tooltip and the rest of the app address the user as
  «شما». It's also a pill.
- Proposed: `فقط نیاز به شما`

**R19. `board:grid.backlogWho`** (nit)
- Current: `یادداشت‌ها، هنوز شکل‌دهی نشده`
- Problem: «شکل‌دهی» is the stage's noun. As a passive participle, "shaped" reads more naturally as
  «شکل داده نشده».
- Proposed: `یادداشت‌ها، هنوز شکل داده نشده`

**R20. `board:ribbon.shapedTipTaken`, `board:ribbon.youShapingTaken`** (nit)
- Current: `…، برداشته‌شده {{ago}}`
- Problem: `{{ago}}` is a full phrase («۵ دقیقه پیش», `lib.ts:281`). An adjectival participle
  followed by it reads stiffly. A finite verb reads better.
- Proposed: `در حال شکل‌دهی توسط {{who}}، {{ago}} برداشته شد` / `شما در حال شکل‌دهی آن هستید، {{ago}} برداشته شد`

**R21. `board:lane.moreAbove`, `board:lane.moreBelow`** (nit)
- Current: `⁦+{{count}}⁩ بالا` / `⁦+{{count}}⁩ پایین`
- Problem: «+۳ بالا» reads as "+3 up". The comparative says "further up".
- Proposed: `⁦+{{count}}⁩ بالاتر` / `⁦+{{count}}⁩ پایین‌تر`

**R22. `board:check.step.notProven`** (nit)
- Current: `اثبات‌نشده: {{list}} · برگردانید، یا خودتان بررسی کنید`
- Problem: «برگردانید» has no object. `check.accept.unproven` says «آن را برگردانید».
- Proposed: `اثبات‌نشده: {{list}} · آن را برگردانید، یا خودتان بررسی کنید`

**R23. `board:check.accept.blocked`** (nit)
- Current: `مسدود ({{why}}): پذیرفتن تا رفع آن منتظر می‌ماند.`
- Problem: "Accept" here names the button. The sibling strings quote it («پذیرفتن»), and unquoted
  it reads as a verbal noun.
- Proposed: `مسدود ({{why}}): «پذیرفتن» تا رفع آن منتظر می‌ماند.`

**R24. `board:newTask.bodyPlaceholder`** (nit)
- Current: `… کجای برنامه، و انجام‌شده چه شکلی است. …`
- Problem: «انجام‌شده چه شکلی است» ("done-thing, what shape is it") is a word-for-word rendering.
- Proposed: `جزئیات به Markdown: مراحل بازتولید، کجای برنامه، و کار تمام‌شده چه شکلی دارد. اسکرین‌شات‌ها را هر جای اینجا بچسبانید یا رها کنید.`

**R25. `board:oldPauses.offer_one`, `_other`** (nit)
- Current: `… پیش از آمدن «توقف» متوقف شده بود …`
- Problem: «آمدن» (the coming of) is loose for "before Pause existed".
- Proposed: replace `پیش از آمدن «توقف»` with `پیش از وجود قابلیت «توقف»` in both

**R26. `board:lessons.intro`** (nit)
- Current: `… تا آن موقع فقط پیشنهادند.`
- Problem: «آن موقع» is conversational, and «پیشنهادند» is a contraction.
- Proposed: `… تا آن زمان فقط پیشنهاد هستند.`

## Proposed glossary rows

For "Muster terms". Each marked "(proposed)" unless a source is given.

| Term | Persian | Notes | Source |
|---|---|---|---|
| pass (a task to the next stage) | فرستادن | «فرستادن به «آماده»»; a history line's kind «فرستادن به بعد»; not ارسال, which is Send for a message | (proposed) |
| sent back (state) | برگردانده‌شده | «برگردانده‌شده توسط {{by}}»; the verb is Drover's برگرداندن | Drover (برگرداندن) |
| Paused (the lane) | لین «متوقف شده» | the lane's name uses the pause state; «پیش‌تر {{stage}}» for "was {{stage}}" | (proposed) |
| take over (a claim) | در دست گرفتن | the history kind and «آن را به دست بگیرد» | (proposed) |
| Who does it: Human / AI | انسان / ایجنت | the new-task chip pair; ایجنت, not هوش مصنوعی, on a chip | (proposed) |
| lesson (to approve) | درس | pl. درس‌ها; «درس‌هایی برای تأیید» | (proposed) |
| recap (a session's) | خلاصه | | common usage |
| handoff chain | زنجیره‌ی واگذاری | built from Drover's واگذاری | Drover (واگذاری) |
| dependency / needed first | وابستگی / پیش‌نیاز | the Graph view | common Persian software usage |
| proven / ticked (a criterion) | اثبات‌شده / تیک‌خورده | «اثبات‌نشده» for not proven | (proposed) |
| Your turn | نوبت شما | the Your check card | common usage |
| due (date) | سررسید | | common usage |
| timeline scale | مقیاس | Day / Week / Month: روز / هفته / ماه | common usage |
| chore (task type) | نگهداری | not «کار جانبی» (a side job) | (proposed) |
| docs (task type) | مستندات | not «مستند» (a documentary) | common Persian software usage |
| capture (old name of the lead session) | سرپرست | the word for the role it became; never Latin "capture" in Persian text | Muster (lead) |

## Checked and fine

- **Mechanical:** no missing or extra keys, identical placeholders and tags (`<reason>`, `<b>`,
  `<code>`, `<chip>`, `<words>`) apart from the two intentional `_one` forms above, Persian ی/ک,
  « » quotes, Persian ، ؛ ؟ and ٪, ZWNJ on می/نمی/ها/ezafe, no الان, no گیت, no suffix after ».
- **Shortcuts and bidi:** `⁦⌘↩⁩` in `note.save` and `newTask.create`, and `⁦+{{count}}⁩` in
  `cue.more`/`lane.more*`. The isolate is needed: a leading + before a number is neutral and would
  land on the number's right in RTL. `⁦## Acceptance criteria⁩` in the note's placeholder stays
  Latin because the file format expects that heading.
- **Arrows:** `sessions.chainArrow` → becomes ←, and `levels.parent` ▸ becomes ◂. Settings paths
  read «تنظیمات ← ایجنت‌ها» and «تنظیمات ← پروژه‌ها».
- **Plurals:** every `_one`/`_other` pair reads right for 0, 1 and many. Persian counts take a
  singular noun, so both forms are the same, and the "one" form at 0 («۰ مورد», «۰ تسک») is fine.
- **Times:** `run.secs`/`minSecs`/`hourMins` give units in full words with و, and `time.dAgo` is
  «روز پیش». Every `{{ago}}` is filled with a phrase that already ends in «پیش» (`agoText`), and
  `note.meta`'s `{{ago}}` with a bare span (`ago()`). Both read right.
- **Months:** ژانویه … دسامبر are the standard Iranian spellings of the Gregorian months (مه، ژوئن،
  ژوئیه، اوت). Whether to show the Solar Hijri calendar is mu-151's call.
- **The builders' flagged choices I'd keep as they are:** «⁦+N⁩ دیگر», «جمع کردن» (Fold),
  «پذیرفتن همه (N)» (made the single form in R4), «امتحان کنید» and «بررسی کنید» as step
  imperatives, «اثبات‌نشده», «تیک خورده», «بدون دارنده» (short, and دارنده is the ribbon's
  glossary word), «سازنده: …» and «تستر: …», «بیرونی» and «اقدام شما» as block leads,
  «فرستادن به «…»». Of the others, «برداشته‌شده» is R20, «قبلاً … متوقف شد» is R2,
  «فقط آنچه به من نیاز دارد» is R18, «یادداشت‌ها، هنوز شکل‌دهی نشده» is R19,
  ««بررسی شما» از سقفش گذشته است» is R5, «بیش از سقف … که تعیین کرده‌اید» is R17, «نیازمند تستر»
  is R3, and «مستندات» and «کار جانبی» are R12.
- **App menu (`native:appMenu`):** فایل، ویرایش، نمایش، پنجره، راهنما، درباره‌ی Muster، خدمات،
  پنهان کردن Muster، پنهان کردن بقیه، نمایش همه، خروج از Muster، بستن پنجره، واگرد، ازنو (Mozilla's
  pair with واگرد)، برش، کپی، چسباندن، انتخاب همه، تغییر حالت تمام‌صفحه، کوچک کردن، بزرگ کردن
  (macOS's Zoom resizes the window, so not «بزرگ‌نمایی»). All are standard, infinitive or noun,
  and short enough.
- **Notifications (`native:notify`):** «مورد تازه‌ای برای شما در …»، «{{count}} مورد تازه برای
  شما»، «تسک خودتان»، «آماده‌ی بررسی شما»، «پرسشی از شما دارد»، «در انتظار شما مسدود است» (matches
  mu-146's finding 9)، «به اجازه‌ی شما نیاز دارد» (glossary: اجازه for a request)، «نشستش منتظر
  ورودی شماست»، «درسی برای تأیید». The meaning, register and placeholders are right.
- **Board areas read with no other finding:** the card (`card.*`, `words.*`, `ribbon.*`), the
  Paused lane and pause controls, the column row and its edge cue (`row.*`, `cue.*`, `modes.*`),
  Your check's effort groups and steps (`effort.*`, `check.*` apart from R5, R17, R22 and R23), the
  feature grid's other lines, the Table, Timeline and Graph views (column heads «شناسه»، «تاریخ
  ثبت»، «آخرین تغییر»، «در دست / در انتظار» are short; «اولویت بالا» in `views.high` is right,
  because it stands alone in a "stage · blocked · high" line), the Backlog and note panel, the
  new-task form, Run, Sessions and recap, Ping, Old pauses, Lessons, `common:needs.*` (the glossary's
  «… مورد به شما نیاز دارد»), `common:block.*`, `common:reach.*` and `common:feed.*`.
- **Removed keys:** `board:edge.needYou_*`, `shell:sidebar.needYou_*`, `shell:heading.needYou_*` and
  `shell:heading.needsChip_*` are gone from both locales and no code uses them. `common:needs.*`
  replaces them.

## Fixes (mu-147)

Every finding was applied with the proposed text. None is disputed. The tests that pinned the old
strings now pin the new ones.

- **R1** fixed: «همان دارنده».
- **R2** fixed: `pause.was` and `pause.wasSince` say «پیش‌تر», and «متوقف‌شده از {{at}}».
- **R3** fixed: `words.built.needsTester` is «نیاز به تستر».
- **R4** fixed: `lessons.acceptAll_one`/`_other` are «پذیرفتن همه ({{count}})», like `effort.all`.
- **R5** fixed: «بیش از سقف است».
- **R6** fixed: «اجرای دوباره» and «اجرای دوباره‌ی {{project}}», the same words as `shell:run.restart`.
- **R7** fixed: «فعلاً نه».
- **R8** fixed: «در انتظار سرپرست». "capture" is a role word, not data: `blockWords.ts:103` shows
  the fixed locale string, with nothing from the task filled in. The en text is unchanged.
- **R9** fixed.
- **R10** fixed: «نشستی نیست».
- **R11** fixed: «چه زمانی».
- **R12** fixed: `kind.chore` is «نگهداری». The glossary had no chore row, so nothing conflicted.
- **R13** fixed: «ایجنت», the glossary's word for agent. The glossary had no AI row.
- **R14** fixed: «فرستادن به بعد».
- **R15** fixed: «{{count}} مورد آماده‌ی شروع» and «هیچ‌کدام آماده‌ی شروع نیست».
- **R16** fixed: «بدون انجام‌شده‌ها».
- **R17** fixed: all eight `check.limit.pile*` and `title_*` strings say «بیش از سقف شما ({{limit}})».
- **R18** fixed: «فقط نیاز به شما».
- **R19** fixed: «هنوز شکل داده نشده».
- **R20** fixed: «{{ago}} برداشته شد» in both.
- **R21** fixed: «بالاتر» and «پایین‌تر».
- **R22** fixed: «آن را برگردانید».
- **R23** fixed: ««پذیرفتن» تا رفع آن منتظر می‌ماند».
- **R24** fixed: «کار تمام‌شده چه شکلی دارد».
- **R25** fixed: «پیش از وجود قابلیت «توقف»» in both forms.
- **R26** fixed: «تا آن زمان فقط پیشنهاد هستند».

**Glossary rows added** to `docs/i18n/glossary-fa.md`'s Muster terms, each with (mu-147) in its
notes. Work items: pass, sent back, Paused (the lane), take over, Who does it: Human / AI, chore,
docs, lesson, dependency / needed first, proven / ticked, due. Roles: capture. The shell's
interface words: recap, handoff chain, Your turn, timeline scale. The (proposed) ones are also
listed under Gaps. No row duplicated an existing one.
