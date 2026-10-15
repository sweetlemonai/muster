//! Muster's languages on the core's side (mu-146), ported from Drover's `i18n.rs` (Drover task
//! 0007, decision 0021, commit a990fc8): which language is in effect (the setting, or macOS's),
//! and the text of the menu bar. The text comes from the same files the UI reads,
//! `src/i18n/locales/<language>/native.json`, in i18next's format (nested keys, `{{name}}`,
//! plural suffixes), compiled in.
//!
//! Drover also writes to agents in a language the person picks (`agent.json`, `agent_t`). Muster
//! leaves that out: what agents read (task files, briefs, `--json`, the protocol) is parsed by
//! Muster's own tools and stays English.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{atomic::AtomicU8, atomic::Ordering, OnceLock, RwLock};

/// The languages, as the UI's `LOCALES` (src/i18n/locales.ts).
pub const LOCALES: [&str; 2] = ["en", "fa"];

/// The UI's development pseudo-locale; the core's text stays English in it.
pub const PSEUDO: &str = "pseudo";

/// The languages offered (files complete and reviewed), as `READY` in
/// src/i18n/locales.ts. Following macOS picks only these.
pub const READY: [&str; 2] = ["en", "fa"];

const FILES: [(&str, &str); 2] = [
    ("en", include_str!("../../src/i18n/locales/en/native.json")),
    ("fa", include_str!("../../src/i18n/locales/fa/native.json")),
];

/// The app's bundle id: macOS keeps a language picked for Muster alone under it.
#[cfg(target_os = "macos")]
const BUNDLE_ID: &str = "ai.sweetlemon.muster";

/// A language code we know, as the static string (None for anything else).
pub fn known(code: &str) -> Option<&'static str> {
    LOCALES.iter().copied().find(|l| *l == code)
}

/// A stored setting as the static string: one of the languages, or the pseudo-locale while
/// developing; None for "" (follow macOS) and anything else. (Muster keeps the setting as a
/// `&'static str`, so `settings::Settings` stays `Copy`.)
pub fn stored(code: &str) -> Option<&'static str> {
    known(code).or((cfg!(debug_assertions) && code == PSEUDO).then_some(PSEUDO))
}

/// Whether a stored setting is valid: "" (follow macOS), one of the languages, or the
/// pseudo-locale while developing.
pub fn valid_setting(code: &str) -> bool {
    code.is_empty() || stored(code).is_some()
}

/// One macOS language tag ("fa-IR", "en_GB", "fa") to one of ours, by the language alone.
/// (Drover's also maps Chinese and Portuguese variants, which Muster doesn't ship.)
pub fn match_tag(tag: &str) -> Option<&'static str> {
    let lower = tag.replace('_', "-").to_lowercase();
    known(lower.split('-').next().unwrap_or(""))
}

/// The language in effect: the setting, else the first of macOS's
/// languages Muster has, else English.
pub fn resolve(setting: &str, system: &[String]) -> &'static str {
    resolve_among(setting, system, &READY)
}

/// `resolve`, with macOS's languages matched only against `offered`.
fn resolve_among(setting: &str, system: &[String], offered: &[&str]) -> &'static str {
    if setting == PSEUDO && cfg!(debug_assertions) {
        return PSEUDO;
    }
    known(setting)
        .or_else(|| {
            system
                .iter()
                .find_map(|t| match_tag(t).filter(|l| offered.contains(l)))
        })
        .unwrap_or("en")
}

/// macOS's preferred languages for Muster, first first: the one picked for Muster alone in
/// System Settings (General › Language & Region › Applications) when there is one, else the
/// Mac's. Read once, the first time it's asked (macOS reads them once per launch too). Drover asks
/// `NSLocale`; Muster's crates don't include objc2, so it asks `defaults`. Empty when they can't
/// be read.
pub fn system_languages() -> Vec<String> {
    static READ: OnceLock<Vec<String>> = OnceLock::new();
    READ.get_or_init(read_system_languages).clone()
}

#[cfg(target_os = "macos")]
fn read_system_languages() -> Vec<String> {
    let read = |args: &[&str]| {
        std::process::Command::new("/usr/bin/defaults")
            .args(args)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| defaults_list(&String::from_utf8_lossy(&o.stdout)))
            .filter(|l| !l.is_empty())
    };
    read(&["read", BUNDLE_ID, "AppleLanguages"])
        .or_else(|| read(&["read", "-g", "AppleLanguages"]))
        .unwrap_or_default()
}

#[cfg(not(target_os = "macos"))]
fn read_system_languages() -> Vec<String> {
    std::env::var("LANG")
        .ok()
        .and_then(|l| l.split('.').next().map(str::to_string))
        .into_iter()
        .collect()
}

