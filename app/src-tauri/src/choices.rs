//! Pick-one questions (task 0049): a question asked with 2 to 6 options, the asker's pick and what
//! it does with no answer. They're written under the question as plain indented lines, so a reader
//! of file contract v1 (`bin/task`, Drover) sees ordinary text:
//!
//! ```text
//! - [ ] **Q3 (wt-a, session aaaa1111, 2026-10-13 09:00):** How long may a lease be?
//!   - (a) 1 h to 7 d (my pick)
//!   - (b) Any length
//!   With no answer: (a)
//! ```
//!
//! They're read back from those lines, so a question written that way by hand has options too.

use serde::Serialize;

pub const MAX_OPTIONS: usize = 6;
pub const MAX_OPTION_CHARS: usize = 200;
const LETTERS: [char; MAX_OPTIONS] = ['a', 'b', 'c', 'd', 'e', 'f'];
const MY_PICK: &str = " (my pick)";
const NO_ANSWER: &str = "With no answer: ";

/// A question's options, as asked or as read back from its lines.
#[derive(Serialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct Choices {
    /// In order: the first is (a).
    pub options: Vec<String>,
    /// The asker's recommendation, as an index into `options`.
    pub pick: Option<usize>,
    /// What the asker does if no answer comes, as an index into `options`.
    pub no_answer: Option<usize>,
    /// The question without these lines, as `read` finds it (what the app shows above the
    /// options); empty in what `clean` makes for asking.
    pub question: String,
}

/// An option's letter: `a` for the first.
pub fn letter(i: usize) -> char {
    LETTERS.get(i).copied().unwrap_or('?')
}

/// Which of `count` options `s` names: a letter (`b`, `B`, `(b)`) or a number from 1 (`2`).
pub fn which(s: &str, count: usize) -> Result<usize, String> {
    let t = s.trim();
    let t = t
        .strip_prefix('(')
        .and_then(|t| t.strip_suffix(')'))
        .unwrap_or(t)
        .to_lowercase();
    let i = match t.parse::<usize>() {
        Ok(n) => n.checked_sub(1),
        Err(_) => {
            let mut chars = t.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => LETTERS.iter().position(|&l| l == c),
                _ => None,
            }
        }
    };
    i.filter(|&i| i < count).ok_or_else(|| {
        format!(
            "{:?} isn't one of the options: name one by its letter, a to {}, or its number, 1 to {count}",
            s.chars().take(20).collect::<String>(),
            letter(count.saturating_sub(1))
        )
    })
}

/// One option as it's written: one line (control characters and runs of spaces become one space),
/// trimmed, at most `MAX_OPTION_CHARS` characters, and not ending in the pick's marker.
fn clean_option(text: &str, i: usize) -> Result<String, String> {
    let t: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    let l = letter(i);
    if t.is_empty() {
        return Err(format!("option ({l}) is empty"));
    }
    if t.chars().count() > MAX_OPTION_CHARS {
        return Err(format!(
            "option ({l}) is longer than {MAX_OPTION_CHARS} characters"
        ));
    }
    if t.to_lowercase().ends_with(MY_PICK.trim_start()) {
        return Err(format!(
            "option ({l}) ends with \"(my pick)\": name your pick with --pick instead"
        ));
    }
    Ok(t)
}

/// The options a question is asked with, checked: none, or 2 to `MAX_OPTIONS` of them, each
/// cleaned and different from the others, with `pick` and `no_answer` (each optional) naming one.
pub fn clean(
    options: &[String],
    pick: Option<&str>,
    no_answer: Option<&str>,
) -> Result<Choices, String> {
    if options.is_empty() {
        return match (pick, no_answer) {
            (None, None) => Ok(Choices::default()),
            (Some(_), _) => Err("--pick names one of the options: give them with --option".into()),
            (_, Some(_)) => {
                Err("--no-answer names one of the options: give them with --option".into())
            }
        };
    }
    if options.len() < 2 || options.len() > MAX_OPTIONS {
        return Err(format!(
            "a question with options has 2 to {MAX_OPTIONS} of them, not {}",
            options.len()
        ));
    }
    let options = options
        .iter()
        .enumerate()
        .map(|(i, o)| clean_option(o, i))
        .collect::<Result<Vec<_>, _>>()?;
    for (i, o) in options.iter().enumerate() {
        if let Some(j) = options[..i]
            .iter()
            .position(|p| p.to_lowercase() == o.to_lowercase())
        {
            return Err(format!(
                "options ({}) and ({}) are the same",
                letter(j),
                letter(i)
            ));
        }
    }
    let named = |flag: &str, s: Option<&str>| {
        s.map(|s| which(s, options.len()).map_err(|e| format!("{flag}: {e}")))
            .transpose()
    };
    Ok(Choices {
        pick: named("--pick", pick)?,
        no_answer: named("--no-answer", no_answer)?,
        options,
        question: String::new(),
    })
}

