# Glossary: Persian (fa), right to left

Task 0146. Copied from Drover's glossary (`docs/i18n/glossary-fa.md` at Drover commit `a990fc8`,
Drover task 0007), so the two apps read as one family: same conventions, same word for every term
they share. Muster adds its own terms in "Muster terms" below. Where Drover settled a word, Muster
uses it; a change to a shared word is made in both glossaries.

Drover's note on its sources: Git words follow the Persian edition of Pro Git; software and AI
words are mostly transliterated in Persian dev media (Zoomit, CodeVelocity); interface words follow
Mozilla's Persian terminology and Gitea's Persian locale. "Drover's own" means no source had the
term and Drover's translation review chose it. "(proposed)" in Muster's section means the same for
Muster: no source has it, this glossary chose it, and a native-speaking review still has to
settle it.

## Conventions

- **Direction:** `lang="fa" dir="rtl"` on the root. Latin runs (branch names, paths, commands,
  task ids like mu-146) isolated with `<bdi>` or `dir="ltr"`; code is always left to right. A Latin
  run that starts with punctuation (`.muster/config.toml`, `~/.ssh`, `/mu`) gets a left-to-right
  mark (U+200E) before it in the text, or the dot or slash lands on the wrong side.
- **Keyboard shortcuts:** a shortcut run written into Persian text (⌘N, ⌘., ⌘+, ⌘−, ⌘0, ⌘F,
  ⌥↑ ⌥↓, anything with ⌘ ⌥ ⇧ ⌃) sits between a left-to-right isolate and its pop (LRI U+2066 …
  PDI U+2069): «تسک جدید (⁦⌘N⁩)». The glyphs, the dot, plus, minus and arrows are all
  bidi-neutral, so without it «(⌘N)» shows as «(N⌘)» and «⌥↑ ⌥↓» as «↓⌥ ↑⌥». Two shortcuts
  given as a pair («⌥↑ ⌥↓») share one isolate. `app/src/i18n/bidi.test.ts` fails on a bare one.
- **ZWNJ (U+200C):** before the plural ها (کامیت‌ها، تسک‌ها), in می‌/نمی‌, and before ی ezafe
  after ه (شاخه‌ی). Compound verbs use a normal space (کامیت کردن).
- **Quotes:** « »; punctuation ، ؛ ؟ and ٪.
- **Digits:** not settled here. Drover writes Persian digits (۰–۹) for every number in Persian
  text, through `Intl.NumberFormat("fa")`; whether Muster does the same is decided in task mu-151.
  The examples below use Persian digits only because they're Drover's. Keyboard glyphs (⌘N, ⌘.),
  code, commands, paths, task ids, colour values (#1a2b3c) and anything inside `<code>` stay Latin
  either way.
- **Letters:** Persian ی and ک (U+06CC, U+06A9), not Arabic ي and ك; search matches both.
- **Address:** formal plural (کنید، دارید) everywhere, in the UI and to agents. Buttons and menu
  items are infinitives or nouns (باز کردن، بایگانی), confirmations are passive questions
  (… حذف شود؟). No colloquial forms (جور نیست، چند تا، گیر کرده).
- **Times:** units in full words (۵ دقیقه، ۲ ساعت و ۱۰ دقیقه). Persian has no established short
  forms like "5m", and an invented one (۵د) reads as a typo. A compound time takes و between
  units.
- **No italics or letter-spacing on Persian text.** Keyboard shortcuts use the Latin layout.
- **Arrows:** "→" for "go to/then" (Settings → Display) becomes ← in Persian text, which reads
  right to left; a back link's "←" becomes →. Arrows that name keys (← → in "Previous (←)") stay
  as the keys are.
- **Product names stay as they are, in Latin:** Muster, Drover, GitHub, Claude Code, Codex, macOS.
  They're isolated like any Latin run. The computer itself is مک in Persian script («این مک»,
  «اعلان‌های مک»); only the operating system's name, macOS, stays Latin. (Drover's README: "names (Drover, GitHub, Claude Code)"
  stay as they are.)
- **Never use گیت for "gate":** it's how Persian spells Git. The gate is دروازه.
- **Now:** اکنون, never the informal الان.
- **No suffix after a closing »:** «زنده»ی or «بررسی شما»یشان renders badly and reads worse.
  Rephrase so the quote ends the word: «نشان «زنده» در پنل تسک», «سقف «بررسی شما» برای هر کدام».

