//! Changing tasks: the one implementation of Muster's rules for creating and managing tasks,
//! used by the app's window and by the `muster` command line alike. Every change holds the
//! queue's lock, writes atomically, stamps the time itself and logs who did it.

use crate::queue::{self, find_task, front_matter, with_lock, write_atomic, STATUS_FOLDERS};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const KINDS: [&str; 3] = ["bug", "task", "idea"];
pub const PRIORITIES: [&str; 3] = ["high", "normal", "low"];
/// Statuses the owner may set. `doing` is a claim, and claims belong to agents.
pub const OWNER_STATUSES: [&str; 4] = ["inbox", "todo", "blocked", "done"];
const MAX_TITLE: usize = 200;
const MAX_BODY: usize = 50_000;
const MAX_ATTACHMENT: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct NewTask {
    pub title: String,
    pub kind: String,
    pub priority: String,
    pub body: String,
    pub tags: Vec<String>,
    pub est_time: String,
    pub est_tokens: String,
}

/// Agent time: a number and m, h or d ("45m", "1.5h", "2d"). Empty is fine (no estimate).
pub fn clean_time(input: &str) -> Result<String, String> {
    let t = input.trim().to_lowercase().replace(' ', "");
    if t.is_empty() {
        return Ok(t);
    }
    let (num, unit) = t.split_at(t.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(t.len()));
    let unit = match unit {
        "m" | "min" | "mins" => "m",
        "h" | "hr" | "hrs" | "hour" | "hours" => "h",
        "d" | "day" | "days" => "d",
        _ => {
            return Err(format!(
                "time {input:?}: use a number and m, h or d, like 45m, 2h or 1.5d"
            ))
        }
    };
    let n: f64 = num
        .parse()
        .map_err(|_| format!("time {input:?}: use a number and m, h or d, like 45m, 2h or 1.5d"))?;
    if !(n > 0.0 && n <= 1000.0) {
        return Err(format!("time {input:?} is out of range"));
    }
    Ok(format!("{}{unit}", trim_num(n)))
}

/// Tokens: a number with an optional k or M ("400k", "1.2M", "50000"). Empty is fine.
pub fn clean_tokens(input: &str) -> Result<String, String> {
    let t = input.trim().replace([' ', ','], "");
    if t.is_empty() {
        return Ok(t);
    }
    let (num, unit) = t.split_at(t.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(t.len()));
    let mult = match unit.to_lowercase().as_str() {
        "" => 1.0,
        "k" => 1e3,
        "m" => 1e6,
        _ => {
            return Err(format!(
                "tokens {input:?}: use a number with k or M, like 400k or 1.2M"
            ))
        }
    };
    let n: f64 = num
        .parse()
        .map_err(|_| format!("tokens {input:?}: use a number with k or M, like 400k or 1.2M"))?;
    let total = n * mult;
    if !(1.0..=1e9).contains(&total) {
        return Err(format!("tokens {input:?} is out of range"));
    }
    Ok(if total >= 1e6 {
        format!("{}M", trim_num(total / 1e6))
    } else if total >= 1e3 {
        format!("{}k", trim_num(total / 1e3))
    } else {
        trim_num(total)
    })
}

fn trim_num(n: f64) -> String {
    let s = format!("{:.1}", n);
    s.trim_end_matches(".0").to_string()
}

/// Minutes in a cleaned time ("1.5h" → 90), for totals.
pub fn time_minutes(t: &str) -> u64 {
    let Some(unit) = t.chars().last() else {
        return 0;
    };
    let n: f64 = t[..t.len() - 1].parse().unwrap_or(0.0);
    (n * match unit {
        'm' => 1.0,
        'h' => 60.0,
        'd' => 480.0,
        _ => 0.0,
    })
    .round() as u64
}

/// Tokens in a cleaned count ("1.2M" → 1_200_000), for totals.
pub fn token_count(t: &str) -> u64 {
    let (num, mult) = match t.chars().last() {
        Some('k') => (&t[..t.len() - 1], 1e3),
        Some('M') => (&t[..t.len() - 1], 1e6),
        _ => (t, 1.0),
    };
    (num.parse::<f64>().unwrap_or(0.0) * mult).round() as u64
}

