//! Muster's help, one source for `muster help` and the app's Help tab: every command with how to
//! use it on the command line, how to use Muster inside a Claude Code session, and the app's keys.

use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub name: &'static str,
    pub usage: &'static str,
    pub what: &'static str,
    pub options: &'static [(&'static str, &'static str)],
    pub examples: &'static [&'static str],
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Topic {
    pub title: &'static str,
    /// Lines of the topic: plain sentences, or `$ ` examples.
    pub lines: &'static [&'static str],
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Help {
    pub commands: &'static [Command],
    pub common: &'static [(&'static str, &'static str)],
    pub session: &'static [Topic],
    pub app: &'static [(&'static str, &'static str)],
}

pub const COMMANDS: &[Command] = &[
    Command {
        name: "new",
        usage: "muster new \"<title>\" [--type bug|task|idea] [--priority high|normal|low] [--tags a,b] [--body <text>|-]",
        what: "Create a task in To do. Its id is taken in order under the queue's lock, and it's logged as you.",
        options: &[
            ("--type", "bug, task (default) or idea"),
            ("--priority", "high, normal (default) or low"),
            ("--tags", "comma-separated: spike, chore, tech-debt, code, docs, design, research, test, perf, security, or your own"),
            ("--body", "the description in Markdown; \\n is a line break; - reads it from stdin"),
        ],
        examples: &[
            "muster new \"Board flickers on resize\" --type bug --priority high --tags code",
            "muster new \"Export as CSV\" --type idea --body \"What: …\\n\\nDone looks like: …\"",
            "pbpaste | muster new \"From the clipboard\" --body -",
        ],
    },
    Command {
        name: "list",
        usage: "muster list [--status <s>] [--tag <t>] [--json]",
        what: "Tasks grouped by status, with type, priority, tags, \"needs you\" and open questions.",
        options: &[("--status", "inbox, todo, doing, blocked or done"), ("--tag", "only tasks with this tag"), ("--json", "machine-readable, for scripts and agents")],
        examples: &["muster list", "muster list --status todo --tag spike", "muster list --queue drover --json"],
    },
    Command {
        name: "show",
        usage: "muster show <id> [--json]",
        what: "One task in full: its file as Markdown, or its parsed fields with --json.",
        options: &[("--json", "fields, questions, attachments, log tail")],
        examples: &["muster show 0007", "muster show 0007 --json"],
    },
    Command {
        name: "move",
        usage: "muster move <id> <inbox|todo|blocked|done> [--on \"<what it waits on>\"]",
        what: "Change a task's status. Blocked needs what it waits on. Moving a claimed task out of doing releases the claim. Only agents claim (doing).",
        options: &[("--on", "for blocked: a task id (0012), owner, or capture, and a few words")],
        examples: &["muster move 0007 blocked --on \"owner: pick a colour\"", "muster move 0007 todo", "muster move 0007 done"],
    },
    Command {
        name: "set",
        usage: "muster set <id> <priority|type|title|tags> <value>",
        what: "Change one field, logged with the time and who.",
        options: &[],
        examples: &["muster set 0007 priority high", "muster set 0007 tags spike,tech-debt", "muster set 0007 tags \"\"", "muster set 0007 title \"A clearer title\""],
    },
    Command {
        name: "note",
        usage: "muster note <id> \"<text>\"",
        what: "Add a line to the task's log (the tool stamps the time).",
        options: &[],
        examples: &["muster note 0007 \"Checked on the Mac, works\""],
    },
    Command {
        name: "attach",
        usage: "muster attach <id> <file>...",
        what: "Copy files into the task's attachments/<id>/ (25 MB each; never overwrites, a clash gets -2).",
        options: &[],
        examples: &["muster attach 0007 ~/Desktop/shot.png", "muster attach 0007 *.png"],
    },
    Command {
        name: "answer",
        usage: "muster answer <id> <Qn> \"<text>\"",
        what: "Answer a question in a task. It's written under the question, and the agent that asked gets it pushed into its session.",
        options: &[],
        examples: &["muster answer 0008 Q2 \"B, the window beside the app\""],
    },
    Command {
        name: "check",
        usage: "muster check",
        what: "Problems in the queue: ids used twice, status not matching the folder, claims with no owner or quiet for 12 h, blocked with no reason, waiting on a task that doesn't exist. Exit 1 if any.",
        options: &[],
        examples: &["muster check", "muster check --queue braindump"],
    },
    Command {
        name: "projects",
        usage: "muster projects [--json]",
        what: "Every queue Muster knows: found under ~/playground and added in the app.",
        options: &[],
        examples: &["muster projects"],
    },
    Command {
        name: "init",
        usage: "muster init <project-folder>",
        what: "Create <project-folder>/tasks: status folders, README with the rules, the tool stand-ins, and its own git. Refuses inside a git repo or over an existing tasks/.",
        options: &[],
        examples: &["muster init ~/playground/braindump"],
    },
    Command {
        name: "help",
        usage: "muster help [<command>]",
        what: "This list, or one command in detail.",
        options: &[],
        examples: &["muster help", "muster help new"],
    },
];

pub const COMMON: &[(&str, &str)] = &[
    ("--queue <name|folder>", "which queue; default $MUSTER_ROOT, then the nearest queue from the current folder (a queue, or tasks/ beside or above it)"),
    ("--as <who>", "who's acting, for the log; default $MUSTER_AS, then $USER"),
];

