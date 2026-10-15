# mu-146 Persian (fa) translation review

- **Who:** an independent reviewer agent (Claude) that didn't write these translations.
- **When:** 2026-10-14, at commit `0c28a35` on `task/mu-146-persian-foundation-drover-s-i18n-layer-ported-a`.
- **What:** `app/src/i18n/locales/fa/{shell,common,native}.json` (498 + 105 + 11 strings)
  against `en/` and `docs/i18n/glossary-fa.md`.
- **Method:** a script flattened both locales and checked missing and extra keys, `{{placeholder}}`
  sets, Arabic ي/ك, ASCII quotes, Latin `? , ; %` in Persian text, می/نمی without ZWNJ, a space
  before ها, and گیت. All of these came back clean: no missing or extra keys, identical placeholders,
  `_one`/`_other` everywhere en has them. Every string was then read side by side with its English
  source for meaning, register, length, glossary use and right-to-left rendering. Strings that
  are fine aren't listed.

## Findings

### Meaning

1. **`shell:move.headingMoved`, `shell:move.projectMoved`**
   - Current: `{{name}} به جای {{at}} از {{of}} رفت`
   - Problem: «به جای» means "instead of", so the sentence reads "X went instead of 3 of 5". It's
     a screen-reader announcement, so the user only hears it, with no visual cue to correct it.
   - Suggested: `{{name}} به جایگاه {{at}} از {{of}} رفت` (and `… از {{of}} در {{heading}} رفت`)
   - Severity: **must-fix**

2. **`shell:appearance.oneTheme`**
   - Current: `یک تم، هر چه مک استفاده کند.`
   - Problem: this reads as "use whatever theme the Mac uses", the opposite of the English
     ("one theme, regardless of the Mac's light or dark").
   - Suggested: `یک تم، فارغ از روشن یا تیره بودن مک.`
   - Severity: **must-fix**

3. **`shell:app.noQueues`**
   - Current: `… Muster پوشه‌های <code>{{path}}</code> و یک سطح پایین‌تر را می‌گردد؛ …`
   - Problem: «پوشه‌ها را می‌گردد» means "searches through the folders", not "looks for these
     folders". The second part also says "searches one level lower" with no object.
   - Suggested: `هنوز صفی نیست. Muster به دنبال پوشه‌های <code>{{path}}</code> می‌گردد، تا یک سطح پایین‌تر؛ یا در تنظیمات، بخش پروژه‌ها، یکی اضافه کنید.`
   - Severity: should-fix

4. **`shell:order.needs.title`**
   - Current: `اول آن‌که بیشتر منتظر شماست، بعد آن‌هایی که رویشان کار می‌شود`
   - Problem: «آن‌که» is singular and reads as "the one who waits longest". The English means the
     projects with the most items waiting on you.
   - Suggested: `اول پروژه‌هایی که بیشترین مورد منتظر شما را دارند، بعد آن‌هایی که رویشان کار می‌شود`
   - Severity: should-fix

5. **`shell:move.cantKeep`**
   - Current: `این ترتیب نگه داشته نشد: {{error}}`
   - Problem: "keep" here means "save". «نگه داشته نشد» reads as "the order wasn't kept/obeyed".
   - Suggested: `این ترتیب ذخیره نشد: {{error}}`
   - Severity: should-fix

6. **`shell:projects.noGate`**
   - Current: `ندارد: ادغام‌ها منتظر یکی می‌مانند. <code>{{command}}</code>`
   - Problem: «منتظر یکی» ("wait for someone/one") is vague, because the antecedent (gate) is
     only in the column header.
   - Suggested: `ندارد: ادغام‌ها تا تعریف یک دروازه منتظر می‌مانند. <code>{{command}}</code>`
   - Severity: should-fix

7. **`shell:recent.kind.all.title`, `shell:recent.kind.merged.title`**
   - Current: `آنچه روی main نشست …` / `تست گذشت و کار روی main نشست`
   - Problem: «نشست» is the glossary word for *session*, and this screen is full of sessions.
     «روی main نشست» parses as "a session on main" before it parses as "landed on main".
   - Suggested: `آنچه به main رسید و آنچه پذیرفتید` / `تست گذشت و کار به main رسید`
   - Severity: should-fix

