//! Claude Code session transcripts on this Mac (task 0025): find a session's transcript by its
//! short id across the allowed account folders, `<account>/projects/<folder key>/<session id>.jsonl`,
//! and sum what it used. This is the one transcript reader: the task pane's Work section and
//! 0017's actuals (`actuals`) both read through it.
//!
//! Only the accounts in Settings' allowlist are read (0099): every read is given them, as loaded
//! for that read, so two reads never share a list one of them didn't load. Never `~/.claude`, and
//! in them only the project folders under `~/playground` (`accounts::is_under`): their `projects`
//! may be links to the company account's, so a work project's transcripts are never listed,
//! opened or counted. Links inside a `projects` folder (a project folder, a transcript, a
//! subagents folder or file) aren't followed.
//!
//! Privacy: transcripts are read here, locally, and nothing in them is sent anywhere. Callers get
//! counts, ids and times; only `transcript_markdown`, for the owner's own pane, carries text.
//!
//! Bounds: a transcript over `MAX_TRANSCRIPT_BYTES` isn't read, a line over `MAX_LINE_BYTES` is
//! skipped without being held in memory, at most `MAX_SUBAGENTS` subagent files count per session,
//! and summaries are cached by (path, size, modified time) in a cache of `CACHE_CAP` files.

use crate::accounts::{is_under, Account};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

/// Transcripts larger than this aren't read (the biggest real ones are around 60 MB).
pub const MAX_TRANSCRIPT_BYTES: u64 = 128 * 1024 * 1024;
/// A line longer than this is skipped, and never held whole in memory.
const MAX_LINE_BYTES: usize = 8 * 1024 * 1024;
/// Subagent transcripts read per session (real sessions have up to about 70).
const MAX_SUBAGENTS: usize = 256;
/// Transcript summaries kept in memory.
const CACHE_CAP: usize = 512;
/// A session is live when its transcript changed this recently.
pub const LIVE_SECS: u64 = 5 * 60;
/// The transcript view keeps the latest this many entries.
const VIEW_ENTRIES: usize = 1500;
/// Characters of one folded entry in the transcript view.
const VIEW_LINE_CHARS: usize = 240;
/// The end of a transcript the Sessions view (0098) reads for a session's last message.
pub const TAIL_BYTES: u64 = 64 * 1024;
/// The start of a transcript read for its first prompt (0098): in real ones the first prompt
/// ends within 62 KB of the start (90% within 8 KB).
pub const HEAD_BYTES: u64 = 128 * 1024;
/// Characters of a last message or a pending tool call (0098).
pub const MESSAGE_CHARS: usize = 160;
/// Characters of a first prompt kept: enough for a `/handoff resume <path>`.
const PROMPT_CHARS: usize = 4096;

/// Tokens a session used, summed over its model calls.
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
}

impl Tokens {
    /// Every kind together.
    pub fn total(&self) -> u64 {
        self.input
            .saturating_add(self.output)
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write)
    }

    pub fn add(&mut self, other: &Tokens) {
        self.input = self.input.saturating_add(other.input);
        self.output = self.output.saturating_add(other.output);
        self.cache_read = self.cache_read.saturating_add(other.cache_read);
        self.cache_write = self.cache_write.saturating_add(other.cache_write);
    }
}

/// Whether a session's transcript was read.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Found {
    Read,
    /// No transcript with that id on this Mac.
    Missing,
    /// Over `MAX_TRANSCRIPT_BYTES`, not read.
    TooBig,
    /// Its stamp names no Claude account Muster reads (0099), so it isn't looked for.
    NotRead,
}

/// What one session's transcript says.
#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub found: Found,
    /// The session's own calls plus its subagents'.
    pub tokens: Tokens,
    /// First and last time stamp in the session's transcript, seconds since 1970 (UTC).
    pub start: Option<u64>,
    pub end: Option<u64>,
    /// Wall-clock minutes from `start` to `end`, rounded.
    pub minutes: u64,
    /// The transcript (or a subagent's) changed in the last `LIVE_SECS`.
    pub live: bool,
    /// Lines that weren't JSON of the expected shape, or were over the line cap: skipped.
    pub skipped_lines: u64,
    /// Subagent transcripts not read: over the size cap or past `MAX_SUBAGENTS`.
    pub skipped_files: u64,
}

impl Usage {
    pub(crate) fn missing() -> Usage {
        Usage {
            found: Found::Missing,
            tokens: Tokens::default(),
            start: None,
            end: None,
            minutes: 0,
            live: false,
            skipped_lines: 0,
            skipped_files: 0,
        }
    }

    /// A session whose stamp names no account Muster reads (0099): nothing read.
    pub(crate) fn not_read() -> Usage {
        Usage {
            found: Found::NotRead,
            ..Usage::missing()
        }
    }
}

/// 0017's actuals for a task: tokens and wall-clock minutes summed over the sessions that worked
/// it. The Work section's totals are this same sum.
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Actuals {
    pub tokens: Tokens,
    pub minutes: u64,
    /// Sessions whose transcript was read (the rest aren't on this Mac, are too big, or aren't
    /// stamped with an account Muster reads).
    pub read: usize,
}

/// The shared reader for 0017's actuals (`act-tokens`, `act-agent-time`) and the Work section's
/// totals: the sum of `usages`, one per session.
pub fn actuals<'a>(usages: impl IntoIterator<Item = &'a Usage>) -> Actuals {
    let mut out = Actuals::default();
    for u in usages {
        if u.found == Found::Read {
            out.tokens.add(&u.tokens);
            out.minutes = out.minutes.saturating_add(u.minutes);
            out.read += 1;
        }
    }
    out
}

/// One transcript file's sums.
#[derive(Clone, Debug, Default, PartialEq)]
struct FileSummary {
    tokens: Tokens,
    first: Option<u64>,
    last: Option<u64>,
    skipped_lines: u64,
}

struct Cached {
    size: u64,
    modified: SystemTime,
    summary: FileSummary,
    used: u64,
}

#[derive(Default)]
struct Cache {
    files: HashMap<PathBuf, Cached>,
    tick: u64,
}

/// Every session transcript by its short id, rebuilt when a project folder changes (a file was
/// added, removed or renamed in it).
#[derive(Default)]
struct Index {
    signature: Vec<(PathBuf, Option<SystemTime>)>,
    files: HashMap<String, Vec<PathBuf>>,
}

/// Reads the transcripts of the allowed accounts' playground projects. Shared by the app's
/// commands; every method is safe to call from several threads, and none holds a lock while it
/// reads a file.
pub struct Reader {
    /// The key of `~/playground` (`accounts::playground_key`): only project folders under it are
    /// read.
    root: String,
    /// The LIVE window in seconds: `LIVE_SECS` unless the settings say otherwise (0036).
    live_secs: AtomicU64,
    index: Mutex<Index>,
    cache: Mutex<Cache>,
    /// The Sessions view's last messages (0098), by (path, size, modified time).
    tails: Mutex<Small<Tail>>,
    /// First prompts (0098): a found one never changes, so it's kept while its file is.
    heads: Mutex<Small<Option<String>>>,
}