pub const SESSION: &[Topic] = &[
    Topic {
        title: "You, in a Claude Code session",
        lines: &[
            "Run any muster command with ! in front. It runs in that session's folder, so it finds the project's queue on its own:",
            "$ ! muster list",
            "$ ! muster new \"Board flickers on resize\" --type bug --priority high",
            "$ ! muster answer 0008 Q2 \"B\"",
            "muster is linked into ~/.local/bin, so ! muster works in every session. Outside ~/playground, pass --queue <name>.",
            "Or just say it: \"file a bug: the board flickers on resize\". That works where the project's CLAUDE.md points at its queue (Drover's does). The session then runs muster new.",
            "Slash commands /muster and /mu are planned (task 0003). Until then, use ! muster or ask.",
        ],
    },
    Topic {
        title: "Agents doing the work (coding sessions)",
        lines: &[
            "Read the queue's README.md first: ../tasks/README.md from the project's repo.",
            "Pick from todo/, then claim by moving the file. One claim per session. muster claim is coming (task 0002):",
            "$ mv ../tasks/todo/0007-*.md ../tasks/doing/",
            "Ask without stopping. The answer is pushed into your session after your next tool call (the answer hook):",
            "$ ../tasks/bin/task ask 0007 \"Child webview or a separate window?\" --from <your branch>",
            "No hook in this checkout? Collect answers yourself:",
            "$ ../tasks/bin/task inbox",
            "Log progress, block, finish, with the tool stamping the time:",
            "$ muster note 0007 \"Phase 1 done, gate green\"",
            "$ muster move 0007 blocked --on 0012",
            "$ muster move 0007 done",
            "Found something new? Drop a note in ../tasks/inbox/, and capture files it.",
        ],
    },
    Topic {
        title: "The capture session (PM and tech lead)",
        lines: &[
            "Writes the owner's ideas and bugs up as tasks, answers agents' technical questions, and takes product questions to the owner in plain words (WAITING-FOR-OWNER.md).",
            "$ muster new \"…\" --tags spike --body \"**What the owner said:** …\\n\\n## Acceptance criteria\\n- [ ] AC1: …\"",
            "$ ../tasks/bin/task answer 0007 Q1 \"Use the window\"",
            "The app's Helper tab does the same job without a separate session.",
        ],
    },
];

pub const APP: &[(&str, &str)] = &[
    ("⌘N", "New task, in the pane beside the board"),
    ("Esc", "close the pane"),
    (
        "⌘↩",
        "create the task, send the answer, or send to the helper",
    ),
    ("Drag the pane's edge", "resize it (remembered)"),
    (
        "Board · Table · Timeline",
        "three views of the same tasks, with the same priority and tag filters",
    ),
    (
        "Paste or drop",
        "screenshots into New task, a task's note box, or the helper",
    ),
];

pub fn help() -> Help {
    Help {
        commands: COMMANDS,
        common: COMMON,
        session: SESSION,
        app: APP,
    }
}

/// `muster help` as plain text.
pub fn text() -> String {
    let mut out = String::from("muster — tasks for agents and people\n\nCommands:\n");
    let w = COMMANDS.iter().map(|c| c.name.len()).max().unwrap_or(4);
    for c in COMMANDS {
        out.push_str(&format!(
            "  {:<w$}  {}\n",
            c.name,
            c.what
                .split(". ")
                .next()
                .unwrap_or(c.what)
                .trim_end_matches('.')
        ));
    }
    out.push_str("\nOn every command:\n");
    for (k, v) in COMMON {
        out.push_str(&format!("  {k:<22} {v}\n"));
    }
    out.push_str("\n`muster help <command>` shows its usage, options and examples.\n\nInside a Claude Code session:\n");
    for t in SESSION {
        out.push_str(&format!("\n  {}\n", t.title));
        for l in t.lines {
            match l.strip_prefix("$ ") {
                Some(cmd) => out.push_str(&format!("      {cmd}\n")),
                None => out.push_str(&format!("    {l}\n")),
            }
        }
    }
    out
}

/// `muster help <command>`.
pub fn command_text(name: &str) -> Option<String> {
    let c = COMMANDS.iter().find(|c| c.name == name)?;
    let mut out = format!("{}\n\n  {}\n", c.usage, c.what);
    if !c.options.is_empty() {
        out.push_str("\nOptions:\n");
        for (k, v) in c.options {
            out.push_str(&format!("  {k:<12} {v}\n"));
        }
    }
    out.push_str("\nExamples:\n");
    for e in c.examples {
        out.push_str(&format!("  {e}\n"));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_the_cli_runs_is_documented() {
        // The commands cli::run_on and cli::run dispatch.
        for name in [
            "new", "list", "show", "move", "set", "note", "attach", "answer", "check", "projects",
            "init", "help",
        ] {
            let c = COMMANDS
                .iter()
                .find(|c| c.name == name)
                .unwrap_or_else(|| panic!("{name} isn't in the help"));
            assert!(
                c.usage.starts_with(&format!("muster {name}")),
                "{name}'s usage"
            );
            assert!(!c.examples.is_empty(), "{name} has examples");
            assert!(
                c.examples
                    .iter()
                    .all(|e| e.contains(&format!("muster {name}"))),
                "{name}'s examples use it"
            );
        }
        assert_eq!(COMMANDS.len(), 12);
    }

    #[test]
    fn text_and_details() {
        let t = text();
        assert!(t.contains("Inside a Claude Code session"));
        assert!(t.contains("      ! muster list"));
        assert!(command_text("new").unwrap().contains("--tags"));
        assert!(command_text("frob").is_none());
    }
}
