//! Which queues the app shows: the ones found under `~/playground/*/tasks` and one level deeper
//! (`~/playground/*/*/tasks`, `queue::discover`), plus ones the owner added from anywhere, minus
//! the ones they hid, each with an optional display name. Saved as `projects.json` in the app's
//! config folder. Creating a queue writes Muster's layout into a new
//! `tasks/` folder, never inside a git work tree, and never over an existing folder.

use crate::{layout, queue};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// Queue folders added by hand.
    pub added: Vec<String>,
    /// Queue folders not to show.
    pub hidden: Vec<String>,
    /// Names by queue folder, from the Rename the window had before labels (0044). Still read, so a
    /// project renamed then keeps that name in commands; the window sets labels now.
    pub names: BTreeMap<String, String>,
    /// What the window calls a project, by queue folder (0044, `labels`): shown only, never the
    /// name commands use. A bad entry is left out when read, as with the categories.
    #[serde(deserialize_with = "crate::labels::read_labels")]
    pub labels: BTreeMap<String, String>,
    /// The sidebar heading a project goes under, by queue folder (0044, `labels`).
    #[serde(deserialize_with = "crate::labels::read_categories")]
    pub categories: BTreeMap<String, String>,
    /// The project's gate command by queue folder (0023): what a merge runs in the task's
    /// worktree, split into words without a shell (`bin/gate`, `make check`).
    pub gate: BTreeMap<String, String>,
    /// The project's run command by queue folder (0077): what the app's Run starts in the main
    /// checkout to try the project (`bin/run`, `swift run`), split into words without a shell as
    /// the gate is; `run::plan` splits it again on every start (0079). An entry `set_run`
    /// wouldn't keep is left out when read, and never stops the rest of the file loading.
    #[serde(deserialize_with = "read_commands")]
    pub run: BTreeMap<String, String>,
    /// The Your check limit by queue folder (0050, `check_limit`): a number, or "off". A bad
    /// entry is left out when read, and never stops the rest of the file loading.
    #[serde(deserialize_with = "crate::check_limit::read_limits")]
    pub your_check: BTreeMap<String, crate::check_limit::Limit>,
    /// A colour per project, by queue folder (0057, `arrange`): one of `arrange::COLOURS`. An
    /// unknown one is left out when read.
    #[serde(deserialize_with = "crate::arrange::read_colours")]
    pub colours: BTreeMap<String, String>,
    /// The sidebar's Mine order (0057, `arrange`): queue folders, first to last.
    #[serde(deserialize_with = "crate::arrange::read_order")]
    pub order: Vec<String>,
    /// The sidebar's category headings in the owner's order (0057, `arrange`).
    #[serde(deserialize_with = "crate::arrange::read_category_order")]
    pub category_order: Vec<String>,
    /// The sidebar's folded headings (0070, `arrange`), by name.
    #[serde(deserialize_with = "crate::arrange::read_category_order")]
    pub folded: Vec<String>,
    /// A group folder's display name (0070, `arrange`): the heading its projects go under when
    /// they have no category of their own (`persianUtils` → "DashAfzar"). Read as categories are.
    #[serde(deserialize_with = "crate::labels::read_categories")]
    pub group_names: BTreeMap<String, String>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    /// Stable handle for the window: the queue folder. The window only sends it back to name a
    /// project from this list; it's never used unless it matches one.
    pub key: String,
    pub name: String,
    pub path: String,
    /// "found" (under ~/playground, one level deeper too) or "added". A queue that is both shows
    /// once, as found.
    pub source: String,
    /// The folder that holds a found queue's project one level deeper (`persianUtils` for
    /// `~/playground/persianUtils/colors/tasks`); "" for the rest.
    pub group: String,
    pub hidden: bool,
    /// The folder still looks like a Muster queue.
    pub ok: bool,
    /// Its gate command (0023), or "" when none is set.
    pub gate: String,
    /// Its run command (0077), or "" when none is set.
    pub run: String,
    /// What its tasks' keys start with (0033): `mu` in `mu-12`, stored or its default
    /// (`key::prefixes`).
    pub prefix: String,
    /// Its label (0044), what the window calls it, or "" for its name; `labels::shown_label`.
    pub label: String,
    /// Its category (0044), the sidebar heading it goes under, or "" for its group or Other;
    /// `labels::shown_category`.
    pub category: String,
    /// Its colour (0057), one of `arrange::COLOURS`, or "" for none.
    pub colour: String,
    /// Its group's display name (0070), "" for none: the group folder's own name shows then.
    pub group_name: String,
}

/// Where the projects Muster knows come from (task 0062): the folder queues are found under and
/// projects.json. Every read of them starts at `places`.
#[derive(Debug, Clone, PartialEq)]
pub struct Places {
    /// `$MUSTER_BASE`, else `~/playground`.
    pub base: PathBuf,
    /// `projects.json` in the app's config folder.
    pub config_file: PathBuf,
}

impl Places {
    /// This Mac's, from `$MUSTER_BASE` and `$HOME`.
    fn this_mac() -> Places {
        let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
        Places {
            base: std::env::var_os("MUSTER_BASE")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join("playground")),
            config_file: home
                .join("Library/Application Support/ai.sweetlemon.muster/projects.json"),
        }
    }

    /// Empty ones in a folder of this thread's own, removed when the thread ends: what a test
    /// that gives none gets.
    #[cfg(test)]
    fn empty() -> Places {
        thread_local! {
            static DIR: tempfile::TempDir =
                tempfile::tempdir().expect("a temporary folder for a test's empty places");
        }
        DIR.with(|d| Places {
            base: d.path().join("playground"),
            config_file: d.path().join("projects.json"),
        })
    }
}

thread_local! {
    static GIVEN: std::cell::RefCell<Option<Places>> = const { std::cell::RefCell::new(None) };
}

/// The places the projects Muster knows are read from (`key::known`, the command line, the app):
/// the ones this thread was given (`use_places`), else this Mac's. In a test, else empty ones of
/// its own, so nothing on this Mac changes what a test sees.
pub fn places() -> Places {
    GIVEN
        .with(|g| g.borrow().clone())
        .unwrap_or_else(by_default)
}

#[cfg(not(test))]
fn by_default() -> Places {
    Places::this_mac()
}

#[cfg(test)]
fn by_default() -> Places {
    Places::empty()
}

/// Read the projects Muster knows from `p` for the rest of this thread: a test's own projects
/// (an integration test's too, whose build of the core reads this Mac's otherwise). A thread it
/// starts gets its own.
pub fn use_places(p: Places) {
    GIVEN.with(|g| *g.borrow_mut() = Some(p));
}

/// In a test, refuse to read or write this Mac's projects.json or look under its ~/playground:
/// a test does that only through `places`, never by working the path out itself.
#[cfg(test)]
pub(crate) fn not_this_macs(path: &Path) {
    let canon = |p: &Path| fs::canonicalize(p).ok();
    let mac = Places::this_mac();
    for (real, what) in [
        (&mac.config_file, "this Mac's projects.json"),
        (&mac.base, "this Mac's ~/playground"),
    ] {
        let same = path == real.as_path() || canon(path).is_some_and(|c| Some(c) == canon(real));
        assert!(
            !same,
            "a test used {what}: give it its own with projects::use_places(Places {{ base, config_file }}), or leave it to projects::places(), which gives a test empty ones"
        );
    }
}

pub fn load(file: &Path) -> Config {
    #[cfg(test)]
    not_this_macs(file);
    fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Write projects.json whole: to a temporary file of this writer's own (process id and a
/// counter), then renamed over it, so a reader sees the old file or the new one, never half.
/// Changes go through `edit`, which holds the lock around the load, the change and this.
pub fn save(file: &Path, config: &Config) -> Result<(), String> {
    #[cfg(test)]
    not_this_macs(file);
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = file.with_extension(format!("json.{}-{n}.tmp", std::process::id()));
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())? + "\n";
    fs::write(&tmp, text)
        .and_then(|_| fs::rename(&tmp, file))
        .map_err(|e| {
            let _ = fs::remove_file(&tmp);
            e.to_string()
        })
}

/// Change projects.json: load, `f`, save, holding an exclusive flock on `projects.json.lock`
/// beside it the whole time, so the app and `muster projects set` (or two of either) never lose
/// each other's edits. Nothing is written when `f` refuses.
pub fn edit<T>(file: &Path, f: impl FnOnce(&mut Config) -> Result<T, String>) -> Result<T, String> {
    #[cfg(test)]
    not_this_macs(file);
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(file.with_extension("json.lock"))
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    let result = (|| {
        let mut c = load(file);
        let out = f(&mut c)?;
        save(file, &c)?;
        Ok(out)
    })();
    let _ = lock.unlock();
    result
}

/// A queue: a repo's `.muster/` (task 0005), or an old `<project>/tasks` (a README plus a
/// status folder or a manifest).
pub fn looks_like_queue(dir: &Path) -> bool {
    layout::is_queue(dir)
        || (dir.join("README.md").is_file()
            && (dir.join("todo").is_dir() || dir.join("tasks.json").is_file()))
}

pub fn default_name(dir: &Path) -> String {
    // `<project>/<checkout>/.muster` and `<project>/tasks` → the project's folder name; anything
    // else → its own name.
    if layout::is_muster(dir) {
        if let Some(name) = layout::project_name(dir) {
            return name;
        }
    }
    let own = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if own == "tasks" {
        if let Some(p) = dir.parent().and_then(|p| p.file_name()) {
            return p.to_string_lossy().to_string();
        }
    }
    own
}