/// The list `defaults read … AppleLanguages` prints, `(\n    "en-US",\n    fa\n)`, as its
/// strings; empty for anything else.
pub fn defaults_list(text: &str) -> Vec<String> {
    let Some(inner) = text
        .trim()
        .strip_prefix('(')
        .and_then(|r| r.strip_suffix(')'))
    else {
        return Vec::new();
    };
    inner
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

static CURRENT: RwLock<&str> = RwLock::new("en");

/// The language of the core's text now.
pub fn current() -> &'static str {
    #[cfg(test)]
    if let Some(l) = stub::app_locale() {
        return l;
    }
    *CURRENT.read().unwrap_or_else(|e| e.into_inner())
}

pub fn set_current(locale: &'static str) {
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = locale;
}

fn parse_all(files: &[(&'static str, &'static str)]) -> HashMap<&'static str, Value> {
    files
        .iter()
        // A file that doesn't parse is caught by the tests; at run time
        // its language falls back to English.
        .filter_map(|(l, text)| serde_json::from_str(text).ok().map(|v| (*l, v)))
        .collect()
}

fn files() -> &'static HashMap<&'static str, Value> {
    static PARSED: OnceLock<HashMap<&'static str, Value>> = OnceLock::new();
    PARSED.get_or_init(|| parse_all(&FILES))
}

fn dig<'a>(v: &'a Value, path: &str) -> Option<&'a str> {
    path.split('.')
        .try_fold(v, |at, part| at.get(part))
        .and_then(Value::as_str)
}

/// CLDR's plural category of a whole number, as `Intl.PluralRules` gives it.
pub fn plural(locale: &str, n: u64) -> &'static str {
    let millions = n != 0 && n.is_multiple_of(1_000_000);
    match locale {
        "ru" => match (n % 10, n % 100) {
            (1, h) if h != 11 => "one",
            (2..=4, h) if !(12..=14).contains(&h) => "few",
            _ => "many",
        },
        "ar" => match (n, n % 100) {
            (0, _) => "zero",
            (1, _) => "one",
            (2, _) => "two",
            (_, 3..=10) => "few",
            (_, 11..=99) => "many",
            _ => "other",
        },
        "fr" | "pt-BR" => match n {
            0 | 1 => "one",
            _ if millions => "many",
            _ => "other",
        },
        "es" => match n {
            1 => "one",
            _ if millions => "many",
            _ => "other",
        },
        "fa" => match n {
            0 | 1 => "one",
            _ => "other",
        },
        "zh-Hans" | "ja" => "other",
        _ => {
            if n == 1 {
                "one"
            } else {
                "other"
            }
        }
    }
}

fn pick(locale: &str, key: &str, count: Option<u64>) -> Option<&'static str> {
    pick_in(files().get(locale)?, locale, key, count)
}

/// The string for `key` in one file, with i18next's plural rule.
fn pick_in<'a>(file: &'a Value, locale: &str, key: &str, count: Option<u64>) -> Option<&'a str> {
    match count {
        None => dig(file, key),
        Some(n) => (if n == 0 {
            dig(file, &format!("{key}_zero"))
        } else {
            None
        })
        .or_else(|| dig(file, &format!("{key}_{}", plural(locale, n))))
        .or_else(|| dig(file, &format!("{key}_other")))
        .or_else(|| dig(file, key)),
    }
}