/// A poisoned lock only means another reader panicked mid-update; the data is a cache, so use it.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Is `id` a short session id: exactly 8 lowercase hex characters?
pub fn is_short_id(id: &str) -> bool {
    id.len() == 8 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

impl Reader {
    /// A reader of the transcripts in project folders under the one keyed `playground_key`
    /// (`accounts::playground_key`), in the accounts each read is given.
    pub fn new(playground_key: String) -> Reader {
        Reader {
            root: playground_key,
            index: Mutex::new(Index::default()),
            live_secs: AtomicU64::new(LIVE_SECS),
            cache: Mutex::new(Cache::default()),
            tails: Mutex::new(Small::default()),
            heads: Mutex::new(Small::default()),
        }
    }

    /// Set the LIVE window, in seconds (the per-machine setting, 0036); every live check after
    /// this uses it.
    pub fn set_live_secs(&self, secs: u64) {
        self.live_secs.store(secs, Ordering::Relaxed);
    }

    /// `accounts`' `projects` folders, canonical and each once: `.claude-hg/projects` is often a
    /// link to the same folder as `.claude-mg/projects`, and its transcripts must not count twice.
    fn projects_dirs(accounts: &[Account]) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = accounts
            .iter()
            .filter_map(|a| fs::canonicalize(a.real().join("projects")).ok())
            .filter(|p| p.is_dir())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// The project folders under `~/playground` and when each last changed. A project folder
    /// that's a link isn't one (`DirEntry::metadata` doesn't follow it).
    fn signature(&self, dirs: &[PathBuf]) -> Vec<(PathBuf, Option<SystemTime>)> {
        let mut out = Vec::new();
        for projects in dirs {
            let Ok(rd) = fs::read_dir(projects) else {
                continue;
            };
            for e in rd.flatten() {
                let name = e.file_name();
                if !name.to_str().is_some_and(|n| is_under(n, &self.root)) {
                    continue;
                }
                let Ok(meta) = e.metadata() else { continue };
                if meta.is_dir() {
                    out.push((e.path(), meta.modified().ok()));
                }
            }
        }
        out.sort();
        out
    }

    /// Bring the index up to date: rebuilt only when a project folder was added, removed or
    /// changed. Each public call refreshes once, however many sessions it looks up, and gets
    /// back `accounts`' `projects` folders: the only ones its lookups (`candidates`) may answer
    /// from, since a read given other accounts may rebuild the shared index in between.
    fn refresh(&self, accounts: &[Account]) -> Vec<PathBuf> {
        let dirs = Self::projects_dirs(accounts);
        let signature = self.signature(&dirs);
        let mut index = lock(&self.index);
        if index.signature != signature {
            let mut files: HashMap<String, Vec<PathBuf>> = HashMap::new();
            for (dir, _) in &signature {
                let Ok(rd) = fs::read_dir(dir) else { continue };
                for e in rd.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    let Some(stem) = name.strip_suffix(".jsonl") else {
                        continue;
                    };
                    let key = stem.get(..8).unwrap_or("").to_ascii_lowercase();
                    if is_short_id(&key) && e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                        files.entry(key).or_default().push(e.path());
                    }
                }
            }
            *index = Index { signature, files };
        }
        dirs
    }

    /// The transcript files whose name starts with `short` in the index as it is, inside one of
    /// `dirs` (what `refresh` gave this call), newest first.
    fn candidates(&self, short: &str, dirs: &[PathBuf]) -> Vec<PathBuf> {
        if !is_short_id(short) {
            return Vec::new();
        }
        let mut found: Vec<PathBuf> = lock(&self.index)
            .files
            .get(short)
            .into_iter()
            .flatten()
            .filter(|p| dirs.iter().any(|d| p.starts_with(d)))
            .cloned()
            .collect();
        found.sort_by_key(|p| std::cmp::Reverse(fs::metadata(p).and_then(|m| m.modified()).ok()));
        found
    }

    /// The session's transcript in `accounts`: the most recently changed file with that short id.
    pub fn locate(&self, accounts: &[Account], short: &str) -> Option<PathBuf> {
        let dirs = self.refresh(accounts);
        self.candidates(short, &dirs).into_iter().next()
    }

    /// The session's subagent transcripts (`<session>/subagents/*.jsonl`), at most
    /// `MAX_SUBAGENTS`, and how many more there were. The session folder, its `subagents` and
    /// the files in it count only as themselves, not as links.
    fn subagents(main: &Path) -> (Vec<PathBuf>, u64) {
        let session = main.with_extension("");
        let dir = session.join("subagents");
        let real_dir = |p: &Path| fs::symlink_metadata(p).is_ok_and(|m| m.is_dir());
        if !real_dir(&session) || !real_dir(&dir) {
            return (Vec::new(), 0);
        }
        let mut files: Vec<PathBuf> = fs::read_dir(&dir)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| {
                        p.extension().is_some_and(|x| x == "jsonl")
                            && fs::symlink_metadata(p).is_ok_and(|m| m.is_file())
                    })
                    .collect()
            })
            .unwrap_or_default();
        files.sort();
        let over = files.len().saturating_sub(MAX_SUBAGENTS) as u64;
        files.truncate(MAX_SUBAGENTS);
        (files, over)
    }

    /// One file's sums, from the cache while its size and modified time match.
    fn summary(&self, path: &Path) -> io::Result<FileSummary> {
        let meta = fs::metadata(path)?;
        let modified = meta.modified()?;
        {
            let mut cache = lock(&self.cache);
            cache.tick += 1;
            let tick = cache.tick;
            if let Some(c) = cache.files.get_mut(path) {
                if c.size == meta.len() && c.modified == modified {
                    c.used = tick;
                    return Ok(c.summary.clone());
                }
            }
        }
        let summary = summarize(path)?;
        let mut cache = lock(&self.cache);
        cache.tick += 1;
        let tick = cache.tick;
        if cache.files.len() >= CACHE_CAP && !cache.files.contains_key(path) {
            let oldest = cache
                .files
                .iter()
                .min_by_key(|(_, c)| c.used)
                .map(|(p, _)| p.clone());
            if let Some(p) = oldest {
                cache.files.remove(&p);
            }
        }
        cache.files.insert(
            path.to_path_buf(),
            Cached {
                size: meta.len(),
                modified,
                summary: summary.clone(),
                used: tick,
            },
        );
        Ok(summary)
    }

    /// Files in the summary cache now.
    #[cfg(test)]
    fn cached(&self) -> usize {
        lock(&self.cache).files.len()
    }

    /// What session `short` used in `accounts`, at `now` (seconds since 1970) for whether it's
    /// live.
    pub fn usage(&self, accounts: &[Account], short: &str, now: u64) -> Usage {
        let dirs = self.refresh(accounts);
        self.usage_indexed(short, &dirs, now)
    }

    /// `usage` for each of `shorts`, in order, with one index refresh for all of them.
    pub fn usages(&self, accounts: &[Account], shorts: &[String], now: u64) -> Vec<Usage> {
        let dirs = self.refresh(accounts);
        shorts
            .iter()
            .map(|s| self.usage_indexed(s, &dirs, now))
            .collect()
    }

    fn usage_indexed(&self, short: &str, dirs: &[PathBuf], now: u64) -> Usage {
        let Some(main) = self.candidates(short, dirs).into_iter().next() else {
            return Usage::missing();
        };
        let Ok(meta) = fs::metadata(&main) else {
            return Usage::missing();
        };
        let mut u = Usage::missing();
        u.live = self.live_at(&main, now);
        if meta.len() > MAX_TRANSCRIPT_BYTES {
            u.found = Found::TooBig;
            return u;
        }
        let Ok(s) = self.summary(&main) else {
            return u;
        };
        u.found = Found::Read;
        u.tokens = s.tokens;
        u.start = s.first;
        u.end = s.last;
        u.minutes = match (s.first, s.last) {
            (Some(a), Some(b)) => (b.saturating_sub(a) + 30) / 60,
            _ => 0,
        };
        u.skipped_lines = s.skipped_lines;
        let (subs, over) = Self::subagents(&main);
        u.skipped_files = over;
        for sub in subs {
            let small = fs::metadata(&sub).is_ok_and(|m| m.len() <= MAX_TRANSCRIPT_BYTES);
            match small.then(|| self.summary(&sub)) {
                Some(Ok(s)) => {
                    u.tokens.add(&s.tokens);
                    u.skipped_lines += s.skipped_lines;
                }
                _ => u.skipped_files += 1,
            }
        }
        u
    }

    /// Whether the transcript at `main`, or one of its subagents', changed in the last
    /// LIVE window (`set_live_secs`) before `now`. Only file times are read.
    fn live_at(&self, main: &Path, now: u64) -> bool {
        let window = self.live_secs.load(Ordering::Relaxed);
        let fresh = |p: &Path| {
            fs::metadata(p)
                .and_then(|m| m.modified())
                .is_ok_and(|t| now.saturating_sub(secs(t)) <= window)
        };
        fresh(main) || Self::subagents(main).0.iter().any(|p| fresh(p))
    }

    /// Whether session `short` is live in `accounts` at `now`. Cheap: a lookup and a few file
    /// times, no reading.
    pub fn live(&self, accounts: &[Account], short: &str, now: u64) -> bool {
        self.locate(accounts, short)
            .is_some_and(|p| self.live_at(&p, now))
    }

    /// Which of `shorts` are live in `accounts` at `now`, with one index refresh for all of them:
    /// what the window's state read uses for the cards, every poll.
    pub fn live_set<'a>(
        &self,
        accounts: &[Account],
        shorts: impl IntoIterator<Item = &'a str>,
        now: u64,
    ) -> HashSet<String> {
        let mut shorts = shorts.into_iter().peekable();
        if shorts.peek().is_none() {
            // Nothing to look up: don't list the account folders at all.
            return HashSet::new();
        }
        let dirs = self.refresh(accounts);
        shorts
            .filter(|s| {
                self.candidates(s, &dirs)
                    .first()
                    .is_some_and(|p| self.live_at(p, now))
            })
            .map(str::to_string)
            .collect()
    }

    /// Every transcript path the index holds now, for dropping cached text of a transcript that's
    /// gone (0098).
    fn indexed_paths(&self) -> HashSet<PathBuf> {
        lock(&self.index)
            .files
            .values()
            .flatten()
            .cloned()
            .collect()
    }

    /// The end of each of `shorts`' transcripts in `accounts` (0098, `read_tail`), in order, with
    /// one index refresh for all of them: None for one that isn't there or can't be read. Cached
    /// by (path, size, modified time); what's cached of a transcript that's gone is dropped.
    pub fn tails(&self, accounts: &[Account], shorts: &[String]) -> Vec<Option<Tail>> {
        let dirs = self.refresh(accounts);
        let out = shorts
            .iter()
            .map(|s| {
                let path = self.candidates(s, &dirs).into_iter().next()?;
                let meta = fs::metadata(&path).ok()?;
                let stamp = (meta.len(), meta.modified().ok()?);
                if let Some(t) = lock(&self.tails).get(&path, |c| c.stamp == stamp) {
                    return Some(t);
                }
                let tail = read_tail(&path, secs(stamp.1)).ok()?;
                lock(&self.tails).put(path, stamp, tail.clone());
                Some(tail)
            })
            .collect();
        let alive = self.indexed_paths();
        lock(&self.tails).keep(&alive);
        out
    }

    /// The first prompt of each of `shorts`' transcripts in `accounts` (0098, `read_first_prompt`),
    /// in order, with one index refresh: None when there's no transcript or no prompt in its first
    /// `HEAD_BYTES`. A prompt once found is kept while the file is there; none found is looked for
    /// again when the file changes.
    pub fn first_prompts(&self, accounts: &[Account], shorts: &[String]) -> Vec<Option<String>> {
        let dirs = self.refresh(accounts);
        let out = shorts
            .iter()
            .map(|s| {
                let path = self.candidates(s, &dirs).into_iter().next()?;
                let meta = fs::metadata(&path).ok()?;
                let stamp = (meta.len(), meta.modified().ok()?);
                let cached =
                    lock(&self.heads).get(&path, |c| c.value.is_some() || c.stamp == stamp);
                if let Some(found) = cached {
                    return found;
                }
                let found = read_first_prompt(&path).ok()?;
                lock(&self.heads).put(path, stamp, found.clone());
                found
            })
            .collect();
        let alive = self.indexed_paths();
        lock(&self.heads).keep(&alive);
        out
    }

    /// Text cached for the Sessions view now (tails and first prompts).
    #[cfg(test)]
    pub(crate) fn texts_cached(&self) -> usize {
        lock(&self.tails).files.len() + lock(&self.heads).files.len()
    }

    /// Session `short`'s transcript in `accounts` as Markdown for the owner's pane: one line per
    /// prompt, reply and tool call, folded and capped, with the text escaped so it shows as
    /// written (no links, images or HTML from it). Tool output and thinking are left out. The
    /// latest `VIEW_ENTRIES` entries are kept.
    pub fn transcript_markdown(&self, accounts: &[Account], short: &str) -> Result<String, String> {
        let path = self.locate(accounts, short).ok_or("not on this Mac")?;
        let meta = fs::metadata(&path).map_err(|_| "not on this Mac")?;
        if meta.len() > MAX_TRANSCRIPT_BYTES {
            return Err(format!(
                "the transcript is over {} MB, so it isn't read",
                MAX_TRANSCRIPT_BYTES / (1024 * 1024)
            ));
        }
        let file = fs::File::open(&path).map_err(|e| e.to_string())?;
        let mut reader = BufReader::with_capacity(64 * 1024, file);
        let mut buf = Vec::new();
        let mut entries: VecDeque<String> = VecDeque::new();
        let mut dropped = 0u64;
        while let Some(whole) =
            read_line_capped(&mut reader, &mut buf, MAX_LINE_BYTES).map_err(|e| e.to_string())?
        {
            if !whole {
                continue;
            }
            let Ok(line) = serde_json::from_slice::<ViewLine>(&buf) else {
                continue;
            };
            for entry in view_entries(&line) {
                if entries.len() == VIEW_ENTRIES {
                    entries.pop_front();
                    dropped += 1;
                }
                entries.push_back(entry);
            }
        }
        let mut out = format!(
            "**Session {short}**, folded: one line per prompt, reply and tool call. Tool output and thinking are left out.\n\n"
        );
        if dropped > 0 {
            out.push_str(&format!(
                "*The first {dropped} entries aren't shown; these are the latest {VIEW_ENTRIES}.*\n\n"
            ));
        }
        if entries.is_empty() {
            out.push_str("*No prompts, replies or tool calls in it yet.*\n");
        }
        for e in entries {
            out.push_str("- ");
            out.push_str(&e);
            out.push('\n');
        }
        Ok(out)
    }
}