/// Every project, in a stable order, with unique names (a clash gets "-2", "-3"…).
pub fn list(base: &Path, config: &Config) -> Vec<Project> {
    let hidden: HashSet<&str> = config.hidden.iter().map(String::as_str).collect();
    let mut out: Vec<Project> = Vec::new();
    let mut push = |path: PathBuf, source: &str, group: String| {
        let key = path.to_string_lossy().to_string();
        if out.iter().any(|p| p.key == key) {
            return;
        }
        out.push(Project {
            name: config
                .names
                .get(&key)
                .cloned()
                .unwrap_or_else(|| default_name(&path)),
            ok: looks_like_queue(&path),
            hidden: hidden.contains(key.as_str()),
            gate: config.gate.get(&key).cloned().unwrap_or_default(),
            run: config.run.get(&key).cloned().unwrap_or_default(),
            prefix: String::new(),
            label: config.labels.get(&key).cloned().unwrap_or_default(),
            category: config.categories.get(&key).cloned().unwrap_or_default(),
            colour: config.colours.get(&key).cloned().unwrap_or_default(),
            group_name: config.group_names.get(&group).cloned().unwrap_or_default(),
            path: key.clone(),
            key,
            source: source.to_string(),
            group,
        });
    };
    for f in queue::discover(base) {
        push(f.path, "found", f.group);
    }
    for a in &config.added {
        push(PathBuf::from(a), "added", String::new());
    }
    let mut seen: HashMap<String, usize> = HashMap::new();
    for p in &mut out {
        let n = seen.entry(p.name.clone()).or_default();
        *n += 1;
        if *n > 1 {
            p.name = format!("{}-{}", p.name, n);
        }
    }
    let named: Vec<(String, Option<String>)> = out
        .iter()
        .map(|p| {
            (
                p.name.clone(),
                crate::key::stored_prefix(Path::new(&p.path)),
            )
        })
        .collect();
    for (p, prefix) in out.iter_mut().zip(crate::key::prefixes(&named)) {
        p.prefix = prefix;
    }
    out
}

/// The queues to read: shown, still valid, as (name, folder).
pub fn active(base: &Path, config: &Config) -> Vec<(String, PathBuf)> {
    shown(&list(base, config))
}

/// The state the window's sidebar and board are drawn from (0073, review R12): the shown queues of
/// `list` read with their tasks' keys (each project's prefix, 0033) and this Mac's `settings`, each
/// between writes (`queue::read_state_settled`, mu-103), then
/// each queue's Your check limit (0050), label and heading (0044), and colour, places and fold
/// (0057, 0070) from `config`. The poll calls it, and so does the test that holds the window's
/// DashAfzar fixture to it; the poll adds the live dot, Claude Code's states and notifications.
pub fn read_state(
    config: &Config,
    list: &[Project],
    settings: &crate::settings::Settings,
    cache: &mut queue::Cache,
) -> queue::State {
    let prefix_of = |name: &str| {
        list.iter()
            .find(|p| p.name == name)
            .map(|p| p.prefix.clone())
            .unwrap_or_default()
    };
    let mut s = queue::read_state_settled(&shown(list), cache, settings, &prefix_of);
    crate::check_limit::apply(&config.your_check, &mut s.queues);
    crate::labels::apply(list, &mut s.queues);
    crate::arrange::apply(config, list, &mut s.queues);
    s
}

/// `active`, of a list already made.
pub fn shown(list: &[Project]) -> Vec<(String, PathBuf)> {
    list.iter()
        .filter(|p| !p.hidden && p.ok)
        .map(|p| (p.name.clone(), PathBuf::from(&p.path)))
        .collect()
}

/// The longest gate or run command projects.json keeps.
const MAX_GATE: usize = 500;

/// Set (or, with an empty `cmd`, clear) the gate command of the project named `name`. The caller
/// has checked it splits into words (`cli::split_words`); here it's one line of at most 500
/// characters. Returns the project's queue folder.
pub fn set_gate(config: &mut Config, base: &Path, name: &str, cmd: &str) -> Result<String, String> {
    set_command(config, base, name, cmd, "gate", |c| &mut c.gate)
}

/// `set_gate` for the project's run command (0077), with the same checks.
pub fn set_run(config: &mut Config, base: &Path, name: &str, cmd: &str) -> Result<String, String> {
    set_command(config, base, name, cmd, "run", |c| &mut c.run)
}

/// Why `cmd`, trimmed, isn't a command projects.json keeps as `what`: not one line (a line or
/// paragraph separator counts), a character that reorders the text around it (an RTL override
/// can make the command read as another one, `index::is_bidi_control`), or too long.
pub(crate) fn command_refused(what: &str, cmd: &str) -> Option<String> {
    if cmd
        .chars()
        .any(|c| c.is_control() || c == '\u{2028}' || c == '\u{2029}')
    {
        return Some(format!(
            "the {what} command is one line, without control characters"
        ));
    }
    if cmd.chars().any(crate::index::is_bidi_control) {
        return Some(format!(
            "the {what} command has a direction mark or override, which can make it read as another command"
        ));
    }
    (cmd.chars().count() > MAX_GATE)
        .then(|| format!("the {what} command is longer than {MAX_GATE} characters"))
}

fn set_command(
    config: &mut Config,
    base: &Path,
    name: &str,
    cmd: &str,
    what: &str,
    map: fn(&mut Config) -> &mut BTreeMap<String, String>,
) -> Result<String, String> {
    let p = list(base, config)
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("no project named {name:?} (see `muster projects`)"))?;
    let cmd = cmd.trim();
    if let Some(why) = command_refused(what, cmd) {
        return Err(why);
    }
    if cmd.is_empty() {
        map(config).remove(&p.key);
    } else {
        map(config).insert(p.key.clone(), cmd.to_string());
    }
    Ok(p.key)
}

/// projects.json's `run` (0077), entry by entry: one that isn't text, or that `set_run` would
/// refuse or clear, is left out, and anything but an object reads as none, so the rest of the
/// file still loads.
fn read_commands<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, String>, D::Error> {
    let v = serde_json::Value::deserialize(d)?;
    Ok(v.as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(folder, v)| {
                    let cmd = v.as_str()?.trim();
                    (!cmd.is_empty() && command_refused("run", cmd).is_none())
                        .then(|| (folder.clone(), cmd.to_string()))
                })
                .collect()
        })
        .unwrap_or_default())
}

/// The gate command of the project whose queue is `root` (the same folder, through symlinks
/// too), if one is set.
pub fn gate_of(base: &Path, config: &Config, root: &Path) -> Option<(String, String)> {
    let canon = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let root = canon(root);
    list(base, config)
        .into_iter()
        .find(|p| canon(Path::new(&p.path)) == root)
        .map(|p| (p.name, p.gate))
}

fn inside_git_work_tree(dir: &Path) -> bool {
    dir.ancestors().any(|a| a.join(".git").exists())
}

/// Is `dir` in a git repo, as `new_queue` and `muster init` take it: a `.git` on the way up from
/// its real path, also where git's folder isn't a `.git` folder (a submodule, a bare repo's
/// worktree), which `init` then refuses for that reason.
pub fn in_a_repo(dir: &Path) -> bool {
    dir.canonicalize().is_ok_and(|d| inside_git_work_tree(&d))
}

/// Add an existing queue folder picked by the owner.
pub fn add(config: &mut Config, base: &Path, dir: &Path) -> Result<String, String> {
    let dir = dir.canonicalize().map_err(|e| e.to_string())?;
    if !looks_like_queue(&dir) {
        return Err(format!(
            "{} isn't a Muster queue (a repo's .muster/ with its config.toml, or a README.md with a todo/ folder). To start one, use New queue.",
            dir.display()
        ));
    }
    let key = dir.to_string_lossy().to_string();
    config.hidden.retain(|h| h != &key);
    let known = list(base, config).iter().any(|p| p.key == key);
    if !known {
        config.added.push(key.clone());
    }
    Ok(key)
}

/// Store `prefix` (0033) for the project named `name`, refused when it isn't 2 to 4 lowercase
/// letters or another project has it. Returns the project's queue folder.
pub fn set_prefix(
    config: &Config,
    base: &Path,
    name: &str,
    prefix: &str,
) -> Result<String, String> {
    set_prefix_where(config, base, prefix, &|p| p.name == name)
        .ok_or_else(|| format!("no project named {name:?} (see `muster projects`)"))?
}

/// `set_prefix` for the project the window names by its key.
pub fn set_prefix_by_key(
    config: &Config,
    base: &Path,
    key: &str,
    prefix: &str,
) -> Result<String, String> {
    set_prefix_where(config, base, prefix, &|p| p.key == key).ok_or("no such project")?
}

fn set_prefix_where(
    config: &Config,
    base: &Path,
    prefix: &str,
    pick: &dyn Fn(&Project) -> bool,
) -> Option<Result<String, String>> {
    let all = list(base, config);
    let p = all.iter().find(|p| pick(p))?;
    if !p.ok {
        return Some(Err(format!(
            "{} isn't a queue any more ({})",
            p.name, p.path
        )));
    }
    let root = Path::new(&p.path);
    Some(
        crate::key::check_new_prefix(prefix, root, &all)
            // Under the queue's lock, which brings INDEX.md up to date with the new keys.
            .and_then(|()| queue::with_lock(root, || crate::key::store_prefix(root, prefix)))
            .map(|()| p.key.clone()),
    )
}