/// Fill `{{name}}` from `args`; a name not given stays as written.
fn fill(text: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        match after.find("}}") {
            Some(close) => {
                let name = after[..close].trim();
                match args.iter().find(|(k, _)| *k == name) {
                    Some((_, v)) => out.push_str(v),
                    None => out.push_str(&rest[open..open + 2 + close + 2]),
                }
                rest = &after[close + 2..];
            }
            None => {
                out.push_str(&rest[open..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

/// A count as the language writes it: Persian digits in Persian, as
/// `Intl.NumberFormat` does. No grouping: the core's counts are small.
fn number(locale: &str, n: u64) -> String {
    digits(locale, &n.to_string())
}

/// Settings › Language's Digits (mu-151, `settings::DIGITS`): 0 follows the language, 1 is
/// Persian, 2 Latin. The window's numbers follow the same setting (numbering.ts).
static DIGITS: AtomicU8 = AtomicU8::new(0);

/// The Digits setting the core's text follows from now on: `persian`, `latin`, or None for the
/// language's own.
pub fn set_digits(picked: Option<&str>) {
    DIGITS.store(digits_code(picked), Ordering::Relaxed);
}

fn digits_code(picked: Option<&str>) -> u8 {
    match picked {
        Some("persian") => 1,
        Some("latin") => 2,
        _ => 0,
    }
}

/// Whether `locale` writes Persian digits with the Digits setting `code`.
fn persian_digits(code: u8, locale: &str) -> bool {
    match code {
        1 => true,
        2 => false,
        _ => locale == "fa",
    }
}

/// A number written in ASCII digits ("80", "-1"), in the language's
/// digits: Persian's in Persian, the rest as they are; or the ones
/// Settings › Language's Digits picked (mu-151).
pub(crate) fn digits(locale: &str, s: &str) -> String {
    digits_with(DIGITS.load(Ordering::Relaxed), locale, s)
}

fn digits_with(code: u8, locale: &str, s: &str) -> String {
    if !persian_digits(code, locale) {
        return s.to_string();
    }
    s.chars()
        .map(|c| match c.to_digit(10) {
            Some(d) => char::from_u32(0x06F0 + d).unwrap_or(c),
            None => c,
        })
        .collect()
}

/// A number the core fills in other than `{{count}}`, in the current language's digits.
pub fn num(n: impl std::fmt::Display) -> String {
    digits(current(), &n.to_string())
}

/// Items one after another, with the current language's separator
/// (`list.separator`: ", " in English, "، " in Persian).
pub fn list<S: AsRef<str>>(items: &[S]) -> String {
    list_in(current(), items)
}

/// `list`, in `locale`.
pub fn list_in<S: AsRef<str>>(locale: &str, items: &[S]) -> String {
    join(items, &text_in(locale, "list.separator", None, &[]))
}

fn join<S: AsRef<str>>(items: &[S], sep: &str) -> String {
    items
        .iter()
        .map(AsRef::as_ref)
        .collect::<Vec<_>>()
        .join(sep)
}

/// The languages written right to left, as `RTL` in src/i18n/locales.ts.
const RTL: [&str; 1] = ["fa"];

/// `value` isolated in a right-to-left `locale` (Drover's task 0020): first-strong isolate … pop
/// directional isolate, what `<bdi>` does, in plain text (as `FSI`/`PDI` in
/// src/i18n/runtime.ts). A path, a command or a name then keeps its own direction in the
/// sentence around it. The value's own bidi controls (`index::is_bidi_control`) go first, in
/// every language (mu-152): a title's lone U+2069 would close the isolate early, an unclosed
/// U+2066 would swallow its end, and an override would reorder the line around it.
pub fn isolate_in(locale: &str, value: &str) -> String {
    let clean: String = value
        .chars()
        .filter(|&c| !crate::index::is_bidi_control(c))
        .collect();
    wrap(locale, &clean)
}

/// `value` as its own run in a right-to-left `locale`, as it is.
fn wrap(locale: &str, value: &str) -> String {
    if RTL.contains(&locale) && !value.is_empty() {
        format!("\u{2068}{value}\u{2069}")
    } else {
        value.to_string()
    }
}

/// The text for `key` in `locale`, falling back to English, then to the key.
/// Right to left, each value is its own run (`isolate_in`); `{{count}}` is a
/// number and stays as it is.
pub fn text_in(locale: &str, key: &str, count: Option<u64>, args: &[(&str, &str)]) -> String {
    text_with(locale, key, count, args, &[])
}

/// `text_in`, with `made` filled in too: text this layer made (a notification's line, filled into
/// the line naming its project), whose values were cleaned and isolated when it was made. Each is
/// its own run as a value is, keeping the isolates inside it.
pub fn text_with(
    locale: &str,
    key: &str,
    count: Option<u64>,
    args: &[(&str, &str)],
    made: &[(&str, &str)],
) -> String {
    let template = pick(locale, key, count)
        .or_else(|| pick("en", key, count))
        .unwrap_or(key);
    let values: Vec<String> = args
        .iter()
        .map(|(_, v)| isolate_in(locale, v))
        .chain(made.iter().map(|(_, v)| wrap(locale, v)))
        .collect();
    let args: Vec<(&str, &str)> = args
        .iter()
        .chain(made)
        .zip(&values)
        .map(|((k, _), v)| (*k, v.as_str()))
        .collect();
    render(locale, template, count, &args)
}

/// A template with `{{count}}` (in the language's digits) and `args` filled.
fn render(locale: &str, template: &str, count: Option<u64>, args: &[(&str, &str)]) -> String {
    match count {
        Some(n) => {
            let n = number(locale, n);
            let mut all: Vec<(&str, &str)> = vec![("count", n.as_str())];
            all.extend_from_slice(args);
            fill(template, &all)
        }
        None => fill(template, args),
    }
}

/// The text for `key` in the current language.
pub fn t(key: &str) -> String {
    text_in(current(), key, None, &[])
}

/// The text for `key` in the current language, `{{name}}`s filled.
pub fn t_args(key: &str, args: &[(&str, &str)]) -> String {
    text_in(current(), key, None, args)
}

/// `t_args`, with text this layer made filled in as well (`text_with`).
pub fn t_args_with(key: &str, args: &[(&str, &str)], made: &[(&str, &str)]) -> String {
    text_with(current(), key, None, args, made)
}

/// The plural form of `key` for `n` in the current language, `{{count}}`
/// written in its digits and the other `{{name}}`s filled.
pub fn t_count(key: &str, n: u64, args: &[(&str, &str)]) -> String {
    text_in(current(), key, Some(n), args)
}

/// The command line's text (mu-152), `src/i18n/locales/<language>/cli.json`, compiled in.
const CLI_FILES: [(&str, &str); 2] = [
    ("en", include_str!("../../src/i18n/locales/en/cli.json")),
    ("fa", include_str!("../../src/i18n/locales/fa/cli.json")),
];

/// One language's `cli.json`, parsed the first time that language is asked for, so a run in
/// English never parses Persian. None for a language without the file (or one that doesn't
/// parse, which the tests catch).
fn cli_file(locale: &str) -> Option<&'static Value> {
    static PARSED: [OnceLock<Option<Value>>; CLI_FILES.len()] =
        [const { OnceLock::new() }; CLI_FILES.len()];
    let i = CLI_FILES.iter().position(|(l, _)| *l == locale)?;
    PARSED[i]
        .get_or_init(|| serde_json::from_str(CLI_FILES[i].1).ok())
        .as_ref()
}

/// The command line's text for `key` in `locale`, falling back to English, then to the key.
/// Unlike `text_in`, `{{count}}` and `args` go in as they are: Latin digits (a count or a key is
/// copied into the next command) and no isolates (a terminal shows them as boxes). What reaches
/// the terminal has its controls escaped at the output (`escape_controls`).
pub fn cli_text(locale: &str, key: &str, count: Option<u64>, args: &[(&str, &str)]) -> String {
    let in_file = |l: &str| cli_file(l).and_then(|f| pick_in(f, l, key, count));
    let template = in_file(locale).or_else(|| in_file("en")).unwrap_or(key);
    cli_fill(template, count, args)
}

/// A command-line template with `{{count}}` (in Latin digits) and `args` filled in.
fn cli_fill(template: &str, count: Option<u64>, args: &[(&str, &str)]) -> String {
    match count {
        Some(n) => {
            let n = n.to_string();
            let mut all: Vec<(&str, &str)> = vec![("count", n.as_str())];
            all.extend_from_slice(args);
            fill(template, &all)
        }
        None => fill(template, args),
    }
}

/// `key`'s text in `locale`'s own `cli.json`, without falling back: whether that language has it.
pub fn cli_has(locale: &str, key: &str) -> Option<&'static str> {
    dig(cli_file(locale)?, key)
}

/// `s` as a terminal may be given it: what a command prints for a person shows a control in queue
/// text instead of obeying it. Every bidi control (`index::is_bidi_control`) is `<U+XXXX>`
/// (mu-152), so a crafted title can't reorder the line (Trojan Source). Every terminal control
/// (mu-221), C0 but newline and tab, DEL and C1, is a visible escape: ESC `\e`, CR `\r`, the rest
/// `\xNN` (BEL `\x07`, DEL `\x7f`, the C1 CSI `\x9b`), so a title can't clear the screen, hide a
/// line, set the window's title, make a link or write the clipboard (CSI, OSC 8, OSC 52). Newline
/// and tab stay: the output's own layout uses them. A CR before a newline goes, so a file with
/// CRLF line ends prints as plain lines.
pub fn escape_controls(s: &str) -> String {
    let shown_as_is = |c: char| c == '\n' || c == '\t';
    let escaped = |c: char| crate::index::is_bidi_control(c) || (c.is_control() && !shown_as_is(c));
    if !s.chars().any(escaped) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 16);
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' if chars.peek() == Some(&'\n') => {}
            '\r' => out.push_str(r"\r"),
            '\x1b' => out.push_str(r"\e"),
            c if crate::index::is_bidi_control(c) => {
                out.push_str(&format!("<U+{:04X}>", u32::from(c)));
            }
            c if escaped(c) => out.push_str(&format!(r"\x{:02x}", u32::from(c))),
            c => out.push(c),
        }
    }
    out
}

