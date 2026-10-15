//! What the Testing stage's records share (0027's review of record, mu-131's gate records): the
//! tool's sections in a task file, found by their heading with fenced code skipped (L10), the
//! commit a record names, and who may record one.

use crate::queue;
use crate::session::{is_owner, same_holder};

/// The `## ` headings of `text` that aren't inside a fenced code block (``` or ~~~), each as
/// (byte offset of its line, the line without its line break). Only a fence that closes in the
/// write-up hides headings: one still open where the tool's Comments (or, without them, its Log)
/// start hides nothing, so a write-up's stray fence can't hide the tool's sections below it (each
/// record would add another copy). Fences are the write-up's alone: the tool's Comments, Log and
/// Questions content (continuation lines indented two spaces) neither open nor close one, and
/// headings from the Comments on count plainly, as `queue::comments_span` finds them. A backtick
/// fence's info string can't hold a backtick (CommonMark), so ```x` is text, not a fence.
pub(crate) fn headings(text: &str) -> Vec<(usize, &str)> {
    let tail = queue::comments_span(text)
        .map(|(at, _, _)| at)
        .or_else(|| queue::log_heading(text))
        .unwrap_or(text.len());
    let Some(questions) = queue::questions_span(text) else {
        return scan(text, tail, (0, 0));
    };
    let out = scan(text, tail, questions);
    // A `## Questions` the write-up quotes inside closed fenced code isn't the tool's: scan again
    // with its lines read as the write-up's.
    let head = text[..questions.0]
        .trim_end_matches('\n')
        .rfind('\n')
        .map_or(0, |i| i + 1);
    if out.iter().any(|(at, _)| *at == head) {
        out
    } else {
        scan(text, tail, (0, 0))
    }
}

/// `headings`, with fences read only above `tail` and outside the `questions` byte range.
fn scan(text: &str, tail: usize, questions: (usize, usize)) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    // The headings inside the open fence: dropped when it closes, kept when it never does.
    let mut hidden = Vec::new();
    let mut pos = 0;
    for line in text.split_inclusive('\n') {
        let tool = pos >= tail || (questions.0..questions.1).contains(&pos);
        let bare = line.trim_end_matches(['\n', '\r']);
        let lead = bare.len() - bare.trim_start_matches(' ').len();
        let rest = &bare[lead..];
        let mark = rest
            .chars()
            .next()
            .filter(|c| !tool && (*c == '`' || *c == '~'));
        let run = mark.map_or(0, |c| rest.chars().take_while(|x| *x == c).count());
        let heading = line.starts_with("## ").then(|| (pos, line.trim_end()));
        match (fence, mark) {
            // Opened by three or more of one mark, indented three spaces at most.
            (None, Some(c))
                if lead <= 3 && run >= 3 && !(c == '`' && rest[run..].contains('`')) =>
            {
                fence = Some((c, run))
            }
            // Closed by at least as many of the same mark, and nothing after them.
            (Some((c, n)), Some(m))
                if m == c && lead <= 3 && run >= n && rest[run..].trim().is_empty() =>
            {
                fence = None;
                hidden.clear();
            }
            (None, _) => out.extend(heading),
            (Some(_), _) => hidden.extend(heading),
        }
        pos += line.len();
    }
    out.extend(hidden);
    out
}

/// The tool's section headed `head` in `text`, as (start of its heading line, end) byte offsets:
/// the last such heading above the last `## Log` (or the last one when there's no Log), ending at
/// the next `## ` heading. Headings inside fenced code don't count, either way.
pub(crate) fn section_span(text: &str, head: &str) -> Option<(usize, usize)> {
    let heads = headings(text);
    let upto = heads
        .iter()
        .rposition(|(_, h)| *h == "## Log")
        .unwrap_or(heads.len());
    let i = heads[..upto].iter().rposition(|(_, h)| *h == head)?;
    let end = heads.get(i + 1).map_or(text.len(), |(p, _)| *p);
    Some((heads[i].0, end))
}