## Terms (from Drover)

Drover's table as of `a990fc8`. Terms Muster doesn't use are kept so the shared words stay in one
place; Muster's own entries further down don't override any of these.

| Term | Persian | Notes | Source |
|---|---|---|---|
| agent | ایجنت | pl. ایجنت‌ها; formal عامل | Zoomit |
| session | نشست | pl. نشست‌ها; جلسه is kept for "meeting" | fa.wikipedia |
| subagent | زیرایجنت | Drover's own: built like ایجنت, the way Claude Code's "subagent" is built | — |
| lane | لین | Drover's own: transliterated like the other dev nouns (ایجنت، کامیت); مسیر and خط are too common to read as a term | — |
| worktree | worktree (Latin) | pl. worktree‌ها | Pro Git fa |
| branch | شاخه | pl. شاخه‌ها | Pro Git fa |
| main checkout | چک‌اوت اصلی | | Pro Git fa (چک‌اوت) |
| project | پروژه | | Gitea fa |
| space | فضا | Drover's own: not فضای کاری (workspace) | — |
| ask | درخواست | Drover's own: the everyday word for a request | — |
| plan | طرح | پلن is kept for a pricing plan (accounts) | Zoomit |
| piece | بخش | Drover's own | — |
| epic | اپیک | Drover's own: transliterated, as Persian Jira users say it | — |
| contract | قرارداد | Drover's own; "conventions" in project notes is قراردادهای کدنویسی to keep them apart | — |
| gate | دروازه | Drover's own: not گیت (Git) | — |
| handoff / hand off | واگذاری / واگذار کردن | Drover's own; the document an agent writes is یادداشت واگذاری | — |
| context | کانتکست | not Mozilla's محتوا (content) | Zoomit |
| context window | پنجره‌ی کانتکست | | Zoomit |
| token | توکن | pl. توکن‌ها | CodeVelocity |
| ledger | دفتر کل | Drover's own: the accounting term, which is what "ledger" names | — |
| outline | فهرست پرامپت‌ها | Drover's own: says what the tab lists; the panel's label is فهرست پرامپت‌های نشست | — |
| inbox | صندوق ورودی | Drover's own, from mail clients | — |
| report | گزارش | | Mozilla |
| output | خروجی | Drover's own | — |
| task creator | سازنده‌ی تسک | Drover's own | — |
| tracker | ردیاب | Drover's own | — |
| guard rails | حفاظ‌ها | Drover's own; each rail's name is a noun phrase (دست نزدن به اطلاعات محرمانه) | — |
| trash | سطل زباله | Drover's own, as Persian desktop users call the macOS Trash | — |
| commit | کامیت / کامیت کردن | pl. کامیت‌ها | Pro Git fa |
| push | پوش / پوش کردن | Drover's own: not Gitea's درج کردن, which Persian developers don't say | — |
| pull | دریافت | Drover's own: as in "n to pull" | — |
| remote | ریموت | Drover's own: transliterated, as developers say it | — |
| pull request | پول ریکوئست, PR | Gitea says درخواست ادغام | Kilo PR #6837, Pars Host |
| draft pull request | پول ریکوئست پیش‌نویس | | Gitea (پیش‌نویس) |
| review | بازبینی | | Gitea, Mozilla |
| reviewer | بازبین | | Gitea |
| diff / changes | Diff / تغییرات | | Gitea |
| base branch | شاخه‌ی پایه | | Faradars |
| stage / unstage (git) | استیج کردن / خارج کردن از استیج | Drover's own; the button is استیج / خروج از استیج. Git's staging only: a Muster stage is مرحله (below) | Pro Git fa |
| merge | ادغام / ادغام کردن | | Pro Git fa |
| conflict | تعارض | تعارض ادغام | Pro Git fa |
| check (CI) | بررسی خودکار | Drover's own: keeps it apart from review and from the person's own check (بررسی) | — |
| working | در حال کار | Drover's own | — |
| needs you | نیاز به شما | Drover's own; a count reads «۳ مورد به شما نیاز دارد» | — |
| done | انجام شد | | Gitea |
| idle | بی‌کار | Drover's own | — |
| ready to check | آماده‌ی بررسی | | Zoomit |
| allow / deny | اجازه / رد | Drover's own; "Allow once" is یک بار اجازه | Gitea (رد) |
| permission | اجازه (a request for it) / مجوز (a setting) | درخواست اجازه for a permission prompt; مجوزها for the settings list | Mozilla |
| approve | تأیید | | Gitea |
| answer | پاسخ | | — |
| send back | برگرداندن | Drover's own | — |
| accept | پذیرفتن | | Gitea |
| drop | کنار گذاشتن | Drover's own | — |
| archive | بایگانی / بایگانی کردن | | Gitea |
| sleep / wake | خواباندن / بیدار کردن | Drover's own: one word that fits a menu item | — |
| resume | ادامه | | — |
| start the plan | شروع طرح | Drover's own | — |
| steer | هدایت | Drover's own | — |
| Home | خانه | | Gitea, Mozilla |
| Agents | ایجنت‌ها | | — |
| Reviews | بازبینی‌ها | | — |
| Settings | تنظیمات | | Mozilla, Gitea |
| Terminal / Blocks | ترمینال / بلاک‌ها | Drover's two ways a session shows | Zoomit |
| menu bar panel | پنل نوار منو | Drover's own | — |