/// Tests: the core's own text in one language, on the calling thread only (tests run side by
/// side).
#[cfg(test)]
pub mod stub {
    use std::cell::RefCell;

    thread_local! {
        static APP: RefCell<Option<&'static str>> = const { RefCell::new(None) };
    }

    /// Clears the app's language when `app` ends, even by a failed assertion.
    struct ResetApp;
    impl Drop for ResetApp {
        fn drop(&mut self) {
            APP.with(|s| *s.borrow_mut() = None);
        }
    }

    /// Run `f` with the app (the core's own text) in `locale`, read from
    /// its real `native` file.
    pub fn app<R>(locale: &'static str, f: impl FnOnce() -> R) -> R {
        APP.with(|s| *s.borrow_mut() = Some(locale));
        let _reset = ResetApp;
        f()
    }

    pub(super) fn app_locale() -> Option<&'static str> {
        APP.with(|s| *s.borrow())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[test]
    fn every_file_parses_and_english_is_complete() {
        assert_eq!(files().len(), LOCALES.len());
        let en = &files()["en"];
        assert_eq!(dig(en, "tray.quit"), Some("Quit Muster"));
        assert_eq!(dig(&files()["fa"], "tray.quit"), Some("خروج از Muster"));
    }

    /// The UI's lists (src/i18n/locales.ts) and these stay the same.
    #[test]
    fn languages_match_the_ui() {
        let ts = include_str!("../../src/i18n/locales.ts");
        let list = |name: &str| -> Vec<String> {
            let start = ts.find(name).expect("the list is in locales.ts");
            let rest = &ts[start + name.len()..];
            let open = rest.find('[').expect("a list opens");
            let close = rest.find(']').expect("a list closes");
            rest[open + 1..close]
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };
        assert_eq!(list("export const LOCALES"), LOCALES);
        assert_eq!(list("export const READY: readonly Locale[] ="), READY);
        let rtl = list("const RTL: ReadonlySet<string> = new Set(");
        assert_eq!(rtl, RTL);
    }