/// Create a queue: `<project>/tasks` with Muster's layout, its tools' stand-ins and its own git,
/// and its prefix (0033) stored: `wanted`, refused when it isn't one or one of `known` has it, else
/// the default it gets beside them. Returns the queue's folder and its prefix.
pub fn create(
    project_dir: &Path,
    tools: &Path,
    wanted: Option<&str>,
    known: &[Project],
) -> Result<(PathBuf, String), String> {
    let project_dir = project_dir.canonicalize().map_err(|e| e.to_string())?;
    if inside_git_work_tree(&project_dir) {
        return Err(format!(
            "{} is inside a git repository. A queue lives outside code repos, so code commits can't sweep it up. Pick the folder that holds the repo (for example ~/playground/<project>), not the repo itself.",
            project_dir.display()
        ));
    }
    let q = project_dir.join("tasks");
    if q.exists() {
        return Err(format!(
            "{} already exists. To show it, use Add existing queue.",
            q.display()
        ));
    }
    let project = default_name(&q);
    let prefix = match wanted {
        Some(p) => {
            crate::key::check_new_prefix(p, &q, known)?;
            p.to_string()
        }
        None => {
            let taken: Vec<String> = known.iter().map(|p| p.prefix.clone()).collect();
            crate::key::prefix_beside(&project, &taken)
        }
    };
    fs::create_dir(&q).map_err(|e| e.to_string())?;
    let result = (|| {
        for d in queue::STATUS_FOLDERS
            .iter()
            .chain(["attachments", "bin"].iter())
        {
            fs::create_dir(q.join(d)).map_err(|e| e.to_string())?;
            if *d != "bin" {
                fs::write(q.join(d).join(".gitkeep"), "").map_err(|e| e.to_string())?;
            }
        }
        fs::write(q.join("README.md"), readme(&project, &q, &prefix)).map_err(|e| e.to_string())?;
        fs::write(q.join("WAITING-FOR-OWNER.md"), WAITING).map_err(|e| e.to_string())?;
        fs::write(q.join(".gitignore"), ".DS_Store\nmail/\n__pycache__/\n")
            .map_err(|e| e.to_string())?;
        for tool in ["task", "answer-hook.py", "manifest.py"] {
            let p = q.join("bin").join(tool);
            fs::write(&p, stand_in(tool, tools)).map_err(|e| e.to_string())?;
            make_executable(&p)?;
        }
        git(&q, &["init", "-q", "-b", "main"])?;
        git(&q, &["add", "-A"])?;
        let msg = format!("Muster queue for {project}");
        // With no git identity for this folder, commit as Muster rather than fail.
        if git(&q, &["commit", "-q", "-m", &msg]).is_err() {
            git(
                &q,
                &[
                    "-c",
                    "user.name=Muster",
                    "-c",
                    "user.email=muster@localhost",
                    "commit",
                    "-q",
                    "-m",
                    &msg,
                ],
            )?;
        }
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&q);
        return Err(e);
    }
    Ok((q, prefix))
}

/// After `muster migrate` (0005): what `projects.json` says of the old queue `old` (its gate,
/// run command, name, label and category, colour and place in the sidebar's order, hidden, Your check limit), said of the new one `new` too, under the
/// key the app lists it by; a new queue the base doesn't find is added, so it shows as the old one
/// did. The old queue keeps its own, until it's retired. Returns a line for each thing carried.
pub fn carry_over(config: &mut Config, base: &Path, old: &Path, new: &Path) -> Vec<String> {
    let canon = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let (old, new) = (canon(old), canon(new));
    let is_old = |k: &String| canon(Path::new(k)) == old;
    let gate = config
        .gate
        .iter()
        .find(|(k, _)| is_old(k))
        .map(|(_, v)| v.clone());
    let run = config
        .run
        .iter()
        .find(|(k, _)| is_old(k))
        .map(|(_, v)| v.clone());
    let name = config
        .names
        .iter()
        .find(|(k, _)| is_old(k))
        .map(|(_, v)| v.clone());
    let label = config
        .labels
        .iter()
        .find(|(k, _)| is_old(k))
        .map(|(_, v)| v.clone());
    let category = config
        .categories
        .iter()
        .find(|(k, _)| is_old(k))
        .map(|(_, v)| v.clone());
    let colour = config
        .colours
        .iter()
        .find(|(k, _)| is_old(k))
        .map(|(_, v)| v.clone());
    let placed = config.order.iter().position(is_old);
    let hidden = config.hidden.iter().any(is_old);
    let your_check = config
        .your_check
        .iter()
        .find(|(k, _)| is_old(k))
        .map(|(_, l)| *l);
    let listed = list(base, config);
    let old_project = listed.iter().find(|p| canon(Path::new(&p.path)) == old);
    let old_listed = old_project.is_some();
    let new_project = listed.iter().find(|p| canon(Path::new(&p.path)) == new);
    // Under its group's heading, with no category of its own (0070): a new queue outside that
    // group keeps the heading as its category, so it doesn't drop to another one.
    let heading = old_project
        .filter(|p| category.is_none() && !p.group.is_empty())
        .filter(|p| new_project.is_none_or(|n| n.group != p.group))
        .map(crate::labels::shown_category);
    let found = new_project.map(|p| p.key.clone());
    let carried = gate.is_some()
        || run.is_some()
        || name.is_some()
        || label.is_some()
        || category.is_some()
        || heading.is_some()
        || colour.is_some()
        || placed.is_some()
        || hidden
        || your_check.is_some();
    // Nothing to say of a queue nobody listed, or of one the new queue is found in place of.
    if !carried && (found.is_some() || !old_listed) {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let key = found.unwrap_or_else(|| {
        let key = new.to_string_lossy().to_string();
        lines.push(format!(
            "projects.json: added {key}, which Muster doesn't find on its own, as the old queue was listed"
        ));
        config.added.push(key.clone());
        key
    });
    if let Some(g) = gate {
        lines.push(format!(
            "projects.json: gate {g:?} carried to the new queue"
        ));
        config.gate.insert(key.clone(), g);
    }
    if let Some(r) = run {
        lines.push(format!("projects.json: run {r:?} carried to the new queue"));
        config.run.insert(key.clone(), r);
    }
    if let Some(n) = name {
        lines.push(format!(
            "projects.json: name {n:?} carried to the new queue"
        ));
        config.names.insert(key.clone(), n);
    }
    if let Some(l) = label {
        lines.push(format!(
            "projects.json: label {l:?} carried to the new queue"
        ));
        config.labels.insert(key.clone(), l);
    }
    if let Some(g) = category {
        lines.push(format!(
            "projects.json: category {g:?} carried to the new queue"
        ));
        config.categories.insert(key.clone(), g);
    }
    if let Some(h) = heading {
        lines.push(format!(
            "projects.json: heading {h:?} carried to the new queue, as its category"
        ));
        config.categories.insert(key.clone(), h);
    }
    if let Some(c) = colour {
        lines.push(format!(
            "projects.json: colour {c:?} carried to the new queue"
        ));
        config.colours.insert(key.clone(), c);
    }
    // In the old one's place, just before it, so the sidebar shows it where the old one was.
    if let Some(i) = placed.filter(|_| !config.order.contains(&key)) {
        lines.push("projects.json: place in the sidebar's order carried to the new queue".into());
        config.order.insert(i, key.clone());
    }
    if let Some(l) = your_check {
        let said = match l {
            crate::check_limit::Limit::At(n) => n.to_string(),
            crate::check_limit::Limit::Off => "off".into(),
        };
        lines.push(format!(
            "projects.json: Your check limit {said} carried to the new queue"
        ));
        config.your_check.insert(key.clone(), l);
    }
    if hidden && !config.hidden.contains(&key) {
        lines.push("projects.json: hidden, like the old queue".into());
        config.hidden.push(key);
    }
    lines
}

/// A new queue for the folder `dir`, as `muster init` and the app's New queue make it (task
/// 0004): in a git repo, its `.muster/` (`init`; `local` unless `committed` is `Some(true)`);
/// elsewhere the old `<dir>/tasks` (`create`), where a mode is refused, since it's for a repo.
/// A repo without a main checkout gets `init`'s refusal, never `tasks/`.
pub fn new_queue(
    dir: &Path,
    committed: Option<bool>,
    wanted: Option<&str>,
    known: &[Project],
    tools: &Path,
) -> Result<(PathBuf, String), String> {
    if layout::main_checkout(dir).is_some() || in_a_repo(dir) {
        init(dir, committed != Some(true), wanted, known)
    } else if committed.is_some() {
        Err(format!(
            "{} isn't in a git repo: --local and --committed are for a repo's .muster/",
            dir.display()
        ))
    } else {
        create(dir, tools, wanted, known)
    }
}

/// `muster init <folder> [--local]` in a repo (task 0005, decision 0003): a `.muster/` queue in
/// the repo's main checkout, wherever in the repo or its worktrees `dir` is. `local` keeps it on
/// this Mac (what `muster init` and New queue pass unless `--committed`), else it's committed
/// with the code. Refused outside a git repo, and when the repo already has a `.muster`, which
/// is never written over.
pub fn init(
    dir: &Path,
    local: bool,
    wanted: Option<&str>,
    known: &[Project],
) -> Result<(PathBuf, String), String> {
    let main = layout::main_checkout(dir).ok_or_else(|| {
        format!(
            "{} isn't in a git repo with a main checkout (a .git folder): muster init puts the queue in that checkout's .muster/",
            dir.display()
        )
    })?;
    let q = main.join(layout::DIR);
    // As `create` says it: the app's New queue lands here too.
    let exists = || {
        format!(
            "{} already exists. To show it, use Add existing queue.",
            q.display()
        )
    };
    if fs::symlink_metadata(&q).is_ok() {
        return Err(exists());
    }
    let project = default_name(&q);
    // As `create` picks it (0033): the one asked for, checked, else the default beside the others.
    let prefix = match wanted {
        Some(p) => {
            crate::key::check_new_prefix(p, &q, known)?;
            p.to_string()
        }
        None => {
            let taken: Vec<String> = known.iter().map(|p| p.prefix.clone()).collect();
            crate::key::prefix_beside(&project, &taken)
        }
    };
    // Made here, not before: if it's there, it's someone's, and it's left as it is.
    fs::create_dir(&q).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => exists(),
        _ => format!("{}: {e}", q.display()),
    })?;
    let made = write_muster(&q, local, "", &project, &prefix)
        .and_then(|()| crate::index::rebuild(&q).map(|_| ()));
    if let Err(e) = made {
        let _ = fs::remove_dir_all(&q);
        return Err(e);
    }
    // A local queue's history (0109). One that can't start now starts on the first write, and
    // `muster check` says so until then.
    let _ = crate::history::start(&q, crate::history::STARTS);
    Ok((q, prefix))
}

