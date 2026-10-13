//! A task's external links and its comments (task 0017), the one implementation the app and the
//! `muster` command share. Links live in the front matter, one line:
//! `links: Spec <https://…>; <https://…>`. Comments live in the task's `## Comments` section,
//! above the Log, written only by the tool: `- **<who>, <stamp>:** <text>`, with every line after
//! the first indented by two spaces (file contract rule 3). Every change holds the queue's lock,
//! writes atomically, stamps the time itself and logs who did it. Task text is data: it's parsed,
//! never run, and a link is opened only when it's a plain https address.

use crate::ops::{free_text_line, one_line, set_front};
use crate::queue::{self, actor, append_log, find_task, front_matter, with_lock, write_atomic};
use serde::Serialize;
use std::path::Path;

/// At most this many links on a task.
pub const MAX_LINKS: usize = 20;
const MAX_URL: usize = 2000;
const MAX_LINK_TITLE: usize = 200;

/// An external link from the task's `links:` line. `title` is "" when it has none.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub title: String,
    pub url: String,
}

/// One comment from the task's `## Comments`: who wrote it, the tool's stamp, and the text as
/// written (the continuation indent taken off).
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Comment {
    pub who: String,
    pub at: String,
    pub text: String,
}

/// A link's address as muster keeps and opens it: https with a host, at most 2,000 characters,
/// printable ASCII without spaces, `<`, `>`, `"`, `\` or a backtick (percent-encode anything
/// else), no user name or password in it, and no percent-encoding in the host. The scheme is
/// written in lowercase.
pub fn clean_url(input: &str) -> Result<String, String> {
    let u = input.trim();
    if u.is_empty() {
        return Err("the link is empty".into());
    }
    if let Some(c) = u
        .chars()
        .find(|c| !c.is_ascii_graphic() || "<>\"\\`".contains(*c))
    {
        return Err(format!(
            "the link has {c:?} in it; percent-encode spaces, quotes, <, >, backslashes and non-ASCII characters"
        ));
    }
    if u.len() > MAX_URL {
        return Err(format!("the link is longer than {MAX_URL} characters"));
    }
    let rest = match u.get(..8) {
        Some(s) if s.eq_ignore_ascii_case("https://") => &u[8..],
        _ => return Err("only https links (https://…)".into()),
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or("");
    if host.is_empty() {
        return Err("the link has no host".into());
    }
    if host.contains('@') {
        return Err("a link can't carry a user name or password".into());
    }
    // A percent-encoded host could read as one host and open as another.
    if host.contains('%') {
        return Err("the link's host can't be percent-encoded".into());
    }
    Ok(format!("https://{rest}"))
}

/// A link's title: one line of at most 200 characters, without `<`, `>` or `;` (they separate
/// links in the front matter). Empty is fine (no title).
fn clean_title(title: &str) -> Result<String, String> {
    if title.trim().is_empty() {
        return Ok(String::new());
    }
    let t = one_line(title, MAX_LINK_TITLE, "the link's title")?;
    if t.contains(['<', '>', ';']) {
        return Err(
            "the link's title can't have <, > or ; in it (they separate links in the front matter)"
                .into(),
        );
    }
    Ok(t)
}

/// The links in a `links:` value, in order: `title <url>` or `<url>`, separated by `;` outside
/// the brackets. A hand-written entry without brackets is read as an address (`check` says when
/// it isn't a plain https one).
pub fn parse_links(value: &str) -> Vec<Link> {
    let mut entries = Vec::new();
    let mut inside = false;
    let mut start = 0;
    for (i, c) in value.char_indices() {
        match c {
            '<' => inside = true,
            '>' => inside = false,
            ';' if !inside => {
                entries.push(&value[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    entries.push(&value[start..]);
    entries
        .into_iter()
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .map(|e| match (e.strip_suffix('>'), e.rfind('<')) {
            (Some(head), Some(i)) => Link {
                title: e[..i].trim().to_string(),
                url: head[i + 1..].trim().to_string(),
            },
            _ => Link {
                title: String::new(),
                url: e.to_string(),
            },
        })
        .collect()
}

/// The `links:` value for `links`, as `parse_links` reads it back.
fn links_value(links: &[Link]) -> String {
    links
        .iter()
        .map(|l| {
            if l.title.is_empty() {
                format!("<{}>", l.url)
            } else {
                format!("{} <{}>", l.title, l.url)
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
}

/// Task `id`'s text and its links, read under the lock the caller holds.
fn read_links(root: &Path, id: &str) -> Result<(String, Vec<Link>), String> {
    let text = queue::read_body(root, id)?;
    let (fm, _) = front_matter(&text).ok_or("the task has no front matter")?;
    let links = parse_links(fm.get("links").map(String::as_str).unwrap_or(""));
    Ok((text, links))
}

/// Add a link to task `id` as `who`: an https address (`clean_url`) and an optional title.
/// Refused when the task already has that address, or has `MAX_LINKS` links. Logged.
pub fn add_link(
    root: &Path,
    id: &str,
    url: &str,
    title: &str,
    who: &str,
    stamp: &str,
) -> Result<Link, String> {
    let who = actor(who)?;
    let link = Link {
        url: clean_url(url)?,
        title: clean_title(title)?,
    };
    let line = free_text_line(format!("{stamp} {who}: linked {}", link.url), "the link")?;
    with_lock(root, || {
        let path = find_task(root, id)?;
        let (text, mut links) = read_links(root, id)?;
        if links.iter().any(|l| l.url == link.url) {
            return Err(format!("{id} already links {}", link.url));
        }
        if links.len() >= MAX_LINKS {
            return Err(format!(
                "{id} has {MAX_LINKS} links already; remove one first (muster link {id} --remove <n>)"
            ));
        }
        links.push(link.clone());
        let new = set_front(&text, "links", &links_value(&links))?;
        write_atomic(&path, &append_log(&new, &line))?;
        Ok(link.clone())
    })
}

/// Remove one of task `id`'s links, by its position (`2`, from 1) or its address as written.
/// Logged. Returns the link that went.
pub fn remove_link(
    root: &Path,
    id: &str,
    which: &str,
    who: &str,
    stamp: &str,
) -> Result<Link, String> {
    let who = actor(who)?;
    let which = which.trim();
    if which.is_empty() {
        return Err("say which link: its number (2) or its address".into());
    }
    with_lock(root, || {
        let path = find_task(root, id)?;
        let (text, mut links) = read_links(root, id)?;
        let at = match which.parse::<usize>() {
            Ok(n) if n >= 1 && n <= links.len() => n - 1,
            Ok(_) => return Err(format!("{id} has no link {which}")),
            Err(_) => links
                .iter()
                .position(|l| l.url == which)
                .ok_or_else(|| format!("{id} has no link {which}"))?,
        };
        let gone = links.remove(at);
        let new = set_front(&text, "links", &links_value(&links))?;
        // A hand-written address could read as anything; then the log names its number.
        let line = free_text_line(format!("{stamp} {who}: unlinked {}", gone.url), "the link")
            .unwrap_or_else(|_| format!("{stamp} {who}: unlinked link {}", at + 1));
        write_atomic(&path, &append_log(&new, &line))?;
        Ok(gone)
    })
}

/// The address to open for the owner's click on one of task `id`'s links: only an address the
/// task has, and only a plain https one (`clean_url`, which `check` holds hand-written links to).
pub fn link_to_open(root: &Path, id: &str, url: &str) -> Result<String, String> {
    let (_, links) = read_links(root, id)?;
    let link = links
        .iter()
        .find(|l| l.url == url)
        .ok_or_else(|| format!("{id} has no link {url:?}"))?;
    clean_url(&link.url).map_err(|e| format!("{e}; muster opens only plain https links"))
}

/// What `check` says about a hand-written link muster won't open, or None for a fine one.
pub fn link_problem(link: &Link) -> Option<String> {
    let e = clean_url(&link.url).err()?;
    let shown: String = link.url.chars().take(200).collect();
    Some(format!(
        "a link that isn't a plain https link: {shown:?} ({e})"
    ))
}

/// Would this line of free text read as tool-written stage history, as it is or as a log item?
fn reads_as_stage(line: &str) -> bool {
    let l = line.trim_start();
    queue::stage_line(l).is_some() || queue::stage_line(&format!("- {l}")).is_some()
}

/// The comment's first line: who, the stamp, and the text after it. `- **<who>, <stamp>:** <text>`;
/// the stamp is the first `, YYYY-MM-DD HH:MM:**` in the line.
fn comment_head(line: &str) -> Option<Comment> {
    let rest = line.strip_prefix("- **")?;
    let mut from = 0;
    while let Some(k) = rest[from..].find(":**") {
        let i = from + k;
        let sep = i.checked_sub(18).and_then(|s| rest.get(s..i - 16));
        let at = i.checked_sub(16).and_then(|s| rest.get(s..i));
        if let (Some(", "), Some(at)) = (sep, at) {
            let who = &rest[..i - 18];
            if !who.trim().is_empty() && crate::ops::stamp_minutes(at).is_some() {
                let text = &rest[i + 3..];
                return Some(Comment {
                    who: who.to_string(),
                    at: at.to_string(),
                    text: text.strip_prefix(' ').unwrap_or(text).to_string(),
                });
            }
        }
        from = i + 3;
    }
    None
}

/// The task's comments, oldest first, from its `## Comments` section (`queue::comments_span`).
/// Lines that aren't a comment or its continuation are left out.
pub fn comments(text: &str) -> Vec<Comment> {
    let Some((_, start, end)) = queue::comments_span(text) else {
        return Vec::new();
    };
    let mut out: Vec<Comment> = Vec::new();
    let mut open = false;
    for line in text[start..end].lines() {
        if let Some(c) = comment_head(line) {
            out.push(c);
            open = true;
        } else if let Some(c) = out.last_mut().filter(|_| open) {
            if let Some(rest) = line.strip_prefix("  ") {
                c.text.push('\n');
                c.text.push_str(rest);
            } else if line.trim().is_empty() {
                // A blank line an editor trimmed: kept only if the comment goes on after it.
                c.text.push('\n');
            } else {
                open = false;
            }
        }
    }
    for c in &mut out {
        let kept = c.text.trim_end_matches('\n').len();
        c.text.truncate(kept);
    }
    out
}

/// The text with `item` added at the end of its Comments section, or in a new one right above
/// the Log (at the end, without one).
fn with_comment(text: &str, item: &str) -> String {
    let trimmed = |s: &str| s.trim_end_matches(['\n', '\r']).to_string();
    match queue::comments_span(text) {
        Some((_, _, end)) => {
            let (head, tail) = text.split_at(end);
            if tail.is_empty() {
                format!("{}\n{item}\n", trimmed(head))
            } else {
                format!("{}\n{item}\n\n{tail}", trimmed(head))
            }
        }
        None => match queue::log_heading(text) {
            Some(at) => format!(
                "{}\n\n## Comments\n{item}\n\n{}",
                trimmed(&text[..at]),
                &text[at..]
            ),
            None => format!("{}\n\n## Comments\n{item}\n", trimmed(text)),
        },
    }
}

/// Add a comment to task `id` as `who`, at any stage, whoever holds it: comments don't touch
/// the write-up. The text is cleaned like an answer (control characters out, newline kept,
/// trimmed, 4,000 characters), refused when a line of it would read as a stage change, and
/// written with its continuation lines indented. Logged as `commented`. Returns the task's path.
pub fn add_comment(
    root: &Path,
    id: &str,
    text: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let who = actor(who)?;
    let text = queue::clean_text_as(text, "the comment")?;
    if let Some(line) = text.lines().find(|l| reads_as_stage(l)) {
        return Err(format!(
            "the comment has a line that would read as a stage change in the log (\"<who>: stage <from> → <to>\"); only muster pass, return and stage write those: {line:?}"
        ));
    }
    let item = format!("- **{who}, {stamp}:** {}", queue::indent_text(&text));
    let line = free_text_line(format!("{stamp} {who}: commented"), "the comment")?;
    with_lock(root, || {
        let path = find_task(root, id)?;
        let body = queue::read_body(root, id)?;
        let mut new = append_log(&with_comment(&body, &item), &line);
        // The holder's comment is the holder being heard from: it keeps the claim alive.
        let t = queue::parse_task("", root, &queue::folder_of(root, &path), &path, &body, 0);
        if t.is_some_and(|t| {
            t.stage == "building" && !t.claimed_by.is_empty() && t.claimed_by == who
        }) {
            new = set_front(&new, "heartbeat", stamp)?;
        }
        write_atomic(&path, &new)?;
        Ok(path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{read_state_at, read_task, Cache, STATUS_FOLDERS};
    use std::fs;

    const T: &str = "2026-10-12 14:05";

    fn queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        fs::write(d.path().join("README.md"), "q").unwrap();
        d
    }

    /// Task 0001 in todo/, with `fm` added to its front matter and `write_up` above the Log.
    fn put(d: &Path, fm: &str, write_up: &str) -> std::path::PathBuf {
        let p = d.join("todo/0001-t.md");
        fs::write(
            &p,
            format!("---\nid: 0001\ntype: code\npriority: normal\nstage: ready\nstatus: todo\nclaimed-by:\nwaiting_on:\n{fm}---\n\n# Linked\n\n{write_up}## Log\n- 2026-10-12 09:00 capture: created\n"),
        )
        .unwrap();
        p
    }

    fn text(p: &Path) -> String {
        fs::read_to_string(p).unwrap()
    }

    fn problems(d: &Path) -> Vec<String> {
        read_state_at(&[("p".into(), d.to_path_buf())], &mut Cache::default(), T).problems
    }

    #[test]
    fn urls_are_https_plain_and_bounded() {
        assert_eq!(
            clean_url(" https://example.com/a?b=c;d#e ").unwrap(),
            "https://example.com/a?b=c;d#e"
        );
        assert_eq!(
            clean_url("HTTPS://Example.com").unwrap(),
            "https://Example.com"
        );
        let long = format!("https://x.com/{}", "a".repeat(MAX_URL - 14));
        assert_eq!(long.len(), MAX_URL);
        assert_eq!(clean_url(&long).unwrap(), long);
        let refused = |u: &str, want: &str| {
            let e = clean_url(u).unwrap_err();
            assert!(e.contains(want), "{u:?}: {e}");
        };
        refused(&format!("{long}a"), "longer than 2000");
        refused("", "empty");
        refused("http://example.com", "only https");
        refused("javascript:alert(1)", "only https");
        refused("file:///etc/passwd", "only https");
        refused("example.com", "only https");
        refused("https://", "no host");
        refused("https:///path", "no host");
        refused("https://user:pw@example.com/", "user name or password");
        // A percent-encoded host could show as one host and open as another.
        refused("https://ex%61mple.com/", "host can't be percent-encoded");
        refused(
            "https://example.com%2Fevil.com/",
            "host can't be percent-encoded",
        );
        assert_eq!(
            clean_url("https://example.com/a%20b?q=%2F#%C3%A9").unwrap(),
            "https://example.com/a%20b?q=%2F#%C3%A9"
        );
        refused("https://example.com/a b", "percent-encode");
        refused("https://example.com/\u{7}", "percent-encode");
        refused("https://example.com/\nx", "percent-encode");
        refused("https://de.wikipedia.org/wiki/Straße", "percent-encode");
        refused("https://x.com/<script>", "percent-encode");
        refused("https://x.com/a>b", "percent-encode");
        refused("https://x.com/\"", "percent-encode");
    }

    #[test]
    fn links_are_added_listed_and_removed_in_the_front_matter() {
        let d = queue();
        let p = put(d.path(), "", "");
        let l = add_link(
            d.path(),
            "0001",
            "https://example.com/spec",
            "The spec",
            "owner",
            T,
        )
        .unwrap();
        assert_eq!(
            l,
            Link {
                title: "The spec".into(),
                url: "https://example.com/spec".into()
            }
        );
        add_link(
            d.path(),
            "0001",
            "https://github.com/o/r/pull/7",
            "",
            "wt-a",
            T,
        )
        .unwrap();
        add_link(
            d.path(),
            "0001",
            "https://x.com/a;b",
            "  Two\n  lines ",
            "wt-a",
            T,
        )
        .unwrap();
        let after = text(&p);
        assert!(
            after.contains("\nlinks: The spec <https://example.com/spec>; <https://github.com/o/r/pull/7>; Two lines <https://x.com/a;b>\n---\n"),
            "{after}"
        );
        assert!(after.ends_with(&format!(
            "- {T} owner: linked https://example.com/spec\n- {T} wt-a: linked https://github.com/o/r/pull/7\n- {T} wt-a: linked https://x.com/a;b\n"
        )));
        let t = read_task(d.path(), "0001").unwrap();
        assert_eq!(
            t.links
                .iter()
                .map(|l| (l.title.as_str(), l.url.as_str()))
                .collect::<Vec<_>>(),
            [
                ("The spec", "https://example.com/spec"),
                ("", "https://github.com/o/r/pull/7"),
                ("Two lines", "https://x.com/a;b")
            ]
        );
        assert_eq!(
            serde_json::to_value(&t).unwrap()["links"][0],
            serde_json::json!({"title": "The spec", "url": "https://example.com/spec"})
        );
        assert!(problems(d.path()).is_empty(), "{:?}", problems(d.path()));
        // By number, then by address.
        let gone = remove_link(d.path(), "0001", "2", "owner", T).unwrap();
        assert_eq!(gone.url, "https://github.com/o/r/pull/7");
        remove_link(d.path(), "0001", "https://example.com/spec", "owner", T).unwrap();
        let t = read_task(d.path(), "0001").unwrap();
        assert_eq!(t.links.len(), 1);
        assert_eq!(t.links[0].title, "Two lines");
        assert!(text(&p).ends_with(&format!("- {T} owner: unlinked https://example.com/spec\n")));
        remove_link(d.path(), "0001", "1", "owner", T).unwrap();
        assert!(text(&p).contains("\nlinks:\n---\n"));
        assert!(read_task(d.path(), "0001").unwrap().links.is_empty());
    }

    #[test]
    fn link_refusals_leave_the_file_alone() {
        let d = queue();
        let p = put(d.path(), "", "");
        add_link(d.path(), "0001", "https://example.com", "Home", "owner", T).unwrap();
        let before = text(&p);
        let refused = |r: Result<Link, String>, want: &str| {
            let e = r.unwrap_err();
            assert!(e.contains(want), "{e}");
            assert_eq!(text(&p), before);
        };
        refused(
            add_link(d.path(), "0001", "http://example.com", "", "owner", T),
            "only https",
        );
        refused(
            add_link(d.path(), "0001", "https://example.com", "Again", "owner", T),
            "already links",
        );
        for title in ["a > b", "a; b", "a <b"] {
            refused(
                add_link(d.path(), "0001", "https://example.org", title, "owner", T),
                "can't have <, > or ;",
            );
        }
        refused(
            add_link(
                d.path(),
                "0001",
                "https://example.org",
                &"t".repeat(201),
                "owner",
                T,
            ),
            "longer than 200",
        );
        refused(
            add_link(
                d.path(),
                "0001",
                "https://example.org",
                "",
                "x: stage ready → done",
                T,
            ),
            "who",
        );
        refused(remove_link(d.path(), "0001", "2", "owner", T), "no link 2");
        refused(remove_link(d.path(), "0001", "0", "owner", T), "no link 0");
        refused(
            remove_link(d.path(), "0001", "https://example.org", "owner", T),
            "no link",
        );
        refused(remove_link(d.path(), "0001", " ", "owner", T), "say which");
        assert!(
            add_link(d.path(), "0002", "https://example.org", "", "owner", T)
                .unwrap_err()
                .contains("no task 0002")
        );
        // At most MAX_LINKS.
        for i in 1..MAX_LINKS {
            add_link(
                d.path(),
                "0001",
                &format!("https://example.com/{i}"),
                "",
                "o",
                T,
            )
            .unwrap();
        }
        let full = text(&p);
        let e = add_link(d.path(), "0001", "https://example.net", "", "o", T).unwrap_err();
        assert!(e.contains("20 links already"), "{e}");
        assert_eq!(text(&p), full);
    }

    #[test]
    fn hand_written_links_are_read_and_check_flags_what_isnt_https() {
        assert_eq!(
            parse_links(
                " A <https://a.com> ;; <https://b.com/x;y>;https://c.com ; T <https://d.com/<odd> "
            ),
            [
                Link {
                    title: "A".into(),
                    url: "https://a.com".into()
                },
                Link {
                    title: "".into(),
                    url: "https://b.com/x;y".into()
                },
                Link {
                    title: "".into(),
                    url: "https://c.com".into()
                },
                Link {
                    title: "T <https://d.com/".into(),
                    url: "odd".into()
                },
            ]
        );
        assert!(parse_links("").is_empty());
        let d = queue();
        let p = put(
            d.path(),
            "links: Good <https://ok.com>; Plain <http://plain.com>; <javascript:alert(1)>\n",
            "",
        );
        let all = problems(d.path()).join("\n");
        assert!(all.contains("http://plain.com"), "{all}");
        assert!(all.contains("javascript:alert(1)"), "{all}");
        assert!(!all.contains("https://ok.com"), "{all}");
        assert_eq!(problems(d.path()).len(), 2);
        // A bad one can still be removed, and then check is clean.
        remove_link(d.path(), "0001", "http://plain.com", "owner", T).unwrap();
        remove_link(d.path(), "0001", "2", "owner", T).unwrap();
        assert!(problems(d.path()).is_empty());
        assert!(text(&p).contains("\nlinks: Good <https://ok.com>\n"));
    }

    #[test]
    fn only_a_plain_https_link_the_task_has_is_opened() {
        let d = queue();
        put(
            d.path(),
            "links: A <https://a.com/x>; B <http://b.com>; C <HTTPS://c.com>\n",
            "",
        );
        assert_eq!(
            link_to_open(d.path(), "0001", "https://a.com/x").unwrap(),
            "https://a.com/x"
        );
        assert!(link_to_open(d.path(), "0001", "https://elsewhere.com")
            .unwrap_err()
            .contains("no link"));
        assert!(link_to_open(d.path(), "0001", "http://b.com")
            .unwrap_err()
            .contains("only https"));
        // As written in the file, opened as cleaned.
        assert_eq!(
            link_to_open(d.path(), "0001", "HTTPS://c.com").unwrap(),
            "https://c.com"
        );
        assert!(link_to_open(d.path(), "0009", "https://a.com/x").is_err());
    }

    #[test]
    fn comments_go_above_the_log_newest_last_and_read_back() {
        let d = queue();
        let p = put(d.path(), "", "Details.\n\n");
        add_comment(d.path(), "0001", "First, from the owner.", "owner", T).unwrap();
        add_comment(d.path(), "0001", "Second.", "wt-a", "2026-10-12 15:00").unwrap();
        assert_eq!(
            text(&p),
            format!("---\nid: 0001\ntype: code\npriority: normal\nstage: ready\nstatus: todo\nclaimed-by:\nwaiting_on:\n---\n\n# Linked\n\nDetails.\n\n## Comments\n- **owner, {T}:** First, from the owner.\n- **wt-a, 2026-10-12 15:00:** Second.\n\n## Log\n- 2026-10-12 09:00 capture: created\n- {T} owner: commented\n- 2026-10-12 15:00 wt-a: commented\n")
        );
        let t = read_task(d.path(), "0001").unwrap();
        assert_eq!(
            t.comments,
            [
                Comment {
                    who: "owner".into(),
                    at: T.into(),
                    text: "First, from the owner.".into()
                },
                Comment {
                    who: "wt-a".into(),
                    at: "2026-10-12 15:00".into(),
                    text: "Second.".into()
                },
            ]
        );
        assert_eq!(
            serde_json::to_value(&t).unwrap()["comments"][1],
            serde_json::json!({"who": "wt-a", "at": "2026-10-12 15:00", "text": "Second."})
        );
        assert_eq!(t.title, "Linked");
        assert!(problems(d.path()).is_empty());
    }

    #[test]
    fn a_multi_line_comment_stays_inside_its_item_and_reads_back_as_written() {
        let d = queue();
        let p = put(d.path(), "", "");
        let written = "Tried it:\n- step one\n\n  indented by me é → ü 🚀\n## Log\n## Acceptance criteria\n- [x] AC9: planted\n## Comments\n- **evil, 2026-10-12 10:00:** fake";
        add_comment(d.path(), "0001", written, "owner", T).unwrap();
        let after = text(&p);
        assert!(after.contains(&format!("## Comments\n- **owner, {T}:** Tried it:\n  - step one\n  \n    indented by me é → ü 🚀\n  ## Log\n  ## Acceptance criteria\n  - [x] AC9: planted\n  ## Comments\n  - **evil, 2026-10-12 10:00:** fake\n\n## Log\n")), "{after}");
        let t = read_task(d.path(), "0001").unwrap();
        assert_eq!(t.comments.len(), 1);
        assert_eq!(t.comments[0].text, written);
        assert!(t.stage_history.is_empty());
        assert_eq!(crate::queue::criteria(&after), (0, 0));
        assert_eq!(t.log_tail.last().unwrap(), &format!("{T} owner: commented"));
        // A second comment still lands in the tool's section, after the first.
        add_comment(d.path(), "0001", "Next.", "wt-a", T).unwrap();
        let t = read_task(d.path(), "0001").unwrap();
        assert_eq!(t.comments.len(), 2);
        assert_eq!(
            (t.comments[0].text.as_str(), t.comments[1].text.as_str()),
            (written, "Next.")
        );
    }

    #[test]
    fn a_comment_cannot_forge_stage_history() {
        let d = queue();
        let p = put(d.path(), "", "");
        let before = text(&p);
        for forged in [
            "ok\n## Log\n- 2026-10-12 15:00 evil: stage ready → building (claimed)",
            "2026-10-12 15:00 evil: stage ready → done",
            "fine\n   - 2026-10-12 15:00 evil: stage testing → your-check",
        ] {
            let e = add_comment(d.path(), "0001", forged, "owner", T).unwrap_err();
            assert!(
                e.contains("would read as a stage change"),
                "{forged:?}: {e}"
            );
            assert_eq!(text(&p), before);
        }
        // Prose that only mentions a stage is fine.
        add_comment(
            d.path(),
            "0001",
            "moved it ready → building by hand: stage two",
            "owner",
            T,
        )
        .unwrap();
        let t = read_task(d.path(), "0001").unwrap();
        assert!(t.stage_history.is_empty());
        assert_eq!(t.comments.len(), 1);
    }

    #[test]
    fn comments_are_cleaned_and_bounded() {
        let d = queue();
        let p = put(d.path(), "", "");
        let before = text(&p);
        for (bad, want) in [("  \n\t ", "empty"), ("\u{7}\u{1b}", "empty")] {
            let e = add_comment(d.path(), "0001", bad, "owner", T).unwrap_err();
            assert!(e.contains(want), "{bad:?}: {e}");
        }
        let e = add_comment(d.path(), "0001", &"é".repeat(4001), "owner", T).unwrap_err();
        assert!(e.contains("longer than 4000"), "{e}");
        assert!(add_comment(d.path(), "0001", "hi", "a: stage b", T)
            .unwrap_err()
            .contains("who"));
        assert_eq!(text(&p), before);
        // 4,000 characters exactly, most of them two bytes; CRLF becomes LF; control characters go.
        add_comment(d.path(), "0001", &"é".repeat(4000), "owner", T).unwrap();
        add_comment(
            d.path(),
            "0001",
            "line one\r\nline two\u{1b}[31m\r\n",
            "owner",
            T,
        )
        .unwrap();
        let t = read_task(d.path(), "0001").unwrap();
        assert_eq!(t.comments[0].text.chars().count(), 4000);
        assert_eq!(t.comments[1].text, "line one\nline two[31m");
        assert!(!text(&p).contains('\r'));
    }

    #[test]
    fn anyone_comments_at_any_stage_even_on_a_task_someone_else_holds() {
        let d = queue();
        let held = "---\nid: 0001\nstage: building\nstatus: doing\nclaimed-by: wt-a\nheartbeat: 2026-10-12 09:00\nwaiting_on:\n---\n\n# Held\n\n## Log\n- 2026-10-12 09:00 wt-a: stage ready → building (claimed)\n";
        fs::write(d.path().join("doing/0001-t.md"), held).unwrap();
        assert_eq!(
            add_comment(d.path(), "0001", "Looks close.", "owner", T).unwrap(),
            "doing/0001-t.md"
        );
        add_comment(d.path(), "0001", "Agreed.", "wt-b", T).unwrap();
        let t = read_task(d.path(), "0001").unwrap();
        assert_eq!(
            (
                t.stage.as_str(),
                t.claimed_by.as_str(),
                t.heartbeat.as_str()
            ),
            ("building", "wt-a", "2026-10-12 09:00"),
            "the claim and its lease are the holder's"
        );
        assert_eq!(t.comments.len(), 2);
        assert_eq!(t.stage_history.len(), 1);
        // The holder's own comment is the holder being heard from: it renews the lease.
        add_comment(d.path(), "0001", "Halfway.", "wt-a", "2026-10-12 16:30").unwrap();
        let t = read_task(d.path(), "0001").unwrap();
        assert_eq!(
            (
                t.heartbeat.as_str(),
                t.claimed_by.as_str(),
                t.comments.len()
            ),
            ("2026-10-12 16:30", "wt-a", 3)
        );
        // Done, and a flat-layout task without a Log: the section and a Log are made.
        let flat = tempfile::tempdir().unwrap();
        let f = flat.path().join("0001-t.md");
        fs::write(
            &f,
            "---\nid: 0001\nstage: done\nstatus: done\n---\n\n# Bare\n\nNo log yet.",
        )
        .unwrap();
        add_comment(flat.path(), "0001", "After the fact.", "owner", T).unwrap();
        assert_eq!(
            text(&f),
            format!("---\nid: 0001\nstage: done\nstatus: done\n---\n\n# Bare\n\nNo log yet.\n\n## Comments\n- **owner, {T}:** After the fact.\n\n## Log\n- {T} owner: commented\n")
        );
        assert_eq!(read_task(flat.path(), "0001").unwrap().comments.len(), 1);
    }

    #[test]
    fn crlf_files_and_sections_after_the_log() {
        let d = queue();
        let p = d.path().join("todo/0001-t.md");
        // A Comments heading after the Log isn't the tool's: a new one goes above the Log.
        fs::write(
            &p,
            "---\nid: 0001\nstage: ready\nstatus: todo\n---\n\r\n# Windows\r\n\r\nText.\r\n\r\n## Log\r\n- 2026-10-12 09:00 capture: created\r\n\r\n## Comments\r\n- **old, 2026-10-01 09:00:** after the log\r\n",
        )
        .unwrap();
        add_comment(d.path(), "0001", "Mine.", "owner", T).unwrap();
        let after = text(&p);
        assert!(
            after.contains(&format!("Text.\n\n## Comments\n- **owner, {T}:** Mine.\n\n## Log\r\n- 2026-10-12 09:00 capture: created\r\n")),
            "{after:?}"
        );
        assert!(
            after.ends_with(&format!(
                "{T} owner: commented\n\n## Comments\r\n- **old, 2026-10-01 09:00:** after the log\r\n"
            )),
            "{after:?}"
        );
        let t = read_task(d.path(), "0001").unwrap();
        assert_eq!(t.comments.len(), 1);
        assert_eq!(t.comments[0].who, "owner");
        add_comment(d.path(), "0001", "Again.", "owner", T).unwrap();
        assert_eq!(read_task(d.path(), "0001").unwrap().comments.len(), 2);
    }

    #[test]
    fn a_question_asked_later_goes_above_the_comments() {
        let d = queue();
        let p = put(d.path(), "", "");
        add_comment(d.path(), "0001", "A comment first.", "owner", T).unwrap();
        crate::queue::ask(d.path(), "0001", "Which port?", "wt-a", "aaaa1111", T).unwrap();
        add_comment(d.path(), "0001", "Then another.", "owner", T).unwrap();
        let after = text(&p);
        let (q, c, l) = (
            after.find("## Questions").unwrap(),
            after.find("## Comments").unwrap(),
            after.find("## Log").unwrap(),
        );
        assert!(q < c && c < l, "{after}");
        let t = read_task(d.path(), "0001").unwrap();
        assert_eq!((t.questions.len(), t.comments.len()), (1, 2));
    }

    #[test]
    fn concurrent_comments_all_land() {
        for layout in ["todo", ""] {
            let d = tempfile::tempdir().unwrap();
            if !layout.is_empty() {
                for f in STATUS_FOLDERS {
                    fs::create_dir(d.path().join(f)).unwrap();
                }
            }
            fs::write(
                d.path().join(layout).join("0001-t.md"),
                "---\nid: 0001\nstage: ready\nstatus: todo\n---\n\n# Busy\n\n## Log\n- 2026-10-12 09:00 capture: created\n",
            )
            .unwrap();
            let go = std::sync::Arc::new(std::sync::Barrier::new(8));
            let runs: Vec<_> = (0..8)
                .map(|i| {
                    let (go, root) = (go.clone(), d.path().to_path_buf());
                    std::thread::spawn(move || {
                        go.wait();
                        add_comment(
                            &root,
                            "0001",
                            &format!("comment {i}"),
                            &format!("wt-{i}"),
                            T,
                        )
                    })
                })
                .collect();
            for r in runs {
                r.join().unwrap().unwrap();
            }
            let t = read_task(d.path(), "0001").unwrap();
            let mut texts: Vec<_> = t.comments.iter().map(|c| c.text.clone()).collect();
            texts.sort();
            assert_eq!(
                texts,
                (0..8).map(|i| format!("comment {i}")).collect::<Vec<_>>()
            );
            let body = queue::read_body(d.path(), "0001").unwrap();
            assert_eq!(body.matches(": commented\n").count(), 8);
            assert_eq!(body.matches("## Comments").count(), 1);
        }
    }

    #[test]
    fn comment_lines_are_read_strictly() {
        let section = |items: &str| {
            comments(&format!(
                "---\nid: 0001\n---\n\n# T\n\n## Comments\n{items}\n## Log\n"
            ))
        };
        // The stamp is the first `, <stamp>:**`; a who with a comma or colon stays whole.
        let c = section("- **wt-a, (x), 2026-10-12 10:00:** text: with **bold**:** in it\n");
        assert_eq!(
            c,
            [Comment {
                who: "wt-a, (x)".into(),
                at: "2026-10-12 10:00".into(),
                text: "text: with **bold**:** in it".into()
            }]
        );
        // Not comments: no stamp, a stamp that isn't a date, no who, prose; a trimmed blank line
        // inside one is kept.
        let c = section("- **x:** no stamp\n- **x, 2026-13-40 10:00:** bad date\n- **, 2026-10-12 10:00:** no who\nprose\n- **o, 2026-10-12 10:00:** a\n\n  b\n");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].text, "a\n\nb");
        assert!(comments("---\nid: 0001\n---\n\n# T\n\n## Log\n").is_empty());
    }
}