/// Read one line (without its newline) into `buf`, keeping at most `cap` bytes. Returns None at
/// the end of the file, Some(true) for a whole line, Some(false) for one over the cap (the rest
/// of it is read past, never held).
fn read_line_capped(
    r: &mut impl BufRead,
    buf: &mut Vec<u8>,
    cap: usize,
) -> io::Result<Option<bool>> {
    buf.clear();
    let mut any = false;
    let mut over = false;
    loop {
        let chunk = r.fill_buf()?;
        if chunk.is_empty() {
            return Ok(any.then_some(!over));
        }
        any = true;
        let (part, done) = match chunk.iter().position(|&b| b == b'\n') {
            Some(i) => (&chunk[..i], Some(i + 1)),
            None => (chunk, None),
        };
        if !over {
            if buf.len() + part.len() > cap {
                over = true;
                buf.clear();
            } else {
                buf.extend_from_slice(part);
            }
        }
        let used = done.unwrap_or(chunk.len());
        r.consume(used);
        if done.is_some() {
            return Ok(Some(!over));
        }
    }
}

#[derive(Deserialize)]
struct UsageLine {
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    message: Option<UsageMessage>,
}

#[derive(Deserialize)]
struct UsageMessage {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    usage: Option<UsageCounts>,
}

#[derive(Deserialize)]
struct UsageCounts {
    #[serde(default)]
    input_tokens: Option<u64>,
    #[serde(default)]
    output_tokens: Option<u64>,
    #[serde(default)]
    cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    cache_creation_input_tokens: Option<u64>,
}

/// A transcript's sums, streamed line by line. Claude Code writes one line per content block of
/// a reply, each repeating the reply's usage, so usage counts once per message id.
fn summarize(path: &Path) -> io::Result<FileSummary> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);
    let mut buf = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    let mut s = FileSummary::default();
    while let Some(whole) = read_line_capped(&mut reader, &mut buf, MAX_LINE_BYTES)? {
        if !whole {
            s.skipped_lines += 1;
            continue;
        }
        if buf.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let Ok(line) = serde_json::from_slice::<UsageLine>(&buf) else {
            s.skipped_lines += 1;
            continue;
        };
        if let Some(t) = line.timestamp.as_deref().and_then(iso_secs) {
            s.first = Some(s.first.map_or(t, |f| f.min(t)));
            s.last = Some(s.last.map_or(t, |l| l.max(t)));
        }
        if line.kind.as_deref() != Some("assistant") {
            continue;
        }
        let Some(m) = line.message else { continue };
        let Some(u) = m.usage else { continue };
        if let Some(id) = m.id {
            if !seen.insert(id) {
                continue;
            }
        }
        s.tokens.add(&Tokens {
            input: u.input_tokens.unwrap_or(0),
            output: u.output_tokens.unwrap_or(0),
            cache_read: u.cache_read_input_tokens.unwrap_or(0),
            cache_write: u.cache_creation_input_tokens.unwrap_or(0),
        });
    }
    Ok(s)
}

/// A transcript time stamp, `2026-10-12T21:47:12.795Z` (UTC), as seconds since 1970.
pub fn iso_secs(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 20 || b[10] != b'T' || b[16] != b':' || *b.last()? != b'Z' {
        return None;
    }
    if !(b.len() == 20 || b[19] == b'.') {
        return None;
    }
    let minutes = crate::ops::stamp_minutes(&format!("{} {}", s.get(..10)?, s.get(11..16)?))?;
    let sec: u64 = s.get(17..19)?.parse().ok().filter(|n| *n < 60)?;
    if !s.get(17..19)?.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if b.len() > 20 && !s.get(20..b.len() - 1)?.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    u64::try_from(minutes)
        .ok()?
        .checked_mul(60)?
        .checked_add(sec)
}

#[derive(Deserialize)]
struct ViewLine {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default, rename = "isMeta")]
    is_meta: Option<bool>,
    #[serde(default)]
    message: Option<ViewMessage>,
}

#[derive(Deserialize)]
struct ViewMessage {
    #[serde(default)]
    content: Option<Content>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Content {
    Text(String),
    Blocks(Vec<Block>),
}

#[derive(Deserialize)]
struct Block {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    input: Option<serde_json::Value>,
    /// A tool call's id, and the call a tool result answers (0098's pending call).
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    tool_use_id: Option<String>,
}

/// One line of the view per prompt, reply text and tool call in a transcript line.
fn view_entries(line: &ViewLine) -> Vec<String> {
    let who = match line.kind.as_deref() {
        Some("user") if line.is_meta != Some(true) => "You",
        Some("assistant") => "Claude",
        _ => return Vec::new(),
    };
    let Some(content) = line.message.as_ref().and_then(|m| m.content.as_ref()) else {
        return Vec::new();
    };
    let said = |text: &str| {
        let t = fold(text);
        (!t.is_empty()).then(|| format!("**{who}:** {t}"))
    };
    match content {
        Content::Text(t) => said(t).into_iter().collect(),
        Content::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b.kind.as_deref() {
                Some("text") => b.text.as_deref().and_then(said),
                Some("tool_use") => Some(format!(
                    "**Tool:** {}{}",
                    fold(b.name.as_deref().unwrap_or("tool")),
                    b.input
                        .as_ref()
                        .and_then(tool_gist)
                        .map(|g| format!(" · {}", fold(&g)))
                        .unwrap_or_default()
                )),
                _ => None,
            })
            .collect(),
    }
}

/// What a tool call was about, in a few words: its description, command, path, pattern or
/// prompt, whichever it has first.
fn tool_gist(input: &serde_json::Value) -> Option<String> {
    [
        "description",
        "command",
        "file_path",
        "path",
        "pattern",
        "url",
        "query",
        "prompt",
    ]
    .iter()
    .find_map(|k| input.get(k).and_then(|v| v.as_str()).map(str::to_string))
}

/// What a call waiting for the owner's yes would do, as the owner approves it: its command, path,
/// address, pattern or query first, and the model's description of it only when it has none.
fn pending_gist(input: &serde_json::Value) -> Option<String> {
    [
        "command",
        "file_path",
        "path",
        "url",
        "pattern",
        "query",
        "description",
        "prompt",
    ]
    .iter()
    .find_map(|k| input.get(k).and_then(|v| v.as_str()).map(str::to_string))
}