/// The new layout's own files in an empty `.muster` folder `q`: its folders, `config.toml` (with
/// the queue's `prefix`, where a `.muster` queue keeps it), `.gitignore` and `README.md` for
/// `project`, which starts with `front` (an old queue's README front matter, when migrating;
/// else "").
pub(crate) fn write_muster(
    q: &Path,
    local: bool,
    front: &str,
    project: &str,
    prefix: &str,
) -> Result<(), String> {
    let io = |what: &str, e: std::io::Error| format!("{}: {e}", q.join(what).display());
    for d in ["tasks", "backlog", "attachments"] {
        fs::create_dir_all(q.join(d)).map_err(|e| io(d, e))?;
    }
    fs::write(
        q.join(layout::CONFIG),
        layout::config_text(local, Some(prefix)),
    )
    .map_err(|e| io(layout::CONFIG, e))?;
    fs::write(q.join(".gitignore"), layout::gitignore_text(local))
        .map_err(|e| io(".gitignore", e))?;
    let readme = format!("{front}{}", muster_readme(project, prefix));
    fs::write(q.join("README.md"), readme).map_err(|e| io("README.md", e))
}

/// The README of a `.muster/` queue: where things are, and the rules in short.
fn muster_readme(project: &str, prefix: &str) -> String {
    format!(
        "# {project} — task queue\n\nA Muster queue, in the repo's `.muster/`. **The rules are in Muster's `docs/PROTOCOL.md`. Read\n\
         them before you claim anything.** The short version:\n\n\
         - Tasks are `tasks/NNNN-slug.md`. A file never moves: `stage:` in it says where the task is.\n  \
         A task moves through stages: Backlog → Shaping → Ready → Building → Testing → Your check → Done.\n  \
         Shaping is for product, tech and UI agents, Building for the code agent, Testing for the QA\n  \
         agent, Your check for the owner. Find a task by its key (`muster show {prefix}-1`; its file\n  \
         keeps the id, 0001). The prefix is in `config.toml`.\n\
         - Only `muster` changes a stage. Never edit `stage:`, `claimed-by:` or `heartbeat:` by hand.\n  \
         `muster` finds this queue from the repo and from every worktree of it.\n\
         - One claim per session, made by `muster`: `muster next --as <your branch>` takes the top\n  \
         Ready task, `muster claim NNNN --as <your branch>` a given one. It says which queue it claimed in.\n\
         - On long work, `muster heartbeat NNNN --as <your branch>` keeps the claim alive (your notes and\n  \
         passes do too). A claim quiet for 12 h goes back to Ready (`muster reclaim`).\n\
         - Ask without stopping, and the answer is pushed to you:\n  \
         `muster ask NNNN \"<question>\" --as <your branch>`. No answer hook? `muster inbox` collects them.\n\
         - Built and the gate is green: `muster pass NNNN --as <your branch> --from building --gate \"<the gate's result line>\" --summary \"<what was built, one plain line>\"`.\n  \
         The summary (160 characters, in the owner's words) leads the owner's check; `muster set NNNN summary` fixes it.\n  \
         Testing ticks each criterion with its proof (`— proof: …`, naming any screenshot in `attachments/NNNN/`), then passes it on to the owner's check.\n\
         - Something's wrong: `muster return NNNN \"<why>\"` sends it back a stage.\n\
         - Waiting on something: `muster block NNNN --on <0014,0016|you|permission|question|outside> \"<what>\"`,\n  \
         then `muster unblock NNNN`. It keeps its stage.\n\
         - New bugs or ideas you find: `muster add \"<one line>\" --as <your branch>` puts a note in `backlog/`\n  \
         (a file dropped there by hand counts too). Shaping turns it into a task: `muster shape <note-file>`\n  \
         claims the note for you and prints the brief (`--peek` only reads it; the board shows who's on a note).\n  \
         `muster file <note-file> \"<title>\" --body -` files it in Shaping, still yours, and\n  \
         `muster drop <note-file> \"<why>\"` sets it aside in `backlog/dropped/`; either ends the claim, and\n  \
         `muster release <note-file>` lets it go. One note at a time.\n\
         - Files go in `attachments/NNNN/`; `INDEX.md` and `tasks.json` are rebuilt by the tools, never edited.\n\
         - `config.toml`: `local = true` keeps the queue on this Mac (only `config.toml` is committed).\n  \
         Its history is in `.git/muster/history.git`, a commit per muster write: never run `git clean -x`\n  \
         or `-X` in the main checkout (`git clean -fd` is safe once config.toml is committed); `muster restore\n  \
         --all` brings files back.\n"
    )
}

#[cfg(unix)]
fn make_executable(p: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(p, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())
}