/// The lines that go under the question, without the item's indent (the caller indents them with
/// the question, contract rule 3): `- (a) …`, the pick marked `(my pick)`, then
/// `With no answer: (a)`. Empty without options.
pub fn lines(c: &Choices) -> String {
    let mut out: Vec<String> = c
        .options
        .iter()
        .enumerate()
        .map(|(i, o)| {
            let mine = if c.pick == Some(i) { MY_PICK } else { "" };
            format!("- ({}) {o}{mine}", letter(i))
        })
        .collect();
    if let Some(i) = c.no_answer {
        out.push(format!("{NO_ANSWER}({})", letter(i)));
    }
    out.join("\n")
}

/// `- (b) text` as (1, "text").
fn option_line(line: &str) -> Option<(usize, &str)> {
    let rest = line.strip_prefix("- (")?;
    let mut chars = rest.chars();
    let c = chars.next()?;
    let i = LETTERS.iter().position(|&l| l == c)?;
    let text = chars.as_str().strip_prefix(") ")?;
    (!text.trim().is_empty()).then_some((i, text))
}

/// A question's options, from its text as written (`Question::text`, the indent taken off): the
/// run of `- (a) …` lines, in order from (a), 2 to `MAX_OPTIONS` of them, that ends the question
/// (before its escalation and answers, after its first line), with an optional
/// `With no answer: (x)` after them naming one. None when the question has no such run, so it
/// reads as text.
pub fn read(text: &str) -> Option<Choices> {
    let lines: Vec<&str> = text.split('\n').collect();
    let end = lines
        .iter()
        .position(|l| l.starts_with("**Escalated to owner") || l.starts_with("**A ("))
        .unwrap_or(lines.len());
    let (last, fallback) = match end.checked_sub(1).map(|i| lines[i].strip_prefix(NO_ANSWER)) {
        Some(Some(named)) => (end - 1, Some(named)),
        _ => (end, None),
    };
    let (top, _) = option_line(lines.get(last.checked_sub(1)?)?)?;
    let count = top + 1;
    if count < 2 || last < count + 1 {
        return None;
    }
    let first = last - count;
    let mut options = Vec::new();
    let mut pick = None;
    for (i, line) in lines[first..last].iter().enumerate() {
        let (j, text) = option_line(line)?;
        if j != i {
            return None;
        }
        let text = match text.strip_suffix(MY_PICK) {
            Some(t) => {
                pick = pick.or(Some(i));
                t
            }
            None => text,
        };
        options.push(text.to_string());
    }
    let no_answer = match fallback {
        Some(named) => Some(which(named, count).ok()?),
        None => None,
    };
    let question = lines[..first]
        .iter()
        .chain(&lines[end..])
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    Some(Choices {
        options,
        pick,
        no_answer,
        question,
    })
}