8. **`shell:about.found`, `shell:about.foundNone`**
   - Current: `خودش پیدا کرده ({{where}} …)`
   - Problem: the subject is missing ("it found itself…"). The English is a passive heading:
     queues found automatically.
   - Suggested: `پیداشده به‌طور خودکار ({{where}} و یک سطح پایین‌تر)`, and the same with `: هیچ.`
   - Severity: should-fix

9. **`shell:notifications.what`** (the "blocked" clause)
   - Current: `تسکی که به‌خاطر شما یا یک مجوز مسدود است`
   - Problem: «به‌خاطر شما» ("because of you") sounds like blame. «مجوز» is the glossary's word
     for a permission *setting*; a pending prompt is «درخواست اجازه».
   - Suggested: `تسکی که منتظر شما یا یک درخواست اجازه مسدود مانده`
   - Severity: should-fix

10. **`shell:notifications.once`**
    - Current: `برای یک چیز هرگز دو بار نیست، و نه برای آنچه از وقت شروع Muster منتظر بوده. وقتی پنجره‌ی Muster جلوست، …`
    - Problem: «هرگز دو بار نیست» has no subject (what is never twice?). «جلوست» is colloquial.
    - Suggested: `برای یک مورد هرگز دو بار اعلان نمی‌آید، و برای آنچه از زمان شروع Muster منتظر بوده هم نمی‌آید. وقتی پنجره‌ی Muster جلو است، آنچه نشان می‌دهد (پروژه‌اش، یا همه) دیده‌شده حساب می‌شود و خبر پروژه‌ای دیگر صبر می‌کند تا به جای دیگری بروید.`
    - Severity: should-fix

11. **`shell:app.noteGone`**
    - Current: `آن یادداشت دیگر نیست: یا تسک شده یا کنار گذاشته شده.`
    - Problem: «تسک شده» ("got tasked") is colloquial and unclear.
    - Suggested: `آن یادداشت دیگر وجود ندارد: یا به تسک تبدیل شده یا کنار گذاشته شده.`
    - Severity: should-fix

12. **`shell:ping.unreached_one/_other`, `shell:ping.noneReach`**
    - Current: `{{count}} مورد نشستی برای دسترسی ندارد` / `هیچ‌کدام از برداشت‌های ساکتش نشستی برای دسترسی ندارد`
    - Problem: «نشستی برای دسترسی» is a word-for-word rendering. «برداشت‌ها» (noun) reads first as
      "withdrawals" or "interpretations", not "claims" (see glossary finding 34).
    - Suggested: `{{count}} مورد نشست در دسترسی ندارد` (and `ندارند` for _other) / `هیچ‌کدام از تسک‌های برداشته‌شده‌ی ساکتش نشست در دسترسی ندارد`
    - Severity: should-fix

13. **`shell:agents.leaseWhat`**
    - Current: `برداشتنی در «در حال ساخت» که بیش از این مدت از آن خبری نرسیده، …`
    - Problem: «برداشتنی» (infinitive plus indefinite ی) isn't a usable noun. It reads as
      "something to be picked up".
    - Suggested: `تسک برداشته‌شده‌ای در «در حال ساخت» که بیش از این مدت از آن خبری نرسیده، …` (rest unchanged)
    - Severity: should-fix

14. **`shell:display.elements.sidebar.full`** (the tail)
    - Current: `… اول آنچه به شما نیاز دارد، باز یا جمع‌شده یکسان`
    - Problem: «باز یا جمع‌شده یکسان» doesn't parse. The English means the line is the same
      whether the heading is open or folded.
    - Suggested: `… اول آنچه به شما نیاز دارد، چه باز باشد چه جمع‌شده`
    - Severity: should-fix

15. **`shell:projects.col.prefixTitle`**
    - Current: `آنچه کلید تسک‌هایش با آن شروع می‌شود: mu در mu-12`
    - Problem: the rest of the UI calls a task's key «شناسه» (sort.id, filter.placeholder).
      «کلید» here reads as a keyboard key.
    - Suggested: `آنچه شناسه‌ی تسک‌هایش با آن شروع می‌شود: mu در mu-12`
    - Severity: should-fix