/// Tags the app suggests. Any other tag works too.
pub const SUGGESTED_TAGS: [&str; 10] = [
    "spike",
    "chore",
    "tech-debt",
    "code",
    "docs",
    "design",
    "research",
    "test",
    "perf",
    "security",
];
const MAX_TAGS: usize = 8;

/// Clean a list of tags: lowercase, spaces and underscores become hyphens, only letters, digits
/// and hyphens kept, 24 characters at most, no duplicates, 8 tags at most.
pub fn clean_tags(input: &[String]) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for raw in input.iter().flat_map(|t| t.split(',')) {
        let t: String = raw
            .trim()
            .to_lowercase()
            .chars()
            .map(|c| if c == ' ' || c == '_' { '-' } else { c })
            .filter(|c| c.is_alphanumeric() || *c == '-')
            .collect();
        let t = t.trim_matches('-').to_string();
        if t.is_empty() {
            continue;
        }
        if t.chars().count() > 24 {
            return Err(format!("the tag {t:?} is longer than 24 characters"));
        }
        if !out.contains(&t) {
            out.push(t);
        }
    }
    if out.len() > MAX_TAGS {
        return Err(format!("a task can have at most {MAX_TAGS} tags"));
    }
    Ok(out)
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Created {
    pub id: String,
    pub path: String,
}

fn one_line(text: &str, max: usize, what: &str) -> Result<String, String> {
    let t: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        return Err(format!("{what} is empty"));
    }
    if t.chars().count() > max {
        return Err(format!("{what} is longer than {max} characters"));
    }
    Ok(t)
}

fn body_text(text: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    let t: String = text
        .chars()
        .filter(|&c| c == '\n' || c == '\t' || !(c < '\u{20}' || c == '\u{7f}'))
        .collect();
    if t.chars().count() > MAX_BODY {
        return Err(format!(
            "the description is longer than {MAX_BODY} characters"
        ));
    }
    Ok(t.trim().to_string())
}

fn pick<'a>(
    value: &str,
    allowed: &[&'a str],
    default: &'a str,
    what: &str,
) -> Result<&'a str, String> {
    if value.is_empty() {
        return Ok(default);
    }
    allowed
        .iter()
        .find(|a| a.eq_ignore_ascii_case(value))
        .copied()
        .ok_or_else(|| format!("{what} must be one of {}", allowed.join(", ")))
}

/// A file-name slug: lowercase ASCII words, cut at a word boundary under 48 characters.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    for word in title
        .to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        if out.len() + word.len() + 1 > 48 {
            break;
        }
        if !out.is_empty() {
            out.push('-');
        }
        out.push_str(word);
    }
    if out.is_empty() {
        "task".into()
    } else {
        out
    }
}

/// The next free id: one more than the highest in any task file (four digits).
pub fn next_id(root: &Path) -> Result<String, String> {
    // The id written inside each task file, falling back to its file name.
    let max = queue::task_files(root)
        .iter()
        .filter_map(|(_, p)| {
            let text = std::fs::read_to_string(p).ok();
            text.as_deref()
                .and_then(front_matter)
                .and_then(|(fm, _)| fm.get("id").and_then(|i| i.parse::<u32>().ok()))
                .or_else(|| p.file_name()?.to_str()?.get(..4)?.parse::<u32>().ok())
        })
        .max()
        .unwrap_or(0);
    if max >= 9999 {
        return Err("the queue is out of four-digit ids".into());
    }
    Ok(format!("{:04}", max + 1))
}

/// Is this queue laid out with status folders (v2) or flat?
fn has_status_folders(root: &Path) -> bool {
    root.join("todo").is_dir()
}

fn folder_for(root: &Path, status: &str) -> PathBuf {
    if has_status_folders(root) {
        root.join(status)
    } else {
        root.to_path_buf()
    }
}