/// How many lines of `text` are the heading `head`, outside fenced code.
pub(crate) fn heading_count(text: &str, head: &str) -> usize {
    headings(text).iter().filter(|(_, h)| *h == head).count()
}

/// `text` with the tool's section `sect` (headed `head`): in place of the task's last one when
/// `replace`, else right above the tool's Comments (0017), else above the Log, or at the end
/// without either.
pub(crate) fn put_section(text: &str, head: &str, sect: &str, replace: bool) -> String {
    let below = queue::comments_span(text)
        .map(|(at, _, _)| at)
        .or_else(|| queue::log_heading(text));
    let (top, tail) = match section_span(text, head).filter(|_| replace) {
        Some((a, b)) => (&text[..a], &text[b..]),
        None => match below {
            Some(at) => (&text[..at], &text[at..]),
            None => (text, ""),
        },
    };
    let top = top.trim_end_matches('\n');
    if tail.is_empty() {
        format!("{top}\n\n{sect}")
    } else {
        format!("{top}\n\n{sect}\n{tail}")
    }
}

/// The commit a record says it was taken at, as typed: 7 to 64 hex characters, lowercased.
pub(crate) fn clean_head(raw: &str, what: &str) -> Result<String, String> {
    let head = raw.trim().to_lowercase();
    if !(7..=64).contains(&head.len()) || !head.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!(
            "head must be the commit {what} (7 to 64 hex characters), got {:?}",
            raw.chars().take(70).collect::<String>()
        ));
    }
    Ok(head)
}

/// The first 7 characters of a commit.
pub(crate) fn short(sha: &str) -> String {
    sha.chars().take(7).collect()
}

/// Why task `id` has no branch to record against.
pub(crate) fn not_started(id: &str, what: &str) -> String {
    format!("{id} wasn't started on its own branch (muster start), so there's no branch to {what}")
}

