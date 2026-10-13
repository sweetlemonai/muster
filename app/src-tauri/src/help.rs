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
        usage: "muster new \"<title>\" [--type bug|task|idea] [--priority high|normal|low] [--tags a,b] [--time 2h] [--tokens 400k] [--body <text>|-]",
        what: "Create a task in To do. Its id is taken in order under the queue's lock, and it's logged as you.",
        options: &[
            ("--type", "bug, task (default) or idea"),
            ("--priority", "high, normal (default) or low"),
            ("--tags", "comma-separated: spike, chore, tech-debt, code, docs, design, research, test, perf, security, or your own"),
            ("--time", "estimated agent time: 45m, 2h, 1.5d"),
            ("--tokens", "estimated tokens: 150k, 1.2M"),
            ("--body", "the description in Markdown; \\n is a line break; - reads it from stdin"),
        ],
        examples: &[
            "muster new \"Board flickers on resize\" --type bug --priority high --tags code --time 1h --tokens 300k",
            "muster new \"Export as CSV\" --type idea --body \"What: …\\n\\nDone looks like: …\"",
            "pbpaste | muster new \"From the clipboard\" --body -",
        ],
    },
    Command {
        name: "list",
        usage: "muster list [--stage <s>] [--status <s>] [--tag <t>] [--json]",
        what: "Tasks grouped by stage (Backlog, Shaping, Ready, Building, Testing, Your check, Done), with type, priority, what a blocked task waits on, estimate, tags, \"needs you\" and open questions. Backlog notes come first, under Backlog notes, by file name and title (not under a status or tag filter).",
        options: &[
            ("--stage", "backlog, shaping, ready, building, testing, your-check or done"),
            ("--status", "the old status: inbox, todo, doing, blocked or done"),
            ("--tag", "only tasks with this tag"),
            ("--json", "machine-readable: {\"tasks\": […], \"backlog\": […]}, for scripts and agents"),
        ],
        examples: &["muster list", "muster list --stage ready --tag spike", "muster list --queue drover --json"],
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
        what: "The old way to change a task, kept for scripts: inbox puts it in Backlog, todo in Ready, done in Done (and unblocks it), blocked is muster block. Use pass, return, block and stage instead.",
        options: &[("--on", "for blocked: a task id (0012), owner, or capture, and a few words")],
        examples: &["muster move 0007 blocked --on \"owner: pick a colour\"", "muster move 0007 todo", "muster move 0007 done"],
    },
    Command {
        name: "claim",
        usage: "muster claim <id> [--as <who>]",
        what: "Take a Ready task: it moves to Building, held by you, with its lease started (heartbeat). Refused while you already hold another task in Building in this queue (pass, return or block that first), when it's blocked, or when it isn't in Ready any more. Says which queue it claimed in.",
        options: &[("--as", "who you are (your branch or worktree); default $MUSTER_AS, then $USER")],
        examples: &["muster claim 0007 --as wt-a", "muster claim 0007 --as wt-a --queue drover"],
    },
    Command {
        name: "next",
        usage: "muster next [--as <who>]",
        what: "Claim the top Ready task you may take: not blocked, not held by someone else; high priority first, then normal, then low, then the lowest id. Prints \"nothing ready\" and exits 1 when there's none. Refused while you already hold a task in Building.",
        options: &[("--as", "who you are; default $MUSTER_AS, then $USER")],
        examples: &["muster next --as wt-a", "muster next --as wt-a --queue muster"],
    },
    Command {
        name: "heartbeat",
        usage: "muster heartbeat <id> [--as <who>]",
        what: "Keep your claim alive on long work: stamps the task's heartbeat, no log line. Only the holder, only in Building. Your note, ask, pass, return and block stamp it too, and so does an unblock. A claim quiet for 12 h can be released to Ready.",
        options: &[("--as", "who you are; must be who holds it")],
        examples: &["muster heartbeat 0007 --as wt-a"],
    },
    Command {
        name: "reclaim",
        usage: "muster reclaim [--after 12h] [--json]",
        what: "Release every claim whose lease ran out: a task in Building, held, not blocked, with no heartbeat (or, without one, no change to its file) for longer than --after goes back to Ready, its claim cleared and the release logged as a stage change. Live claims are left alone. Prints what it released.",
        options: &[
            ("--after", "the lease: 30m, 2h, 1d (a day is 24 h); default 12h"),
            ("--json", "what was released, for scripts"),
        ],
        examples: &["muster reclaim", "muster reclaim --after 1d --queue drover"],
    },
    Command {
        name: "pass",
        usage: "muster pass <id> [--from <stage>] [--gate \"<gate result line>\"] [--commit <sha>]",
        what: "Hand a task to the next stage: Backlog → Shaping → Ready → Building → Testing → Your check → Done. Ready → Building claims it for you (--as). Shaping → Ready needs acceptance criteria. Building → Testing needs the gate's result line and releases your claim. Testing → Your check needs every criterion ticked except the owner's (\"— verify: owner\"); Your check → Done needs those ticked too (muster tick). Refused while it's blocked, or when someone else holds it.",
        options: &[
            ("--from", "the stage you saw it in; refused if it has moved on since (use it, so a repeat never moves it twice)"),
            ("--gate", "Building → Testing: the gate's result line, verbatim"),
            ("--commit", "Your check → Done: the commit that ships it"),
            ("--as", "who you are; must match who claimed it to pass it out of Building"),
        ],
        examples: &[
            "muster pass 0007 --as wt-a --from ready",
            "muster pass 0007 --as wt-a --from building --gate \"gate full: green (9 steps)\"",
            "muster pass 0007 --from your-check --commit 3f2a9c1",
        ],
    },
    Command {
        name: "return",
        usage: "muster return <id> \"<why>\" [--to <stage>]",
        what: "Send a task back one stage, or to an earlier stage with --to, with the reason (one line, 500 characters). Back to Building gives it to whoever last claimed it there.",
        options: &[("--to", "an earlier stage: backlog, shaping, ready, building, testing or your-check")],
        examples: &[
            "muster return 0007 \"AC2 fails: the board still flickers\"",
            "muster return 0007 \"the spec misses the empty state\" --to shaping",
        ],
    },
    Command {
        name: "block",
        usage: "muster block <id> --on \"<task id|owner|capture> …\"",
        what: "Mark a task blocked. It stays in its stage and keeps its claim; the card shows what it waits on. Blocked is a flag, not a stage.",
        options: &[("--on", "starts with a task id (0012), owner or capture, then a few words")],
        examples: &["muster block 0007 --on 0012", "muster block 0007 --on \"owner: which colour?\""],
    },
    Command {
        name: "unblock",
        usage: "muster unblock <id>",
        what: "Clear the blocked flag. The task carries on in its stage.",
        options: &[],
        examples: &["muster unblock 0007"],
    },
    Command {
        name: "stage",
        usage: "muster stage <id> <stage>",
        what: "Put a task in any stage (the owner's or capture's override), logged as set. Leaving Building releases its claim.",
        options: &[],
        examples: &["muster stage 0007 ready", "muster stage 0007 your-check"],
    },
    Command {
        name: "set",
        usage: "muster set <id> <priority|type|title|tags|time|tokens> <value>",
        what: "Change one field, logged with the time and who.",
        options: &[],
        examples: &["muster set 0007 priority high", "muster set 0007 tags spike,tech-debt", "muster set 0007 tags \"\"", "muster set 0007 time 3h", "muster set 0007 tokens 600k", "muster set 0007 title \"A clearer title\""],
    },
    Command {
        name: "edit",
        usage: "muster edit <id> [--body -]",
        what: "Edit a task's write-up: the description, To do and acceptance criteria between the front matter and the Log. Opens $VISUAL or $EDITOR (else vi; quotes work as in a shell, but no shell runs it) on a copy and saves it when the editor exits, logged as an edit. Unchanged text prints \"no change\" and writes nothing. The front matter and the Log stay the tool's: a \"## Log\" line or a stage line is refused. Refused when the write-up changed since you opened it, or when the editor fails; either way, what you typed is kept in the temp file it names. Refused too while someone else holds it in Building: their branch owns the write-up then, so add a note instead.",
        options: &[
            ("--body -", "read the new write-up from stdin instead of opening an editor (for scripts)"),
            ("--as", "who's editing; the holder of a task in Building may edit it"),
        ],
        examples: &[
            "muster edit 0007",
            "EDITOR=\"code --wait\" muster edit 0007",
            "VISUAL='\"/Applications/Sublime Text.app/Contents/SharedSupport/bin/subl\" -w' muster edit 0007",
            "pbpaste | muster edit 0007 --body -",
        ],
    },
    Command {
        name: "note",
        usage: "muster note <id> \"<text>\"",
        what: "Add a line to the task's log (the tool stamps the time).",
        options: &[],
        examples: &["muster note 0007 \"Checked on the Mac, works\""],
    },
    Command {
        name: "tick",
        usage: "muster tick <id> <AC2|2>",
        what: "Tick one of the criteria only the owner verifies (\"— verify: owner\"), in Your check. Agents tick their own in the file, with the proof; these have no proof but the owner's look. Testing passes to Your check without them; Done needs them.",
        options: &[],
        examples: &["muster tick 0026 AC1", "muster tick 0026 2"],
    },
    Command {
        name: "untick",
        usage: "muster untick <id> <AC2|2>",
        what: "Undo muster tick: untick one of the owner's criteria.",
        options: &[],
        examples: &["muster untick 0026 AC1"],
    },
    Command {
        name: "attach",
        usage: "muster attach <id> <file>...",
        what: "Copy files into the task's attachments/<id>/ (25 MB each; never overwrites, a clash gets -2).",
        options: &[],
        examples: &["muster attach 0007 ~/Desktop/shot.png", "muster attach 0007 *.png"],
    },
    Command {
        name: "ask",
        usage: "muster ask <id> \"<question>\" [--as <who>] [--session <id>]",
        what: "Ask a question in a task without stopping: numbered, written under ## Questions, and your session recorded so the answer is pushed to you (the answer hook) or waits in muster inbox. Lists the task's earlier questions, and refuses one that's already answered there.",
        options: &[
            ("--as", "who's asking (your branch); default $MUSTER_AS, then $USER"),
            ("--session", "your session id; default $CLAUDE_CODE_SESSION_ID"),
        ],
        examples: &[
            "muster ask 0007 \"Child webview or a separate window?\" --as wt-a",
            "muster ask 0007 -- \"--verbose or --quiet by default?\"",
        ],
    },
    Command {
        name: "answer",
        usage: "muster answer <id> <Qn> \"<text>\"",
        what: "Answer a question in a task. It's written under the question, and the agent that asked gets it pushed into its session.",
        options: &[],
        examples: &["muster answer 0008 Q2 \"B, the window beside the app\""],
    },
    Command {
        name: "escalate",
        usage: "muster escalate <id> <Qn> [--as capture]",
        what: "Mark a question \"Escalated to owner\": it shows as waiting for you. Capture then adds it to WAITING-FOR-OWNER.md. Once per question.",
        options: &[("--as", "who takes it to the owner, usually capture")],
        examples: &["muster escalate 0008 Q2 --as capture"],
    },
    Command {
        name: "inbox",
        usage: "muster inbox [--session <id>] [--keep]",
        what: "Answers waiting for your session, each handed over once and moved to delivered/ (unless --keep). Without the answer hook, run it between steps.",
        options: &[
            ("--session", "your session id; default $CLAUDE_CODE_SESSION_ID"),
            ("--keep", "show them without marking them delivered"),
        ],
        examples: &["muster inbox", "muster inbox --session 3fba2222 --keep"],
    },
    Command {
        name: "check",
        usage: "muster check",
        what: "Problems in the queue: ids used twice, status not matching the folder, a stage that isn't one, Building with no claim or quiet for 12 h, blocked with no reason, waiting on a task that doesn't exist, a backlog note over 1 MB or that's a symlink. Exit 1 if any.",
        options: &[],
        examples: &["muster check", "muster check --queue braindump"],
    },
    Command {
        name: "add",
        usage: "muster add \"<one line>\" [--body <text>|-] [--as <who>]",
        what: "Drop a one-liner in the backlog, to be shaped into a task later. It's a file in the backlog folder (inbox/ today), named by the time and the line; prints its name. No id, no front matter. Line breaks in the line become spaces, and it's refused over 200 characters; the body, if any, is Markdown up to 50,000. Checked like a claim: with $MUSTER_ROOT at another project's queue, pass --queue to mean it.",
        options: &[
            ("--body", "more detail in Markdown; \\n is a line break; - reads it from stdin"),
            ("--as", "who found it; default $MUSTER_AS, then $USER"),
        ],
        examples: &[
            "muster add \"Board flickers when the pane opens\"",
            "muster add \"Export as CSV\" --body \"The owner asked for it on the call.\"",
            "pbpaste | muster add \"From the clipboard\" --body -",
        ],
    },
    Command {
        name: "shape",
        usage: "muster shape <note-file> [--json]",
        what: "The shaping brief for a backlog note: its full text, what the product, tech and UI roles each write, the project's docs to read (CLAUDE.md, docs/VISION.md, docs/DESIGN.md, docs/decisions/), open tasks that share words with it (to catch a duplicate), and the exact muster file command with the task template. Writes nothing.",
        options: &[("--json", "the same as JSON, for Drover and scripts")],
        examples: &[
            "muster shape 2026-10-12-1405-board-flickers.md",
            "muster shape 2026-10-12-1405-board-flickers.md --json --queue drover",
        ],
    },
    Command {
        name: "file",
        usage: "muster file <note-file> \"<title>\" [--type bug|task|idea] [--priority high|normal|low] [--tags a,b] [--time 2h] [--tokens 400k] --body <text>|-",
        what: "File a shaped backlog note as a task: the next id, in Shaping, with your body after a \"From the backlog\" line quoting the note. The note moves into the task's attachments/<id>/, so nothing is lost; all under the queue's lock, so a note is filed once. Then muster pass <id> --from shaping puts it in Ready (it needs acceptance criteria).",
        options: &[
            ("--type", "bug, task (default) or idea"),
            ("--priority", "high, normal (default) or low"),
            ("--tags", "comma-separated tags"),
            ("--time", "estimated agent time: 45m, 2h, 1.5d"),
            ("--tokens", "estimated tokens: 150k, 1.2M"),
            ("--body", "the shaped task in Markdown, with ## Acceptance criteria (required); - reads it from stdin"),
        ],
        examples: &[
            "muster file 2026-10-12-1405-board-flickers.md \"Board flickers when the pane opens\" --type bug --time 1h --tokens 300k --body -",
            "muster file my-idea.md \"Export as CSV\" --body \"**What the owner said:** …\\n\\n## Acceptance criteria\\n- [ ] AC1: …\"",
        ],
    },
    Command {
        name: "drop",
        usage: "muster drop <note-file> \"<why>\"",
        what: "Set a backlog note aside: it moves to the backlog's dropped/ folder with the reason added at its end (one line, 500 characters). It's off the board but kept.",
        options: &[],
        examples: &["muster drop 2026-10-12-1405-board-flickers.md \"a duplicate of 0012\""],
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
            "A task moves through stages: Backlog (anyone), Shaping (product, tech and UI agents), Ready (claimable), Building (code agent), Testing (QA agent), Your check (the owner), Done. Only muster changes a stage: never move or edit task files for that.",
            "Never edit stage:, status:, claimed-by: or heartbeat: by hand. Only muster writes them.",
            "Claim with muster, under your branch name, in the queue of the project you're in (it says which queue it claimed in). One claim per session: take the top Ready task, or a given one:",
            "$ muster next --as <your branch>",
            "$ muster claim 0007 --as <your branch>",
            "On long work, keep the claim alive. A claim with no heartbeat for 12 h goes back to Ready (muster reclaim). Your notes and passes count too:",
            "$ muster heartbeat 0007 --as <your branch>",
            "Ask without stopping. The answer is pushed into your session after your next tool call (the answer hook):",
            "$ muster ask 0007 \"Child webview or a separate window?\" --as <your branch>",
            "No hook in this checkout? Collect answers yourself:",
            "$ muster inbox",
            "Log progress, block, hand on, with the tool stamping the time:",
            "$ muster note 0007 \"Phase 1 done, gate green\" --as <your branch>",
            "$ muster block 0007 --on 0012",
            "$ muster pass 0007 --as <your branch> --from building --gate \"<the gate's result line>\"",
            "Testing found a problem? Send it back with the reason:",
            "$ muster return 0007 \"AC2 fails: …\"",
            "Found something new? muster add puts a one-liner in the backlog, and a shaping session files it:",
            "$ muster add \"<one line>\" --as <your branch>",
        ],
    },
    Topic {
        title: "The capture session (PM and tech lead)",
        lines: &[
            "Writes the owner's ideas and bugs up as tasks, answers agents' technical questions, and takes product questions to the owner in plain words (WAITING-FOR-OWNER.md).",
            "$ muster new \"…\" --tags spike --body \"**What the owner said:** …\\n\\n## Acceptance criteria\\n- [ ] AC1: …\"",
            "$ muster answer 0007 Q1 \"Use the window\" --as capture",
            "$ muster escalate 0007 Q2 --as capture",
            "$ muster reclaim",
            "Shapes backlog notes into tasks: the brief says what to read, who writes what, and the command that files it. The note goes into the task's attachments; one not worth a task is dropped with the reason:",
            "$ muster shape <note-file>",
            "$ muster file <note-file> \"<title>\" --type bug --time 1h --tokens 300k --body -",
            "$ muster drop <note-file> \"<why>\"",
            "../tasks/bin/task ask, answer, escalate and inbox still work: they run the same muster commands.",
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
        "Pass to <next stage>",
        "in a task's pane: hand it on (from Building it asks for the gate's result line); in Your check the button reads Accept and marks it Done",
    ),
    (
        "Send back",
        "in a task's pane: back one stage, or to any earlier one, with the reason",
    ),
    (
        "Block · Unblock",
        "in a task's pane: it waits on a task id, owner or capture, and keeps its stage",
    ),
    (
        "Stage picker",
        "in a task's pane: your override, any stage. Use it when someone else holds the task, since only they can pass it",
    ),
    (
        "Release",
        "in a task's pane, when its card says quiet 12 h+: the holder hasn't been heard from for longer than the lease, so it goes back to Ready, logged as reclaimed",
    ),
    (
        "Add to the backlog…",
        "the field at the top of the Backlog column: Enter adds the line as a note, Esc clears it",
    ),
    (
        "File as task · Drop · Copy shaping brief",
        "in a note's pane: file it as a task in Shaping (the note goes into its files), set it aside with the reason, or copy the brief for a shaping session",
    ),
    (
        "Edit",
        "in a task's pane, beside the description: edit the write-up in place, ⌘S to save, Esc to cancel (twice when you've typed). Refused if the file changed since you opened it, or while someone else holds it in Building (add a note instead); your text stays in the box",
    ),
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
            "new",
            "list",
            "show",
            "move",
            "claim",
            "next",
            "heartbeat",
            "reclaim",
            "pass",
            "return",
            "block",
            "unblock",
            "stage",
            "set",
            "edit",
            "note",
            "tick",
            "untick",
            "attach",
            "ask",
            "answer",
            "escalate",
            "inbox",
            "add",
            "shape",
            "file",
            "drop",
            "check",
            "projects",
            "init",
            "help",
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
        assert_eq!(COMMANDS.len(), 31);
    }

    #[test]
    fn the_backlog_commands_and_where_sessions_put_what_they_find() {
        let t = text();
        // Found something new: muster add, not a hand-written file.
        assert!(t.contains("Found something new? muster add"), "{t}");
        assert!(t.contains("      muster add \"<one line>\""), "{t}");
        assert!(!t.contains("Drop a note in ../tasks/inbox/"), "{t}");
        assert!(t.contains("      muster shape "), "{t}");
        assert!(command_text("add").unwrap().contains("--body"));
        assert!(command_text("add")
            .unwrap()
            .contains("refused over 200 characters"));
        assert!(!command_text("add").unwrap().contains("cut to"));
        let list = command_text("list").unwrap();
        assert!(
            list.contains("Backlog notes") && list.contains("\"backlog\""),
            "{list}"
        );
        assert!(command_text("shape").unwrap().contains("--json"));
        let file = command_text("file").unwrap();
        for opt in [
            "--type",
            "--priority",
            "--tags",
            "--time",
            "--tokens",
            "--body",
        ] {
            assert!(file.contains(opt), "{opt}: {file}");
        }
        assert!(file.contains("Shaping") && file.contains("attachments"));
        assert!(command_text("drop").unwrap().contains("dropped/"));
        let app: String = APP.iter().map(|(k, v)| format!("{k} {v}\n")).collect();
        for word in [
            "Add to the backlog",
            "File as task",
            "Copy shaping brief",
            "Drop",
        ] {
            assert!(app.contains(word), "{word}: {app}");
        }
    }

    #[test]
    fn edit_says_who_may_edit_and_how() {
        let t = command_text("edit").unwrap();
        for words in [
            "$VISUAL",
            "$EDITOR",
            "vi",
            "--body -",
            "no change",
            "changed since you opened it",
            "holds it in Building",
            "add a note instead",
        ] {
            assert!(t.contains(words), "{words}: {t}");
        }
        let app: String = APP.iter().map(|(k, v)| format!("{k} {v}\n")).collect();
        for words in ["Edit", "⌘S", "Esc", "add a note instead"] {
            assert!(app.contains(words), "{words}: {app}");
        }
    }

    #[test]
    fn text_and_details() {
        let t = text();
        assert!(t.contains("Inside a Claude Code session"));
        assert!(t.contains("      ! muster list"));
        // Agents claim, keep claims alive, ask and hand work on with muster, never by hand.
        assert!(t.contains("      muster next --as <your branch>\n"));
        assert!(t.contains("      muster claim 0007 --as <your branch>\n"));
        assert!(t.contains("      muster heartbeat 0007 --as <your branch>\n"));
        assert!(t.contains("      muster ask 0007 "));
        assert!(t.contains("      muster inbox\n"));
        assert!(t.contains(
            "Never edit stage:, status:, claimed-by: or heartbeat: by hand. Only muster writes them."
        ));
        assert!(
            !t.contains("--as <your branch> --from ready"),
            "claims use claim/next"
        );
        assert!(command_text("reclaim").unwrap().contains("--after"));
        assert!(command_text("inbox").unwrap().contains("--keep"));
        assert!(t.contains("      muster block 0007 --on 0012\n"));
        assert!(t.contains("      muster return 0007"));
        assert!(!t.contains("mv ../tasks"));
        assert!(!t.contains("muster move 0007"));
        assert!(command_text("pass").unwrap().contains("--gate"));
        assert!(command_text("return").unwrap().contains("--to"));
        assert!(command_text("new").unwrap().contains("--tags"));
        assert!(command_text("frob").is_none());
        assert!(command_text("pass").unwrap().contains("--from"));
        // The app's help names the pane's stage actions.
        let app: String = APP.iter().map(|(k, v)| format!("{k} {v}\n")).collect();
        for word in [
            "Pass to",
            "Accept",
            "Send back",
            "Block",
            "Unblock",
            "Stage picker",
            "Release",
        ] {
            assert!(app.contains(word), "{word}: {app}");
        }
    }
}