/// Create a task in `todo/` (or the queue folder in the flat layout). `stamp` is local time.
pub fn create_task(root: &Path, new: &NewTask, who: &str, stamp: &str) -> Result<Created, String> {
    let title = one_line(&new.title, MAX_TITLE, "the title")?;
    let kind = pick(&new.kind, &KINDS, "task", "type")?;
    let priority = pick(&new.priority, &PRIORITIES, "normal", "priority")?;
    let body = body_text(&new.body)?;
    let tags = clean_tags(&new.tags)?.join(", ");
    let tags = if tags.is_empty() {
        String::new()
    } else {
        format!(" {tags}")
    };
    let sp = |v: String| if v.is_empty() { v } else { format!(" {v}") };
    let est_time = sp(clean_time(&new.est_time)?);
    let est_tokens = sp(clean_tokens(&new.est_tokens)?);
    let date = &stamp[..stamp.len().min(10)];
    with_lock(root, || {
        let id = next_id(root)?;
        let dir = folder_for(root, "todo");
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("{id}-{}.md", slug(&title)));
        let details = if body.is_empty() {
            String::new()
        } else {
            format!("{body}\n\n")
        };
        let text = format!(
            "---\nid: {id}\ntype: {kind}\npriority: {priority}\ntags:{tags}\nest-time:{est_time}\nest-tokens:{est_tokens}\nstatus: todo\ncaptured: {date}\nclaimed-by:\nheartbeat:\nwaiting_on:\ncommit:\n---\n\n# {title}\n\n{details}## Log\n- {stamp} {who}: created\n"
        );
        write_atomic(&path, &text)?;
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        Ok(Created { id, path: rel })
    })
}

/// Replace a front-matter field's value (adding it before the closing `---` if missing).
fn set_front(text: &str, key: &str, value: &str) -> Result<String, String> {
    let rest = text
        .strip_prefix("---\n")
        .ok_or("the task has no front matter")?;
    let end = rest
        .find("\n---\n")
        .ok_or("the task's front matter isn't closed")?;
    let mut lines: Vec<String> = rest[..end].lines().map(String::from).collect();
    let prefix = format!("{key}:");
    match lines.iter_mut().find(|l| l.starts_with(&prefix)) {
        Some(l) => {
            *l = if value.is_empty() {
                prefix
            } else {
                format!("{prefix} {value}")
            }
        }
        None => lines.push(if value.is_empty() {
            prefix
        } else {
            format!("{prefix} {value}")
        }),
    }
    Ok(format!("---\n{}{}", lines.join("\n"), &rest[end..]))
}

fn append_log(text: &str, line: &str) -> String {
    if text.contains("\n## Log\n") || text.starts_with("## Log\n") {
        format!("{}\n- {line}\n", text.trim_end_matches('\n'))
    } else {
        format!("{}\n\n## Log\n- {line}\n", text.trim_end_matches('\n'))
    }
}

fn edit(
    root: &Path,
    id: &str,
    f: impl FnOnce(&Path, String) -> Result<(String, Option<PathBuf>), String>,
) -> Result<String, String> {
    with_lock(root, || {
        let path = find_task(root, id)?;
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let (new, move_to) = f(&path, text)?;
        match move_to {
            Some(dest) if dest != path => {
                if dest.exists() {
                    return Err(format!("{} already exists", dest.display()));
                }
                write_atomic(&path, &new)?;
                fs::create_dir_all(dest.parent().unwrap_or(root)).map_err(|e| e.to_string())?;
                fs::rename(&path, &dest).map_err(|e| e.to_string())?;
                Ok(dest
                    .strip_prefix(root)
                    .unwrap_or(&dest)
                    .to_string_lossy()
                    .to_string())
            }
            _ => {
                write_atomic(&path, &new)?;
                Ok(path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string())
            }
        }
    })
}

/// Set priority, type or title.
pub fn set_field(
    root: &Path,
    id: &str,
    field: &str,
    value: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let (key, val) = match field {
        "priority" => (
            "priority",
            pick(value, &PRIORITIES, "", "priority")?.to_string(),
        ),
        "type" | "kind" => ("type", pick(value, &KINDS, "", "type")?.to_string()),
        "title" => ("title", one_line(value, MAX_TITLE, "the title")?),
        "tags" => ("tags", clean_tags(&[value.to_string()])?.join(", ")),
        "time" | "est-time" => ("est-time", clean_time(value)?),
        "tokens" | "est-tokens" => ("est-tokens", clean_tokens(value)?),
        _ => {
            return Err(format!(
                "can't set {field:?}: use priority, type, title, tags, time or tokens"
            ))
        }
    };
    if val.is_empty() && !matches!(key, "tags" | "est-time" | "est-tokens") {
        return Err(format!("{field} is empty"));
    }
    edit(root, id, |_, text| {
        let new = if key == "title" {
            let (_, body) = front_matter(&text).ok_or("the task has no front matter")?;
            let head_at = text.len() - body.len();
            let line = body
                .lines()
                .find(|l| l.starts_with("# "))
                .ok_or("the task has no title line")?;
            let i = head_at + body.find(line).unwrap_or(0);
            format!("{}# {val}{}", &text[..i], &text[i + line.len()..])
        } else {
            set_front(&text, key, &val)?
        };
        Ok((
            append_log(&new, &format!("{stamp} {who}: {key} set to {val}")),
            None,
        ))
    })
}