/// Refused unless `who` may record `what` (the review, a gate) on task `t`: it's started, in
/// Testing, not paused or blocked, and `who` isn't any holder of its builder chain (0047: Testing
/// is a different session's). The owner may always.
pub(crate) fn check_tester(id: &str, t: &queue::Task, who: &str, what: &str) -> Result<(), String> {
    if t.branch.is_empty() {
        return Err(not_started(id, "check"));
    }
    if t.stage != "testing" {
        return Err(format!(
            "{id} is in {}; {what} runs in Testing",
            queue::stage_label(&t.stage)
        ));
    }
    if !t.paused.is_empty() {
        return Err(crate::pause::refusal(id, &t.paused));
    }
    if t.blocked {
        return Err(format!(
            "{id} is blocked on {}; unblock it first",
            t.waiting_on
        ));
    }
    if !is_owner(who) {
        if let Some(b) = t
            .builders
            .iter()
            .chain([&t.built_by, &t.gated_by])
            .find(|b| !b.is_empty() && same_holder(b, who))
        {
            return Err(format!(
                "{id} was built by {b}; {what} is a different session's (the tester's, as Testing is), or the owner's"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_inside_fenced_code_are_not_sections() {
        let text = "# T\n\n```md\n## Gates\n```\n\n~~~~\n## Review\n~~~\nstill in\n~~~~\n## Gates\nreal\n## Log\n- x\n";
        let heads: Vec<&str> = headings(text).into_iter().map(|(_, h)| h).collect();
        assert_eq!(heads, ["## Gates", "## Log"]);
        let (a, b) = section_span(text, "## Gates").unwrap();
        assert_eq!(&text[a..b], "## Gates\nreal\n");
        assert_eq!(heading_count(text, "## Gates"), 1);
        assert_eq!(heading_count(text, "## Review"), 0);
        // Only a fence that closes hides headings: one left open hides nothing (R2).
        assert!(section_span("# T\n```\n## Gates\n", "## Gates").is_some());
        let open = "# T\n```md\n```rust\nx\n```\n```\n## Review\nr\n## Gates\ng\n## Log\n";
        let heads: Vec<&str> = headings(open).into_iter().map(|(_, h)| h).collect();
        assert_eq!(heads, ["## Review", "## Gates", "## Log"]);
        // A backtick fence's info string can't hold a backtick (CommonMark): ```x` is text, so
        // the fence below it is a pair of its own and Gates stays out of it (R10).
        let inline = "# T\n```x`\n## Gates\ng\n```\ncode\n```\n";
        let (a, b) = section_span(inline, "## Gates").unwrap();
        assert_eq!(&inline[a..b], "## Gates\ng\n```\ncode\n```\n");
        // ~~~ may: it opens a fence.
        assert!(section_span("# T\n~~~x`\n## Gates\n~~~\n", "## Gates").is_none());
        // A fence indented four spaces is code text, not a fence.
        assert!(section_span("# T\n    ```\n## Gates\nx\n", "## Gates").is_some());
    }

    #[test]
    fn fences_are_the_write_ups_alone_and_end_where_the_tools_text_starts() {
        let heads = |t| -> Vec<&str> { headings(t).into_iter().map(|(_, h)| h).collect() };
        // A comment's code block (indented by muster comment) doesn't close the write-up's stray
        // fence, so Review and Gates stay the tool's (R2).
        let comment = "# T\n```\n## Review\nr\n## Gates\ng\n## Comments\n- **a, 2026-10-14 10:00:** see\n  ```\n  x\n  ```\n## Log\n- x\n";
        assert_eq!(
            heads(comment),
            ["## Review", "## Gates", "## Comments", "## Log"]
        );
        // Without Comments the Log is where the write-up ends.
        let log = "# T\n```\n## Gates\ng\n## Log\n- x\n```\n";
        assert_eq!(heads(log), ["## Gates", "## Log"]);
        // Nor does an answer's fence in Questions below the tool's sections.
        let answer = "# T\n```\n## Review\nr\n## Questions\n- [x] **Q1 (a, 2026-10-14 10:00):** q\n  **A (owner, 2026-10-14 10:05):** this\n  ```\n## Log\n- x\n";
        assert_eq!(heads(answer), ["## Review", "## Questions", "## Log"]);
        // An answer's fence doesn't open one either: the Gates after it count.
        let opens = "# T\n## Questions\n- [x] **Q1 (a, 2026-10-14 10:00):** q\n  ```\n## Gates\ng\n```\n## Review\n```\n## Log\n";
        assert_eq!(heads(opens), ["## Questions", "## Gates", "## Log"]);
        // A `## Questions` the write-up quotes in closed fenced code is the write-up's text, and
        // its fence lines pair as written.
        let quoted = "# T\n```md\n## Questions\n- q\n```\n## Gates\ng\n```\ncode\n```\n## Log\n";
        assert_eq!(heads(quoted), ["## Gates", "## Log"]);
    }

    #[test]
    fn the_section_is_the_last_one_above_the_log_and_put_replaces_or_inserts_it() {
        let text =
            "# T\n\n## Gates\nold\n\n## Log\n- 2026-10-14 10:00 a: created\n## Gates\nquoted\n";
        let (a, b) = section_span(text, "## Gates").unwrap();
        assert_eq!(&text[a..b], "## Gates\nold\n\n");
        let new = put_section(text, "## Gates", "## Gates\nnew\n", true);
        assert!(new.starts_with("# T\n\n## Gates\nnew\n\n## Log\n"), "{new}");
        let fresh = put_section("# T\n\n## Log\n- x\n", "## Gates", "## Gates\nnew\n", false);
        assert_eq!(fresh, "# T\n\n## Gates\nnew\n\n## Log\n- x\n");
        assert_eq!(
            put_section("# T\n", "## Gates", "## Gates\n", false),
            "# T\n\n## Gates\n"
        );
    }

    #[test]
    fn clean_head_takes_7_to_64_hex_characters() {
        assert_eq!(clean_head(" ABCDEF1 ", "x").unwrap(), "abcdef1");
        for bad in ["abc", "zzzzzzz", &"a".repeat(65), ""] {
            assert!(clean_head(bad, "you checked").is_err(), "{bad}");
        }
        assert!(clean_head("xyz", "you checked")
            .unwrap_err()
            .contains("the commit you checked"));
    }
}
