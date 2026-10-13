//! Which queues the app shows: the ones found under `~/playground/*/tasks` and one level deeper
//! (`~/playground/*/*/tasks`, `queue::discover`), plus ones the owner added from anywhere, minus
//! the ones they hid, each with an optional display name. Saved as `projects.json` in the app's
//! config folder. Creating a queue writes Muster's layout into a new
//! `tasks/` folder, never inside a git work tree, and never over an existing folder.

use crate::queue;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Default, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    /// Queue folders added by hand.
    pub added: Vec<String>,
    /// Queue folders not to show.
    pub hidden: Vec<String>,
    /// Display names by queue folder.
    pub names: HashMap<String, String>,
    /// The project's gate command by queue folder (0023): what a merge runs in the task's
    /// worktree, split into words without a shell (`bin/gate`, `make check`).
    pub gate: HashMap<String, String>,
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
    /// What its tasks' keys start with (0033): `mu` in `mu-12`, stored or its default
    /// (`key::prefixes`).
    pub prefix: String,
}

pub fn load(file: &Path) -> Config {
    fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save(file: &Path, config: &Config) -> Result<(), String> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = file.with_extension("json.tmp");
    fs::write(
        &tmp,
        serde_json::to_string_pretty(config).map_err(|e| e.to_string())? + "\n",
    )
    .map_err(|e| e.to_string())?;
    fs::rename(&tmp, file).map_err(|e| e.to_string())
}

pub fn looks_like_queue(dir: &Path) -> bool {
    dir.join("README.md").is_file()
        && (dir.join("todo").is_dir() || dir.join("tasks.json").is_file())
}

pub fn default_name(dir: &Path) -> String {
    // `<project>/tasks` → the project's folder name; anything else → its own name.
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
            prefix: String::new(),
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

/// `active`, of a list already made.
pub fn shown(list: &[Project]) -> Vec<(String, PathBuf)> {
    list.iter()
        .filter(|p| !p.hidden && p.ok)
        .map(|p| (p.name.clone(), PathBuf::from(&p.path)))
        .collect()
}

/// The longest gate command projects.json keeps.
const MAX_GATE: usize = 500;

/// Set (or, with an empty `cmd`, clear) the gate command of the project named `name`. The caller
/// has checked it splits into words (`cli::split_words`); here it's one line of at most 500
/// characters. Returns the project's queue folder.
pub fn set_gate(config: &mut Config, base: &Path, name: &str, cmd: &str) -> Result<String, String> {
    let p = list(base, config)
        .into_iter()
        .find(|p| p.name == name)
        .ok_or_else(|| format!("no project named {name:?} (see `muster projects`)"))?;
    let cmd = cmd.trim();
    if cmd.chars().any(char::is_control) {
        return Err("the gate command is one line, without control characters".into());
    }
    if cmd.chars().count() > MAX_GATE {
        return Err(format!(
            "the gate command is longer than {MAX_GATE} characters"
        ));
    }
    if cmd.is_empty() {
        config.gate.remove(&p.key);
    } else {
        config.gate.insert(p.key.clone(), cmd.to_string());
    }
    Ok(p.key)
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

/// Add an existing queue folder picked by the owner.
pub fn add(config: &mut Config, base: &Path, dir: &Path) -> Result<String, String> {
    let dir = dir.canonicalize().map_err(|e| e.to_string())?;
    if !looks_like_queue(&dir) {
        return Err(format!(
            "{} isn't a Muster queue (no README.md with a todo/ folder). To start one, use New queue.",
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
            .and_then(|()| crate::key::store_prefix(root, prefix))
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
         - Built and the gate is green: `muster pass NNNN --as <your branch> --from building --gate \"<the gate's result line>\"`.\n  \
         Testing ticks each criterion with its proof, then passes it on to the owner's check.\n\
         - Something's wrong: `muster return NNNN \"<why>\"` sends it back a stage.\n\
         - Waiting on something: `muster block NNNN --on <0014,0016|you|permission|question|outside> \"<what>\"`,\n  \
         then `muster unblock NNNN`. It keeps its stage. Task ids go into its depends and it clears by\n  \
         itself once they're merged (in Your check or Done); a block on a question (`--on question Q2`)\n  \
         clears when it's answered.\n\
         - Code goes in the project's repo, never in this folder.\n\
         - New bugs or ideas you find: `muster add \"<one line>\" --as <your branch>` puts a note in the backlog\n  \
         (`inbox/`; a file dropped there by hand counts too). Shaping turns it into a task: `muster shape <note-file>`\n  \
         prints the brief, `muster file <note-file> \"<title>\" --body -` files it in Shaping (the note goes into the\n  \
         task's attachments), and `muster drop <note-file> \"<why>\"` sets it aside.\n\
         - `bin/` holds stand-ins that run Muster's tools on this queue.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_queue(dir: &Path) {
        fs::create_dir_all(dir.join("todo")).unwrap();
        fs::write(dir.join("README.md"), "q").unwrap();
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
}