/// Move a task to an owner status. Leaving `doing` releases the claim. `blocked` needs a reason.
pub fn move_task(
    root: &Path,
    id: &str,
    status: &str,
    waiting_on: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let status = pick(status, &OWNER_STATUSES, "", "status").map_err(|_| {
        format!(
            "status must be one of {} (doing is an agent's claim)",
            OWNER_STATUSES.join(", ")
        )
    })?;
    let waiting = if status == "blocked" {
        one_line(waiting_on, 200, "what it's waiting on")?
    } else {
        String::new()
    };
    edit(root, id, |path, text| {
        let (fm, _) = front_matter(&text).ok_or("the task has no front matter")?;
        let before = fm.get("status").cloned().unwrap_or_default();
        let mut new = set_front(&text, "status", status)?;
        new = set_front(&new, "waiting_on", &waiting)?;
        if before == "doing" && status != "done" {
            new = set_front(&new, "claimed-by", "")?;
        }
        let note = if waiting.is_empty() {
            format!("{stamp} {who}: moved {before} → {status}")
        } else {
            format!("{stamp} {who}: moved {before} → {status} (waiting on {waiting})")
        };
        new = append_log(&new, &note);
        let dest = if has_status_folders(root) {
            Some(folder_for(root, status).join(path.file_name().ok_or("bad path")?))
        } else {
            None
        };
        Ok((new, dest))
    })
}

/// Add a line to the task's log.
pub fn add_note(
    root: &Path,
    id: &str,
    note: &str,
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    let note = one_line(note, 2000, "the note")?;
    edit(root, id, |_, text| {
        Ok((append_log(&text, &format!("{stamp} {who}: {note}")), None))
    })
}

/// A safe attachment name: the base name, with only letters, digits and `-_.` kept.
pub fn attachment_name(name: &str) -> Result<String, String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "-_.".contains(c) {
                c
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim_start_matches(['.', '-']).to_string();
    if cleaned.is_empty() || cleaned.len() > 120 {
        return Err("bad file name".into());
    }
    Ok(cleaned)
}

/// Save bytes as an attachment of task `id`. Never overwrites: a clash gets `-2`, `-3`…
pub fn attach(
    root: &Path,
    id: &str,
    name: &str,
    bytes: &[u8],
    who: &str,
    stamp: &str,
) -> Result<String, String> {
    if bytes.len() > MAX_ATTACHMENT {
        return Err("attachments are limited to 25 MB".into());
    }
    let name = attachment_name(name)?;
    with_lock(root, || {
        let path = find_task(root, id)?;
        let dir = root.join("attachments").join(id);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let (stem, ext) = match name.rsplit_once('.') {
            Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
            _ => (name.clone(), String::new()),
        };
        let mut file = name.clone();
        let mut n = 2;
        while dir.join(&file).exists() {
            file = format!("{stem}-{n}{ext}");
            n += 1;
        }
        let tmp = dir.join(format!(".{file}.tmp"));
        fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        fs::rename(&tmp, dir.join(&file)).map_err(|e| e.to_string())?;
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        write_atomic(
            &path,
            &append_log(&text, &format!("{stamp} {who}: attached {file}")),
        )?;
        Ok(file)
    })
}

/// Decode standard base64 (what a browser's data URL carries). Whitespace is ignored.
pub fn unbase64(text: &str) -> Result<Vec<u8>, String> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let clean: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    let trimmed: &[u8] = {
        let mut e = clean.len();
        while e > 0 && clean[e - 1] == b'=' {
            e -= 1;
        }
        &clean[..e]
    };
    let mut out = Vec::with_capacity(trimmed.len() * 3 / 4);
    for chunk in trimmed.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= val(c).ok_or("not base64")? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
        if chunk.len() == 1 {
            return Err("not base64".into());
        }
    }
    Ok(out)
}