16. **`shell:projects.col.projectTitle`**
    - Current: `آنچه پنجره صدایش می‌کند؛ نام زیرش همانی است که فرمان‌ها می‌گیرند`
    - Problem: «صدایش می‌کند» ("calls out to it") is colloquial and literal.
    - Suggested: `نامی که پنجره نشان می‌دهد؛ نام زیر آن همانی است که فرمان‌ها می‌پذیرند`
    - Severity: should-fix

### Register (glossary: formal, no colloquial forms)

17. **`shell:sidebar.noLast`**
    - Current: `چیزی نمی‌گوید آخرین بار کی روی این کار شده`
    - Problem: «کی» ("when") is colloquial, and the English idiom is rendered literally.
    - Suggested: `معلوم نیست آخرین بار چه زمانی روی این کار شده`
    - Severity: should-fix

18. **`shell:display.elements.cardLines.adhd`**
    - Current: `چه کسی و از کی، …`
    - Problem: «از کی» is colloquial.
    - Suggested: `چه کسی و از چه زمانی، …`
    - Severity: should-fix

19. **`shell:appearance.leftOut`**
    - Current: `… مال 127، Drover و Braindump هستند. … از WCAG AA می‌گذرد.`
    - Problem: «مال» is colloquial. «از WCAG AA می‌گذرد» is literal; the usual phrase is
      «… را برآورده می‌کند».
    - Suggested: `آبی و نارنجی در رنگ‌های تأکیدی نیستند: به 127، Drover و Braindump تعلق دارند. هر جفت تم و رنگ تأکیدی معیار WCAG AA را برآورده می‌کند.`
    - Severity: should-fix

20. **`shell:about.private`**
    - Current: `همه‌اش روی همین مک خوانده می‌شود …`
    - Problem: «همه‌اش» is colloquial.
    - Suggested: `همه‌ی این‌ها روی همین مک خوانده می‌شود …`
    - Severity: should-fix

21. **`shell:filter.hideTitle`**
    - Current: `دوباره کنار گذاشتن نوار: ⌘F برش می‌گرداند`
    - Problem: «کنار گذاشتن» is the glossary's word for *drop* (a task). «برش می‌گرداند» is
      colloquial. The same «برش می‌گرداند» appears in `display.elements.columns.focusReach` and
      `display.elements.rail.focusReach`.
    - Suggested: `پنهان کردن دوباره‌ی نوار: ⌘F آن را برمی‌گرداند`, and «… آن را برمی‌گرداند» in the two focusReach strings
    - Severity: should-fix

22. **`shell:waiting.other`**
    - Current: `پرسش‌ها، مسدودی‌ها و تسک‌های خودتان`
    - Problem: «مسدودی‌ها» isn't a standard word.
    - Suggested: `پرسش‌ها، موارد مسدود و تسک‌های خودتان`
    - Severity: should-fix

23. **`shell:heading.removeTitle`**
    - Current: `این سرفصل جدید را بردارید`
    - Problem: «برداشتن» is the glossary's word for *claim*. Using it for "remove" makes the two
      collide. The glossary also asks for an infinitive or noun on buttons.
    - Suggested: `حذف این سرفصل جدید`
    - Severity: should-fix

### Glossary conformance and consistency

24. **`common:none`, `common:groupBy.none`, `shell:colour.none`**
    - Current: `هیچ`
    - Problem: «هیچ» on its own is an incomplete word as a picker option. Persian UIs (Mozilla
      included) use «هیچ‌کدام».
    - Suggested: `هیچ‌کدام`
    - Severity: should-fix

25. **`shell:display.elements.paneSections.adhd`, `shell:display.elements.paneSections.folded`**
    - Current: `… کار، گزارش کار و گزارش‌های وضعیت …`
    - Problem: the Log becomes «گزارش کار», right next to «گزارش وضعیت» (check-in), and the
      glossary also uses «گزارش» for *report*. That makes three different things called گزارش in
      one line. The Log has no glossary entry.
    - Suggested: `… کار، لاگ و گزارش‌های وضعیت …` (or «سابقه»), and add the word to the glossary
    - Severity: should-fix

