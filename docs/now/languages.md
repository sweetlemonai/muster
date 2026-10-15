---
covers: app/src/i18n/format*, app/src/i18n/numbering*, app/src/jalali*, app/src/search*, app/src-tauri/src/search.rs, app/src/fields*, app/src/Dates.tsx, app/src/labels.ts
---
# Digits, dates and search in فارسی

- In فارسی, what people read is in Persian digits and the Jalali calendar: counts, ages, estimates, dates and times. (mu-151)
- What's copied or stored stays Latin and Gregorian: keys, ids, commands, paths, branches, code, front matter, log lines, `--json`. (mu-151)
- Every number and date goes through one set of Intl formatters, made once per language, digits, calendar and options. (mu-151)
- In English with Latin digits and Gregorian, a stored date shows as stored (`2026-10-14`, `10-14 20:22`). (mu-151)
- Estimates read in words in Persian («۲ ساعت», «۴۰۰ هزار») and as written in English (`2h`, `400k`). (mu-151)
- Dates in the pane, the table, Recently finished and the meter carry `<time datetime>` with what's stored. (mu-151)
- Number and date fields take Persian or Arabic-Indic digits and send Latin ones to the core. (mu-151)
- A goal's due date is typed in the calendar picked; in Jalali the ISO day it saves shows beside it, and a day that doesn't exist is refused with why. (mu-151)
- Jalali ↔ Gregorian is a port of `@dastafzar/calendar` (MIT), checked against Intl for every day 1990–2040. (mu-151)
- Search folds Persian on both sides alike: ي ى ئ → ی, ك → ک, ۀ ة → ه, آ أ إ ٱ → ا, ؤ → و; ZWNJ, tatweel, harakat and ء dropped; Persian and Arabic-Indic digits are 0–9. (mu-151)
- So «میشود» finds «می‌شود», «کتاب» finds «كتاب», and ۱۲ finds mu-12. (mu-151)
- The menu bar's counts follow the Digits setting too. (mu-151)