/// The answer picking `pick` (as `which` reads it) writes: `(b) <the option's text>`, then the
/// extra words after " — ".
pub fn answer_text(c: &Choices, pick: &str, extra: &str) -> Result<String, String> {
    let i = which(pick, c.options.len())?;
    let chosen = format!("({}) {}", letter(i), c.options[i]);
    Ok(match extra.trim() {
        "" => chosen,
        more => format!("{chosen} — {more}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn which_takes_a_letter_or_a_number_and_only_an_option() {
        for (s, want) in [("a", 0), ("B", 1), ("(c)", 2), (" 3 ", 2), ("1", 0)] {
            assert_eq!(which(s, 3), Ok(want), "{s:?}");
        }
        for bad in ["d", "4", "0", "", "ab", "(a", "-1", "é", "z"] {
            let e = which(bad, 3).unwrap_err();
            assert!(e.contains("isn't one of the options"), "{bad:?}: {e}");
            assert!(e.contains("a to c") && e.contains("1 to 3"), "{e}");
        }
    }

    #[test]
    fn clean_keeps_2_to_6_one_line_options_and_a_pick_and_fallback_that_name_one() {
        let c = clean(
            &opts(&["  Lease 1 h\tto\n7 d ", "Any length\u{1b}[31m"]),
            Some("a"),
            Some("2"),
        )
        .unwrap();
        assert_eq!(c.options, opts(&["Lease 1 h to 7 d", "Any length [31m"]));
        assert_eq!((c.pick, c.no_answer), (Some(0), Some(1)));
        assert_eq!(clean(&[], None, None).unwrap(), Choices::default());
        let six = opts(&["1", "2", "3", "4", "5", "6"]);
        assert_eq!(clean(&six, Some("f"), None).unwrap().pick, Some(5));
        let line = "é".repeat(MAX_OPTION_CHARS);
        assert_eq!(
            clean(&[line.clone(), "b".into()], None, None)
                .unwrap()
                .options[0],
            line
        );
    }

    #[test]
    fn clean_refuses_too_few_too_many_long_empty_same_and_a_pick_that_isnt_one() {
        let e = |o: &[&str], p: Option<&str>, n: Option<&str>| clean(&opts(o), p, n).unwrap_err();
        assert!(e(&["only"], None, None).contains("2 to 6 of them, not 1"));
        assert!(e(&["1", "2", "3", "4", "5", "6", "7"], None, None).contains("not 7"));
        let long = "x".repeat(MAX_OPTION_CHARS + 1);
        assert!(e(&["a", &long], None, None).contains("option (b) is longer than 200 characters"));
        assert!(e(&[" \u{7} ", "b"], None, None).contains("option (a) is empty"));
        assert!(e(&["Yes", " yes "], None, None).contains("options (a) and (b) are the same"));
        assert!(e(&["Yes (MY PICK)", "No"], None, None).contains("--pick instead"));
        assert!(
            e(&["a", "b"], Some("c"), None).starts_with("--pick: \"c\" isn't one of the options")
        );
        assert!(e(&["a", "b"], None, Some("3")).starts_with("--no-answer: \"3\" isn't one"));
        assert!(e(&[], Some("a"), None).contains("--pick names one of the options"));
        assert!(e(&[], None, Some("a")).contains("--no-answer names one of the options"));
    }

    #[test]
    fn lines_and_read_round_trip_and_read_finds_only_a_run_that_ends_the_question() {
        let mut c = clean(&opts(&["One", "Two", "Three"]), Some("b"), Some("a")).unwrap();
        assert_eq!(
            lines(&c),
            "- (a) One\n- (b) Two (my pick)\n- (c) Three\nWith no answer: (a)"
        );
        let text = format!(
            "Which?\n- (a) not an option, the question's own\n{}\n**Escalated to owner (capture, 2026-10-13 09:05).**",
            lines(&c)
        );
        c.question = "Which?\n- (a) not an option, the question's own\n**Escalated to owner (capture, 2026-10-13 09:05).**".into();
        assert_eq!(read(&text), Some(c));
        let plain = clean(&opts(&["x", "y"]), None, None).unwrap();
        let read_back = read(&format!("Q?\n{}", lines(&plain))).unwrap();
        assert_eq!(
            (read_back.options, read_back.pick, read_back.no_answer),
            (plain.options, None, None)
        );
        assert_eq!(read_back.question, "Q?");
        for not in [
            "Which?",
            "- (a) one\n- (b) two",     // no question line above them
            "Q?\n- (a) one",            // one option
            "Q?\n- (b) one\n- (c) two", // not from (a)
            "Q?\n- (a) one\n- (c) two", // a gap
            "Q?\n- (a) one\n- (b) two\nmore text", // they don't end the question
            "Q?\n- (a) one\n- (b) two\nWith no answer: (c)", // a fallback that isn't one
            "Q?\n- (a) one\n- (b)",     // an empty option
        ] {
            assert_eq!(read(not), None, "{not:?}");
        }
    }

    #[test]
    fn answer_text_is_the_options_letter_and_text_then_the_extra_words() {
        let c = clean(&opts(&["Lease 1 h to 7 d", "Any length"]), None, None).unwrap();
        assert_eq!(answer_text(&c, "b", "").unwrap(), "(b) Any length");
        assert_eq!(
            answer_text(&c, "1", "  but 12 h by default ").unwrap(),
            "(a) Lease 1 h to 7 d — but 12 h by default"
        );
        assert!(answer_text(&c, "c", "")
            .unwrap_err()
            .contains("isn't one of the options"));
    }
}