26. **`shell:sidebar.runningTitle`**
    - Current: `… بلوک «اجرا»ی آن، در تسکی در «بررسی شما»، …`
    - Problem: «بلوک» isn't in the glossary, and Drover's entry spells it بلاک. A ی attached after
      a closing » also renders badly. The same pattern is in `agents.liveWhat` («زنده»ی) and
      `about.projectsJson` («بررسی شما»یشان).
    - Suggested: `یک اجرا از Muster در جریان است: بخش «اجرا» در تسکی در «بررسی شما»، یا تنظیمات ← پروژه‌ها آن را متوقف می‌کند`. Elsewhere, rephrase so no suffix follows » (e.g. `سقف «بررسی شما» برای هر کدام`).
    - Severity: nit

27. **`shell:filter.saveViewTitle`**
    - Current: `ذخیره‌ی جستجو، فیلترها و ترتیب با یک نام`
    - Problem: "Sort" is «مرتب‌سازی» in the glossary and in `filter.sort` and `filter.filtersTitle`.
      «ترتیب» is the sidebar's *order*.
    - Suggested: `ذخیره‌ی جستجو، فیلترها و مرتب‌سازی با یک نام`
    - Severity: should-fix

28. **`shell:account.use`**
    - Current: `استفاده از این`
    - Problem: the demonstrative dangles ("use this").
    - Suggested: `استفاده از این حساب`
    - Severity: should-fix

29. **Wording that uses «الان» and «اکنون» interchangeably** (`sidebar.workingDot`,
    `sidebar.tasksWorkingTitle_*`, `sidebar.projectsWorkingTitle_*`, `order.working.title`,
    `appearance.pickTwo`, `display.modes.focus.says` vs `common:time.now` «اکنون»)
    - Problem: both words are used for "now". «الان» is borderline informal for the glossary's
      formal register.
    - Suggested: pick «اکنون» throughout, or record «الان» as accepted in the glossary.
    - Severity: nit

### Right to left

30. **Keyboard shortcuts written into Persian text:** `shell:app.newTaskTitle`,
    `toolbar.modeChip`, `order.mine.title`, `heading.canMove`, `heading.titleMove`, `heading.empty`,
    `move.newHeading`, `filter.placeholder`, `filter.hideTitle`, `appearance.textSizeMeta`,
    `projects.col.mineTitle`, `display.modeMeta`, `display.elements.filters.focusReach`,
    `display.elements.sidebar.focusReach`
    - Current: e.g. `تسک جدید (⌘N)`, `⌘. از هر حالتی …`, `⌘+ و ⌘−`, `⌥↑ ⌥↓`
    - Problem: ⌘ and ⌥ are bidi-neutral, and so are `.`, `+`, `−` and arrows. In an RTL paragraph
      they resolve to RTL, so these render as `(N⌘)`, `.⌘`, `+⌘` and `↓⌥ ↑⌥`. None of the strings
      have a direction mark. The layer isolates `{{values}}` only, and `bidi.test.ts` checks only
      Latin runs that start with `~ . /`. This is by Unicode bidi reasoning; I didn't check it with
      a screenshot.
    - Suggested: wrap each shortcut run in LRI…PDI (U+2066 … U+2069), e.g.
      `تسک جدید (⁦⌘N⁩)`. A better fix is in the layer: a `<kbd>` tag or auto-isolation
      of runs that start with ⌘/⌥/⇧/⌃. Then extend the bidi test to these glyphs.
    - Severity: should-fix

### Nits

31. **`common:splitter`, `shell:sidebar.renameHint`, `shell:heading.titleRename`**
    - Current: `دوبار کلیک`
    - Problem: "double" here is two words (دو بار). «دوبار» reads as "twice-load".
    - Suggested: `دو بار کلیک`
    - Severity: nit

32. **`common:sort.oldest.title`, `common:sort.changed.title`**
    - Current: `اولین ثبت‌شده اول` / `آخرین تغییرکرده اول`
    - Problem: «اول… اول» repeats. «تغییرکرده» is awkward.
    - Suggested: `قدیمی‌ترین ثبت‌شده اول` / `تازه‌ترین تغییر اول`
    - Severity: nit

33. **`shell:ping.asked_one/_other`**
    - Current: `از {{count}} نشست درخواست شد.`
    - Problem: what was asked for is missing ("was requested from N sessions").
    - Suggested: `از {{count}} نشست گزارش وضعیت خواسته شد.`
    - Severity: nit

## Glossary findings