    #[test]
    fn the_bundle_declares_every_language_so_macos_offers_one_per_app() {
        // Without CFBundleLocalizations macOS doesn't list Muster under Language & Region ›
        // Applications, and `defaults read <bundle id> AppleLanguages` is never set.
        let plist = include_str!("../Info.plist");
        let key = plist
            .find("<key>CFBundleLocalizations</key>")
            .expect("the plist lists the languages");
        let rest = &plist[key..];
        let array = &rest
            [rest.find("<array>").expect("an array")..rest.find("</array>").expect("it closes")];
        let listed: Vec<&str> = array
            .split("<string>")
            .skip(1)
            .map(|s| s.split("</string>").next().unwrap_or("").trim())
            .collect();
        assert_eq!(listed, LOCALES);
        assert!(plist.contains("<key>CFBundleDevelopmentRegion</key>\n  <string>en</string>"));
    }

    #[test]
    fn matches_macos_tags() {
        assert_eq!(match_tag("fa-IR"), Some("fa"));
        assert_eq!(match_tag("fa"), Some("fa"));
        assert_eq!(match_tag("en-GB"), Some("en"));
        assert_eq!(match_tag("en_US"), Some("en"));
        assert_eq!(match_tag("FA-ir"), Some("fa"));
        assert_eq!(match_tag("ar"), None);
        assert_eq!(match_tag("it-IT"), None);
        assert_eq!(match_tag(""), None);
    }

    #[test]
    fn reads_the_list_defaults_prints() {
        let printed = "(\n    \"fa-IR\",\n    \"en-US\",\n    de\n)\n";
        assert_eq!(defaults_list(printed), ["fa-IR", "en-US", "de"]);
        assert_eq!(defaults_list("(\n)\n"), Vec::<String>::new());
        assert_eq!(
            defaults_list("The domain/default pair of (-g, AppleLanguages) does not exist"),
            Vec::<String>::new()
        );
        assert_eq!(defaults_list(""), Vec::<String>::new());
    }

    #[test]
    fn the_setting_wins_then_macos_then_english() {
        let sys = vec!["it-IT".to_string(), "fa-IR".into(), "en-US".into()];
        let all = LOCALES;
        assert_eq!(resolve_among("en", &sys, &all), "en");
        assert_eq!(resolve_among("", &sys, &all), "fa");
        assert_eq!(resolve_among("", &["de-DE".to_string()], &all), "en");
        assert_eq!(resolve_among("", &[], &all), "en");
        // A setting from a newer or broken file: as if none.
        assert_eq!(resolve_among("xx", &sys, &all), "fa");
        // macOS's Persian isn't followed until Persian is offered.
        assert_eq!(resolve_among("", &sys, &["en"]), "en");
        assert_eq!(resolve("", &sys), "fa");
        assert_eq!(resolve(PSEUDO, &sys), PSEUDO, "while developing");
        assert!(valid_setting("") && valid_setting("fa") && valid_setting("en"));
        assert!(!valid_setting("fa-IR") && !valid_setting("de") && !valid_setting(" fa"));
        assert_eq!(stored("fa"), Some("fa"));
        assert_eq!(stored(""), None);
        assert_eq!(stored(PSEUDO), Some(PSEUDO), "while developing");
    }

    #[test]
    fn plural_categories_follow_cldr() {
        let forms = |l: &str, ns: &[u64]| ns.iter().map(|n| plural(l, *n)).collect::<Vec<_>>();
        assert_eq!(forms("en", &[0, 1, 2]), ["other", "one", "other"]);
        assert_eq!(
            forms("ru", &[1, 2, 5, 11, 21, 22, 25, 100, 111, 112]),
            ["one", "few", "many", "many", "one", "few", "many", "many", "many", "many"]
        );
        assert_eq!(
            forms("ar", &[0, 1, 2, 3, 10, 11, 99, 100, 102, 103, 111]),
            ["zero", "one", "two", "few", "few", "many", "many", "other", "other", "few", "many"]
        );
        assert_eq!(
            forms("fr", &[0, 1, 2, 1_000_000]),
            ["one", "one", "other", "many"]
        );
        assert_eq!(forms("pt-BR", &[0, 1, 2]), ["one", "one", "other"]);
        assert_eq!(
            forms("es", &[0, 1, 2, 2_000_000]),
            ["other", "one", "other", "many"]
        );
        assert_eq!(forms("fa", &[0, 1, 2]), ["one", "one", "other"]);
        assert_eq!(forms("ja", &[1, 2]), ["other", "other"]);
        assert_eq!(forms("de", &[1, 2]), ["one", "other"]);
    }