### More terms, from Drover's translation

| Term | Persian | Notes |
|---|---|---|
| command | فرمان | as in "Start command"; Mozilla |
| investigate | کندوکاو | the session type and the step; بررسی is taken by "check" |
| research | پژوهش | |
| prompt | پرامپت | Zoomit |
| plugin / skill / hook | پلاگین / اسکیل / هوک | Claude Code's names, transliterated |
| brief | شرح کار | |
| shell | شل | |
| secrets | اطلاعات محرمانه | |
| snapshot | اسنپ‌شات | |
| revert | بازگردانی | Mozilla |
| restore | بازیابی | Mozilla |
| discard | دور ریختن | |
| clean up | پاک‌سازی | |
| undo | واگرد | Mozilla |
| theme / look / Appearance | تم / سبک / ظاهر | |
| accent | رنگ تأکیدی | |
| hover (colour) | زیر نشانگر | not شناور, which means "floating" |
| severity high / medium / low | بالا / متوسط / پایین | |
| skipped (a step) | از … صرف‌نظر شد | not رد شد, which is "deny" |
| the person | کاربر | in agent text |
| owner | مالک | in agent text |

## Muster terms

Muster's own words. Sources: Drover's glossary (above), Mozilla's Persian terminology, Apple's and
Microsoft's Persian UI wording where they have the term, Gitea's Persian locale, and common Persian
software usage (Persian Scrum and Jira users, dev media). "(proposed)" marks this glossary's own
coinage, open until a native-speaking review settles it.

### Stages (the board's columns)

A stage is مرحله. Column labels are short nouns or states; the same word is used in the chip, the
column header, the stage track and the task's detail.