/// Text on one line: whitespace runs as one space, cut at `VIEW_LINE_CHARS` characters, and every
/// ASCII punctuation character escaped so Markdown shows it as written.
fn fold(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    let joined = words.join(" ");
    let mut out = String::with_capacity(joined.len().min(VIEW_LINE_CHARS * 2));
    for (n, c) in joined.chars().enumerate() {
        if n == VIEW_LINE_CHARS {
            out.push('…');
            break;
        }
        if c.is_ascii_punctuation() {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The end of a session's transcript as the Sessions view shows it (0098). Held in memory only.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tail {
    /// The last text Claude wrote, on one line, at most `MESSAGE_CHARS` characters; "" for none.
    pub text: String,
    /// The last tool call with no result yet (what a permission prompt asks about), as
    /// `Bash: git push origin…`, at most `MESSAGE_CHARS` characters; "" for none.
    pub pending: String,
    /// When the transcript last changed, seconds since 1970.
    pub modified: u64,
}

struct SmallEntry<T> {
    stamp: (u64, SystemTime),
    value: T,
    used: u64,
}

/// A small cache by path, at most `CACHE_CAP` entries, the least recently used dropped first.
struct Small<T> {
    files: HashMap<PathBuf, SmallEntry<T>>,
    tick: u64,
}

impl<T> Default for Small<T> {
    fn default() -> Self {
        Small {
            files: HashMap::new(),
            tick: 0,
        }
    }
}

impl<T: Clone> Small<T> {
    /// The value for `path`, when `fresh` says the cached one still holds.
    fn get(&mut self, path: &Path, fresh: impl Fn(&SmallEntry<T>) -> bool) -> Option<T> {
        self.tick += 1;
        let tick = self.tick;
        let e = self.files.get_mut(path).filter(|e| fresh(e))?;
        e.used = tick;
        Some(e.value.clone())
    }

    fn put(&mut self, path: PathBuf, stamp: (u64, SystemTime), value: T) {
        self.tick += 1;
        if self.files.len() >= CACHE_CAP && !self.files.contains_key(&path) {
            let oldest = self
                .files
                .iter()
                .min_by_key(|(_, e)| e.used)
                .map(|(p, _)| p.clone());
            if let Some(p) = oldest {
                self.files.remove(&p);
            }
        }
        let used = self.tick;
        self.files.insert(path, SmallEntry { stamp, value, used });
    }

    /// Drop what's cached of every file not in `alive`.
    fn keep(&mut self, alive: &HashSet<PathBuf>) {
        self.files.retain(|p, _| alive.contains(p));
    }
}

/// A transcript line as the tail and head reads take it.
#[derive(Deserialize)]
struct TailLine {
    #[serde(default, rename = "type")]
    kind: Option<String>,
    #[serde(default, rename = "isMeta")]
    is_meta: Option<bool>,
    #[serde(default, rename = "isSidechain")]
    is_sidechain: Option<bool>,
    #[serde(default)]
    message: Option<ViewMessage>,
}

/// The last `TAIL_BYTES` of the transcript at `path` (whose file changed at `modified`): the
/// last text Claude wrote, and the last tool call no result answers yet. Only whole lines count:
/// the first, cut by the window, is skipped unless the window starts the file. Subagent lines
/// (`isSidechain`) don't count.
pub fn read_tail(path: &Path, modified: u64) -> io::Result<Tail> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = fs::File::open(path)?;
    let len = f.metadata()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    f.seek(SeekFrom::Start(start))?;
    let mut buf = Vec::with_capacity(usize::try_from(len - start).unwrap_or(0));
    f.take(TAIL_BYTES).read_to_end(&mut buf)?;
    let body: &[u8] = if start == 0 {
        &buf
    } else {
        match buf.iter().position(|&b| b == b'\n') {
            Some(i) => &buf[i + 1..],
            None => &[],
        }
    };
    let mut tail = Tail {
        modified,
        ..Tail::default()
    };
    // Read from the end: a tool result seen there answers the call it names.
    let mut answered: HashSet<String> = HashSet::new();
    let mut pending_done = false;
    for raw in body.split(|&b| b == b'\n').rev() {
        if !tail.text.is_empty() && pending_done {
            break;
        }
        let Ok(line) = serde_json::from_slice::<TailLine>(raw) else {
            continue;
        };
        if line.is_sidechain == Some(true) {
            continue;
        }
        let Some(Content::Blocks(blocks)) = line.message.and_then(|m| m.content) else {
            continue;
        };
        match line.kind.as_deref() {
            Some("user") => {
                for b in &blocks {
                    if b.kind.as_deref() == Some("tool_result") {
                        if let Some(id) = &b.tool_use_id {
                            answered.insert(id.clone());
                        }
                    }
                }
            }
            Some("assistant") => {
                for b in blocks.iter().rev() {
                    match b.kind.as_deref() {
                        Some("text") if tail.text.is_empty() => {
                            let t = crate::agents::plain_line(
                                b.text.as_deref().unwrap_or(""),
                                MESSAGE_CHARS,
                            );
                            if !t.is_empty() {
                                tail.text = t;
                            }
                        }
                        Some("tool_use") if !pending_done => {
                            // Only the latest call can be waiting; once it's seen, it's settled.
                            pending_done = true;
                            let open = b.id.as_ref().is_some_and(|id| !answered.contains(id));
                            if open {
                                let name = b.name.as_deref().unwrap_or("tool");
                                let gist =
                                    b.input.as_ref().and_then(pending_gist).unwrap_or_default();
                                let both = if gist.is_empty() {
                                    name.to_string()
                                } else {
                                    format!("{name}: {gist}")
                                };
                                tail.pending = crate::agents::plain_line(&both, MESSAGE_CHARS);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(tail)
}

/// The first prompt in the first `HEAD_BYTES` of the transcript at `path`: the first user line
/// that isn't a meta line, its text (a string, or its text blocks joined), at most `PROMPT_CHARS`
/// characters. None when there's none in that window.
pub fn read_first_prompt(path: &Path) -> io::Result<Option<String>> {
    use std::io::Read;
    let f = fs::File::open(path)?;
    let mut reader = BufReader::with_capacity(16 * 1024, f.take(HEAD_BYTES));
    let mut buf = Vec::new();
    while let Some(whole) = read_line_capped(&mut reader, &mut buf, HEAD_BYTES as usize)? {
        if !whole {
            continue;
        }
        let Ok(line) = serde_json::from_slice::<TailLine>(&buf) else {
            continue;
        };
        if line.kind.as_deref() != Some("user")
            || line.is_meta == Some(true)
            || line.is_sidechain == Some(true)
        {
            continue;
        }
        let text = match line.message.and_then(|m| m.content) {
            Some(Content::Text(t)) => t,
            Some(Content::Blocks(blocks)) => blocks
                .iter()
                .filter(|b| b.kind.as_deref() == Some("text"))
                .filter_map(|b| b.text.as_deref())
                .collect::<Vec<_>>()
                .join("\n"),
            None => continue,
        };
        if text.trim().is_empty() {
            continue;
        }
        return Ok(Some(text.chars().take(PROMPT_CHARS).collect()));
    }
    Ok(None)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::fs::File;
    use std::time::Duration;

    /// The key of the fixture's `~/playground`, and a project folder under it.
    pub(crate) const ROOT: &str = "-Users-someone-playground";
    pub(crate) const PROJECT: &str = "-Users-someone-playground-demo";

    /// A home folder with four account folders: `.claude` and `.claude-mg` with their own
    /// projects, `.claude-hg/projects` a link to `.claude/projects`, as on the owner's Mac, and
    /// `.claude-ih`, which isn't allowed.
    pub(crate) fn home() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        fs::create_dir_all(d.path().join(".claude/projects").join(PROJECT)).unwrap();
        fs::create_dir_all(d.path().join(".claude-mg/projects").join(PROJECT)).unwrap();
        fs::create_dir_all(d.path().join(".claude-ih/projects").join(PROJECT)).unwrap();
        fs::create_dir(d.path().join(".claude-hg")).unwrap();
        std::os::unix::fs::symlink(
            d.path().join(".claude/projects"),
            d.path().join(".claude-hg/projects"),
        )
        .unwrap();
        // Not an account folder.
        fs::create_dir_all(d.path().join("other/projects").join(PROJECT)).unwrap();
        d
    }

    /// A reader of `home`'s playground projects (`ROOT`), and the default allowlist, mg and hg,
    /// to read them in: `.claude` is read only through `.claude-hg`'s link.
    pub(crate) fn reader(home: &Path) -> (Reader, Vec<Account>) {
        (
            Reader::new(ROOT.to_string()),
            crate::accounts::resolve(&crate::accounts::default_labels(), home),
        )
    }

    pub(crate) fn reply(id: &str, at: &str, i: u64, o: u64, r: u64, w: u64) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"{at}","requestId":"req_{id}","message":{{"id":"{id}","role":"assistant","content":[{{"type":"text","text":"ok"}}],"usage":{{"input_tokens":{i},"output_tokens":{o},"cache_read_input_tokens":{r},"cache_creation_input_tokens":{w},"cache_creation":{{"ephemeral_1h_input_tokens":{w}}},"service_tier":"standard"}}}}}}"#
        )
    }

    pub(crate) fn prompt(at: &str, text: &str) -> String {
        serde_json::json!({"type": "user", "timestamp": at, "message": {"role": "user", "content": text}})
            .to_string()
    }

    /// Write a session's transcript under `account` (`.claude`, `.claude-mg`), modified at `at`.
    pub(crate) fn write(
        home: &Path,
        account: &str,
        session: &str,
        lines: &[String],
        at: u64,
    ) -> PathBuf {
        let p = home
            .join(account)
            .join("projects")
            .join(PROJECT)
            .join(format!("{session}.jsonl"));
        fs::write(&p, lines.join("\n") + "\n").unwrap();
        touch(&p, at);
        p
    }

    pub(crate) fn touch(p: &Path, at: u64) {
        File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(at))
            .unwrap();
    }

    pub(crate) const NOW: u64 = 2_000_000_000;
    const A: &str = "7b5e8856-c9c8-45ec-aa03-c0ddbf1d4394";
    const B: &str = "4d517d9d-1173-481b-80cc-27e72f903f3c";

    #[test]
    fn sums_each_reply_once_and_times_first_to_last() {
        let h = home();
        write(
            h.path(),
            ".claude",
            A,
            &[
                prompt("2026-10-12T21:47:10.995Z", "hello"),
                // Two content blocks of one reply: the same usage twice, counted once.
                reply("msg_1", "2026-10-12T21:47:12.795Z", 2, 97, 0, 33151),
                reply("msg_1", "2026-10-12T21:47:12.796Z", 2, 97, 0, 33151),
                reply("msg_2", "2026-10-12T21:47:14.205Z", 2, 116, 33151, 3562),
                prompt("2026-10-12T22:29:39.000Z", "more"),
            ],
            NOW - 3600,
        );
        let (r, a) = reader(h.path());
        let u = r.usage(&a, "7b5e8856", NOW);
        assert_eq!(u.found, Found::Read);
        assert_eq!(
            u.tokens,
            Tokens {
                input: 4,
                output: 213,
                cache_read: 33151,
                cache_write: 36713
            }
        );
        assert_eq!(u.tokens.total(), 4 + 213 + 33151 + 36713);
        assert_eq!(u.start, iso_secs("2026-10-12T21:47:10.995Z"));
        assert_eq!(u.end, iso_secs("2026-10-12T22:29:39.000Z"));
        // 42 min 29 s rounds to 42.
        assert_eq!(u.minutes, 42);
        assert!(!u.live);
        assert_eq!((u.skipped_lines, u.skipped_files), (0, 0));
    }

    #[test]
    fn reads_time_stamps_as_utc_seconds() {
        assert_eq!(iso_secs("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso_secs("1970-01-01T00:01:05.123Z"), Some(65));
        assert_eq!(iso_secs("2026-10-12T21:47:12.795Z"), Some(1_791_841_632));
        for bad in [
            "",
            "2026-10-12 21:47:12Z",
            "2026-10-12T21:47:12",
            "2026-10-12T21:47:12+02:00",
            "2026-02-30T21:47:12Z",
            "2026-10-12T21:47:60Z",
            "2026-10-12T21:47:1xZ",
            "2026-10-12T21:47:12.7x5Z",
            "2026-10-12T21:47:12.795Zé",
        ] {
            assert_eq!(iso_secs(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn finds_a_session_by_its_short_id_in_any_account_folder_once() {
        let h = home();
        write(
            h.path(),
            ".claude-mg",
            B,
            &[reply("m", "2026-10-12T10:00:00Z", 1, 2, 3, 4)],
            NOW - 9999,
        );
        // In .claude, so also seen through the .claude-hg link: still one transcript.
        write(
            h.path(),
            ".claude",
            A,
            &[reply("m", "2026-10-12T10:00:00Z", 10, 0, 0, 0)],
            NOW - 9999,
        );
        // A transcript outside the account folders isn't read.
        fs::write(
            h.path()
                .join("other/projects")
                .join(PROJECT)
                .join("aaaaaaaa-0000.jsonl"),
            reply("m", "2026-10-12T10:00:00Z", 1, 0, 0, 0),
        )
        .unwrap();
        let (r, a) = reader(h.path());
        assert_eq!(r.usage(&a, "4d517d9d", NOW).tokens.total(), 10);
        assert_eq!(r.usage(&a, "7b5e8856", NOW).tokens.input, 10);
        let dirs = r.refresh(&a);
        assert_eq!(r.candidates("7b5e8856", &dirs).len(), 1);
        assert_eq!(r.usage(&a, "aaaaaaaa", NOW).found, Found::Missing);
        // Only exact short ids are looked up.
        for bad in ["7B5E8856", "7b5e885", "7b5e8856-", "../../x", ""] {
            assert_eq!(r.locate(&a, bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_transcript_added_later_is_found() {
        let h = home();
        let (r, a) = reader(h.path());
        assert_eq!(r.usage(&a, "7b5e8856", NOW).found, Found::Missing);
        // Adding the file changes its project folder's time, so the index is rebuilt.
        write(
            h.path(),
            ".claude",
            A,
            &[reply("m", "2026-10-12T10:00:00Z", 5, 0, 0, 0)],
            NOW,
        );
        assert_eq!(r.usage(&a, "7b5e8856", NOW).tokens.input, 5);
    }

    #[test]
    fn a_missing_transcript_reads_not_on_this_mac() {
        let h = home();
        let (r, a) = reader(h.path());
        assert_eq!(r.usage(&a, "deadbeef", NOW), Usage::missing());
        assert!(!r.live(&a, "deadbeef", NOW));
        assert_eq!(
            r.transcript_markdown(&a, "deadbeef").unwrap_err(),
            "not on this Mac"
        );
        // No account folders at all.
        let empty = tempfile::tempdir().unwrap();
        let (r, a) = reader(empty.path());
        assert_eq!(r.usage(&a, "deadbeef", NOW).found, Found::Missing);
    }

    #[test]
    fn an_oversized_transcript_is_not_read() {
        let h = home();
        let p = write(
            h.path(),
            ".claude",
            A,
            &[reply("m", "2026-10-12T10:00:00Z", 5, 0, 0, 0)],
            NOW,
        );
        // Sparse: the size is over the cap without writing 128 MB.
        File::options()
            .write(true)
            .open(&p)
            .unwrap()
            .set_len(MAX_TRANSCRIPT_BYTES + 1)
            .unwrap();
        touch(&p, NOW - 10);
        let (r, a) = reader(h.path());
        let u = r.usage(&a, "7b5e8856", NOW);
        assert_eq!(
            (u.found, u.tokens.total(), u.live),
            (Found::TooBig, 0, true)
        );
        assert_eq!(r.cached(), 0);
        assert!(r
            .transcript_markdown(&a, "7b5e8856")
            .unwrap_err()
            .contains("over 128 MB"));
    }

    #[test]
    fn malformed_lines_are_skipped_and_counted() {
        let h = home();
        write(
            h.path(),
            ".claude",
            A,
            &[
                "not json".into(),
                "{\"type\":\"assistant\",\"message\":{\"id\":\"x\",\"usage\":{\"input_tokens\":-1}}}".into(),
                "{\"type\":\"assistant\",\"message\":\"text\"}".into(),
                "[1,2,3]".into(),
                "".into(),
                "   ".into(),
                reply("m1", "2026-10-12T10:00:00Z", 5, 6, 0, 0),
                // A reply without an id or usage: nothing to count, but its time counts.
                r#"{"type":"assistant","timestamp":"2026-10-12T10:10:00Z","message":{"content":[]}}"#.into(),
                r#"{"type":"user","timestamp":"yesterday"}"#.into(),
                // Cut off mid-line, as a live session's last line can be.
                r#"{"type":"assistant","timestamp":"2026-10-12T11:00:00Z","message":{"id":"m2","usage":{"input"#.into(),
            ],
            NOW - 9999,
        );
        let (r, a) = reader(h.path());
        let u = r.usage(&a, "7b5e8856", NOW);
        assert_eq!(u.found, Found::Read);
        assert_eq!((u.tokens.input, u.tokens.output), (5, 6));
        assert_eq!(u.skipped_lines, 5);
        assert_eq!(u.minutes, 10);
    }

    #[test]
    fn a_line_over_the_cap_is_skipped_without_losing_the_next() {
        let h = home();
        let huge = format!(
            r#"{{"type":"user","message":{{"content":"{}"}}}}"#,
            "x".repeat(MAX_LINE_BYTES + 10)
        );
        write(
            h.path(),
            ".claude",
            A,
            &[huge, reply("m1", "2026-10-12T10:00:00Z", 7, 0, 0, 0)],
            NOW - 9999,
        );
        let (r, a) = reader(h.path());
        let u = r.usage(&a, "7b5e8856", NOW);
        assert_eq!((u.tokens.input, u.skipped_lines), (7, 1));
    }

    #[test]
    fn reads_lines_whole_or_marks_them_over_the_cap() {
        let mut r = io::BufReader::with_capacity(4, &b"abc\n0123456789\n\nlast"[..]);
        let mut buf = Vec::new();
        let mut got = Vec::new();
        while let Some(whole) = read_line_capped(&mut r, &mut buf, 6).unwrap() {
            got.push((whole, String::from_utf8(buf.clone()).unwrap()));
        }
        assert_eq!(
            got,
            [
                (true, "abc".to_string()),
                (false, String::new()),
                (true, String::new()),
                (true, "last".to_string())
            ]
        );
    }

    #[test]
    fn subagents_count_toward_their_session() {
        let h = home();
        let main = write(
            h.path(),
            ".claude",
            A,
            &[reply("m", "2026-10-12T10:00:00Z", 1, 0, 0, 0)],
            NOW - 9999,
        );
        let subs = main.with_extension("").join("subagents");
        fs::create_dir_all(&subs).unwrap();
        fs::write(
            subs.join("agent-a1.jsonl"),
            reply("s1", "2026-10-12T10:01:00Z", 100, 0, 0, 0),
        )
        .unwrap();
        fs::write(
            subs.join("agent-a2.jsonl"),
            reply("s2", "2026-10-12T10:02:00Z", 1000, 0, 0, 0),
        )
        .unwrap();
        fs::write(subs.join("agent-a2.meta.json"), "{}").unwrap();
        touch(&subs.join("agent-a1.jsonl"), NOW - 9999);
        touch(&subs.join("agent-a2.jsonl"), NOW - 9999);
        // One over the size cap (sparse) is skipped and counted, not read.
        let big = File::create(subs.join("agent-a3.jsonl")).unwrap();
        big.set_len(MAX_TRANSCRIPT_BYTES + 1).unwrap();
        big.set_modified(UNIX_EPOCH + Duration::from_secs(NOW - 9999))
            .unwrap();
        let (r, a) = reader(h.path());
        let u = r.usage(&a, "7b5e8856", NOW);
        assert_eq!(u.tokens.input, 1101);
        assert_eq!(u.skipped_files, 1);
        assert_eq!(r.cached(), 3);
        // Minutes are the session's own, first to last line.
        assert_eq!(u.minutes, 0);
        assert!(!u.live);
        // A subagent working now makes the session live.
        touch(&subs.join("agent-a2.jsonl"), NOW - 60);
        assert!(r.live(&a, "7b5e8856", NOW));
        assert!(r.usage(&a, "7b5e8856", NOW).live);
    }

    #[test]
    fn live_means_changed_in_the_last_five_minutes() {
        let h = home();
        let p = write(
            h.path(),
            ".claude",
            A,
            &[reply("m", "2026-10-12T10:00:00Z", 1, 0, 0, 0)],
            NOW - LIVE_SECS,
        );
        let (r, a) = reader(h.path());
        assert!(r.live(&a, "7b5e8856", NOW));
        assert!(r.usage(&a, "7b5e8856", NOW).live);
        assert!(!r.live(&a, "7b5e8856", NOW + 1));
        assert!(!r.usage(&a, "7b5e8856", NOW + 1).live);
        touch(&p, NOW + 1);
        assert!(r.live(&a, "7b5e8856", NOW + 1));
        // A file time ahead of the clock still reads as live, not as an error.
        assert!(r.live(&a, "7b5e8856", NOW - 100));
    }

    #[test]
    fn the_live_window_is_the_one_set_on_the_reader() {
        let h = home();
        write(
            h.path(),
            ".claude",
            A,
            &[reply("m", "2026-10-12T10:00:00Z", 1, 0, 0, 0)],
            NOW,
        );
        let (r, a) = reader(h.path());
        // Settings' LIVE window (0036): 10 minutes.
        r.set_live_secs(600);
        assert!(r.live(&a, "7b5e8856", NOW + 600));
        assert!(r.usage(&a, "7b5e8856", NOW + 600).live);
        assert!(r.live_set(&a, ["7b5e8856"], NOW + 600).contains("7b5e8856"));
        assert!(!r.live(&a, "7b5e8856", NOW + 601));
        // 1 minute.
        r.set_live_secs(60);
        assert!(!r.live(&a, "7b5e8856", NOW + 61));
        assert!(r.live_set(&a, ["7b5e8856"], NOW + 61).is_empty());
    }

    #[test]
    fn summaries_are_cached_by_size_and_modified_time() {
        let h = home();
        let p = write(
            h.path(),
            ".claude",
            A,
            &[reply("m", "2026-10-12T10:00:00Z", 1, 0, 0, 0)],
            NOW - 9999,
        );
        let (r, a) = reader(h.path());
        assert_eq!(r.usage(&a, "7b5e8856", NOW).tokens.input, 1);
        // Same size and time, different content: the cached sums are used.
        let same = fs::read_to_string(&p)
            .unwrap()
            .replace("\"input_tokens\":1,", "\"input_tokens\":9,");
        fs::write(&p, same).unwrap();
        touch(&p, NOW - 9999);
        assert_eq!(r.usage(&a, "7b5e8856", NOW).tokens.input, 1);
        // A new time: read again.
        touch(&p, NOW - 9998);
        assert_eq!(r.usage(&a, "7b5e8856", NOW).tokens.input, 9);
        assert_eq!(r.cached(), 1);
    }

    #[test]
    fn the_cache_is_capped() {
        let h = home();
        let (r, _) = reader(h.path());
        let dir = h.path().join("loose");
        fs::create_dir(&dir).unwrap();
        for n in 0..CACHE_CAP + 20 {
            let p = dir.join(format!("{n}.jsonl"));
            fs::write(&p, reply("m", "2026-10-12T10:00:00Z", n as u64, 0, 0, 0)).unwrap();
            assert_eq!(r.summary(&p).unwrap().tokens.input, n as u64);
        }
        assert_eq!(r.cached(), CACHE_CAP);
        // The least recently used went first; the latest are still there.
        let last = dir.join(format!("{}.jsonl", CACHE_CAP + 19));
        assert!(lock(&r.cache).files.contains_key(&last));
        assert!(!lock(&r.cache).files.contains_key(&dir.join("0.jsonl")));
    }

    #[test]
    fn actuals_sum_only_what_was_read() {
        let mut a = Usage::missing();
        a.found = Found::Read;
        a.tokens = Tokens {
            input: 1,
            output: 2,
            cache_read: 3,
            cache_write: 4,
        };
        a.minutes = 10;
        let mut b = a.clone();
        b.minutes = 5;
        let gone = Usage::missing();
        let mut big = Usage::missing();
        big.found = Found::TooBig;
        let s = actuals([&a, &b, &gone, &big]);
        assert_eq!(
            s.tokens,
            Tokens {
                input: 2,
                output: 4,
                cache_read: 6,
                cache_write: 8
            }
        );
        assert_eq!((s.minutes, s.read), (15, 2));
        assert_eq!(actuals([]), Actuals::default());
    }

    #[test]
    fn the_view_folds_prompts_replies_and_tool_calls_to_one_line_each() {
        let h = home();
        let lines = vec![
            prompt("2026-10-12T10:00:00Z", "Fix the  board\n\nplease <b>now</b> ![x](https://e.example/i.png)"),
            serde_json::json!({"type": "user", "isMeta": true, "message": {"content": "meta, hidden"}}).to_string(),
            serde_json::json!({"type": "assistant", "message": {"id": "m1", "content": [
                {"type": "thinking", "thinking": "secret thoughts"},
                {"type": "text", "text": "On it.\n# Not a heading"},
                {"type": "tool_use", "name": "Bash", "input": {"command": "ls -la\npwd", "description": "List files"}},
                {"type": "tool_use", "name": "Read", "input": {"file_path": "/tmp/x.rs"}},
                {"type": "tool_use", "name": "Odd", "input": {"n": 1}}
            ]}}).to_string(),
            serde_json::json!({"type": "user", "message": {"content": [
                {"type": "tool_result", "content": "tool output, hidden"},
                {"type": "text", "text": "and this"}
            ]}}).to_string(),
            serde_json::json!({"type": "attachment", "attachment": {"x": 1}}).to_string(),
            "garbage".into(),
        ];
        write(h.path(), ".claude", A, &lines, NOW);
        let (r, a) = reader(h.path());
        let md = r.transcript_markdown(&a, "7b5e8856").unwrap();
        let entries: Vec<&str> = md.lines().filter(|l| l.starts_with("- ")).collect();
        assert_eq!(
            entries,
            [
                r"- **You:** Fix the board please \<b\>now\<\/b\> \!\[x\]\(https\:\/\/e\.example\/i\.png\)",
                r"- **Claude:** On it\. \# Not a heading",
                r"- **Tool:** Bash · List files",
                r"- **Tool:** Read · \/tmp\/x\.rs",
                r"- **Tool:** Odd",
                r"- **You:** and this",
            ]
        );
        assert!(!md.contains("secret") && !md.contains("hidden"));
    }

    #[test]
    fn the_view_is_capped_and_keeps_the_latest() {
        let h = home();
        let long = "é".repeat(VIEW_LINE_CHARS + 50);
        let mut lines: Vec<String> = (0..VIEW_ENTRIES + 3)
            .map(|n| prompt("2026-10-12T10:00:00Z", &format!("p{n}")))
            .collect();
        lines.push(prompt("2026-10-12T10:00:00Z", &long));
        write(h.path(), ".claude", A, &lines, NOW);
        let (r, a) = reader(h.path());
        let md = r.transcript_markdown(&a, "7b5e8856").unwrap();
        let entries: Vec<&str> = md.lines().filter(|l| l.starts_with("- ")).collect();
        assert_eq!(entries.len(), VIEW_ENTRIES);
        assert_eq!(entries[0], "- **You:** p4");
        assert!(md.contains("The first 4 entries aren't shown"));
        assert_eq!(
            *entries.last().unwrap(),
            format!("- **You:** {}…", "é".repeat(VIEW_LINE_CHARS))
        );
        let empty = home();
        fs::write(
            empty
                .path()
                .join(".claude-mg/projects")
                .join(PROJECT)
                .join(format!("{B}.jsonl")),
            "",
        )
        .unwrap();
        let (r, a) = reader(empty.path());
        let md = r.transcript_markdown(&a, "4d517d9d").unwrap();
        assert!(md.contains("No prompts, replies or tool calls in it yet."));
    }

    /// Write `<projects>/<project>/<session>.jsonl` with one reply of `input` tokens.
    fn put(projects: &Path, project: &str, session: &str, input: u64) -> PathBuf {
        let dir = projects.join(project);
        fs::create_dir_all(&dir).unwrap();
        let p = dir.join(format!("{session}.jsonl"));
        fs::write(
            &p,
            reply("m", "2026-10-12T10:00:00Z", input, 0, 0, 0) + "\n",
        )
        .unwrap();
        p
    }

    #[test]
    fn only_the_allowed_accounts_playground_projects_are_read() {
        let h = home();
        let company = h.path().join(".claude/projects");
        let mg = h.path().join(".claude-mg/projects");
        // The playground folder itself, and a project under it: read.
        put(&mg, ROOT, "00000001-x", 1);
        put(&company, PROJECT, "00000002-x", 2);
        // A work project, seen through `.claude-hg`'s link to the company folder: never.
        put(&company, "-Users-someone-code-app", "00000003-x", 3);
        // A look-alike of the playground's key, and a key that only contains it.
        put(&mg, "-Users-someone-playground2-x", "00000004-x", 4);
        put(
            &mg,
            "-Users-someone-code-Users-someone-playground",
            "00000005-x",
            5,
        );
        // An account that isn't allowed.
        put(
            &h.path().join(".claude-ih/projects"),
            PROJECT,
            "00000006-x",
            6,
        );
        let (r, a) = reader(h.path());
        assert_eq!(r.usage(&a, "00000001", NOW).tokens.input, 1);
        assert_eq!(r.usage(&a, "00000002", NOW).tokens.input, 2);
        for refused in ["00000003", "00000004", "00000005", "00000006"] {
            assert_eq!(r.usage(&a, refused, NOW), Usage::missing(), "{refused}");
            assert!(!r.live(&a, refused, NOW), "{refused}");
            assert!(r.transcript_markdown(&a, refused).is_err(), "{refused}");
        }
        // Nothing outside the playground is even listed.
        let listed = r.signature(&Reader::projects_dirs(&a));
        assert_eq!(listed.len(), 3, "{listed:?}");
        for (dir, _) in &listed {
            let name = dir.file_name().unwrap().to_str().unwrap();
            assert!(is_under(name, ROOT), "{name}");
        }
        // An empty allowlist: nothing at all.
        assert_eq!(r.usage(&[], "00000001", NOW).found, Found::Missing);
        assert!(r.signature(&Reader::projects_dirs(&[])).is_empty());
        // Only mg: the company folder's transcripts, reached only through hg, go too.
        let mg_only = crate::accounts::resolve(&["mg".into()], h.path());
        assert_eq!(r.usage(&mg_only, "00000001", NOW).tokens.input, 1);
        assert_eq!(r.usage(&mg_only, "00000002", NOW).found, Found::Missing);
    }

    /// R6 (0099): each read is given its own accounts, and the index the reads share answers a
    /// read only from that read's folders. A read loading the list from before hg was removed
    /// can rebuild the index between another read's refresh and its lookups; that read still
    /// finds nothing of hg's.
    #[test]
    fn an_index_built_for_other_accounts_never_answers_a_read() {
        let h = home();
        let company = h.path().join(".claude/projects");
        put(
            &h.path().join(".claude-mg/projects"),
            PROJECT,
            "00000001-x",
            1,
        );
        // Reached only through hg's link.
        put(&company, PROJECT, "00000002-x", 2);
        let (r, both) = reader(h.path());
        let mg_only = crate::accounts::resolve(&["mg".into()], h.path());
        // This read refreshes for mg alone…
        let dirs = r.refresh(&mg_only);
        // …then a read with mg and hg rebuilds the shared index before it looks anything up.
        assert_eq!(r.usage(&both, "00000002", NOW).tokens.input, 2);
        assert!(r.candidates("00000002", &dirs).is_empty(), "hg's file");
        assert_eq!(r.candidates("00000001", &dirs).len(), 1);
        // Through every read, one after the other, the same.
        assert_eq!(r.usage(&mg_only, "00000002", NOW).found, Found::Missing);
        assert!(r.live_set(&mg_only, ["00000002"], NOW).is_empty());
        assert!(r.transcript_markdown(&mg_only, "00000002").is_err());
        assert_eq!(r.usage(&both, "00000002", NOW).tokens.input, 2);
    }

    #[test]
    fn links_inside_the_projects_folders_are_not_followed() {
        use std::os::unix::fs::symlink;
        let h = home();
        let mg = h.path().join(".claude-mg/projects");
        let work = put(&h.path().join("other"), "work", "00000001-x", 1);
        // A project folder that's a link to a work folder.
        symlink(work.parent().unwrap(), mg.join(format!("{ROOT}-linked"))).unwrap();
        // A transcript that's a link to a work transcript.
        symlink(&work, mg.join(PROJECT).join("00000002-x.jsonl")).unwrap();
        let (r, a) = reader(h.path());
        assert_eq!(r.usage(&a, "00000001", NOW).found, Found::Missing);
        assert_eq!(r.usage(&a, "00000002", NOW).found, Found::Missing);
        // A session whose subagents are links, or whose subagents folder is one.
        let main = put(&mg, PROJECT, "00000003-x", 3);
        let subs = main.with_extension("").join("subagents");
        fs::create_dir_all(&subs).unwrap();
        symlink(&work, subs.join("agent-a1.jsonl")).unwrap();
        fs::write(
            subs.join("agent-a2.jsonl"),
            reply("s", "2026-10-12T10:00:00Z", 10, 0, 0, 0),
        )
        .unwrap();
        let u = r.usage(&a, "00000003", NOW);
        assert_eq!((u.tokens.input, u.skipped_files), (13, 0));
        let other = put(&mg, PROJECT, "00000004-x", 4);
        fs::create_dir(other.with_extension("")).unwrap();
        symlink(&subs, other.with_extension("").join("subagents")).unwrap();
        assert_eq!(r.usage(&a, "00000004", NOW).tokens.input, 4);
        let linked = put(&mg, PROJECT, "00000005-x", 5);
        symlink(main.with_extension(""), linked.with_extension("")).unwrap();
        assert_eq!(r.usage(&a, "00000005", NOW).tokens.input, 5);
    }

    /// The owner's Mac (`accounts::tests::fake_home`): mg's and hg's `projects` are links to the
    /// company account's. A playground transcript is read through them; a work one never is, and
    /// the company account's settings and login are never opened: here they're FIFOs, which
    /// block whoever opens one to read, so a full read on a thread that opened either would
    /// never finish.
    #[test]
    fn the_company_account_is_never_opened_and_its_work_transcripts_never_found() {
        let d = crate::accounts::tests::fake_home();
        let h = d.path().to_path_buf();
        for f in [".claude/settings.json", ".claude.json"] {
            fs::remove_file(h.join(f)).unwrap();
            let made = std::process::Command::new("mkfifo")
                .arg(h.join(f))
                .status()
                .unwrap();
            assert!(made.success(), "mkfifo {f}");
        }
        let projects = h.join(".claude/projects");
        put(&projects, "-Users-x-code-app", "aaaaaaaa-0000", 7);
        put(
            &projects,
            "-Users-x-playground-muster-main",
            "bbbbbbbb-0000",
            9,
        );
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let r = Reader::new("-Users-x-playground".to_string());
            let a = crate::accounts::resolve(&crate::accounts::default_labels(), &h);
            let ids = ["aaaaaaaa".to_string(), "bbbbbbbb".to_string()];
            let usages = r.usages(&a, &ids, NOW);
            // Both were just written, so both would be live if found.
            let live = r.live_set(&a, ["aaaaaaaa", "bbbbbbbb"], secs(SystemTime::now()));
            let work = (
                r.locate(&a, "aaaaaaaa"),
                r.transcript_markdown(&a, "aaaaaaaa").is_ok(),
            );
            let mine = r.transcript_markdown(&a, "bbbbbbbb").is_ok();
            let dirs = r.signature(&Reader::projects_dirs(&a)).len();
            tx.send((usages, live, work, mine, dirs)).unwrap();
        });
        let (usages, live, work, mine, dirs) = rx
            .recv_timeout(Duration::from_secs(20))
            .expect("the read finished: nothing opened the company account's FIFOs");
        assert_eq!(usages[0], Usage::missing());
        assert_eq!((usages[1].found, usages[1].tokens.input), (Found::Read, 9));
        assert!(live.contains("bbbbbbbb") && !live.contains("aaaaaaaa"));
        assert_eq!(work, (None, false));
        assert!(mine);
        // mg's and hg's links are one folder, listed once, with only its playground project.
        assert_eq!(dirs, 1);
    }

    pub(crate) fn said(text: &str) -> String {
        serde_json::json!({"type": "assistant", "message": {"role": "assistant", "content": [{"type": "text", "text": text}]}})
            .to_string()
    }

    pub(crate) fn calls(id: &str, tool: &str, command: &str) -> String {
        serde_json::json!({"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "text", "text": "Running it."},
            {"type": "tool_use", "id": id, "name": tool, "input": {"command": command}}
        ]}})
        .to_string()
    }

    pub(crate) fn answers(id: &str) -> String {
        serde_json::json!({"type": "user", "message": {"role": "user", "content": [
            {"type": "tool_result", "tool_use_id": id, "content": "done"}
        ]}})
        .to_string()
    }

    /// `lines` as the transcript of `session` in mg's playground project.
    fn put_lines(h: &Path, session: &str, lines: &[String]) -> PathBuf {
        let p = h
            .join(".claude-mg/projects")
            .join(PROJECT)
            .join(format!("{session}.jsonl"));
        fs::write(&p, lines.join("\n") + "\n").unwrap();
        p
    }

    #[test]
    fn the_tail_says_the_last_text_and_the_call_still_waiting() {
        let h = home();
        let p = put_lines(
            h.path(),
            "00000001-x",
            &[
                prompt("2026-10-12T10:00:00Z", "go"),
                said("First answer."),
                calls("t1", "Read", "x"),
                answers("t1"),
                said("Now   the\nsecond, ب‌فارسی."),
                calls("t2", "Bash", "git push origin main && echo ok"),
            ],
        );
        let t = read_tail(&p, 7).unwrap();
        // The last text is in the line with the waiting call ("Running it."), on one line.
        assert_eq!(t.text, "Running it.");
        assert_eq!(t.pending, "Bash: git push origin main && echo ok");
        assert_eq!(t.modified, 7);
        // Answered: nothing waits, and the last text stays.
        let p = put_lines(
            h.path(),
            "00000002-x",
            &[
                said("Now   the\nsecond, ب‌فارسی."),
                calls("t2", "Bash", "ls"),
                answers("t2"),
            ],
        );
        let t = read_tail(&p, 0).unwrap();
        assert_eq!((t.text.as_str(), t.pending.as_str()), ("Running it.", ""));
        let p = put_lines(
            h.path(),
            "00000003-x",
            &[said("Now   the\nsecond, ب‌فارسی.")],
        );
        assert_eq!(read_tail(&p, 0).unwrap().text, "Now the second, ب‌فارسی.");
        // Only an older call is open: the latest call settled it, so nothing waits.
        let p = put_lines(
            h.path(),
            "00000004-x",
            &[
                calls("old", "Bash", "a"),
                calls("new", "Read", "b"),
                answers("new"),
            ],
        );
        assert_eq!(read_tail(&p, 0).unwrap().pending, "");
        // R4: a call with a description too shows what the owner approves: the command, the
        // path or the address, the description only when it has none of those.
        let described = |id: &str, tool: &str, input: serde_json::Value| {
            serde_json::json!({"type": "assistant", "message": {"role": "assistant", "content": [
                {"type": "tool_use", "id": id, "name": tool, "input": input}
            ]}})
            .to_string()
        };
        for (input, want) in [
            (
                serde_json::json!({"description": "Push main to origin", "command": "git push origin main"}),
                "Bash: git push origin main",
            ),
            (
                serde_json::json!({"description": "Edit it", "file_path": "/x/a.rs"}),
                "Bash: /x/a.rs",
            ),
            (
                serde_json::json!({"description": "Fetch it", "url": "https://example.com/a"}),
                "Bash: https://example.com/a",
            ),
            (
                serde_json::json!({"description": "Look around", "prompt": "find the bug"}),
                "Bash: Look around",
            ),
        ] {
            let p = put_lines(h.path(), "00000006-x", &[described("d1", "Bash", input)]);
            assert_eq!(read_tail(&p, 0).unwrap().pending, want);
        }
        // A subagent's line and broken lines don't count; an empty file is empty.
        let side = serde_json::json!({"type": "assistant", "isSidechain": true, "message": {"content": [{"type": "text", "text": "sub"}]}}).to_string();
        let p = put_lines(
            h.path(),
            "00000005-x",
            &[said("main"), side, "{broken".into(), "42".into()],
        );
        assert_eq!(read_tail(&p, 0).unwrap().text, "main");
        let p = put_lines(h.path(), "00000006-x", &[]);
        assert_eq!(read_tail(&p, 0).unwrap(), Tail::default());
    }

    #[test]
    fn the_tail_reads_only_whole_lines_of_the_last_64_kb_and_cuts_by_characters() {
        let h = home();
        let long = "ش".repeat(400);
        // A first line bigger than the window: only whole lines after it count.
        let p = put_lines(
            h.path(),
            "00000001-x",
            &[said(&"x".repeat(TAIL_BYTES as usize)), said(&long)],
        );
        let t = read_tail(&p, 0).unwrap();
        assert_eq!(t.text.chars().count(), MESSAGE_CHARS + 1);
        assert!(t.text.starts_with('ش') && t.text.ends_with('…'));
        // A last line bigger than the window: nothing whole in it, so no message.
        let p = put_lines(
            h.path(),
            "00000002-x",
            &[said("early"), said(&"y".repeat(TAIL_BYTES as usize + 10))],
        );
        assert_eq!(read_tail(&p, 0).unwrap().text, "");
        // Hostile text stays text: no escaping here, the window shows it plain.
        let p = put_lines(
            h.path(),
            "00000003-x",
            &[said("<img src=x onerror=alert(1)> **b**\u{202e}")],
        );
        assert_eq!(
            read_tail(&p, 0).unwrap().text,
            "<img src=x onerror=alert(1)> **b**"
        );
    }

    #[test]
    fn the_first_prompt_is_the_first_real_user_text_in_the_head() {
        let h = home();
        let meta =
            serde_json::json!({"type": "user", "isMeta": true, "message": {"content": "meta"}})
                .to_string();
        let resume = "<command-message>handoff</command-message>\n<command-name>/handoff</command-name>\n<command-args>resume /Users/someone/playground/x/.claude/work/handoffs/a.md</command-args>";
        let p = put_lines(
            h.path(),
            "00000001-x",
            &[
                r#"{"type":"custom-title","customTitle":"x"}"#.into(),
                meta,
                answers("t0"),
                prompt("2026-10-12T10:00:00Z", resume),
                prompt("2026-10-12T10:01:00Z", "later"),
            ],
        );
        assert_eq!(read_first_prompt(&p).unwrap().as_deref(), Some(resume));
        // Text blocks are joined.
        let blocks = serde_json::json!({"type": "user", "message": {"content": [{"type": "text", "text": "a"}, {"type": "image"}, {"type": "text", "text": "b"}]}}).to_string();
        let p = put_lines(h.path(), "00000002-x", &[blocks]);
        assert_eq!(read_first_prompt(&p).unwrap().as_deref(), Some("a\nb"));
        // None in the head: a prompt after 128 KB isn't looked for.
        let p = put_lines(
            h.path(),
            "00000003-x",
            &[
                said(&"z".repeat(HEAD_BYTES as usize)),
                prompt("2026-10-12T10:00:00Z", "late"),
            ],
        );
        assert_eq!(read_first_prompt(&p).unwrap(), None);
        // Cut to PROMPT_CHARS characters.
        let p = put_lines(
            h.path(),
            "00000004-x",
            &[prompt("2026-10-12T10:00:00Z", &"گ".repeat(5000))],
        );
        assert_eq!(
            read_first_prompt(&p).unwrap().unwrap().chars().count(),
            PROMPT_CHARS
        );
    }

    #[test]
    fn tails_and_first_prompts_read_only_allowed_accounts_are_cached_and_dropped_when_gone() {
        let h = home();
        let p = put_lines(
            h.path(),
            "00000001-x",
            &[prompt("2026-10-12T10:00:00Z", "hi"), said("one")],
        );
        touch(&p, NOW - 10);
        // In an account that isn't allowed.
        put(
            &h.path().join(".claude-ih/projects"),
            PROJECT,
            "00000002-x",
            1,
        );
        let (r, a) = reader(h.path());
        let ids: Vec<String> = ["00000001", "00000002", "nothex!!"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let tails = r.tails(&a, &ids);
        assert_eq!(tails[0].as_ref().unwrap().text, "one");
        assert_eq!(tails[0].as_ref().unwrap().modified, NOW - 10);
        assert_eq!((tails[1].clone(), tails[2].clone()), (None, None));
        assert_eq!(
            r.first_prompts(&a, &ids),
            [Some("hi".to_string()), None, None]
        );
        assert_eq!(r.texts_cached(), 2);
        // Changed: read again. A found first prompt is kept.
        fs::write(
            &p,
            [prompt("2026-10-12T10:00:00Z", "hi"), said("two")].join("\n") + "\n",
        )
        .unwrap();
        touch(&p, NOW - 5);
        assert_eq!(r.tails(&a, &ids)[0].as_ref().unwrap().text, "two");
        // Gone: what was cached of it goes at the next read.
        fs::remove_file(&p).unwrap();
        assert_eq!(r.tails(&a, &ids)[0], None);
        assert_eq!(r.first_prompts(&a, &ids)[0], None);
        assert_eq!(r.texts_cached(), 0);
        // Only through the accounts given: none, nothing.
        put_lines(h.path(), "00000003-x", &[said("three")]);
        assert_eq!(r.tails(&[], &["00000003".to_string()]), [None]);
    }
}