    #[test]
    fn looks_up_fills_and_falls_back() {
        assert_eq!(
            text_in("en", "tray.waiting", Some(1), &[]),
            "1 thing waits for you"
        );
        assert_eq!(
            text_in("en", "tray.waiting", Some(3), &[]),
            "3 things wait for you"
        );
        assert_eq!(
            text_in("fa", "tray.waiting", Some(3), &[]),
            "۳ مورد منتظر شماست"
        );
        // A language without the key (or the file): English. A key nobody has: the key.
        assert_eq!(text_in("xx", "tray.quit", None, &[]), "Quit Muster");
        assert_eq!(text_in("en", "no.such", None, &[]), "no.such");
        assert_eq!(
            fill("{{a}} and {{ b }}", &[("a", "x"), ("b", "y")]),
            "x and y"
        );
        assert_eq!(
            fill("{{a}} {{missing}} {{", &[("a", "x")]),
            "x {{missing}} {{"
        );
        assert_eq!(fill("é{{a}}ü", &[("a", "ß")]), "éßü");
        assert_eq!(number("fa", 1209), "۱۲۰۹");
        // Settings › Language's Digits (mu-151): either language, either digits.
        assert_eq!(digits_with(digits_code(Some("latin")), "fa", "12"), "12");
        assert_eq!(digits_with(digits_code(Some("persian")), "en", "12"), "۱۲");
        assert_eq!(digits_with(digits_code(None), "fa", "12"), "۱۲");
        assert_eq!(digits_with(digits_code(Some("other")), "en", "12"), "12");
        assert_eq!(number("en", 12), "12");
    }

    /// Every key the core names as a literal ("tray.quit") whose first part is a group of `en`
    /// is in `en`, as is or with plural forms: a typo would show the key itself in the menu.
    #[test]
    fn every_key_the_core_names_is_in_english() {
        let en = &files()["en"];
        let mut checked = 0;
        for src in [
            include_str!("tray.rs"),
            include_str!("notify.rs"),
            include_str!("app_menu.rs"),
        ] {
            for (i, _) in src.match_indices('"') {
                let rest = &src[i + 1..];
                let Some(end) = rest.find(|c: char| !(c.is_ascii_alphanumeric() || c == '.'))
                else {
                    continue;
                };
                let (key, after) = rest.split_at(end);
                let top = key.split('.').next().unwrap_or("");
                if !en.get(top).is_some_and(Value::is_object)
                    || !key.contains('.')
                    || !after.starts_with('"')
                {
                    continue;
                }
                let found = dig(en, key).is_some() || dig(en, &format!("{key}_other")).is_some();
                assert!(found, "{key} isn't in en/native.json");
                checked += 1;
            }
        }
        assert!(checked >= 6, "only {checked} keys found");
    }

    /// Lists and numbers the core fills in follow the language: its list separator, its digits.
    #[test]
    fn lists_and_numbers_follow_the_language() {
        assert_eq!(list(&["a", "b", "c"]), "a, b, c");
        assert_eq!(list::<&str>(&[]), "");
        assert_eq!(num(80), "80");
        stub::app("fa", || {
            assert_eq!(list(&["a", "b"]), "a، b");
            assert_eq!(num(80), "۸۰");
            assert_eq!(num(-1), "-۱");
        });
        assert_eq!(current(), "en");
        for l in LOCALES {
            assert!(pick(l, "list.separator", None).is_some(), "{l}");
        }
    }

    /// Right to left (Drover's task 0020): each value the core fills into its own text keeps its
    /// direction; counts and left-to-right languages are as written.
    #[test]
    fn isolates_filled_values_right_to_left() {
        assert_eq!(isolate_in("fa", "~/.ssh"), "\u{2068}~/.ssh\u{2069}");
        assert_eq!(isolate_in("fa", ""), "");
        assert_eq!(isolate_in("en", "~/.ssh"), "~/.ssh");
        assert_eq!(fill("{{a}}: {{b}}", &[("a", "x")]), "x: {{b}}");
        let count = text_in("fa", "tray.waiting", Some(3), &[]);
        assert!(!count.contains(['\u{2068}', '\u{2069}']), "{count}");
        stub::app("fa", || {
            assert_eq!(t_args("tray.show", &[("unused", "x")]), "نمایش Muster");
        });
    }