/// The data part of a `data:…;base64,…` URL.
pub fn data_url_bytes(url: &str) -> Result<Vec<u8>, String> {
    let (head, data) = url.split_once(',').ok_or("not a data URL")?;
    if !head.starts_with("data:") || !head.ends_with(";base64") {
        return Err("not a base64 data URL".into());
    }
    unbase64(data)
}

/// Statuses that exist as folders (for checks in tests and the CLI's list).
pub fn status_folders() -> &'static [&'static str] {
    &STATUS_FOLDERS
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{base64, read_state, Cache};

    fn queue() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for f in STATUS_FOLDERS {
            fs::create_dir(d.path().join(f)).unwrap();
        }
        fs::write(d.path().join("README.md"), "q").unwrap();
        d
    }
    const T: &str = "2026-10-12 14:05";
    fn state(d: &Path) -> crate::queue::State {
        read_state(&[("p".into(), d.to_path_buf())], &mut Cache::default())
    }

    #[test]
    fn creates_tasks_with_ids_and_template() {
        let d = queue();
        let a = create_task(
            d.path(),
            &NewTask {
                title: "Board  flickers\non resize".into(),
                kind: "Bug".into(),
                priority: "high".into(),
                body: "Steps:\n1. resize\n\tindented".into(),
                ..Default::default()
            },
            "owner",
            T,
        )
        .unwrap();
        assert_eq!(
            a,
            Created {
                id: "0001".into(),
                path: "todo/0001-board-flickers-on-resize.md".into()
            }
        );
        let b = create_task(
            d.path(),
            &NewTask {
                title: "Second".into(),
                ..Default::default()
            },
            "cli",
            T,
        )
        .unwrap();
        assert_eq!(b.id, "0002");
        let text = fs::read_to_string(d.path().join(&a.path)).unwrap();
        assert!(text.starts_with(
            "---\nid: 0001\ntype: bug\npriority: high\ntags:\nest-time:\nest-tokens:\nstatus: todo\ncaptured: 2026-10-12\n"
        ));
        assert!(text.contains("# Board flickers on resize\n\nSteps:\n1. resize\n\tindented\n\n## Log\n- 2026-10-12 14:05 owner: created\n"));
        let s = state(d.path());
        assert_eq!(s.tasks.len(), 2);
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        assert_eq!(s.tasks[1].kind, "task");
        assert_eq!(s.tasks[1].priority, "normal");
    }

    #[test]
    fn create_refusals() {
        let d = queue();
        let e = |n: NewTask| create_task(d.path(), &n, "o", T).unwrap_err();
        assert!(e(NewTask {
            title: "  ".into(),
            ..Default::default()
        })
        .contains("title is empty"));
        assert!(e(NewTask {
            title: "x".repeat(201),
            ..Default::default()
        })
        .contains("longer"));
        assert!(e(NewTask {
            title: "x".into(),
            kind: "epic".into(),
            ..Default::default()
        })
        .contains("type must be"));
        assert!(e(NewTask {
            title: "x".into(),
            priority: "P0".into(),
            ..Default::default()
        })
        .contains("priority must be"));
        assert!(queue::task_files(d.path()).is_empty());
    }

    #[test]
    fn flat_layout_creates_in_the_queue_folder() {
        let d = tempfile::tempdir().unwrap();
        let c = create_task(
            d.path(),
            &NewTask {
                title: "Flat".into(),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        assert_eq!(c.path, "0001-flat.md");
        let p = move_task(d.path(), "0001", "done", "", "o", T).unwrap();
        assert_eq!(p, "0001-flat.md", "flat files never move");
        assert_eq!(state(d.path()).tasks[0].status, "done");
    }

    #[test]
    fn moves_between_status_folders_and_releases_claims() {
        let d = queue();
        create_task(
            d.path(),
            &NewTask {
                title: "Move me".into(),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        // An agent claimed it.
        let p = d.path().join("todo/0001-move-me.md");
        let t = fs::read_to_string(&p)
            .unwrap()
            .replace("status: todo", "status: doing")
            .replace("claimed-by:", "claimed-by: wt (session aaaa1111)");
        fs::remove_file(&p).unwrap();
        fs::write(d.path().join("doing/0001-move-me.md"), t).unwrap();
        assert!(move_task(d.path(), "0001", "blocked", "", "o", T)
            .unwrap_err()
            .contains("waiting on"));
        assert!(move_task(d.path(), "0001", "doing", "", "o", T)
            .unwrap_err()
            .contains("claim"));
        assert_eq!(
            move_task(
                d.path(),
                "0001",
                "blocked",
                "owner: pick a colour",
                "owner",
                T
            )
            .unwrap(),
            "blocked/0001-move-me.md"
        );
        let s = state(d.path());
        let t = &s.tasks[0];
        assert_eq!(
            (t.status.as_str(), t.folder.as_str(), t.claimed_by.as_str()),
            ("blocked", "blocked", "")
        );
        assert_eq!(t.waiting_on, "owner: pick a colour");
        assert!(t.waiting_for_owner);
        assert!(s.problems.is_empty(), "{:?}", s.problems);
        move_task(d.path(), "0001", "todo", "", "owner", T).unwrap();
        let t = &state(d.path()).tasks[0];
        assert_eq!((t.status.as_str(), t.waiting_on.as_str()), ("todo", ""));
        assert!(t.log_tail.last().unwrap().contains("moved blocked → todo"));
    }

    #[test]
    fn sets_fields_and_notes() {
        let d = queue();
        create_task(
            d.path(),
            &NewTask {
                title: "Old title".into(),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        set_field(d.path(), "0001", "priority", "low", "owner", T).unwrap();
        set_field(d.path(), "0001", "type", "idea", "owner", T).unwrap();
        set_field(d.path(), "0001", "title", "New title", "owner", T).unwrap();
        add_note(d.path(), "0001", "Checked on the Mac.", "owner", T).unwrap();
        assert!(set_field(d.path(), "0001", "status", "done", "o", T)
            .unwrap_err()
            .contains("can't set"));
        assert!(set_field(d.path(), "0001", "priority", "urgent", "o", T)
            .unwrap_err()
            .contains("must be"));
        let t = &state(d.path()).tasks[0];
        assert_eq!(
            (t.priority.as_str(), t.kind.as_str(), t.title.as_str()),
            ("low", "idea", "New title")
        );
        assert!(t
            .log_tail
            .last()
            .unwrap()
            .ends_with("owner: Checked on the Mac."));
    }

    #[test]
    fn attaches_without_overwriting() {
        let d = queue();
        create_task(
            d.path(),
            &NewTask {
                title: "Shots".into(),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        assert_eq!(
            attach(d.path(), "0001", "Screen Shot 1.png", b"one", "owner", T).unwrap(),
            "Screen-Shot-1.png"
        );
        assert_eq!(
            attach(d.path(), "0001", "Screen Shot 1.png", b"two", "owner", T).unwrap(),
            "Screen-Shot-1-2.png"
        );
        assert_eq!(
            attach(d.path(), "0001", "../../etc/passwd", b"x", "owner", T).unwrap(),
            "passwd"
        );
        assert!(attach(d.path(), "0001", "...", b"x", "o", T).is_err());
        assert!(attach(
            d.path(),
            "0001",
            "big.bin",
            &vec![0u8; 25 * 1024 * 1024 + 1],
            "o",
            T
        )
        .unwrap_err()
        .contains("25 MB"));
        assert_eq!(
            fs::read(d.path().join("attachments/0001/Screen-Shot-1.png")).unwrap(),
            b"one"
        );
        let t = &state(d.path()).tasks[0];
        assert_eq!(
            t.attachments,
            vec!["Screen-Shot-1-2.png", "Screen-Shot-1.png", "passwd"]
        );
        assert!(t.log_tail.last().unwrap().contains("attached passwd"));
    }

    #[test]
    fn base64_round_trips() {
        for data in [
            &b""[..],
            b"f",
            b"fo",
            b"foo",
            b"foob",
            b"\x00\xff\x10binary",
        ] {
            assert_eq!(unbase64(&base64(data)).unwrap(), data);
        }
        assert_eq!(
            data_url_bytes("data:image/png;base64,iVBORw==").unwrap(),
            vec![137, 80, 78, 71]
        );
        assert!(data_url_bytes("data:text/plain,hi").is_err());
        assert!(unbase64("@@@@").is_err());
    }

    #[test]
    fn tags_are_cleaned_and_round_trip() {
        let v = |xs: &[&str]| xs.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(
            clean_tags(&v(&["Tech Debt", "spike,  CODE", "spike", "my_tag!"])).unwrap(),
            v(&["tech-debt", "spike", "code", "my-tag"])
        );
        assert!(clean_tags(&v(&["a,b,c,d,e,f,g,h,i"]))
            .unwrap_err()
            .contains("at most 8"));
        assert!(clean_tags(&v(&["x".repeat(25).as_str()]))
            .unwrap_err()
            .contains("24"));
        assert!(clean_tags(&v(&[" , "])).unwrap().is_empty());
        let d = queue();
        create_task(
            d.path(),
            &NewTask {
                title: "Tagged".into(),
                tags: v(&["Spike", "tech debt"]),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        assert_eq!(state(d.path()).tasks[0].tags, v(&["spike", "tech-debt"]));
        set_field(d.path(), "0001", "tags", "chore, code", "o", T).unwrap();
        assert_eq!(state(d.path()).tasks[0].tags, v(&["chore", "code"]));
        set_field(d.path(), "0001", "tags", "", "o", T).unwrap();
        assert!(state(d.path()).tasks[0].tags.is_empty());
        assert!(state(d.path()).problems.is_empty());
    }

    #[test]
    fn estimates_are_cleaned_set_and_counted() {
        assert_eq!(clean_time(" 45 min ").unwrap(), "45m");
        assert_eq!(clean_time("1.5H").unwrap(), "1.5h");
        assert_eq!(clean_time("2 days").unwrap(), "2d");
        assert_eq!(clean_time("").unwrap(), "");
        assert!(
            clean_time("soon").is_err() && clean_time("5w").is_err() && clean_time("0h").is_err()
        );
        assert_eq!(clean_tokens("400k").unwrap(), "400k");
        assert_eq!(clean_tokens("1200000").unwrap(), "1.2M");
        assert_eq!(clean_tokens("1,500").unwrap(), "1.5k");
        assert_eq!(clean_tokens("2m").unwrap(), "2M");
        assert!(clean_tokens("lots").is_err() && clean_tokens("5g").is_err());
        assert_eq!(
            (
                time_minutes("1.5h"),
                time_minutes("45m"),
                time_minutes("1d"),
                time_minutes("")
            ),
            (90, 45, 480, 0)
        );
        assert_eq!(
            (
                token_count("400k"),
                token_count("1.2M"),
                token_count("900"),
                token_count("")
            ),
            (400_000, 1_200_000, 900, 0)
        );
        let d = queue();
        create_task(
            d.path(),
            &NewTask {
                title: "Est".into(),
                est_time: "2h".into(),
                est_tokens: "300k".into(),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        let t = &state(d.path()).tasks[0];
        assert_eq!(
            (
                t.est_time.as_str(),
                t.est_tokens.as_str(),
                t.est_minutes,
                t.est_token_count
            ),
            ("2h", "300k", 120, 300_000)
        );
        set_field(d.path(), "0001", "time", "30m", "o", T).unwrap();
        set_field(d.path(), "0001", "tokens", "", "o", T).unwrap();
        let t = &state(d.path()).tasks[0];
        assert_eq!((t.est_time.as_str(), t.est_tokens.as_str()), ("30m", ""));
        assert!(state(d.path()).problems.is_empty());
    }

    #[test]
    fn next_id_ignores_dated_notes() {
        let d = queue();
        fs::write(d.path().join("inbox/2026-10-12-0900-note.md"), "note").unwrap();
        create_task(
            d.path(),
            &NewTask {
                title: "One".into(),
                ..Default::default()
            },
            "o",
            T,
        )
        .unwrap();
        assert_eq!(next_id(d.path()).unwrap(), "0002");
    }

    #[test]
    fn slugs() {
        assert_eq!(
            slug("Fix: the cart total (when empty)!"),
            "fix-the-cart-total-when-empty"
        );
        assert_eq!(slug("ÄÖÜ"), "task");
        assert!(slug(&"word ".repeat(30)).len() <= 48);
    }
}