34. **claim (noun) is missing.**
    - Problem: the glossary gives only the verb, «برداشتن». The translation then improvises
      «برداشتنی» (13) and «برداشت‌ها» (12), and both misread. «برداشت» means withdrawal or
      interpretation.
    - Suggested: add the noun, «تسک برداشته‌شده» (or «برداشتِ تسک» in compounds), and say not to
      use bare «برداشت».
    - Severity: should-fix

35. **chip → «تراشه» (proposed).**
    - Problem: «تراشه» is first of all a microchip (or a shaving). It's used about 20 times, so
      it's highly visible.
    - Suggested: «چیپ», transliterated like تگ and کارت, as Persian product UIs say it.
    - Severity: should-fix (this is the native-speaker call the "(proposed)" note asks for)

36. **Missing entries for words the translation already uses.**
    - Problem: «مخزن» (repo), «مک» (Mac; 22 uses, while the product-names rule lists macOS as
      Latin), Log (finding 25), «پنل» (pane), «سرصفحه» (header) and «شمار» (count) have no entries.
    - Suggested: add them so mu-147+ stay consistent. Recommended: مخزن, مک (Persian script is
      fine for the hardware name, since only "macOS" stays Latin), پنل, سرصفحه, شمار.
    - Severity: nit

37. **Conventions › Direction covers only punctuation-led Latin runs.**
    - Problem: keyboard glyph runs (⌘ ⌥ ⇧ ⌃ plus a key) need the same treatment (finding 30).
    - Suggested: add "keyboard shortcuts are isolated with LRI…PDI (or by the layer)".
    - Severity: should-fix

38. **board → «بورد» (proposed).**
    - Problem: Persian Scrum and Trello users more often write «برد». «بورد» is attested too.
    - Suggested: let the native review choose one. No change is needed if «بورد» is kept.
    - Severity: nit

## Summary

38 findings: **2 must-fix** (1, 2), 29 should-fix, 7 nits (26, 29, 31, 32, 33, 36, 38). Of these, 33 are on strings and 5
on the glossary (34–38). Mechanical checks are all clean: keys, placeholders, plurals, Persian
letters, «» quotes, Persian punctuation, ZWNJ on می/ها, product names in Latin, and no گیت.

## Fixes

Applied the same day, on the same branch. Shortcut runs below are written as `⁦…⁩` (LRI … PDI).