| Term | Persian | Notes | Source |
|---|---|---|---|
| Backlog | بک‌لاگ | transliterated, as Persian Scrum and Jira teams say it; فهرست انتظار reads as a hospital list | common Persian software usage |
| Shaping | شکل‌دهی | the stage where a task is written up; not طراحی (design) and not طرح (Drover's plan) | (proposed) |
| Ready | آماده | | Gitea, Mozilla |
| Building | در حال ساخت | a state, like Drover's در حال کار | (proposed) |
| Testing | در حال تست | تست, as Persian developers say it; آزمون reads as an exam | common Persian software usage |
| Your check | بررسی شما | Drover's بررسی for the person's own check; same orange as «نیاز به شما», which is the same thing | Drover (بررسی) |
| Done | انجام شد | Drover's word; "Recently finished" is «اخیراً انجام شد» | Drover, Gitea |

### Work items and the queue

| Term | Persian | Notes | Source |
|---|---|---|---|
| task | تسک | pl. تسک‌ها; Drover already uses it (سازنده‌ی تسک); وظیفه is a duty, not a ticket | Drover, common Persian software usage |
| goal | هدف | pl. اهداف in prose, هدف‌ها in labels | common usage |
| feature | قابلیت | pl. قابلیت‌ها; ویژگی is kept for "property/attribute" | common Persian software usage |
| idea | ایده | pl. ایده‌ها; an older task type (mu-194), said like task and feature but never offered; فکر is a thought, not a proposal | common Persian usage |
| claim | برداشتن | a session takes a task: «برداشتن تسک»; state «برداشته شده توسط اصغر». Not واگذاری (Drover's handoff). Not for "remove" either: that's حذف | (proposed) |
| claim (noun) | تسک برداشته‌شده | pl. تسک‌های برداشته‌شده; «برداشتن» in compounds («مهلت برداشتن»). Never bare برداشت, which reads as a withdrawal or an interpretation, and not برداشتنی | (proposed) |
| lease | مهلت | how long a claim holds without a heartbeat; «مهلت تمام شد». اجاره is the literal, financial sense | (proposed) |
| gate | دروازه | the repo's check script (`bin/gate`); never گیت | Drover |
| note | یادداشت | pl. یادداشت‌ها | Mozilla |
| lens | زاویه‌ی دید | pl. زاویه‌های دید; the shaping concerns (a11y, i18n, privacy…). لنز reads as a camera lens | (proposed) |
| blocked | مسدود | state «مسدود شده»; "unblock" is رفع انسداد | Mozilla, Gitea |
| needs you | نیاز به شما | | Drover |
| display modes | حالت‌های نمایش | singular حالت نمایش (Drover uses it for a Blocks reading style, which Muster doesn't have). The modes: ADHD stays Latin, Standard استاندارد, Focus تمرکز | Drover (حالت نمایش), Apple (تمرکز for Focus) |
| queue | صف | «صف تسک‌ها» | Mozilla, Microsoft |
| stage | مرحله | pl. مراحل; never استیج, which is Git's staging in Drover | common usage |
| priority | اولویت | | Gitea, Mozilla |
| check-in | گزارش وضعیت | a worker's progress line; verb گزارش وضعیت دادن | (proposed) |
| Log (the task pane's section) | لاگ | transliterated, as developers say it; گزارش is taken by report and گزارش وضعیت by check-in | common Persian software usage |
| heartbeat | ضربان | the stamp that keeps a claim alive; «آخرین ضربان: ۵ دقیقه پیش» | (proposed) |
| pause | توقف | button توقف, state «متوقف شده»; resume is Drover's ادامه | Apple, Mozilla |
| review | بازبینی | | Drover |
| criteria / acceptance criterion | معیارها / معیار پذیرش | pl. معیارهای پذیرش | common Persian software usage |
| worktree | worktree (Latin) | pl. worktree‌ها | Drover, Pro Git fa |
| pass (a task to the next stage) | فرستادن | «فرستادن به «آماده»»; a history line's kind «فرستادن به بعد»; not ارسال, which is Send for a message (mu-147) | (proposed) |
| sent back (state) | برگردانده‌شده | «برگردانده‌شده توسط {{by}}»; the verb is Drover's برگرداندن (mu-147) | Drover (برگرداندن) |
| Paused (the lane) | لین «متوقف شده» | the lane's name uses the pause state; «پیش‌تر {{stage}}» for "was {{stage}}" (mu-147) | (proposed) |
| take over (a claim) | در دست گرفتن | the history kind and «آن را به دست بگیرد» (mu-147) | (proposed) |
| Who does it: Human / AI | انسان / ایجنت | the new-task chip pair; ایجنت, not هوش مصنوعی, on a chip (mu-147) | (proposed) |
| chore (task type) | نگهداری | not «کار جانبی» (a side job) (mu-147) | (proposed) |
| docs (task type) | مستندات | not «مستند» (a documentary) (mu-147) | common Persian software usage |
| lesson (to approve) | درس | pl. درس‌ها; «درس‌هایی برای تأیید» (mu-147) | (proposed) |
| dependency / needed first | وابستگی / پیش‌نیاز | the Graph view (mu-147) | common Persian software usage |
| proven / ticked (a criterion) | اثبات‌شده / تیک‌خورده | «اثبات‌نشده» for not proven (mu-147) | (proposed) |
| due (date) | سررسید | (mu-147) | common usage |

### Roles

| Term | Persian | Notes | Source |
|---|---|---|---|
| builder | سازنده | pl. سازنده‌ها; the session that builds a task. Drover's سازنده‌ی تسک is a different role and keeps its full form | (proposed) |
| tester | تستر | pl. تسترها; as Persian QA teams say it | common Persian software usage |
| shaper | شکل‌دهنده | the session that writes up tasks; built from شکل‌دهی | (proposed) |
| lead / coordinator | سرپرست / هماهنگ‌کننده | the one session that runs the others; سرپرست on short labels | common usage |
| owner | مالک | the person who owns the queue | Drover |
| role | نقش | pl. نقش‌ها; a role profile in roles.toml (0112) | common usage |
| slot | جایگاه | pl. جایگاه‌ها; a role's numbered place (Kaveh-2) that a session claims | (proposed) |
| capture (old name of the lead session) | سرپرست | the word for the role it became; never Latin "capture" in Persian text (mu-147) | Muster (lead) |

The role titles in Settings › Agents › Roles (mu-195): the words for the core's eleven roles,
shell.json `roles.title.<id>`. Shaper, tester (QA tester) and lead use the words above. The role's
id (`coder`) stays Latin: it's what `--role` takes.

| Term | Persian | Notes | Source |
|---|---|---|---|
| architect | معمار | the role that reviews a shaped task's architecture | common usage |
| designer | طراح | the role that reviews a shaped task's UI; built from طراحی (design) | common usage |
| coder | برنامه‌نویس | the role that builds; سازنده stays the word for the session building a task | common usage |
| researcher | پژوهشگر | built from پژوهش (research) | common usage |
| security reviewer | بازبین امنیت | بازبین (reviewer) + امنیت | (proposed) |
| performance reviewer | بازبین کارایی | کارایی for performance, as in Persian software writing | (proposed) |
| accessibility reviewer | بازبین دسترس‌پذیری | دسترس‌پذیری for accessibility, as in Persian web writing | (proposed) |
| release and merge (the role) | انتشار و ادغام | the role that rebases, gates and merges; not «رها کردن», which is releasing a claim | (proposed) |

The permission profiles in a role's opened profile (mu-195), shell.json `roles.permission.<id>`.
Its stage and stage moves use the stage words above; a move reads `آماده ← در حال ساخت`, the arrow
pointing the way a right-to-left line is read.

| Term | Persian | Notes | Source |
|---|---|---|---|
| read-only | فقط‌خواندنی | reads and reports | common Persian software usage |
| shaping | شکل‌دهی | writes the queue; the Shaping stage's word | (proposed) |
| research | پژوهش | writes only its own folders | common usage |
| worktree | worktree (Latin) | edits the task's worktree; Latin, as worktree above | Drover, Pro Git fa |
| release | انتشار | rebases, gates and merges; as the release role, not «رها کردن» | (proposed) |
| lead | سرپرستی | answers and escalates; built from سرپرست | (proposed) |

### Names

Role and slot names are names: they are never translated, only written in Persian script in fa.
Latin script stays in commands, task fields (`claimed-by:`) and anything an agent types.

| Term | Persian | Notes | Source |
|---|---|---|---|
| Shapour | شاپور | the shaper | Persian name |
| Kaveh | کاوه | | Persian name |
| Kia | کیا | | Persian name |
| Asghar | اصغر | a builder | Persian name |
| Gholi | قلی | a builder | Persian name |

### The shell's interface words

| Term | Persian | Notes | Source |
|---|---|---|---|
| project | پروژه | | Drover, Gitea |
| sidebar | نوار کناری | | Apple, Microsoft |
| filter | فیلتر | Mozilla's پالایه is rare in product UI | common Persian software usage |
| theme | تم | | Drover |
| accent | رنگ تأکیدی | | Drover |
| light | روشن | | Apple, Microsoft |
| dark | تیره | not تاریک | Apple, Microsoft |
| notifications | اعلان‌ها | | Apple, Mozilla |
| display | نمایش | Settings › Display is تنظیمات ← نمایش | Apple |
| recent | اخیر | a list: «موارد اخیر» | Mozilla |
| waiting | در انتظار | | Gitea, Mozilla |
| settings | تنظیمات | | Drover, Mozilla |
| language | زبان | | Mozilla, Apple |
| board | بورد | the Board view; transliterated as Persian Scrum and Kanban teams say it, like بک‌لاگ; تابلو reads as a sign or a painting. برد is the other common spelling (Trello users write it), but it's a homograph of برد (range, win, "took"); بورد is used everywhere, and the owner's native-speaker check settles بورد or برد | (proposed) |
| table / timeline / graph | جدول / خط زمانی / گراف | the other project views | (proposed) |
| layout: Columns / Feature grid | چیدمان: ستون‌ها / شبکه‌ی قابلیت‌ها | the board's two layouts (mu-143); شبکه, not جدول, which is the Table view | (proposed) |
| helper | دستیار | the AI helper and its tab | (proposed) |
| activity / problems / help | فعالیت / مشکلات / راهنما | the header's tabs | Mozilla (راهنما) |
| view (saved) | نما | «نماهای ذخیره‌شده»; the header's views are نماها too | (proposed) |
| card | کارت | a task on the board | common Persian software usage |
| holder ribbon | روبان دارنده | the card corner naming who holds the task (0097); دارنده is the one holding it | (proposed) |
| chip | چیپ | a small pill button (the mode chip, a column folded to a chip); transliterated like تگ and کارت. Not تراشه, which reads first as a microchip (review of 2026-10-14) | (proposed) |
| past the edge (the board's) | بیرون از لبه‌ی بورد | the cue naming the open columns the board's scroll hides (mu-198): «بیرون از لبه‌ی بورد: بررسی شما · ۲۱ مورد به شما نیاز دارد، انجام شد»; لبه, not حاشیه (a margin) | (proposed) |
| tag | تگ | pl. تگ‌ها; برچسب is kept for labels | common Persian software usage |
| label (a project's) | نام نمایشی | what the window calls a project; its folder's name stays as it is | (proposed) |
| category / heading | دسته / سرفصل | the sidebar's category headings: «سرفصل دسته»; the Other heading is سایر (projects.json keeps "Other") | (proposed) |
| Mine order | ترتیب دلخواه | the order chip reads دلخواه | (proposed) |
| order chips: Working, A–Z, Needs you | در حال کار، الفبایی، نیاز به شما | | Drover (در حال کار، نیاز به شما) |
| stalled | راکد | work in Building with nothing for 30 minutes; kept apart from paused (متوقف شده) | (proposed) |
| quiet (a claim) | ساکت | no check-in or heartbeat for a while | (proposed) |
| in progress | در جریان | Building or Testing | common usage |
| To do / Doing (the owner's own tasks) | برای انجام / در حال انجام | | (proposed) |
| release (a claim) | رها کردن | the pane's Release | (proposed) |
| LIVE | زنده | the LIVE window and badge | (proposed) |
| prefix / folder | پیشوند / پوشه | | Mozilla |
| estimate | تخمین | | common usage |
| token ribbon | روبان توکن | the card's bottom corner: tokens used against the estimate (mu-172); built like روبان دارنده | (proposed) |
| over (the estimate) | بیش | the token ribbon's one word past the estimate, kept short so the figures and it fit a 100px card; read out in full as «بیشتر از تخمین» | (proposed) |
| no est / shared / not read (the token ribbon's short words) | بی‌تخمین / مشترک / ناخوانده | short so they fit a 100px card; «مشترک» gives way whole on a narrow card, «ناخوانده» never does (it is the whole message when nothing was read; «خوانده نشد» lost its «نشد»); spoken and in the tooltip in full | (proposed) |
| cache read / cache write | خواندن از کش / نوشتن در کش | kinds of tokens; cache reads are shown, never counted («شمرده نشده») | (proposed) |
| shared session | نشست مشترک | a session another task names too; listed, «شمرده نشده» | (proposed) |
| agent time | زمان ایجنت | the Work total's time: «در حال ساخت» + «در حال تست» | (proposed) |
| your wait (Your check's time) | انتظار شما | the owner's wait, never in the agent time | (proposed) |
| total | مجموع | the Work table's last row | common usage |
| Group by / Who | گروه‌بندی / چه کسی | the filter bar's choices | (proposed) |
| sort | مرتب‌سازی | the filter bar's sort; ترتیب is the sidebar's order, kept apart | Mozilla |
| none (a picker option) | هیچ‌کدام | bare هیچ is an unfinished word as an option; «…: هیچ.» after a heading stays | Mozilla |
| repo | مخزن | pl. مخزن‌ها | Gitea |
| Mac (the computer) | مک | Persian script; macOS stays Latin | common usage |
| pane | پنل | «پنل تسک»; Drover's پنل نوار منو | Drover |
| header (the window's) | سرصفحه | | (proposed) |
| count | شمار | «شمار «نیاز به شما»»; a counting is شمارش | common usage |
| double-click | دو بار کلیک | two words; دوبار reads as one | common usage |
| held by | در دست | «در دست {{who}}» | (proposed) |
| vision (check) | چشم‌انداز | the vision check of a shaped task | (proposed) |
| transcript | رونوشت | a session's transcript file | (proposed) |
| menu bar / Dock | نوار منو / Dock | Dock is macOS's own name and stays Latin | Drover (نوار منو) |
| Recently finished | اخیراً انجام شد | merged ادغام‌شده, accepted پذیرفته | Drover (انجام شد) |
| run / stop / restart (a project's run) | اجرا / متوقف کردن / اجرای دوباره | stop is not توقف, which is the pause button | (proposed) |
| colour names | لیمویی، ارکیده‌ای، صورتی، خاکی، نعنایی | project colours; the accents are بنفش، سبزآبی، گلی، طلایی، سبز، گرافیتی | (proposed) |
| Settings › Projects | تنظیمات ← پروژه‌ها | the arrow rule above, for every Settings path | Drover |
| account | حساب | pl. حساب‌ها; a Claude or Codex login Muster runs work on (0113) | common usage |
| profile (an account's) | پروفایل | | common Persian software usage |
| reading (of use) | خوانش | pl. خوانش‌ها; what Claude Code reported, never a guess: «بدون خوانش» | (proposed) |
| use / used | مصرف / مصرف‌شده | «۴۲٪ مصرف‌شده» | common usage |
| window (5-hour, weekly) | بازه | «بازه‌ی ۵ ساعته»، «هفتگی»; the short names on the bars are ۵ ساعت and ۷ روز, in full words per the times rule | Drover (بازه‌ی زنده) |
| reserve | رزرو | the share of a window kept for the owner; not ذخیره, which is Save | (proposed) |
| headroom | جا تا رزرو | «۳۸٪ جا تا رزرو»; past it «وارد رزرو» | (proposed) |
| reset (a window's) | بازنشانی | «بازنشانی ۱۴:۲۰»; a past one «قرار بود ۰۹:۰۰ بازنشانی شود» | Mozilla |
| stale | کهنه | a reading past its window's reset | (proposed) |
| over the limit | بیش از سقف | | common usage |
| concurrency | همزمانی | | common Persian software usage |
| provider | ارائه‌دهنده | Claude or Codex | common usage |
| status line | نوار وضعیت | Claude Code's status line | common Persian software usage |
| meter (the header's) | سنجه | «سنجه‌ی سرصفحه» | (proposed) |
| balancing (across accounts) | توازن | «توازن خودکار بین حساب‌ها در کار نیست» | (proposed) |
| recap (a session's) | خلاصه | (mu-147) | common usage |
| handoff chain | زنجیره‌ی واگذاری | built from Drover's واگذاری (mu-147) | Drover (واگذاری) |
| Your turn | نوبت شما | the Your check card (mu-147) | common usage |
| timeline scale | مقیاس | Day / Week / Month: روز / هفته / ماه (mu-147) | common usage |

## Sources

- Drover's glossary: `docs/i18n/glossary-fa.md` at Drover `a990fc8`, and its sources: Pro Git
  (fa) https://git-scm.com/book/fa/v2, Zoomit, CodeVelocity, Gitea's fa-IR locale, Mozilla
  terminology (fa) https://pontoon.mozilla.org/terminology/fa.tbx, Persian orthography (W3C,
  r12a) https://r12a.github.io/scripts/arab/pes.html, Persian HIG
  https://github.com/shervinafshar/Persian-HIG
- Apple's and Microsoft's Persian UI wording: macOS and Windows in Persian, for the everyday
  interface words (نوار کناری، اعلان‌ها، روشن، تیره، تمرکز).

## Gaps

The "(proposed)" words (شکل‌دهی، در حال ساخت، برداشتن، مهلت، زاویه‌ی دید، گزارش وضعیت، ضربان،
سازنده، شکل‌دهنده، تسک برداشته‌شده, and the shell's: بورد (or برد)، دستیار، نما، چیپ، نام نمایشی، سرصفحه، سرفصل، ترتیب دلخواه،
راکد، ساکت، رها کردن، زنده، چشم‌انداز، رونوشت, and mu-172's: روبان توکن، بیش، خواندن از کش، نوشتن
در کش، نشست مشترک، زمان ایجنت، انتظار شما, and mu-113's: خوانش، رزرو، جا تا رزرو، کهنه، سنجه، توازن, and mu-147's:
فرستادن، لین «متوقف شده»، در دست گرفتن، انسان / ایجنت، نگهداری، درس، اثبات‌شده / تیک‌خورده) have no source and wait on a native-speaking review.
The token ribbon writes its figures in Persian digits, with the k and M as they are (۲۱۰k). The digits rule waits on mu-151: until then a number filled into a Persian string is in
Persian digits (the layer formats it), and a number written alone on screen (a count chip) is not.