    /// mu-152: the command line's text, from `cli.json`: the language's, else English's, else
    /// the key; values and counts filled in as they are, in Latin digits, never isolated.
    #[test]
    fn the_command_lines_text_falls_back_and_fills_in_plainly() {
        let en = cli_text("en", "terminal.vscode", None, &[]);
        assert_eq!(
            en,
            "VS Code's terminal can't show right-to-left text; Terminal or iTerm2 can"
        );
        let fa = cli_text("fa", "terminal.vscode", None, &[]);
        assert!(fa.starts_with("ترمینال VS Code"), "{fa}");
        assert_eq!(cli_has("fa", "terminal.vscode"), Some(fa.as_str()));
        assert_eq!(cli_has("en", "terminal.vscode"), Some(en.as_str()));
        assert_eq!(cli_has("fa", "no.such"), None);
        assert_eq!(cli_has("de", "terminal.vscode"), None);
        // A language or a key nobody has: English, then the key.
        assert_eq!(cli_text("de", "terminal.vscode", None, &[]), en);
        assert_eq!(cli_text("fa", "no.such", Some(3), &[("a", "b")]), "no.such");
        // Counts and values as they are: Latin digits, no isolates, a value's controls kept for
        // the output's own escaping (`escape_controls`).
        let file: Value = serde_json::from_str(
            r#"{"n_one": "{{count}} تسک {{id}}", "n_other": "{{count}} تسک: {{id}}"}"#,
        )
        .unwrap();
        let fill_n = |n: u64, id: &str| {
            let t = pick_in(&file, "fa", "n", Some(n)).unwrap();
            cli_fill(t, Some(n), &[("id", id)])
        };
        assert_eq!(fill_n(1, "mu-1"), "1 تسک mu-1");
        assert_eq!(fill_n(1209, "~/.ssh"), "1209 تسک: ~/.ssh");
        assert_eq!(fill_n(2, "a\u{202e}b"), "2 تسک: a\u{202e}b");
        assert_eq!(cli_fill("{{id}}", None, &[]), "{{id}}");
    }

    /// Every cli.json parses, and holds no bidi control, written or escaped: a terminal shows
    /// one as a box.
    #[test]
    fn every_cli_file_parses_and_holds_no_bidi_control() {
        fn strings(v: &Value, out: &mut Vec<String>) {
            match v {
                Value::String(s) => out.push(s.clone()),
                Value::Object(m) => m.values().for_each(|v| strings(v, out)),
                _ => {}
            }
        }
        for (l, _) in CLI_FILES {
            let file = cli_file(l).unwrap_or_else(|| panic!("{l}/cli.json doesn't parse"));
            let mut all = Vec::new();
            strings(file, &mut all);
            assert!(!all.is_empty(), "{l}/cli.json is empty");
            for s in all {
                assert!(!s.chars().any(crate::index::is_bidi_control), "{l}: {s:?}");
            }
        }
        assert_eq!(
            CLI_FILES.map(|(l, _)| l),
            LOCALES,
            "a cli.json per language"
        );
    }

    /// mu-152 D3: each bidi control as `<U+XXXX>`, the rest as it is.
    #[test]
    fn escape_controls_names_every_bidi_control() {
        for c in bidi_cases().controls {
            let name = format!("<U+{:04X}>", u32::from(c));
            assert_eq!(escape_controls(&format!("a{c}b")), format!("a{name}b"));
        }
        assert_eq!(escape_controls("Pay \u{202e}ecivni"), "Pay <U+202E>ecivni");
        assert_eq!(escape_controls("\u{61c}"), "<U+061C>");
        assert_eq!(escape_controls("تسک mu-1 ~/.ssh\n"), "تسک mu-1 ~/.ssh\n");
        assert_eq!(escape_controls(""), "");
    }

    /// mu-221: every terminal control as a visible escape, newline and tab as they are.
    #[test]
    fn escape_controls_shows_every_terminal_control() {
        let c0 = (0u32..0x20).filter(|&n| n != 0x0a && n != 0x09);
        for n in c0.chain([0x7f]).chain(0x80..0xa0) {
            let c = char::from_u32(n).unwrap();
            let name = match c {
                '\x1b' => r"\e".to_string(),
                '\r' => r"\r".to_string(),
                _ => format!(r"\x{n:02x}"),
            };
            assert_eq!(escape_controls(&format!("a{c}b")), format!("a{name}b"));
        }
        assert_eq!(escape_controls("a\tb\nc\n"), "a\tb\nc\n");
        assert_eq!(
            escape_controls("\x1b[2J\x1b]52;c;ZXZpbA==\x07\u{9b}31m\x7f\0"),
            r"\e[2J\e]52;c;ZXZpbA==\x07\x9b31m\x7f\x00"
        );
        // CRLF line ends are line ends; a lone CR, or one at the end, is shown.
        assert_eq!(escape_controls("a\r\nb\r\n"), "a\nb\n");
        assert_eq!(escape_controls("a\rb\r"), r"a\rb\r");
        assert_eq!(escape_controls("a\r\r\nb"), "a\\r\nb");
        // Both kinds in one string.
        assert_eq!(escape_controls("\u{202e}x\x1by"), r"<U+202E>x\ey");
        // Printable text past Latin-1 is left alone, C1's neighbours too.
        assert_eq!(escape_controls("\u{7e}\u{a0}é—"), "\u{7e}\u{a0}é—");
    }

    /// The bidi cases both sides share (src/i18n/fixtures/bidi-controls.json): every control by
    /// code point, and values with the text each must come out as in the core (`clean`): (about,
    /// value, clean). `window_keeps` are the values the window fills in otherwise (an isolate
    /// pair the value closes itself), with the window's text: (value, window).
    pub(crate) struct BidiCases {
        pub controls: Vec<char>,
        pub cases: Vec<(String, String, String)>,
        pub window_keeps: Vec<(String, String)>,
    }