#[cfg(not(unix))]
fn make_executable(_: &Path) -> Result<(), String> {
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> Result<(), String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

fn stand_in(tool: &str, tools: &Path) -> String {
    let target = tools.join(tool);
    format!(
        "#!/usr/bin/env python3\n# Runs Muster's {tool} on this queue ({}).\nimport os\nimport sys\nfrom pathlib import Path\n\nos.environ[\"MUSTER_ROOT\"] = str(Path(__file__).resolve().parent.parent)\ntarget = {:?}\nos.execv(sys.executable, [sys.executable, \"-I\", target, *sys.argv[1:]])\n",
        target.display(),
        target.to_string_lossy()
    )
}

const WAITING: &str = "# Waiting for you\n\nOnly what really needs the owner, each with what it is, why it matters, what each answer leads\nto, capture's pick, and what happens with no answer.\n\nNothing is waiting for you.\n";

fn readme(project: &str, q: &Path, prefix: &str) -> String {
    let q = q.display();
    format!(
        "---\nprefix: {prefix}\n---\n\n# {project} — task queue\n\nA Muster queue. **The rules are in Muster's `docs/PROTOCOL.md`. Read them before you claim\n\
         anything.** The short version:\n\n\
         - A task moves through stages: Backlog → Shaping → Ready → Building → Testing → Your check → Done.\n  \
         Shaping is for product, tech and UI agents, Building for the code agent, Testing for the QA\n  \
         agent, Your check for the owner. Find a task by its key (`muster show {prefix}-1`; its file keeps the id, 0001), never by a path.\n\
         - Only `muster` changes a stage. Never move task files or edit `stage:`, `status:`, `claimed-by:` or `heartbeat:` by hand.\n  \
         (The folders `inbox/ todo/ doing/ blocked/ done/` mirror the stage for older tools.)\n  \
         `muster` finds this queue from the project's repo; elsewhere add `--queue {q}`.\n\
         - One claim per session, made by `muster`: `muster next --as <your branch>` takes the top\n  \
         Ready task, `muster claim NNNN --as <your branch>` a given one. It says which queue it claimed in.\n\
         - On long work, `muster heartbeat NNNN --as <your branch>` keeps the claim alive (your notes and\n  \
         passes do too). A claim quiet for longer than the lease (12 h unless changed in Settings) goes back to Ready (`muster reclaim`).\n\
         - Ask without stopping, and the answer is pushed to you:\n  \
         `muster ask NNNN \"<question>\" --as <your branch>`. No answer hook? `muster inbox` collects them.\n\
         - Built and the gate is green: `muster pass NNNN --as <your branch> --from building --gate \"<the gate's result line>\" --summary \"<what was built, one plain line>\"`.\n  \
         The summary (160 characters, in the owner's words) leads the owner's check; `muster set NNNN summary` fixes it.\n  \
         Testing ticks each criterion with its proof (`— proof: …`, naming any screenshot in `attachments/NNNN/`), then passes it on to the owner's check.\n\
         - Something's wrong: `muster return NNNN \"<why>\"` sends it back a stage.\n\
         - Waiting on something: `muster block NNNN --on <0014,0016|you|permission|question|outside> \"<what>\"`,\n  \
         then `muster unblock NNNN`. It keeps its stage. Task ids go into its depends and it clears by\n  \
         itself once they're merged (in Your check or Done); a block on a question (`--on question Q2`)\n  \
         clears when it's answered.\n\
         - Code goes in the project's repo, never in this folder.\n\
         - New bugs or ideas you find: `muster add \"<one line>\" --as <your branch>` puts a note in the backlog\n  \
         (`inbox/`; a file dropped there by hand counts too). Shaping turns it into a task: `muster shape <note-file>`\n  \
         claims the note for you and prints the brief (`--peek` only reads it), `muster file <note-file> \"<title>\" --body -`\n  \
         files it in Shaping, still yours (the note goes into the task's attachments), and\n  \
         `muster drop <note-file> \"<why>\"` sets it aside; either ends the claim, and `muster release <note-file>` lets it go.\n\
         - `bin/` holds stand-ins that run Muster's tools on this queue.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_readmes_say_shape_claims_the_note() {
        // 0097: shape claims, --peek only reads, file and drop end the claim, release lets go.
        for text in [
            muster_readme("demo", "dm"),
            readme("demo", Path::new("/q"), "dm"),
        ] {
            for want in [
                "claims the note for you",
                "`--peek` only reads it",
                "either ends the claim",
                "`muster release <note-file>` lets it go",
            ] {
                assert!(text.contains(want), "{want}: {text}");
            }
        }
    }

    #[test]
    fn summary_is_in_both_readmes_builder_steps() {
        // mu-78: a builder passes out of Building with a summary, in either layout's README.
        for text in [
            muster_readme("demo", "dm"),
            readme("demo", Path::new("/q"), "dm"),
        ] {
            assert!(
                text.contains("--from building --gate \"<the gate's result line>\" --summary \"<what was built, one plain line>\"")
                    && text.contains("muster set NNNN summary"),
                "{text}"
            );
        }
    }

    fn make_queue(dir: &Path) {
        fs::create_dir_all(dir.join("todo")).unwrap();
        fs::write(dir.join("README.md"), "q").unwrap();
    }

    #[test]
    fn writers_at_once_keep_every_edit_and_a_reader_never_sees_half_a_file() {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("app").join("projects.json");
        // A big file, so a write takes long enough for the others to land in the middle of it.
        edit(&file, |c| {
            for i in 0..2000 {
                c.added.push(format!("/p/{i:04}/tasks"));
            }
            Ok(())
        })
        .unwrap();
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let reader = {
            let (file, stop) = (file.clone(), stop.clone());
            std::thread::spawn(move || {
                let mut torn = 0;
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    let t = fs::read_to_string(&file).unwrap_or_default();
                    if serde_json::from_str::<Config>(&t).is_err() {
                        torn += 1;
                    }
                }
                torn
            })
        };
        let writers: Vec<_> = (0..4)
            .map(|w| {
                let file = file.clone();
                std::thread::spawn(move || {
                    for i in 0..25 {
                        edit(&file, |c| {
                            c.names.insert(format!("/w{w}/{i}"), format!("{w}-{i}"));
                            Ok(())
                        })
                        .unwrap();
                    }
                })
            })
            .collect();
        for w in writers {
            w.join().unwrap();
        }
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(reader.join().unwrap(), 0, "no torn read");
        let c = load(&file);
        assert_eq!(c.names.len(), 100, "every writer's every edit is kept");
        assert_eq!(c.added.len(), 2000);
        // No temporary file is left behind.
        let left: Vec<_> = fs::read_dir(file.parent().unwrap())
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(left.is_empty(), "{left:?}");
    }

    #[test]
    fn a_refused_edit_writes_nothing() {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("projects.json");
        edit(&file, |c| {
            c.hidden.push("/x".into());
            Ok(())
        })
        .unwrap();
        let before = fs::read_to_string(&file).unwrap();
        let r: Result<(), String> = edit(&file, |c| {
            c.hidden.clear();
            Err("no".into())
        });
        assert_eq!(r.unwrap_err(), "no");
        assert_eq!(fs::read_to_string(&file).unwrap(), before);
    }

    #[test]
    fn lists_found_and_added_with_names_and_hidden() {
        let base = tempfile::tempdir().unwrap();
        make_queue(&base.path().join("alpha/tasks"));
        let elsewhere = tempfile::tempdir().unwrap();
        let other = elsewhere.path().join("alpha/tasks");
        make_queue(&other);
        let mut c = Config::default();
        add(&mut c, base.path(), &other).unwrap();
        let l = list(base.path(), &c);
        assert_eq!(l.len(), 2);
        assert_eq!(
            (l[0].name.as_str(), l[0].source.as_str()),
            ("alpha", "found")
        );
        assert_eq!(l[1].name, "alpha-2", "clashing names get a suffix");
        c.names.insert(l[1].key.clone(), "work".into());
        c.hidden.push(l[0].key.clone());
        let l = list(base.path(), &c);
        assert_eq!(l[1].name, "work");
        assert!(l[0].hidden);
        let a = active(base.path(), &c);
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].0, "work");
    }

    #[test]
    fn a_migrated_queue_discovery_cant_see_is_added_when_the_old_one_was_listed() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        // <base>/g/p/tasks is found (two levels); <base>/g/p/main/.muster is three levels down.
        let old = base.join("g/p/tasks");
        make_queue(&old);
        let main = base.join("g/p/main");
        fs::create_dir_all(main.join(".git")).unwrap();
        let new = main.join(layout::DIR);
        fs::create_dir(&new).unwrap();
        fs::write(new.join(layout::CONFIG), layout::config_text(true, None)).unwrap();
        let mut c = Config::default();
        let lines = carry_over(&mut c, &base, &old, &new);
        let key = new.to_string_lossy().to_string();
        assert_eq!(c.added, vec![key.clone()], "{lines:?}");
        assert!(lines.join("\n").contains("added"), "{lines:?}");
        assert!(list(&base, &c).iter().any(|p| p.key == key && p.ok));
        // A queue nobody listed stays unlisted.
        let elsewhere = tempfile::tempdir().unwrap();
        let stray = elsewhere.path().canonicalize().unwrap().join("x/tasks");
        make_queue(&stray);
        let mut c = Config::default();
        assert!(carry_over(&mut c, &base, &stray, &new).is_empty());
        assert!(c.added.is_empty());
    }

    #[test]
    fn a_migrated_queue_takes_the_old_ones_gate_name_and_hidden() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let old = base.join("proj/tasks");
        make_queue(&old);
        // The new queue, found under the base (a main checkout's .muster).
        let main = base.join("proj/main");
        fs::create_dir_all(main.join(".git")).unwrap();
        let new = main.join(layout::DIR);
        fs::create_dir(&new).unwrap();
        fs::write(new.join(layout::CONFIG), layout::config_text(true, None)).unwrap();
        let key = old.to_string_lossy().to_string();
        let mut c = Config::default();
        c.gate.insert(key.clone(), "bin/gate".into());
        c.names.insert(key.clone(), "Muster".into());
        let lines = carry_over(&mut c, &base, &old, &new);
        let new_key = new.to_string_lossy().to_string();
        assert_eq!(c.gate.get(&new_key).map(String::as_str), Some("bin/gate"));
        assert_eq!(c.names.get(&new_key).map(String::as_str), Some("Muster"));
        assert!(c.added.is_empty(), "found, so not added: {:?}", c.added);
        assert_eq!(
            c.gate.get(&key).map(String::as_str),
            Some("bin/gate"),
            "the old keeps its own"
        );
        let text = lines.join("\n");
        assert!(text.contains("gate \"bin/gate\""), "{text}");
        assert!(text.contains("name \"Muster\""), "{text}");
        // A new queue outside the base is added, so it shows as the old one did; hidden carries.
        let elsewhere = tempfile::tempdir().unwrap();
        let away = elsewhere.path().canonicalize().unwrap().join("repo");
        fs::create_dir_all(away.join(".git")).unwrap();
        let away_q = away.join(layout::DIR);
        fs::create_dir(&away_q).unwrap();
        fs::write(away_q.join(layout::CONFIG), layout::config_text(true, None)).unwrap();
        let mut c = Config {
            hidden: vec![key.clone()],
            ..Config::default()
        };
        let lines = carry_over(&mut c, &base, &old, &away_q);
        let away_key = away_q.to_string_lossy().to_string();
        assert_eq!(c.added, vec![away_key.clone()]);
        assert!(c.hidden.contains(&away_key));
        assert!(lines.join("\n").contains("hidden"), "{lines:?}");
        // Nothing set for the old one: nothing to say, nothing added for a found queue.
        let mut c = Config::default();
        assert!(carry_over(&mut c, &base, &old, &new).is_empty());
        assert_eq!(c, Config::default());
    }

    #[test]
    fn a_migrated_queue_keeps_its_run_command() {
        // 0077: a run command alone carries, as a gate does, and the old queue keeps its own.
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let old = base.join("proj/tasks");
        make_queue(&old);
        let main = base.join("proj/main");
        fs::create_dir_all(main.join(".git")).unwrap();
        let new = main.join(layout::DIR);
        fs::create_dir(&new).unwrap();
        fs::write(new.join(layout::CONFIG), layout::config_text(true, None)).unwrap();
        let key = old.to_string_lossy().to_string();
        let new_key = new.to_string_lossy().to_string();
        let mut c = Config::default();
        c.run.insert(key.clone(), "bin/run".into());
        let lines = carry_over(&mut c, &base, &old, &new);
        assert_eq!(c.run.get(&new_key).map(String::as_str), Some("bin/run"));
        assert_eq!(
            c.run.get(&key).map(String::as_str),
            Some("bin/run"),
            "the old keeps its own"
        );
        assert_eq!(
            lines,
            ["projects.json: run \"bin/run\" carried to the new queue"]
        );
        let p = list(&base, &c)
            .into_iter()
            .find(|p| p.key == new_key)
            .unwrap();
        assert_eq!(p.run, "bin/run");
    }

    #[test]
    fn a_run_command_is_set_shown_and_cleared_with_the_gates_rules() {
        // 0077: one line, at most 500 characters, trimmed, "" clears it, an unknown project is
        // refused; the gate beside it is left alone.
        let base = tempfile::tempdir().unwrap();
        make_queue(&base.path().join("proj/tasks"));
        let mut c = Config::default();
        let key = set_run(&mut c, base.path(), "proj", "  bin/run --fresh ").unwrap();
        assert_eq!(c.run.get(&key).map(String::as_str), Some("bin/run --fresh"));
        let p = list(base.path(), &c).remove(0);
        assert_eq!((p.run.as_str(), p.gate.as_str()), ("bin/run --fresh", ""));
        assert_eq!(serde_json::to_value(&p).unwrap()["run"], "bin/run --fresh");
        for (cmd, says) in [
            ("bin/run\nrm -rf x", "the run command is one line"),
            ("bin/run\tx", "the run command is one line"),
            ("bin/run\u{2028}x", "the run command is one line"),
            ("bin/run\u{2029}x", "the run command is one line"),
            ("bin/run \u{202e}nur", "a direction mark or override"),
            ("bin/run \u{2066}x\u{2069}", "a direction mark or override"),
            ("bin/run\u{200f}", "a direction mark or override"),
            ("bin/run\u{061c}", "a direction mark or override"),
            (
                &"x".repeat(501)[..],
                "the run command is longer than 500 characters",
            ),
        ] {
            let e = set_run(&mut c, base.path(), "proj", cmd).unwrap_err();
            assert!(e.contains(says), "{cmd:?}: {e}");
        }
        let e = set_run(&mut c, base.path(), "nope", "bin/run").unwrap_err();
        assert!(e.contains("no project named \"nope\""), "{e}");
        assert_eq!(c.run.len(), 1, "a refusal changes nothing");
        // 500 characters, counted as characters: a Persian command that long is kept.
        let long = "ب".repeat(500);
        set_run(&mut c, base.path(), "proj", &long).unwrap();
        assert_eq!(c.run.get(&key), Some(&long));
        set_gate(&mut c, base.path(), "proj", "bin/gate").unwrap();
        set_run(&mut c, base.path(), "proj", "  ").unwrap();
        assert!(c.run.is_empty(), "blank clears it");
        assert_eq!(c.gate.get(&key).map(String::as_str), Some("bin/gate"));
        assert_eq!(list(base.path(), &c)[0].run, "");
    }

    #[test]
    fn run_commands_are_read_leniently() {
        // 0077: a file from before run loads with none; a run that isn't an object, or an entry
        // that isn't a command set_run would keep, is left out, and the rest of the file loads.
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("projects.json");
        let read = |text: &str| {
            fs::write(&f, text).unwrap();
            load(&f)
        };
        let old = read(r#"{"added": ["/a"], "gate": {"/a": "bin/gate"}}"#);
        assert!(old.run.is_empty());
        assert_eq!(old.added, ["/a"]);
        for wrong in [r#""bin/run""#, "5", "[\"bin/run\"]", "null", "true"] {
            let c = read(&format!(
                r#"{{"added": ["/a"], "gate": {{"/a": "bin/gate"}}, "run": {wrong}}}"#
            ));
            assert!(c.run.is_empty(), "{wrong}: {:?}", c.run);
            assert_eq!(c.added, ["/a"], "{wrong}: the rest loads");
            assert_eq!(c.gate.get("/a").map(String::as_str), Some("bin/gate"));
        }
        let long = "x".repeat(501);
        let c = read(&format!(
            r#"{{"run": {{"/a": " bin/run ", "/b": 7, "/c": "a\nb", "/d": "", "/e": "{long}", "/f": ["x"], "/g": "bin/run \u202enur", "/h": "x\u2028y", "/i": "x\u2066y"}}}}"#
        ));
        assert_eq!(
            c.run,
            BTreeMap::from([("/a".to_string(), "bin/run".to_string())])
        );
        // And what's saved reads back the same.
        save(&f, &c).unwrap();
        assert_eq!(load(&f), c);
    }

    #[test]
    fn a_migrated_queue_keeps_its_your_check_limit() {
        use crate::check_limit::{limit_for, Limit};
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let old = base.join("proj/tasks");
        make_queue(&old);
        let main = base.join("proj/main");
        fs::create_dir_all(main.join(".git")).unwrap();
        let new = main.join(layout::DIR);
        fs::create_dir(&new).unwrap();
        fs::write(new.join(layout::CONFIG), layout::config_text(true, None)).unwrap();
        let key = old.to_string_lossy().to_string();
        // A limit alone carries, as a gate does.
        let mut c = Config::default();
        c.your_check.insert(key.clone(), Limit::At(8));
        let lines = carry_over(&mut c, &base, &old, &new);
        assert_eq!(limit_for(&c.your_check, &new), Some(8));
        assert_eq!(
            limit_for(&c.your_check, &old),
            Some(8),
            "the old keeps its own"
        );
        assert_eq!(
            lines,
            ["projects.json: Your check limit 8 carried to the new queue"]
        );
        // Off carries as Off.
        let mut c = Config::default();
        c.your_check.insert(key.clone(), Limit::Off);
        let lines = carry_over(&mut c, &base, &old, &new);
        assert_eq!(limit_for(&c.your_check, &new), None);
        assert_eq!(
            lines,
            ["projects.json: Your check limit off carried to the new queue"]
        );
    }

    #[test]
    fn a_migrated_queue_keeps_its_label_and_category() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let old = base.join("proj/tasks");
        make_queue(&old);
        let main = base.join("proj/main");
        fs::create_dir_all(main.join(".git")).unwrap();
        let new = main.join(layout::DIR);
        fs::create_dir(&new).unwrap();
        fs::write(new.join(layout::CONFIG), layout::config_text(true, None)).unwrap();
        let key = old.to_string_lossy().to_string();
        let new_key = new.to_string_lossy().to_string();
        // A label alone carries, as a gate does; so does a category alone.
        let mut c = Config::default();
        c.labels.insert(key.clone(), "Muster".into());
        let lines = carry_over(&mut c, &base, &old, &new);
        assert_eq!(c.labels.get(&new_key).map(String::as_str), Some("Muster"));
        assert_eq!(
            c.labels.get(&key).map(String::as_str),
            Some("Muster"),
            "the old keeps its own"
        );
        assert_eq!(
            lines,
            ["projects.json: label \"Muster\" carried to the new queue"]
        );
        let mut c = Config::default();
        c.categories.insert(key.clone(), "Apps".into());
        let lines = carry_over(&mut c, &base, &old, &new);
        assert_eq!(c.categories.get(&new_key).map(String::as_str), Some("Apps"));
        assert_eq!(
            lines,
            ["projects.json: category \"Apps\" carried to the new queue"]
        );
    }

    #[test]
    fn a_migrated_queue_keeps_its_colour_and_its_place_in_the_sidebar() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let old = base.join("proj/tasks");
        make_queue(&old);
        let main = base.join("proj/main");
        fs::create_dir_all(main.join(".git")).unwrap();
        let new = main.join(layout::DIR);
        fs::create_dir(&new).unwrap();
        fs::write(new.join(layout::CONFIG), layout::config_text(true, None)).unwrap();
        let key = old.to_string_lossy().to_string();
        let new_key = new.to_string_lossy().to_string();
        let mut c = Config::default();
        c.colours.insert(key.clone(), "mint".into());
        c.order = vec!["/a".into(), key.clone(), "/b".into()];
        let lines = carry_over(&mut c, &base, &old, &new);
        assert_eq!(c.colours.get(&new_key).map(String::as_str), Some("mint"));
        assert_eq!(
            c.colours.get(&key).map(String::as_str),
            Some("mint"),
            "the old keeps its own"
        );
        assert_eq!(
            c.order,
            ["/a".to_string(), new_key.clone(), key.clone(), "/b".into()]
        );
        assert_eq!(
            lines,
            [
                "projects.json: colour \"mint\" carried to the new queue",
                "projects.json: place in the sidebar's order carried to the new queue",
            ]
        );
        // A place alone carries too, and never twice.
        let mut c = Config {
            order: vec![key.clone()],
            ..Config::default()
        };
        carry_over(&mut c, &base, &old, &new);
        carry_over(&mut c, &base, &old, &new);
        assert_eq!(c.order, [new_key, key]);
    }

    #[test]
    fn a_migrated_queue_outside_its_group_keeps_the_groups_heading() {
        // 0070: a queue under its group's heading (named DashAfzar) migrates to a .muster/ the
        // base doesn't find in that group: it keeps the heading, as its own category.
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let old = base.join("persianUtils/colors/tasks");
        make_queue(&old);
        let main = base.join("persianUtils/colors/main");
        fs::create_dir_all(main.join(".git")).unwrap();
        let new = main.join(layout::DIR);
        fs::create_dir(&new).unwrap();
        fs::write(new.join(layout::CONFIG), layout::config_text(true, None)).unwrap();
        let new_key = new.to_string_lossy().to_string();
        let mut c = Config::default();
        c.group_names
            .insert("persianUtils".into(), "DashAfzar".into());
        let lines = carry_over(&mut c, &base, &old, &new);
        assert_eq!(
            c.categories.get(&new_key).map(String::as_str),
            Some("DashAfzar")
        );
        assert!(
            lines.contains(
                &"projects.json: heading \"DashAfzar\" carried to the new queue, as its category"
                    .to_string()
            ),
            "{lines:?}"
        );
        let p = list(&base, &c)
            .into_iter()
            .find(|p| p.key == new_key)
            .unwrap();
        assert_eq!(crate::labels::shown_category(&p), "DashAfzar");
        // A queue with a category of its own carries that, and nothing more.
        let mut c = Config::default();
        c.group_names
            .insert("persianUtils".into(), "DashAfzar".into());
        c.categories
            .insert(old.to_string_lossy().to_string(), "Apps".into());
        let lines = carry_over(&mut c, &base, &old, &new);
        assert_eq!(c.categories.get(&new_key).map(String::as_str), Some("Apps"));
        assert!(!lines.iter().any(|l| l.contains("heading")), "{lines:?}");
    }

    #[test]
    fn a_migrated_queue_found_in_the_same_group_takes_no_heading_of_its_own() {
        // Review R6: persianUtils/colors/.muster (a repo of its own, not a main checkout) is found
        // in the group, so it's under the group's heading already.
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let old = base.join("persianUtils/colors/tasks");
        make_queue(&old);
        let repo = base.join("persianUtils/colors");
        fs::create_dir_all(repo.join(".git")).unwrap();
        let new = repo.join(layout::DIR);
        fs::create_dir(&new).unwrap();
        fs::write(new.join(layout::CONFIG), layout::config_text(true, None)).unwrap();
        let mut c = Config::default();
        c.group_names
            .insert("persianUtils".into(), "DashAfzar".into());
        let listed = list(&base, &c);
        let found = listed
            .iter()
            .find(|p| fs::canonicalize(&p.path).ok() == fs::canonicalize(&new).ok())
            .expect("the new queue is found");
        assert_eq!(found.group, "persianUtils");
        let lines = carry_over(&mut c, &base, &old, &new);
        assert!(c.categories.is_empty(), "{:?}", c.categories);
        assert!(!lines.iter().any(|l| l.contains("heading")), "{lines:?}");
        let p = list(&base, &c)
            .into_iter()
            .find(|p| p.key == found.key)
            .unwrap();
        assert_eq!(crate::labels::shown_category(&p), "DashAfzar");
    }

    #[test]
    fn a_queue_one_level_deeper_is_named_by_its_own_folder_and_keeps_its_group() {
        let base = tempfile::tempdir().unwrap();
        make_queue(&base.path().join("colors/tasks"));
        make_queue(&base.path().join("persianUtils/colors/tasks"));
        make_queue(&base.path().join("persianUtils/fonts/tasks"));
        let l = list(base.path(), &Config::default());
        let got: Vec<(&str, &str, &str)> = l
            .iter()
            .map(|p| (p.name.as_str(), p.group.as_str(), p.source.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                ("colors", "", "found"),
                ("colors-2", "persianUtils", "found"),
                ("fonts", "persianUtils", "found"),
            ]
        );
        let json = serde_json::to_value(&l[1]).unwrap();
        assert_eq!(json["group"], "persianUtils");
        assert_eq!(json["source"], "found");
        // An added queue has no group, wherever it is.
        let elsewhere = tempfile::tempdir().unwrap();
        let other = elsewhere.path().join("grp/sheets/tasks");
        make_queue(&other);
        let mut c = Config::default();
        add(&mut c, base.path(), &other).unwrap();
        let added = list(base.path(), &c).pop().unwrap();
        assert_eq!(
            (
                added.name.as_str(),
                added.group.as_str(),
                added.source.as_str()
            ),
            ("sheets", "", "added")
        );
    }

    #[test]
    fn a_queue_both_found_and_added_shows_once_as_found() {
        let dir = tempfile::tempdir().unwrap();
        // Keys are folders as written; the owner's added ones are canonical, so is this base.
        let base = dir.path().canonicalize().unwrap();
        let q = base.join("persianUtils/ai/tasks");
        make_queue(&q);
        let c = Config {
            added: vec![q.to_string_lossy().to_string()],
            ..Config::default()
        };
        let l = list(&base, &c);
        assert_eq!(l.len(), 1, "{l:?}");
        assert_eq!(
            (
                l[0].name.as_str(),
                l[0].group.as_str(),
                l[0].source.as_str()
            ),
            ("ai", "persianUtils", "found")
        );
        assert_eq!(active(&base, &c).len(), 1);
    }

    #[test]
    fn add_refuses_what_isnt_a_queue_and_unhides() {
        let base = tempfile::tempdir().unwrap();
        let mut c = Config::default();
        let not = tempfile::tempdir().unwrap();
        assert!(add(&mut c, base.path(), not.path())
            .unwrap_err()
            .contains("isn't a Muster queue"));
        let q = base.path().join("beta/tasks");
        make_queue(&q);
        let key = q.canonicalize().unwrap().to_string_lossy().to_string();
        c.hidden.push(key.clone());
        add(&mut c, base.path(), &q).unwrap();
        assert!(
            c.hidden.is_empty(),
            "adding a hidden found queue shows it again"
        );
        assert!(
            c.added.is_empty() || c.added == vec![key],
            "a found queue isn't duplicated"
        );
    }

    #[test]
    fn create_makes_a_working_queue() {
        let base = tempfile::tempdir().unwrap();
        let proj = base.path().join("gamma");
        fs::create_dir(&proj).unwrap();
        let (q, prefix) = create(&proj, Path::new("/opt/muster/bin"), None, &[]).unwrap();
        assert_eq!(prefix, "ga");
        assert!(looks_like_queue(&q));
        for d in queue::STATUS_FOLDERS {
            assert!(q.join(d).is_dir());
        }
        let readme = fs::read_to_string(q.join("README.md")).unwrap();
        assert!(readme.starts_with("---\nprefix: ga\n---\n\n# gamma — task queue"));
        assert_eq!(crate::key::stored_prefix(&q).as_deref(), Some("ga"));
        assert!(
            readme.contains("Find a task by its key (`muster show ga-1`"),
            "{readme}"
        );
        // Agents hand work on with muster, never by moving files.
        for cmd in [
            "muster next",
            "muster claim",
            "muster heartbeat",
            "muster ask",
            "muster inbox",
            "muster pass",
            "muster return",
            "muster block",
            "--gate",
            "muster add",
            "muster shape",
            "muster file",
            "muster drop",
        ] {
            assert!(readme.contains(cmd), "README names {cmd}: {readme}");
        }
        assert!(!readme.contains("a note in `inbox/`"), "{readme}");
        // Blocking says why (0024), as PROTOCOL.md's note on kinds says it.
        assert!(
            readme.contains(
                "`muster block NNNN --on <0014,0016|you|permission|question|outside> \"<what>\"`"
            ),
            "{readme}"
        );
        assert!(!readme.contains("<task id|owner|capture>"), "{readme}");
        assert!(readme.contains(
            "Never move task files or edit `stage:`, `status:`, `claimed-by:` or `heartbeat:` by hand."
        ));
        assert!(
            !readme.contains("--from ready"),
            "claims use claim/next: {readme}"
        );
        assert!(
            readme.contains("Backlog → Shaping → Ready → Building → Testing → Your check → Done")
        );
        assert!(!readme.contains("mv todo/"), "{readme}");
        assert!(!readme.contains("move it to"), "{readme}");
        let stand = fs::read_to_string(q.join("bin/task")).unwrap();
        assert!(stand.contains("\"/opt/muster/bin/task\""));
        assert!(q.join(".git").is_dir());
        let log = std::process::Command::new("git")
            .arg("-C")
            .arg(&q)
            .args(["log", "--oneline"])
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&log.stdout).contains("Muster queue for gamma"));
        assert!(create(&proj, Path::new("/x"), None, &[])
            .unwrap_err()
            .contains("already exists"));
    }

    #[test]
    fn create_refuses_inside_a_repo() {
        let base = tempfile::tempdir().unwrap();
        let repo = base.path().join("repo");
        fs::create_dir_all(repo.join(".git")).unwrap();
        fs::create_dir(repo.join("sub")).unwrap();
        assert!(create(&repo.join("sub"), Path::new("/x"), None, &[])
            .unwrap_err()
            .contains("inside a git repository"));
        assert!(!repo.join("sub/tasks").exists());
    }

    #[test]
    fn a_new_queue_goes_in_the_repos_muster_local_unless_committed_else_beside_it() {
        // What the app's New queue makes, and `muster init` (0004).
        use crate::layout::{read_config, resolve::repo, Config};
        let base = tempfile::tempdir().unwrap();
        let canon = |p: &Path| p.canonicalize().unwrap();
        let main = repo(base.path(), "app");
        let (q, prefix) = new_queue(&main.join("."), None, None, &[], Path::new("/x")).unwrap();
        assert_eq!(canon(&q), canon(&main.join(".muster")));
        assert_eq!(
            (read_config(&q), prefix.as_str()),
            (Ok(Config { local: true }), "ap")
        );
        let e = new_queue(&main, Some(true), None, &[], Path::new("/x")).unwrap_err();
        assert!(
            e.contains("already exists. To show it, use Add existing queue."),
            "{e}"
        );
        let other = repo(base.path(), "web");
        let q = new_queue(&other, Some(true), Some("wb"), &[], Path::new("/x"))
            .unwrap()
            .0;
        assert_eq!(read_config(&q), Ok(Config { local: false }));
        // Outside a repo: the old tasks/, and a mode is refused with nothing written.
        let loose = base.path().join("loose");
        fs::create_dir(&loose).unwrap();
        let e = new_queue(&loose, Some(false), None, &[], Path::new("/x")).unwrap_err();
        assert!(e.contains("isn't in a git repo"), "{e}");
        assert_eq!(fs::read_dir(&loose).unwrap().count(), 0);
        let (q, _) = new_queue(&loose, None, None, &[], Path::new("/x")).unwrap();
        assert_eq!(canon(&q), canon(&loose.join("tasks")));
        assert!(looks_like_queue(&q));
    }

    #[test]
    fn a_new_queue_in_a_repo_without_a_main_checkout_is_refused_as_init_refuses_it() {
        // A submodule is in a git repo, but git's folder isn't its own `.git`: it gets init's
        // reason, with or without a mode, never the old tasks/ or "isn't in a git repo".
        use crate::layout::resolve::{git, repo};
        let base = tempfile::tempdir().unwrap();
        let outer = repo(base.path(), "outer");
        let inner = repo(base.path(), "inner");
        git(
            &outer,
            &[
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                "-q",
                inner.to_str().unwrap(),
                "sub",
            ],
        );
        let sub = outer.join("sub");
        for committed in [None, Some(false), Some(true)] {
            let e = new_queue(&sub, committed, None, &[], Path::new("/x")).unwrap_err();
            assert!(e.contains("with a main checkout"), "{committed:?}: {e}");
        }
        assert!(!sub.join("tasks").exists() && !sub.join(".muster").exists());
    }

    #[test]
    fn config_round_trips_and_tolerates_junk() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("cfg/projects.json");
        assert_eq!(load(&f), Config::default());
        let mut c = Config::default();
        c.added.push("/a".into());
        c.names.insert("/a".into(), "A".into());
        save(&f, &c).unwrap();
        assert_eq!(load(&f), c);
        fs::write(&f, "not json").unwrap();
        assert_eq!(load(&f), Config::default());
    }
    #[test]
    fn the_window_sets_a_prefix_by_the_projects_key_with_the_same_checks() {
        let base = tempfile::tempdir().unwrap();
        make_queue(&base.path().join("muster/tasks"));
        make_queue(&base.path().join("braindump/tasks"));
        let c = Config::default();
        let key_of = |name: &str| {
            list(base.path(), &c)
                .into_iter()
                .find(|p| p.name == name)
                .unwrap()
                .key
        };
        let bd = key_of("braindump");
        assert!(set_prefix_by_key(&c, base.path(), &bd, "mu")
            .unwrap_err()
            .contains("muster has mu"));
        assert!(set_prefix_by_key(&c, base.path(), &bd, "B1")
            .unwrap_err()
            .contains("2 to 4 lowercase letters"));
        assert_eq!(
            set_prefix_by_key(&c, base.path(), "/nowhere", "zz").unwrap_err(),
            "no such project"
        );
        assert_eq!(set_prefix_by_key(&c, base.path(), &bd, "bd").unwrap(), bd);
        let prefixes: Vec<String> = list(base.path(), &c)
            .into_iter()
            .map(|p| p.prefix)
            .collect();
        assert_eq!(prefixes, ["bd", "mu"]);
    }

    #[test]
    fn a_test_knows_no_projects_unless_it_gives_its_own() {
        assert!(
            crate::key::known().is_empty(),
            "a test sees none of this Mac's projects"
        );
        let p = places();
        assert_ne!(p, Places::this_mac());
        assert_eq!(
            (crate::cli::base(), crate::cli::config_file()),
            (p.base, p.config_file)
        );
        // Given its own, it sees those, and so does every read that starts at `places`.
        let d = tempfile::tempdir().unwrap();
        make_queue(&d.path().join("zqmine/tasks"));
        let file = d.path().join("projects.json");
        let other = tempfile::tempdir().unwrap();
        let added = other.path().join("elsewhere/tasks");
        make_queue(&added);
        let mut c = Config::default();
        c.added.push(added.to_string_lossy().to_string());
        save(&file, &c).unwrap();
        use_places(Places {
            base: d.path().to_path_buf(),
            config_file: file.clone(),
        });
        let names: Vec<String> = crate::key::known().into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["zqmine", "elsewhere"]);
        assert_eq!(
            (crate::cli::base(), crate::cli::config_file()),
            (d.path().to_path_buf(), file)
        );
    }

    #[test]
    fn a_thread_a_test_starts_gets_empty_places_of_its_own() {
        let d = tempfile::tempdir().unwrap();
        make_queue(&d.path().join("zqmine/tasks"));
        use_places(Places {
            base: d.path().to_path_buf(),
            config_file: d.path().join("projects.json"),
        });
        let theirs = std::thread::spawn(places).join().unwrap();
        assert_ne!(theirs, places(), "not this thread's");
        assert_ne!(theirs, Places::this_mac(), "not this Mac's");
        let known = std::thread::spawn(crate::key::known).join().unwrap();
        assert!(known.is_empty(), "{known:?}");
    }

    #[test]
    #[should_panic(
        expected = "this Mac's projects.json: give it its own with projects::use_places"
    )]
    fn a_test_that_reads_this_macs_projects_json_fails_saying_how_to_give_its_own() {
        load(&Places::this_mac().config_file);
    }

    #[test]
    #[should_panic(
        expected = "this Mac's projects.json: give it its own with projects::use_places"
    )]
    fn a_test_that_writes_this_macs_projects_json_fails_before_it_writes() {
        // A refusal, so that even with the guard broken nothing is saved over the owner's file.
        let _ = edit(&Places::this_mac().config_file, |_| {
            Err::<(), _>("refused".to_string())
        });
    }

    #[test]
    #[should_panic(expected = "this Mac's ~/playground: give it its own with projects::use_places")]
    fn a_test_that_looks_under_this_macs_playground_fails_saying_how_to_give_its_own() {
        list(&Places::this_mac().base, &Config::default());
    }

    #[test]
    fn folding_and_unfolding_a_heading_leaves_projects_json_byte_identical() {
        // mu-88: every map in the file is saved in key order, so a save that changes nothing
        // else (a fold, then its unfold) writes the same bytes, and the file never churns.
        // mu-160: a new map in Config gets filled here too, or it could turn back into a HashMap.
        let base = tempfile::tempdir().unwrap();
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("cfg/projects.json");
        let mut keys = Vec::new();
        for i in 0..24 {
            let q = base.path().join(format!("p{i:02}/tasks"));
            make_queue(&q);
            keys.push(q.to_string_lossy().to_string());
        }
        // As many group folders as queues, inserted out of order: with only a few keys, a map
        // saved in a random order would come out sorted, or the same twice, too often (R1).
        let groups: Vec<String> = (0..24)
            .rev()
            .map(|i| format!("g{:02}", (i * 7) % 24))
            .collect();
        let heads = ["Tools", "Apps", "کارها", "Games"];
        let colours = crate::arrange::COLOURS;
        edit(&file, |c| {
            for (i, k) in keys.iter().enumerate() {
                c.categories
                    .insert(k.clone(), heads[i % heads.len()].into());
                c.labels.insert(k.clone(), format!("Label {i}"));
                c.colours
                    .insert(k.clone(), colours[i % colours.len()].into());
                c.gate.insert(k.clone(), "bin/gate".into());
                c.run.insert(k.clone(), "bin/run".into());
                let limit = if i % 3 == 0 {
                    crate::check_limit::Limit::Off
                } else {
                    crate::check_limit::Limit::At(i)
                };
                c.your_check.insert(k.clone(), limit);
                c.names.insert(k.clone(), format!("n{i:02}"));
            }
            for g in &groups {
                c.group_names.insert(g.clone(), format!("Group {g}"));
            }
            Ok(())
        })
        .unwrap();
        let before = fs::read_to_string(&file).unwrap();
        for folded in [true, false] {
            edit(&file, |c| {
                let list = list(base.path(), c);
                crate::arrange::set_folded(c, &list, "Apps", folded)
            })
            .unwrap();
        }
        assert_eq!(fs::read_to_string(&file).unwrap(), before);
        // The keys are in order in the file itself, in every map: the first save sorts them, once.
        let mut sorted = keys.clone();
        sorted.sort();
        let mut sorted_groups = groups.clone();
        sorted_groups.sort();
        let maps = [
            ("names", &sorted),
            ("labels", &sorted),
            ("categories", &sorted),
            ("gate", &sorted),
            ("run", &sorted),
            ("yourCheck", &sorted),
            ("colours", &sorted),
            ("groupNames", &sorted_groups),
        ];
        for (map, sorted) in maps {
            let start = before
                .find(&format!("\n  \"{map}\": {{\n"))
                .unwrap_or_else(|| panic!("{map} isn't in the file"));
            let body = &before[start..start + before[start..].find("\n  }").unwrap()];
            let at = |k: &str| {
                body.find(&format!("\n    \"{k}\": "))
                    .unwrap_or_else(|| panic!("{k} isn't in {map}"))
            };
            assert!(
                sorted.windows(2).all(|w| at(&w[0]) < at(&w[1])),
                "{map}'s keys aren't in order"
            );
        }
    }

    #[test]
    fn read_state_passes_the_settings_through_to_the_state() {
        // mu-73 review: the window shows State.settings; a regression would show the defaults.
        let base = tempfile::tempdir().unwrap();
        make_queue(&base.path().join("demo/tasks"));
        let c = Config::default();
        let given = crate::settings::Settings {
            lease_minutes: 95,
            live_minutes: 3,
            notify: false,
            menu_bar: false,
            old_pauses_offered: true,
            claude_accounts: vec!["mg".into()],
            lang: None,
        };
        assert_ne!(given, crate::settings::Settings::default());
        let s = read_state(
            &c,
            &list(base.path(), &c),
            &given,
            &mut queue::Cache::default(),
        );
        assert_eq!(s.settings, given);
        assert_eq!(s.queues.len(), 1);
    }
}