1. fixed: `{{name}} به جایگاه {{at}} از {{of}} رفت` / `{{name}} به جایگاه {{at}} از {{of}} در {{heading}} رفت`
2. fixed: `یک تم، فارغ از روشن یا تیره بودن مک.`
3. fixed: `هنوز صفی نیست. Muster به دنبال پوشه‌های <code>{{path}}</code> می‌گردد، تا یک سطح پایین‌تر؛ یا در تنظیمات، بخش پروژه‌ها، یکی اضافه کنید.`
4. fixed: `اول پروژه‌هایی که بیشترین مورد منتظر شما را دارند، بعد آن‌هایی که رویشان کار می‌شود`
5. fixed: `این ترتیب ذخیره نشد: {{error}}`
6. fixed: `ندارد: ادغام‌ها تا تعریف یک دروازه منتظر می‌مانند. <code>{{command}}</code>`
7. fixed: `آنچه به main رسید و آنچه پذیرفتید` / `تست گذشت و کار به main رسید`
8. fixed: `پیداشده به‌طور خودکار ({{where}} و یک سطح پایین‌تر)`, and the same with `: هیچ.`
9. fixed: `تسکی که در انتظار شما یا یک درخواست اجازه مسدود مانده` («در انتظار» rather than the suggested «منتظر», so the clause reads "blocked, waiting on you or a permission request")
10. fixed: `برای یک مورد هرگز دو بار اعلان نمی‌آید، و برای آنچه از زمان شروع Muster منتظر بوده هم نمی‌آید. وقتی پنجره‌ی Muster جلو است، … صبر می‌کند تا به جای دیگری بروید.`
11. fixed: `آن یادداشت دیگر وجود ندارد: یا به تسک تبدیل شده یا کنار گذاشته شده.`
12. fixed: `{{count}} مورد نشست در دسترسی ندارد` / `… ندارند` / `هیچ‌کدام از تسک‌های برداشته‌شده‌ی ساکتش نشست در دسترسی ندارد`
13. fixed: `تسک برداشته‌شده‌ای در «در حال ساخت» که بیش از این مدت از آن خبری نرسیده، …`
14. fixed: `… اول آنچه به شما نیاز دارد، چه باز باشد چه جمع‌شده`; `sidebar.focus`'s «باز یا جمع‌شده» got the same wording
15. fixed: `آنچه شناسه‌ی تسک‌هایش با آن شروع می‌شود: mu در mu-12`
16. fixed: `نامی که پنجره نشان می‌دهد؛ نام زیر آن همانی است که فرمان‌ها می‌پذیرند`
17. fixed: `معلوم نیست آخرین بار چه زمانی روی این کار شده`
18. fixed: `چه کسی و از چه زمانی، …`
19. fixed: `آبی و نارنجی در رنگ‌های تأکیدی نیستند: به 127، Drover و Braindump تعلق دارند. هر جفت تم و رنگ تأکیدی معیار WCAG AA را برآورده می‌کند.`
20. fixed: `همه‌ی این‌ها روی همین مک خوانده می‌شود …`
21. fixed: `پنهان کردن دوباره‌ی نوار: ⁦⌘F⁩ آن را برمی‌گرداند`; both focusReach strings end `«جمع کردن» آن را برمی‌گرداند`. The same colloquial «برش نمی‌دارند» in `common:youRibbon.title` is now `هرگز آن را برنمی‌دارند`
22. fixed: `پرسش‌ها، موارد مسدود و تسک‌های خودتان`
23. fixed: `حذف این سرفصل جدید`
24. fixed: `هیچ‌کدام` (common:none, common:groupBy.none, shell:colour.none); glossary entry added
25. fixed: `چشم‌انداز، بازبینی، کار، لاگ و گزارش‌های وضعیت باز، …` / `… کار، و لاگ و گزارش‌های وضعیت، …`; glossary: Log → لاگ
26. fixed: `یک اجرا از Muster در جریان است: بخش «اجرا» در تسکی در «بررسی شما»، یا تنظیمات ← پروژه‌ها آن را متوقف می‌کند`; `agents.liveWhat` `… و نشان «زنده» در پنل تسک (فایل تسک).`; `about.projectsJson` `… دروازه‌ها و سقف «بررسی شما» برای هر کدام، …`. Glossary: no suffix after a closing ».
27. fixed: `ذخیره‌ی جستجو، فیلترها و مرتب‌سازی با یک نام`
28. fixed: `استفاده از این حساب`
29. fixed: «اکنون» throughout (8 strings); glossary convention added
30. fixed: every shortcut run isolated, e.g. `تسک جدید (⁦⌘N⁩)`, `⁦⌥↑ ⌥↓⁩`, `⁦⌘+⁩ و ⁦⌘−⁩ … ⁦⌘0⁩` (20 runs in 14 strings). `bidi.test.ts` now fails on a ⌘ ⌥ ⇧ ⌃ outside an LRI…PDI pair or `<code>`. The layer-level fix (`<kbd>` or auto-isolation) wasn't done: marks in the strings are what the brief asked for and what the test pins.
31. fixed: `دو بار کلیک` (common:splitter, sidebar.renameHint, heading.titleRename); glossary entry added
32. fixed: `قدیمی‌ترین ثبت‌شده اول` / `تازه‌ترین تغییر اول`
33. fixed: `از {{count}} نشست گزارش وضعیت خواسته شد.` (_one and _other)
34. fixed: glossary row "claim (noun) → تسک برداشته‌شده", never bare برداشت or برداشتنی; the verb row also says "remove" is حذف, not برداشتن
35. fixed: chip → «چیپ» in the glossary and all 14 uses (still "(proposed)", listed in Gaps for the native-speaker check)
36. fixed: glossary rows for مخزن، مک (with the product-names rule saying only macOS stays Latin)، لاگ، پنل، سرصفحه، شمار
37. fixed: Conventions › Direction has a "Keyboard shortcuts" rule (LRI…PDI, a pair shares one isolate, the test enforces it)
38. kept: «بورد». It's used everywhere already, and «برد» is a homograph of برد (range, win, "took"). The glossary row now names «برد» as the alternative and leaves the choice to the owner's native-speaker check.