    pub(crate) fn bidi_cases() -> BidiCases {
        let v: Value =
            serde_json::from_str(include_str!("../../src/i18n/fixtures/bidi-controls.json"))
                .expect("the fixture is JSON");
        let controls = v["controls"]
            .as_array()
            .expect("a list of controls")
            .iter()
            .map(|c| {
                let hex = c
                    .as_str()
                    .and_then(|s| s.strip_prefix("U+"))
                    .expect("U+XXXX");
                char::from_u32(u32::from_str_radix(hex, 16).expect("hex")).expect("a character")
            })
            .collect();
        let field = |c: &Value, k: &str| c[k].as_str().expect(k).to_string();
        let all = v["cases"].as_array().expect("a list of cases");
        let cases = all
            .iter()
            .map(|c| (field(c, "about"), field(c, "value"), field(c, "clean")))
            .collect();
        let window_keeps = all
            .iter()
            .filter(|c| field(c, "window") != field(c, "clean"))
            .map(|c| (field(c, "value"), field(c, "window")))
            .collect();
        BidiCases {
            controls,
            cases,
            window_keeps,
        }
    }

    /// The shared list is the core's set, no more and no less, so both sides drop the same ones.
    #[test]
    fn the_shared_bidi_controls_are_the_cores_set() {
        let listed = bidi_cases().controls;
        assert_eq!(listed.len(), 12);
        let set: Vec<char> = (0..=0x10FFFF)
            .filter_map(char::from_u32)
            .filter(|&c| crate::index::is_bidi_control(c))
            .collect();
        let mut sorted = listed.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, set);
    }

    /// mu-152 AC9: a filled-in value loses every bidi control before it's isolated, in every
    /// language, so a title's lone U+2069 can't close the template's isolate early.
    #[test]
    fn isolate_in_drops_every_bidi_control_from_a_value() {
        let b = bidi_cases();
        // The split with the window, pinned (R6): the core drops a pair the window keeps.
        assert!(!b.window_keeps.is_empty());
        for (value, window) in &b.window_keeps {
            assert!(window.contains(['\u{2066}', '\u{2069}']), "{window:?}");
            assert!(!isolate_in("en", value).contains(['\u{2066}', '\u{2069}']));
        }
        for c in &b.controls {
            let name = format!("U+{:04X}", u32::from(*c));
            assert_eq!(isolate_in("fa", &c.to_string()), "", "{name} alone, fa");
            assert_eq!(isolate_in("en", &c.to_string()), "", "{name} alone, en");
            let inside = format!("mu{c}-1");
            assert_eq!(
                isolate_in("fa", &inside),
                "\u{2068}mu-1\u{2069}",
                "{name}, fa"
            );
            assert_eq!(isolate_in("en", &inside), "mu-1", "{name}, en");
        }
        for (about, value, clean) in &b.cases {
            assert_eq!(isolate_in("en", value), *clean, "{about}, en");
            let fa = if clean.is_empty() {
                String::new()
            } else {
                format!("\u{2068}{clean}\u{2069}")
            };
            assert_eq!(isolate_in("fa", value), fa, "{about}, fa");
            // Through the text a value is filled into, in both languages.
            let en_text = text_in(
                "en",
                "notify.line.own",
                None,
                &[("id", "mu-1"), ("title", value)],
            );
            assert_eq!(en_text, format!("mu-1 {clean}: your task"), "{about}");
            let fa_text = text_in(
                "fa",
                "notify.line.own",
                None,
                &[("id", "mu-1"), ("title", value)],
            );
            assert_eq!(
                fa_text,
                format!("\u{2068}mu-1\u{2069} {fa}: تسک خودتان"),
                "{about}"
            );
        }
    }

    /// Text this layer made, filled into another (a notification's line in its project): its own
    /// run like a value, keeping the isolates its values got; a value beside it is still cleaned.
    #[test]
    fn made_text_keeps_its_own_isolates_inside_another() {
        let line = text_in(
            "fa",
            "notify.line.own",
            None,
            &[("id", "mu-1"), ("title", "\u{2069}x")],
        );
        assert_eq!(line, "\u{2068}mu-1\u{2069} \u{2068}x\u{2069}: تسک خودتان");
        let at = text_with(
            "fa",
            "notify.inProject",
            None,
            &[("project", "\u{202e}A")],
            &[("line", &line)],
        );
        assert_eq!(at, format!("در \u{2068}A\u{2069}: \u{2068}{line}\u{2069}"));
        let en = text_with(
            "en",
            "notify.inProject",
            None,
            &[("project", "A")],
            &[("line", "l")],
        );
        assert_eq!(
            en,
            text_in(
                "en",
                "notify.inProject",
                None,
                &[("project", "A"), ("line", "l")]
            )
        );
    }
}
