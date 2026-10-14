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
    /// The manual's Commands page in Markdown (`commands_page`), which the Help tab shows.
    pub commands_page: String,
}

/// A section of the manual's Commands page: the commands that do one kind of job, in the order
/// the page shows them. Levels has no commands of its own, so it shows examples of `new` and
/// `set` instead (each one also in that command's examples).
#[derive(Clone, Debug)]
pub struct Group {
    pub title: &'static str,
    /// Markdown, written for the page.
    pub intro: &'static str,
    pub commands: &'static [&'static str],
    pub examples: &'static [&'static str],
}

pub const COMMANDS: &[Command] = &[
    Command {
        name: "new",
        usage: "muster new \"<title>\" [--type code|bug|chore|tech-debt|spike|design|docs|research] [--priority high|normal|low] [--tags a,b] [--time 2h] [--test-time 30m] [--tokens 400k] [--touches <folders,globs>] [--level goal|feature] [--parent <id>] [--due YYYY-MM-DD] [--human] [--body <text>|-]",
        what: "Create a task in Ready. Its id is taken in order under the queue's lock, and it's logged as you. With --level it's a goal (an outcome, may have a due date) or a feature (groups tasks); agents build only tasks. With --human it's the owner's own to do (for: human): only the owner makes one, and agents never take it; an agent that finds something only the owner can do proposes it with muster add. An idea that isn't a task yet goes in the backlog instead (muster add).",
        options: &[
            ("--type", "code (default), bug, chore, tech-debt, spike, design, docs or research; task still means code"),
            ("--priority", "high, normal (default) or low"),
            ("--tags", "comma-separated: spike, chore, tech-debt, code, docs, design, research, test, perf, security, or your own"),
            ("--time", "estimated agent time: 45m, 2h, 1.5d"),
            ("--test-time", "estimated test time, the QA agent plus your check: 30m, 1h"),
            ("--tokens", "estimated tokens: 150k, 1.2M"),
            ("--touches", "where in the repo it works, folders and globs, comma-separated (app/src/TaskPanel*, docs/), for muster ready --parallel"),
            ("--level", "goal, feature or task (default)"),
            ("--parent", "the id it sits under: a task's is a feature or a goal, a feature's a goal; a goal has none"),
            ("--due", "a goal's date, YYYY-MM-DD"),
            ("--human", "the owner's own task, To do → Doing → Done, hatched on the board with the You ribbon (only the owner, --as owner)"),
            ("--body", "the description in Markdown; \\n is a line break; - reads it from stdin. It can't have a \"## Comments\" line (comments are the tool's)"),
        ],
        examples: &[
            "muster new \"Board flickers on resize\" --type bug --priority high --tags code --time 1h --test-time 20m --tokens 300k",
            "muster new \"Buy the domain\" --human --as owner",
            "muster new \"Export as CSV\" --type code --body \"What: …\\n\\nDone looks like: …\"",
            "pbpaste | muster new \"From the clipboard\" --body -",
            "muster new \"Ship 1.0\" --level goal --due 2026-12-01",
            "muster new \"Stage track\" --level feature --parent 0030",
            "muster new \"Pass button\" --parent 0031 --time 2h",
        ],
    },
    Command {
        name: "list",
        usage: "muster list [--search \"<words>\"] [--stage <s,…>] [--type <t,…>] [--tag <t,…>] [--holder <who,…>] [--blocked] [--sort <order>] [--status <s>] [--json]",
        what: "Tasks grouped by stage (Backlog, Shaping, Ready, Building, Testing, Your check, Done), with type, priority, [you] on the owner's own tasks (for: human), what a blocked task waits on, estimate, tags, \"needs you\" and open questions. Backlog notes come first, under Backlog notes, by file name and title (not under a filter only tasks have). Search, filters and sort follow the app's rules, so the same query lists the same tasks. Several values in one option (ready,building) mean any of them; options together mean all of them. When more of the agents' tasks wait in Your check than the project's limit (5 unless set in the app's Settings › Projects), a line on stderr under the Your check heading says so; the list and the exit code don't change.",
        options: &[
            ("--search", "words in any order, case and accents aside, each in the id (19 finds 0019), title, tags, text or holder; a word of digits only is an id, and a key (mu-19, MU-19) is that task"),
            ("--stage", "backlog, shaping, ready, building, testing, your-check or done"),
            ("--type", "code, bug, chore, tech-debt, spike, design, docs or research (task means code)"),
            ("--tag", "only tasks with this tag"),
            ("--holder", "who holds it in its stage, as the board shows it, without the session (drover/main, wt-a, owner)"),
            ("--blocked", "only blocked tasks, those waiting only on the tasks they depend on too"),
            ("--sort", "priority (needs you, then high to low; Done last, newest finished first), newest, oldest, changed, estimate or id (the default)"),
            ("--status", "the old status: inbox, todo, doing, blocked or done"),
            ("--json", "machine-readable: {\"tasks\": […], \"backlog\": […]}, for scripts and agents. Each task's sessionState (working, needs-input, ended or unknown) is what Claude Code says its holder's session is doing, read from claude agents --json for this listing only; unknown when it can't be read or the session holds other open tasks too"),
        ],
        examples: &[
            "muster list",
            "muster list --stage ready --tag spike",
            "muster list --search \"board flicker\" --stage ready,building --sort changed",
            "muster list --queue drover --json",
        ],
    },
    Command {
        name: "show",
        usage: "muster show <id> [--json]",
        what: "One task in full: its file as Markdown, then its key, who's on it and the latest check-in; or its parsed fields with --json. Name it by its key (mu-7, MU-7) or its number (7, 0007); another project's key shows that project's task.",
        options: &[("--json", "fields (its key beside its id), questions, attachments, log tail")],
        examples: &["muster show mu-7", "muster show 7", "muster show br-12", "muster show 0007 --json"],
    },
    Command {
        name: "move",
        usage: "muster move <id> <inbox|todo|blocked|done> [--on \"<what it waits on>\"]",
        what: "The old way to change a task, kept for scripts: inbox puts it in Backlog, todo in Ready, done in Done (and unblocks it; refused for whoever built the task, as stage is, and for a task made with muster start whose branch isn't in main yet), blocked is muster block. Use pass, return, block and stage instead.",
        options: &[("--on", "for blocked: as muster block's --on, the kind and what it waits on in one (\"you: pick a colour\", \"0012\")")],
        examples: &["muster move 0007 blocked --on \"owner: pick a colour\"", "muster move 0007 todo", "muster move 0007 done"],
    },
    Command {
        name: "claim",
        usage: "muster claim <id> [--as <who>]",
        what: "Take a Ready task: it moves to Building, held by you, with its lease started (heartbeat). Inside a Claude Code session muster adds the session to who you are (name, short id, checkout, account), so you never type it. Refused while you already hold another task in Building in this queue (pass, return or block that first; a task sent back to you from Testing counts, and comes first), when it's blocked, when a task it depends on isn't merged yet (an agent's task counts once it's in Your check, the owner's own once it's Done), when it isn't in Ready any more, and for a human task (the owner's own) unless you're the owner. Says which queue it claimed in.",
        options: &[("--as", "who you are (your branch or worktree); default $MUSTER_AS, then the folder's git user.name")],
        examples: &["muster claim 0007 --as wt-a", "muster claim 0007 --as wt-a --queue drover"],
    },
    Command {
        name: "next",
        usage: "muster next [--as <who>]",
        what: "Claim the top Ready task you may take: not blocked, not held by someone else, everything it depends on merged (in Your check or Done; the owner's own task, Done), and never a human task (the owner's own) unless you're the owner; high priority first, then normal, then low, then the lowest id. Prints \"nothing ready\" and exits 1 when there's none. Refused while you already hold a task in Building; when one of them was sent back from Testing, it says to fix that one first. When more of the agents' tasks wait in Your check than the project's limit (5 unless set in the app's Settings › Projects; the owner's own tasks don't count), it says so first, on stderr (\"muster: 7 tasks wait in Your check (limit 5): the owner's review is the bottleneck.\"), and still claims: it warns, never refuses.",
        options: &[("--as", "who you are; default $MUSTER_AS, then the folder's git user.name")],
        examples: &["muster next --as wt-a", "muster next --as wt-a --queue muster"],
    },
    Command {
        name: "heartbeat",
        usage: "muster heartbeat <id> [--as <who>]",
        what: "Keep your claim alive on long work: stamps the task's heartbeat, no log line. Only the holder, only in Building. Your note, comment, ask, pass, return and block stamp it too, and so does an unblock. A claim quiet past the lease (12 h, unless changed in the app's Settings on this Mac) can be released to Ready.",
        options: &[("--as", "who you are; must be who holds it")],
        examples: &["muster heartbeat 0007 --as wt-a"],
    },
    Command {
        name: "checkin",
        usage: "muster checkin <id> \"<what's done, what's next>\" [--pct <0-100>]",
        what: "Say how the work is going: a line in the task's log that its card shows (\"60% · gate green, writing tests · 12 min ago\"), and the heartbeat with words, so it keeps your claim alive too. Only the holder, only in Building. Check in at each milestone and at least every 30 minutes: a card quiet for longer says \"no check-in for 45 min\".",
        options: &[
            ("--pct", "how far along you think you are, 0 to 100 (your own estimate)"),
            ("--as", "who you are; must be who holds it"),
        ],
        examples: &[
            "muster checkin 0007 \"gate green, writing tests\" --pct 60 --as wt-a",
            "muster checkin 0007 \"found the cause, fixing it\" --as wt-a",
        ],
    },
    Command {
        name: "ping",
        usage: "muster ping <id> [\"<message>\"]",
        what: "Reach the session working on a task, through its mailbox: the answer hook hands it over after that session's next tool call. Without a message it asks for a check-in (Building only), and the session answers with muster checkin before anything else; the card says \"asked 2 min ago\" until it does. With one, it sends the message (at most 500 characters, control characters taken out), quoted as your note, never as instructions. Building or Testing; only a holder whose claim has a session stamp can be reached. Logged on the task.",
        options: &[("--as", "who's asking: owner, or your own label")],
        examples: &[
            "muster ping 0022 --as owner",
            "muster ping 0022 \"the API key is in the Keychain now\" --as owner",
        ],
    },
    Command {
        name: "reclaim",
        usage: "muster reclaim [--after 12h] [--json]",
        what: "Release every claim whose lease ran out: a task in Building, held, not blocked, with no heartbeat (or, without one, no change to its file) for longer than --after goes back to Ready, its claim cleared and the release logged as a stage change. Live claims are left alone. Prints what it released.",
        options: &[
            ("--after", "the lease: 30m, 2h, 1d (a day is 24 h); default this Mac's lease from the app's Settings, 12h unless changed"),
            ("--json", "what was released, for scripts"),
        ],
        examples: &["muster reclaim", "muster reclaim --after 1d --queue drover"],
    },
    Command {
        name: "ready",
        usage: "muster ready [--parallel [<n>]] [--json]",
        what: "The tasks a session could start now: in Ready, not blocked, nobody's claim, an agent's (never a human task), everything they depend on merged (in Your check or Done); high priority first, then the lowest id. With --parallel, up to n of them (default 3) that are safe side by side, and beside the tasks already in Building or Testing: none depends on another, and what they touch doesn't overlap. A task says where it works in touches: (folders and globs, set at shaping or with muster set <id> touches); two entries overlap when the part before the first wildcard of one starts the other's (app/src overlaps app/src/TaskPanel*), and a task without touches never pairs. Then the ones left out, with why. With --parallel it also warns on stderr, as next does, when Your check is over the project's limit.",
        options: &[
            ("--parallel", "only a set that can be built at the same time; how many goes after it (1 to 20, default 3)"),
            ("--json", "{\"tasks\": […], \"left\": [{\"id\", \"why\"}]}, for a dispatcher"),
        ],
        examples: &["muster ready", "muster ready --parallel", "muster ready --parallel 2 --json"],
    },
    Command {
        name: "start",
        usage: "muster start <id> [--at <path>|--here]",
        what: "Claim a Ready task (claim's rules) in its own git worktree, to work beside other sessions: a branch task/<id>-<slug> from the local main's commit (however dirty the main checkout is) and a worktree for it, at <project>/worktrees/<repo>/<id>-<slug> unless --at says where. Run it in the project's repo or a worktree of it. The task records worktree: and branch:, and start prints both with the brief: the task file, what it touches, its criteria. Work there; pass from Building as always. Passing it from Testing merges it (muster pass). A task started before (sent back to Ready, reclaimed) is picked up in its worktree again. Refused claims leave nothing behind. Warns on stderr, as next does, when Your check is over the project's limit, and still starts.",
        options: &[
            ("--at", "where to make the worktree instead (a path that doesn't exist yet)"),
            ("--here", "use the worktree you're in (one a runner like a Drover lane made, with nothing uncommitted): it's switched to the task's branch, and it stays the runner's: muster never removes it"),
            ("--as", "who you are; default $MUSTER_AS, then the folder's git user.name"),
        ],
        examples: &["muster start 0031 --as wt-a", "muster start 0031 --here --as lane-2", "muster start 0031 --at ~/wt/0031 --as wt-a"],
    },
    Command {
        name: "release",
        usage: "muster release <id> --clean [--force]",
        what: "Give up a started task: back to Ready with its claim released, its worktree removed and its branch deleted (the local one; a branch pushed to origin stays, and so does its work). It first lists what that would lose (uncommitted files in the worktree, commits on no other branch) and refuses it unless --force. Works when the worktree is already gone too. A runner's worktree (start --here) is never removed: it's left detached with its files, and only the branch goes. In Building only its holder or the owner releases it. A paused task is refused: resume it first.",
        options: &[
            ("--clean", "remove the worktree and the branch (required: to send a task back and keep them, use muster return)"),
            ("--force", "throw away the uncommitted files and unpushed commits it listed"),
        ],
        examples: &["muster release 0031 --clean", "muster release 0031 --clean --force --as owner"],
    },
    Command {
        name: "pass",
        usage: "muster pass <id> [--from <stage>] [--gate \"<gate result line>\"] [--commit <sha>]",
        what: "Hand a task to the next stage: Backlog → Shaping → Ready → Building → Testing → Your check → Done. Ready → Building claims it for you (--as). Shaping → Ready needs acceptance criteria and no open vision conflict (muster vision). Building → Testing needs the gate's result line, and every review finding open for the builder answered (muster review), and releases your claim. Testing → Your check needs every criterion ticked except the owner's (\"— verify: owner\"), and a session that didn't build it: the builder (who claimed it, or passed it into Testing; the same --as label, in the same session or with none) is refused, naming who built it, so a fresh QA session with its own --as passes it, or the owner (logged as the owner's override: no separate tester). A tester under another label in the builder's session is let through, and the log says so. Your check → Done needs those ticked too (muster tick). A human task (the owner's own) goes To do (Ready) → Doing (Building) → Done, and only the owner passes it: no gate line, no Testing, no Your check. Refused while it's blocked, when someone else holds it, and into Building (Ready → Building, or the owner's Start) while a task it depends on isn't merged yet, as claim is. A task made with muster start merges when it passes from Testing, once its review of record is clean at its branch's tip (muster review; the owner may merge without one, and the report says so; a commit made while it waits for another merge or while its gate runs stops it, unmerged), in order, stopping at the first failure: one merge at a time per repo; rebase its branch on the local main (a conflict sends it back to Building naming the files); run the project's gate in its worktree (muster projects set <name> gate; red sends it back with the gate line), and again if main moved while it ran (twice at most, then it stops: main keeps moving); fast-forward the local main to it (never a merge commit, so main only gets a tree the gate saw), only if the main checkout is clean and on main; push main to origin (never forced: if origin moved, it fetches and stops); remove the worktree (never a runner's, made with start --here) and delete the branch, and origin's copy only when it's the same commit; record the merge (the commit and a report: gate line, the review, time in Building and Testing, check-ins) and pass it to Your check: the record is written even if the task was blocked or moved meanwhile, and then it says so (use muster stage); then it names the next task to start. Nothing is deleted on any failure before main has the merge. Only the command line does this: the app's Pass is refused for such a task.",
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
            "muster pass 0041 --from ready --as owner",
            "muster pass 0031 --from testing --as qa",
        ],
    },
    Command {
        name: "return",
        usage: "muster return <id> \"<why>\" [--to <stage>]",
        what: "Send a task back one stage, or to an earlier stage with --to, with the reason (one line, 500 characters). Back to Building gives it to whoever last claimed it there. A send-back from Testing is always allowed, even when that holder already holds another task in Building: it isn't a new claim. It comes first (`muster next` says \"fix mu-59 first (sent back by the review)\", and the card does), and the holder's other task waits behind it. Sent back from Your check (merged), the tasks that depend on it and haven't started show blocked by it again; ones already under way keep going, with a note on their log (\"0002 went back to Building: <why>\").",
        options: &[("--to", "an earlier stage: backlog, shaping, ready, building, testing or your-check")],
        examples: &[
            "muster return 0007 \"AC2 fails: the board still flickers\"",
            "muster return 0007 \"the spec misses the empty state\" --to shaping",
        ],
    },
    Command {
        name: "block",
        usage: "muster block <id> --on <0014,0016|you|permission|question|outside> [\"<what it waits on>\"]",
        what: "Mark a task blocked, and say why: the card, the table, the timeline and the task pane say plainly what it waits for (\"Your answer: Q2 …\", \"Your action: …\", \"Permission: …\", \"After mu-14\", \"Outside: …\"). It stays in its stage and keeps its claim; blocked is a flag, not a stage. Task ids go into its depends instead of a flag: it shows blocked by them until they're all merged (in Your check or Done; the owner's own task, Done), and clears by itself then (logged). A block on a question clears when it's answered. You, permission and outside stay until muster unblock. Blocked on you or by a permission counts as waiting for you. Refused: an unknown kind, no detail, a question that isn't open in the task.",
        options: &[("--on", "the kind, then what it waits on (in --on after the kind, or after it): task ids 0014,0016 (added to depends); you \"<what you have to do or decide>\"; permission \"<what access, and where to grant it>\"; question Q2 (an open question in this task); outside \"<a third party, a service, a release>\". owner … and capture … still work, as you and question")],
        examples: &[
            "muster block 0007 --on 0012,0014",
            "muster block 0007 --on you \"set the API key in the Keychain\"",
            "muster block 0007 --on permission \"write to ~/playground/x/tasks (sandbox): grant in the layer's write list\"",
            "muster block 0007 --on question Q2",
            "muster block 0007 --on outside \"Apple review\"",
            "muster block 0007 --on \"owner: which colour?\"",
        ],
    },
    Command {
        name: "unblock",
        usage: "muster unblock <id>",
        what: "Clear the blocked flag (on you, by a permission, a question or outside). The task carries on in its stage. A block on tasks has no flag: it clears by itself once they're done.",
        options: &[],
        examples: &["muster unblock 0007"],
    },
    Command {
        name: "pause",
        usage: "muster pause <id> [\"<why>\"]",
        what: "Not now, keep it as it is: the task keeps its stage, write-up, claim and worktree and goes to the Paused lane, out of the working columns, until Resume puts it back in its column. It isn't blocked. Nobody can claim, pass or pick it (next, ready), and nobody, the owner included, returns it, moves its stage or releases it (return, stage, move, release) until it's resumed; its lease doesn't run while it's paused (reclaim skips it); the tasks that depend on it still wait. A task (not a goal or feature), from Backlog through Testing. In Building or Testing its session gets a stop request through its mailbox: stop at a safe point, check in (in Testing, a note) what's done and next, and end its turn. Logged as \"paused (why)\"; the timeline and the Work section's time leave paused stretches out.",
        options: &[("--as", "who's pausing it: owner, or your own label")],
        examples: &[
            "muster pause 0042 \"the design changes first\" --as owner",
            "muster pause 0042 --as owner",
        ],
    },
    Command {
        name: "resume",
        usage: "muster resume <id>",
        what: "Carry on with a paused task, as it was. In Building its claim's lease starts over, so the holder has one lease to come back before muster reclaim gives it to whoever claims it next; the session working on it in Building or Testing gets a message to carry on from its last check-in. Logged as \"resumed after 2 h\".",
        options: &[("--as", "who's resuming it: owner, or your own label")],
        examples: &["muster resume 0042 --as owner"],
    },
    Command {
        name: "stage",
        usage: "muster stage <id> <stage> [--from <stage>]",
        what: "Put a task in any stage (the owner's or capture's override), logged as set. Leaving Building releases its claim. A builder can't use it to skip its tester: from Testing or before to Your check or Done it's refused for whoever built the task, as pass is; the owner's is logged as the owner's override. A task made with muster start can't be put in Your check or Done until its branch is in main (the merge is how it gets there), for the owner too: it is refused with muster pass <id> --from testing to merge it or muster release <id> --clean to drop the branch; once the branch is in main (merged some other way, even deleted since) it goes. Tasks with no branch, and the owner's own, aren't held to it.",
        options: &[("--from", "the stage you saw it in; refused if it has moved on since (use it, so a repeat never moves it twice)")],
        examples: &["muster stage 0007 ready", "muster stage 0007 your-check --from testing"],
    },
    Command {
        name: "set",
        usage: "muster set <id> <priority|type|title|tags|time|test-time|tokens|touches|level|parent|due|depends|for> <value>",
        what: "Change one field, logged with the time and who. type is one of code, bug, chore, tech-debt, spike, design, docs, research; time is agent time and test-time the QA agent plus your check. touches is where in the repo it works, folders and globs, comma-separated (app/src/TaskPanel*, docs/), which muster ready --parallel keeps apart; empty clears it. level, parent and due keep the rules of muster new (a parent that exists at the right level, no loop, a due date only on a goal); a task in Building or Testing, or held, stays a task. An empty parent or due clears it. for is human or ai: who does it (a human task is the owner's own, hatched on the board with the You ribbon); only a task, and not while it's held, in Building, Testing or Your check; made human in Backlog or Shaping, it moves to Ready (To do).",
        options: &[],
        examples: &["muster set 0007 priority high", "muster set 0041 for human", "muster set 0007 type tech-debt", "muster set 0007 tags spike,tech-debt", "muster set 0007 tags \"\"", "muster set 0007 time 3h", "muster set 0007 test-time 45m", "muster set 0007 tokens 600k", "muster set 0007 title \"A clearer title\"", "muster set 0031 level feature", "muster set 0032 parent 0031", "muster set 0031 parent \"\"", "muster set 0030 due 2026-12-01", "muster set 0018 depends 0014,0016", "muster set 0018 depends \"\"", "muster set 0031 touches \"app/src/TaskPanel*, docs/\""],
    },
    Command {
        name: "edit",
        usage: "muster edit <id> [--body -]",
        what: "Edit a task's write-up: the description, To do and acceptance criteria between the front matter and the Log. Opens $VISUAL or $EDITOR (else vi; quotes work as in a shell, but no shell runs it) on a copy and saves it when the editor exits, logged as an edit. Unchanged text prints \"no change\" and writes nothing. The front matter and the Log stay the tool's: a \"## Log\" line or a stage line is refused. Once a task has a vision check, its ## Vision section is the tool's too: left out of the write-up and kept as it is, and a new \"## Vision\" line is refused. Refused when the write-up changed since you opened it, or when the editor fails; either way, what you typed is kept in the temp file it names. Refused too while someone else holds it in Building: their branch owns the write-up then, so add a note instead.",
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
        name: "comment",
        usage: "muster comment <id> \"<text>\" | --body <text>|-",
        what: "Add a comment to a task's thread: under ## Comments, above the Log, stamped with who and when, and logged as commented. Anyone may comment on any task at any stage, one someone else holds included: comments don't touch the write-up, and edit leaves them alone. The holder's own comment keeps their claim alive, like a note. Lines are kept (each after the first indented in the file); control characters go, at most 4,000 characters, and a line that would read as a stage change is refused.",
        options: &[
            ("--body", "the comment instead of the words after the id; \\n is a line break; - reads it from stdin"),
            ("--as", "who's commenting; default $MUSTER_AS, then $USER"),
        ],
        examples: &[
            "muster comment 0007 \"Tried it on the Mac: the board still flickers once\"",
            "pbpaste | muster comment 0007 --body -",
        ],
    },
    Command {
        name: "link",
        usage: "muster link <id> <https-url> [\"<title>\"] | muster link <id> --remove <n|url>",
        what: "Add an external link to a task (a spec, a PR, a design), or remove one by its number (from 1) or address. Links are https only, up to 2,000 characters, printable ASCII (percent-encode the rest), without a user name or password or a percent-encoded host; a title has no <, > or ;. At most 20 per task. They're in the front matter as links: title <https://…>; <https://…>, the task pane lists them, and a click opens one in your browser. Logged.",
        options: &[("--remove", "the link's number (1 is the first) or its address")],
        examples: &[
            "muster link 0007 https://github.com/sweetlemonai/muster/pull/12 \"The PR\"",
            "muster link 0007 --remove 1",
        ],
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
        usage: "muster ask <id> \"<question>\" [--option \"<text>\"]... [--pick <a|1>] [--no-answer <a|1>] [--as <who>] [--session <id>]",
        what: "Ask a question in a task without stopping: numbered, written under ## Questions, and your session recorded so the answer is pushed to you (the answer hook) or waits in muster inbox. Lists the task's earlier questions, and refuses one that's already answered there. When the answer is a choice, ask with options: 2 to 6, each one line of up to 200 characters, with your pick and what you'll do with no answer. They're written under the question as (a), (b)…, and the owner answers with one click in the app or muster answer --pick.",
        options: &[
            ("--option", "one option, once for each (2 to 6, in order: the first is a)"),
            ("--pick", "the option you recommend, by letter or number"),
            ("--no-answer", "the option you'll take if no answer comes, by letter or number"),
            ("--as", "who's asking (your branch); default $MUSTER_AS, then the folder's git user.name"),
            ("--session", "your session id; default $CLAUDE_CODE_SESSION_ID"),
        ],
        examples: &[
            "muster ask 0007 \"Child webview or a separate window?\" --as wt-a",
            "muster ask 0007 \"Where does Settings open?\" --option \"A window\" --option \"The side pane\" --pick b --no-answer b",
            "muster ask 0007 -- \"--verbose or --quiet by default?\"",
        ],
    },
    Command {
        name: "answer",
        usage: "muster answer <id> <Qn> \"<text>\" | muster answer <id> <Qn> --pick <a|1> [\"<more words>\"]",
        what: "Answer a question in a task. It's written under the question, and the agent that asked gets it pushed into its session. A question asked with options can be answered by picking one: the answer is that option's text, then any words you add. A pick that isn't one of its options is refused, and so is a second answer to a question already answered; any open question can still be answered in words.",
        options: &[("--pick", "answer with this option, by letter or number")],
        examples: &[
            "muster answer 0008 Q2 \"B, the window beside the app\"",
            "muster answer 0008 Q3 --pick b",
            "muster answer 0008 Q3 --pick a \"but keep the old one for a week\"",
        ],
    },
    Command {
        name: "escalate",
        usage: "muster escalate <id> <Qn> [--as capture]",
        what: "Mark a question \"Escalated to owner\": it shows as waiting for you. Capture then adds it to WAITING-FOR-OWNER.md, with its options and the asker's pick when it has them (escalate prints them). Once per question.",
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
        usage: "muster check [--fix-pauses [--dry-run]]",
        what: "Problems in the queue: ids used twice, status not matching the folder, a stage that isn't one, Building with no claim or quiet past the lease (12 h unless changed in the app's Settings), blocked with no reason, waiting on a task that doesn't exist, a link that isn't a plain https link, a backlog note over 1 MB or that's a symlink, a parent that doesn't exist or is at the wrong level, parents in a loop, a for that isn't human or ai, a vision count that doesn't match the task's Vision section, an INDEX.md that shows something the task files no longer say (rebuild with muster index; none yet is fine). Exit 1 if any. A goal past its due date with tasks open, a block from before kinds (read as one from what it waits on until it's blocked again with a kind), a pause from before muster pause (a block whose reason says paused: make it a pause with --fix-pauses), and an INDEX.md only due a refresh (the files changed outside muster, as Drover's answers do), are warnings: shown, without failing. Also lists, without counting them as problems, what waits on the owner's look, lessons from reviews waiting for the owner's yes, Shaping tasks with no vision check yet, and what parallel sessions left behind: a started task whose worktree is gone, a worktree of a task sent back to Ready, and a worktree on a task/<id>- branch whose task is past Testing, each with the command that cleans it. With --fix-pauses, in every queue Muster knows (or the one --queue names): lists each block from before muster pause whose reason says paused (\"paused\", \"owner (paused)\") with its stage, since when and the reason it keeps, then makes each a pause with that reason and time: paused-why is what the reason says besides owner and paused, paused the time of the line that blocked it (else the latest line about the pause, else the last line), and the block is cleared as muster unblock clears it, the stage kept (an old blocked file with no stage is Ready, and goes back to todo/). No session is told. A block that says anything else is never touched, and running it again changes nothing. Exit 1 if one couldn't be converted (it's left as it was).",
        options: &[
            ("--fix-pauses", "make the old paused blocks pauses, in every queue Muster knows"),
            ("--dry-run", "with --fix-pauses: list them and change nothing"),
            ("--as", "with --fix-pauses: who's converting them; default $MUSTER_AS, then the folder's git user.name"),
        ],
        examples: &[
            "muster check",
            "muster check --queue braindump",
            "muster check --fix-pauses --dry-run",
            "muster check --fix-pauses --as owner",
        ],
    },
    Command {
        name: "index",
        usage: "muster index [--check]",
        what: "Rebuild INDEX.md, the queue's master list: every goal with its features and tasks, then the tasks under none by stage, the backlog notes, and what's done, each with its stage, the tasks it still waits on, priority, type, estimate (agent time, test time, tokens), how many comments it has and a link to its file. Muster rewrites it after every change it makes, so this is for after a hand edit. It's generated: never edit it by hand.",
        options: &[(
            "--check",
            "write nothing; exit 1 if INDEX.md is missing or isn't exactly what muster would write now",
        )],
        examples: &["muster index", "muster index --check --queue drover"],
    },
    Command {
        name: "add",
        usage: "muster add \"<one line>\" [--body <text>|-] [--as <who>]",
        what: "Drop a one-liner in the backlog, to be shaped into a task later. It's a file in the backlog folder (inbox/ today), named by the time and the line; prints its name. No id, no front matter. Line breaks in the line become spaces, and it's refused over 200 characters; the body, if any, is Markdown up to 50,000. Checked like a claim: with $MUSTER_ROOT at another project's queue, pass --queue to mean it.",
        options: &[
            ("--body", "more detail in Markdown; \\n is a line break; - reads it from stdin"),
            ("--as", "who found it; default $MUSTER_AS, then the folder's git user.name"),
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
        usage: "muster file <note-file> \"<title>\" [--type code|bug|chore|tech-debt|spike|design|docs|research] [--priority high|normal|low] [--tags a,b] [--time 2h] [--test-time 30m] [--tokens 400k] [--touches <folders,globs>] --body <text>|-",
        what: "File a shaped backlog note as a task: the next id, in Shaping, with your body after a \"From the backlog\" line quoting the note. The note moves into the task's attachments/<id>/, so nothing is lost; all under the queue's lock, so a note is filed once. Then muster pass <id> --from shaping puts it in Ready (it needs acceptance criteria).",
        options: &[
            ("--type", "code (default), bug, chore, tech-debt, spike, design, docs or research"),
            ("--priority", "high, normal (default) or low"),
            ("--tags", "comma-separated tags"),
            ("--time", "estimated agent time: 45m, 2h, 1.5d"),
            ("--test-time", "estimated test time, the QA agent plus your check: 30m, 1h"),
            ("--tokens", "estimated tokens: 150k, 1.2M"),
            ("--touches", "where in the repo it works, folders and globs, comma-separated: what muster ready --parallel keeps apart"),
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
        name: "vision",
        usage: "muster vision <id> [--json] | --record - | --resolve V2 \"<how>\"   ·   muster vision --all [--json]",
        what: "Check a task against the project's one picture (docs/VISION.md, docs/decisions/, docs/DESIGN.md) and the tasks it touches. Muster runs no agent: muster vision <id> prints the vision brief for one, with the task quoted as data, the picture files that exist, the open tasks it touches (sharing two title words, named in its text, its parent and depends), what to look for (conflicts, overlaps, gaps, the cost of pivoting) and the JSON to return. Writes nothing. --record - reads that JSON from stdin (or takes it as its value): the task gets a ## Vision section under its write-up (above its Comments and Log), replacing the last one, with each conflict numbered V1, V2… and both sides quoted; the front matter's vision: <n> open; and a log line. A conflict whose fix changes direction is also escalated to the owner, as one question per record with the cost of pivoting. --resolve V2 \"<how>\" settles one (one line, 500 characters). While any is open, Shaping → Ready is refused; a task never checked isn't held back. --all lists the inputs for every open task.",
        options: &[
            ("--json", "the brief, the record or --all as JSON, for Drover and scripts"),
            ("--record -", "the agent's result, as JSON on stdin; malformed JSON is refused with what's wrong"),
            ("--resolve V2", "mark that conflict resolved, with how after it"),
            ("--all", "every open task's inputs: its vision state and the tasks it touches"),
        ],
        examples: &[
            "muster vision 0031",
            "muster vision 0031 --json --queue drover",
            "pbpaste | muster vision 0031 --record - --as wt-shape",
            "muster vision 0031 --resolve V2 \"0019 keeps depends:; 0024 derives blocked by tasks from it\"",
            "muster vision --all",
        ],
    },
    Command {
        name: "review",
        usage: "muster review <id> [--json] | --record - | --fixed R1 [\"<how>\"] | --dispute R1 \"<why>\" | --settle R1 confirmed|false-positive|wont-fix|open \"<why>\"",
        what: "The review of record for a task started on its own branch: in Testing, the owner's review engine (/deep-review, its lenses and verifiers, its tier picked from the diff's size and risk) reviews the branch, the builder fixes or disputes each finding back in Building, up to 3 rounds, then the owner. Muster runs no agent: muster review <id> prints the brief for the next round (where to run it, the range: the whole branch since it left main, then only the commits since the last round; the task quoted as data, the builder's answers to re-check, the owner's lessons for its area and what not to flag, the kinds of mistake already named, and the JSON to return). Writes nothing. --record - reads that JSON from stdin (or takes it as its value), from a session that didn't build it (or the owner): head must be the branch's tip; every finding the builder answered needs an answer (an accepted fix is labelled confirmed, an accepted dispute false-positive, the rest open again); new findings are numbered on (R1, R2… across rounds); the round is kept in the queue's reviews/<id>.json, the task gets a ## Review section under its write-up (above its Comments and Log) and review: round <n> · <open> open · <answered> answered in its front matter, and a log line with the tokens it took. Then, with anything but nits open, rounds 1 and 2 return it to Building for its builder, and round 3 blocks it on the owner instead of a fourth round; clean, the merge may go ahead. --fixed R1 and --dispute R1 \"<why>\" are the builder's answers, in Building (the holder or the owner; a dispute says why, 500 characters); Building → Testing waits until every open finding but a nit is answered. --settle is the owner's: any finding, any state, also to change a label. Merging a started task (muster pass <id> --from testing) needs the review clean at the branch's tip; the owner may merge without one, and the merge report says which. A finding's kind (a short slug) recurring confirmed on 2 tasks proposes a lesson (muster lessons).",
        options: &[
            ("--json", "the brief or the record's result as JSON"),
            ("--record -", "the reviewer's result, as JSON on stdin; malformed JSON, a head that isn't the branch's tip, a missing answer, or a finding without a confidence or under 75, is refused, and nothing changes"),
            ("--fixed R1", "the builder: fixed, with how after it (optional)"),
            ("--dispute R1", "the builder: it's wrong, with why after it"),
            ("--settle R1", "the owner: confirmed, false-positive, wont-fix or open, with why"),
        ],
        examples: &[
            "muster review mu-27",
            "pbpaste | muster review mu-27 --record - --as muster/qa-0027",
            "muster review mu-27 --fixed R1 \"capped the read at 1 MB\" --as muster/wt-0027",
            "muster review mu-27 --dispute R2 \"the timeout is set two lines up\" --as muster/wt-0027",
            "muster review mu-27 --settle R2 false-positive \"the pipe is bounded\" --as owner",
        ],
    },
    Command {
        name: "lessons",
        usage: "muster lessons [--json] | muster lessons accept <L1,L2|--all> | muster lessons reject <L1,L2> \"<why>\"",
        what: "Lessons from reviews: when a kind of mistake is confirmed on 2 tasks, muster proposes a lesson for builders (one line, the rule, a bad and a good example, the findings it came from, the folders it applies to); when a kind is dismissed as a false positive on 2 tasks, one for reviewers (what not to flag). Nothing becomes an instruction without the owner: accept or reject the proposals in one batch, with --as owner. Accepted lessons for builders go into muster start's brief for the tasks whose touches: overlap their folders (every lesson, for a task without touches:), and both kinds into the review brief. At most 40 are live: accepting more expires the ones that mattered least recently, and one with no finding of its kind and no acceptance for 60 days expires on its own (a new finding of its kind brings it back). A rejected kind isn't proposed again, nor one the cap expired until its kind is found again. The app's Accept all accepts the lessons it shows, by id, never one proposed since. Kept in the queue's reviews/lessons.json, and readable as lessons.md beside the task files (never edit either by hand).",
        options: &[
            ("--json", "the listing or the decision as JSON"),
            ("--all", "accept: every proposal (minus any you reject in the same batch)"),
        ],
        examples: &[
            "muster lessons",
            "muster lessons accept --all --as owner",
            "muster lessons accept L3,L5 --as owner",
            "muster lessons reject L4 \"taste, not a defect\" --as owner",
        ],
    },
    Command {
        name: "projects",
        usage: "muster projects [--json] | muster projects set <name> gate \"<command>\" | muster projects set <name> prefix <p>",
        what: "Every queue Muster knows, each marked found (at ~/playground/<project>/tasks, or one level deeper) or added (in the app), with its prefix and gate, and the label and category the app shows it with, when set (Settings › Projects): a label is only what the app calls it, so commands keep taking the name that starts the line. A prefix is what its tasks' keys start with (mu in mu-12): 2 to 4 lowercase letters, stored in its queue's README.md (prefix: mu), or for a repo's .muster/ in its config.toml (prefix = \"mu\"). Without one it's the first 2 letters of the project's name, or 3 or 4 when another project's start the same. projects set <name> prefix stores one, refused when it isn't 2 to 4 lowercase letters or another project has it; the files keep their ids, so nothing moves, but the old keys stop finding its tasks. projects set <name> gate stores the project's gate command in the app's projects.json: what a merge runs in a task's worktree (muster pass <id> --from testing). It's split into words like a shell would, quotes and all, but no shell runs it, so a pipe or a cd is just a word: point it at a script in the repo (bin/gate) for anything more. \"\" clears it; with no gate, nothing merges.",
        options: &[("--json", "the list, for scripts")],
        examples: &["muster projects", "muster projects set muster gate bin/gate", "muster projects set drover gate \"pnpm check\"", "muster projects set braindump prefix bd"],
    },
    Command {
        name: "init",
        usage: "muster init [<folder>] [--local|--committed] [--prefix <p>]",
        what: "In a git repo (its main checkout or any worktree of it; the current folder when none is named): create the repo's queue, .muster/ in the main checkout, with config.toml (its prefix in it), .gitignore, a README with the rules, tasks/, backlog/, attachments/ and INDEX.md. Task files live in tasks/ and never move. Local by default: it stays on this Mac. Sessions in the repo and its worktrees need no setup of their own: muster finds it from any folder of the repo, and Muster's rule (rules/muster-queue.md) and answer hook (bin/answer-hook.py) reach them through the personal layer. Refuses if .muster/ is there already, and in a repo without a main checkout (a bare repo's worktree, a submodule). Outside a repo it creates <folder>/tasks, the old layout: status folders, README with the rules and its prefix, the tool stand-ins and its own git, never over an existing tasks/; only for a folder named on purpose: with none, it writes nothing and says to run it in the repo. Either way it refuses a prefix that isn't 2 to 4 lowercase letters or that another project has.",
        options: &[
            ("--local", "the default: .muster/.gitignore keeps all but config.toml out of git, and every worktree of the repo uses the main checkout's .muster/"),
            ("--committed", "commit the queue with the code. For now every claim and pass changes tracked files in the main checkout (merges pass over them), until the live state moves to .git/muster/"),
            ("--prefix", "what its tasks' keys start with (mu in mu-12); default the first 2 letters of the project's name, more when another project has them"),
        ],
        examples: &[
            "muster init",
            "muster init --committed",
            "muster init ~/playground/braindump/main",
            "muster init ~/playground/braindump",
            "muster init ~/playground/germ/main --prefix gm",
        ],
    },
    Command {
        name: "migrate",
        usage: "muster migrate <old-queue> <repo> [--local|--committed] [--dry-run]",
        what: "Copy an old queue (<project>/tasks, with status folders or flat) into the repo's .muster/, local by default. Every task file goes to tasks/ with stage: set from its old stage or status and status: dropped, the rest kept byte for byte (a front matter it can't read as it is gets its byte-order mark and CRLF line ends made plain); the backlog folder's notes and dropped notes to backlog/; attachments/, mail/, the review of record's reviews/ and lessons.md, and WAITING-FOR-OWNER.md as they are; the old README's front matter into the new README. Writes config.toml, .gitignore, INDEX.md, tasks.json, and migrated-from.txt with the old queue's last commit, and gives the new queue the old one's gate, name, label, category, colour, place in the sidebar's order, hidden and Your check limit from projects.json. Prints what it copied and what it didn't. Copies, never moves: the old queue is left as it was, to retire once muster check is clean. Refuses while an agent holds a task in Building or Testing (it lists them), when .muster/ is there already, when the repo and the old queue overlap, on a folder it can't read, on a task file without front matter it can read, and on two names that would be one file on this Mac (case or accents).",
        options: &[
            ("--local", "the default: the queue stays on this Mac, as init"),
            ("--committed", "commit the queue with the code, as init --committed"),
            ("--dry-run", "print the plan and write nothing"),
        ],
        examples: &[
            "muster migrate ~/playground/muster/tasks ~/playground/muster/main --dry-run",
            "muster migrate ~/playground/muster/tasks ~/playground/muster/main",
        ],
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
    ("<id>", "a task, by its key (mu-12, MU-12: its project's prefix and number) or its number (12, 0012). A key with another project's prefix runs in that project's queue, wherever you are; with a --queue that names another project it's refused. Keys work in --parent, depends and block --on too, for tasks of the same project"),
    ("--queue <name|folder>", "which queue; default $MUSTER_ROOT, then the current folder's: its repo's .muster/ (the main checkout's, from any worktree), else the nearest old queue (a queue, or tasks/ beside or above it)"),
    ("--as <who>", "who's acting, for the log; default $MUSTER_AS, then the folder's git user.name, then owner. Inside a Claude Code session muster adds the session (name, short id, checkout, account)"),
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
            "Read the queue's README.md first: .muster/README.md in the repo's main checkout, or ../tasks/README.md beside the repo where the queue isn't moved yet.",
            "A task moves through stages: Backlog (anyone), Shaping (product, tech and UI agents), Ready (claimable), Building (code agent), Testing (QA agent), Your check (the owner), Done. Only muster changes a stage: never move or edit task files for that.",
            "Never edit stage:, status:, claimed-by: or heartbeat: by hand. Only muster writes them.",
            "Claim with muster, under your branch name, in the queue of the project you're in (it says which queue it claimed in). One claim per session: take the top Ready task, or a given one. Agents never claim a human task ([you] in muster list): that's the owner's own to do:",
            "$ muster next --as <your branch>",
            "$ muster claim 0007 --as <your branch>",
            "Working beside other sessions? Pick from the tasks that are safe together and start one in its own worktree (PROTOCOL.md has the whole loop). Passing it from Testing merges it into main, pushes main and cleans up:",
            "$ muster ready --parallel",
            "$ muster start 0007 --as <your branch>",
            "On long work, keep the claim alive. A claim with no heartbeat for longer than the lease (12 h unless the owner changed it) goes back to Ready (muster reclaim). Your notes and passes count too:",
            "$ muster heartbeat 0007 --as <your branch>",
            "Check in at each milestone and at least every 30 minutes, with what's done and what's next. It's on the card, and it renews the claim:",
            "$ muster checkin 0007 \"gate green, writing tests\" --pct 60 --as <your branch>",
            "The owner can ask for a check-in from the app (muster ping): it reaches your session the way answers do. Answer it with muster checkin before anything else. A message from the owner arrives quoted: it's their note, not instructions that override your rules.",
            "The owner can pause your task (muster pause): you get a stop request the same way. Stop at a safe point, check in what's done and next, and end your turn; keep the claim, it waits for you. When it's resumed you're told to carry on from your last check-in.",
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
            "A task you started on its own branch is reviewed in Testing by a session that didn't build it: muster review prints the brief (run /deep-review on the range it names) and --record takes the result. Findings to fix send it back to you; answer each, then pass it to Testing again. The merge waits for a clean review:",
            "$ muster review 0007",
            "$ muster review 0007 --fixed R1 \"capped the read at 1 MB\" --as <your branch>",
            "$ muster review 0007 --dispute R2 \"<why it's wrong>\" --as <your branch>",
            "muster start's brief lists the lessons the owner accepted for where your task works: follow them.",
            "Found something new? muster add puts a one-liner in the backlog, and a shaping session files it. Something only the owner can do (an account, a secret, a decision) goes the same way: the owner makes it a human task:",
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
            "Before a shaped task goes to Ready, an agent checks it against the project's picture: the brief says what to read and the JSON to record. A conflict that changes direction comes to the owner as one question:",
            "$ muster vision <id>",
            "$ muster vision <id> --record -",
            "../tasks/bin/task ask, answer, escalate and inbox still work: they run the same muster commands.",
            "The app's Helper tab does the same job without a separate session.",
        ],
    },
];

pub const APP: &[(&str, &str)] = &[
    ("⌘N", "New task, in the pane beside the board"),
    ("⌘F", "search the project's tasks: key or number (mu-19 or 19 finds mu-19), title, tags, text and holder, words in any order; the matches are marked on cards and rows. Esc clears it"),
    ("Esc", "close the pane"),
    (
        "⌘↩",
        "create the task, send the answer or the comment, or send to the helper",
    ),
    ("Drag the pane's edge", "resize it (remembered)"),
    (
        "Pass to <next stage>",
        "in a task's pane: hand it on (from Building it asks for the gate's result line); in Your check the button reads Accept and marks it Done",
    ),
    (
        "Accept all",
        "in Your check's Just accept group (the board and Waiting for you): accepts each task there with nothing of yours left to try, as Accept does, after a second click that names them; each is logged",
    ),
    (
        "Start · Done",
        "in your own task's pane (Human, for: human): Start moves it from To do to Doing, Done finishes it; no gate line, no Testing, no Your check",
    ),
    (
        "Your own tasks",
        "on the same board as the agents' tasks, hatched in gold with the You ribbon, in every card look: To do in Ready, Doing in Building, Done in Done. Who above the Board, Table and Timeline shows All · You · Agents (remembered)",
    ),
    (
        "You",
        "the gold ribbon on a human task's card, row or graph node, wherever human and agents' tasks share a view",
    ),
    (
        "needs you",
        "the orange badge on a card waiting on you, wherever it waits: in Your check, a question escalated to you, blocked on you or by a permission, or its session waiting for input in Claude Code. With your own tasks to do and doing, it's the count in the sidebar and on Waiting for you",
    ),
    (
        "working · waiting for input in Claude Code · session ended",
        "under who's on a card in Building or Testing, and in its pane: what Claude Code says the holder's session is doing (claude agents --json, read every 15 s while the window shows). Waiting for input is yours to answer in Claude Code, so it counts as needs you, and Ask for a check-in waits for it. When Claude Code can't say, or the session holds other open tasks too, the card shows its check-in lines as before; Settings > Agents says whether the states are read",
    ),
    (
        "Send back",
        "in a task's pane: back one stage, or to any earlier one, with the reason",
    ),
    (
        "Block · Unblock",
        "in a task's pane: pick why on the chips (tasks, you, permission, question, outside) and say what it waits on; it keeps its stage. The card, table, timeline and pane say why, the kind first",
    ),
    (
        "Pause · Resume",
        "under a card from Backlog through Testing, and in a task's pane beside its stage: one click, no dialog (the pane takes an optional why). A paused task keeps its stage and claim but goes to the Paused lane, out of the working columns, its card saying where it was, since when and why, never red; nobody claims or passes it until Resume puts it back in its column. In Building or Testing its session is asked to stop at a safe point, and told to carry on when you resume",
    ),
    (
        "Stage picker",
        "in a task's pane: your override, any stage. Use it when someone else holds the task, since only they can pass it",
    ),
    (
        "Release",
        "in a task's pane, when its card says quiet 12 h+ (or the lease set in Settings): the holder hasn't been heard from for longer than the lease, so it goes back to Ready, logged as reclaimed",
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
        "⚠ 2 vision",
        "on a card: open vision conflicts (the task doesn't fit the project's picture); it can't pass from Shaping to Ready until they're resolved",
    ),
    (
        "Resolve · Copy vision brief",
        "in a task's Vision section: settle a conflict with how (one line), or copy the brief a session checks the task against the picture with",
    ),
    (
        "review 1/3 · 2 to fix",
        "on a card in Building or Testing: where its review of record stands (the round, findings for the builder to fix or dispute, answers for the next round to re-check, or clean); not reviewed yet on a started task in Testing. The merge waits for a clean review",
    ),
    (
        "Review · Copy review brief",
        "in a task's pane: every finding with its state and the builder's and the reviewer's words on it, each round, and the brief a session that didn't build it runs the next round with",
    ),
    (
        "Lessons to approve · Accept all · Reject",
        "at the top of Waiting for you: kinds of mistake the reviews kept finding, proposed as rules for the next agents. Accept all takes the batch; Reject one with why first. They count in the needs-you count and the Dock badge until you decide",
    ),
    (
        "Edit",
        "in a task's pane, beside the description: edit the write-up in place, ⌘S to save, Esc to cancel (twice when you've typed). Refused if the file changed since you opened it, or while someone else holds it in Building (add a note instead); your text stays in the box",
    ),
    (
        "Type",
        "in a task's pane: the chip shows its type; click it to lay the types out and pick one",
    ),
    (
        "Links",
        "in a task's pane: its external links (muster link adds them); a click opens one in your browser, https only",
    ),
    (
        "Comments",
        "in a task's pane, under the description: the thread, oldest first, and a box to add yours (⌘↩ sends). While it holds typed text the pane stays open, and Esc twice throws the text away. Anyone may comment at any stage, while someone else holds it too",
    ),
    (
        "Board · Table · Timeline · Graph",
        "four views of the same tasks, with the same search and filters",
    ),
    (
        "Filters",
        "chips by stage, type, priority, tag, who holds it, flags (blocked, paused, needs you, open questions) and dates; any chip within a group, every group together. The line under them reads what's active, each with ✕, how many tasks the filters hide, and Clear all",
    ),
    (
        "Sort · Save view",
        "on the board, the Sort chips order each column (inside each Group by band too); + Save view keeps the search, filters and sort under a name, as a chip, for this project",
    ),
    (
        "Group by None · Feature · Goal",
        "above the board and table: one band per feature or goal, with its progress and the estimate left; tasks without one in the last band (remembered)",
    ),
    (
        "Paste or drop",
        "screenshots into New task, a task's note box, or the helper",
    ),
];

/// The Commands page's groups: every command in exactly one (a test checks it).
pub const GROUPS: &[Group] = &[
    Group {
        title: "Tasks",
        intro: "Make a task, read it, and change what it says: its fields, its write-up, its log, its comments, its links and its files.",
        commands: &["new", "list", "show", "set", "edit", "note", "comment", "link", "attach"],
        examples: &[],
    },
    Group {
        title: "Stages",
        intro: "Move a task on to the next stage, send it back, mark it blocked, or pause it. Only these commands change a stage: never move or edit a task file for that. See [Stages and roles](02-concepts.md#stages-and-roles).",
        commands: &["pass", "return", "block", "unblock", "pause", "resume", "stage", "tick", "untick", "move"],
        examples: &[],
    },
    Group {
        title: "Claims",
        intro: "Who's on a task. A coding session claims a Ready task, keeps the claim alive while it works, and checks in to say how far it got. See [Claims, leases and check-ins](02-concepts.md#claims-leases-and-check-ins). Sessions that work side by side each start a task in its own worktree: `muster ready --parallel` says which tasks are safe together, `muster start` claims one with a branch and a worktree, passing it from Testing merges it into main and cleans up, and `muster release --clean` gives it up.",
        commands: &["claim", "next", "heartbeat", "checkin", "ping", "reclaim", "ready", "start", "release"],
        examples: &[],
    },
    Group {
        title: "Review",
        intro: "The review of record for a task started on its own branch: the owner's review engine reviews it in Testing, its builder fixes or disputes what it finds, up to 3 rounds, and the merge waits for a clean review. Findings that keep coming back become lessons for the next agents, once the owner accepts them. See [The review of record](02-concepts.md#the-review-of-record).",
        commands: &["review", "lessons"],
        examples: &[],
    },
    Group {
        title: "Questions",
        intro: "Ask in a task without stopping. The answer is written into the task and pushed back to the session that asked. See [Questions and answers](02-concepts.md#questions-and-answers).",
        commands: &["ask", "answer", "escalate", "inbox"],
        examples: &[],
    },
    Group {
        title: "Backlog",
        intro: "One-line ideas that aren't tasks yet, shaping them into tasks, and checking a shaped task against the project's vision before it goes to Ready. See [The backlog and shaping](02-concepts.md#the-backlog-and-shaping) and [The vision check](02-concepts.md#the-vision-check).",
        commands: &["add", "shape", "file", "drop", "vision"],
        examples: &[],
    },
    Group {
        title: "Levels",
        intro: "Goals and features group tasks. They have no commands of their own: `muster new` takes a level, a parent and a goal's due date, and `muster set` changes them, and what a task depends on. See [Goals, features and tasks](02-concepts.md#goals-features-and-tasks).",
        commands: &[],
        examples: &[
            "muster new \"Ship 1.0\" --level goal --due 2026-12-01",
            "muster new \"Stage track\" --level feature --parent 0030",
            "muster set 0032 parent 0031",
            "muster set 0018 depends 0014,0016",
        ],
    },
    Group {
        title: "Projects and the queue",
        intro: "Find the queues Muster knows, start one for a project (in its repo's `.muster/`), move an old `<project>/tasks` queue into it, check a queue for problems, and rebuild its master list.",
        commands: &["projects", "init", "migrate", "check", "index", "help"],
        examples: &[],
    },
];

pub fn help() -> Help {
    Help {
        commands: COMMANDS,
        common: COMMON,
        session: SESSION,
        app: APP,
        commands_page: commands_page(),
    }
}

/// Text from the help data as Markdown that reads the same: `<id>` stays text on GitHub, and
/// `_`, `*`, backticks and brackets don't start formatting.
fn md_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if matches!(ch, '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '|') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// A command line as its own code block, so the app copies it with a click.
fn code_block(out: &mut String, line: &str) {
    out.push_str(&format!("\n```\n{line}\n```\n"));
}

/// One command on the Commands page: what it does, its usage, options and examples.
fn command_md(out: &mut String, c: &Command) {
    out.push_str(&format!("\n### muster {}\n\n{}\n", c.name, md_text(c.what)));
    code_block(out, c.usage);
    if !c.options.is_empty() {
        out.push_str("\n| Option | What it does |\n| --- | --- |\n");
        for (k, v) in c.options {
            out.push_str(&format!(
                "| `{}` | {} |\n",
                k.replace('|', "\\|"),
                md_text(v)
            ));
        }
    }
    out.push_str("\nExamples:\n");
    for e in c.examples {
        code_block(out, e);
    }
}

/// The manual's Commands page (`docs/manual/05-commands.md`, and the Help tab's): every command,
/// grouped, then the options every command takes and how to use Muster in a Claude Code session.
pub fn commands_page() -> String {
    let mut out = String::from(
        "# Commands\n\nEvery `muster` command, grouped by the job it does. This page is made from the command line's own help, so it always matches the `muster` you have: in a terminal, `muster help` lists the commands and `muster help <command>` shows one. In the app, click a command line to copy it.\n",
    );
    for g in GROUPS {
        out.push_str(&format!("\n## {}\n\n{}\n", g.title, g.intro));
        for name in g.commands {
            if let Some(c) = COMMANDS.iter().find(|c| c.name == *name) {
                command_md(&mut out, c);
            }
        }
        if !g.examples.is_empty() {
            out.push_str("\nExamples:\n");
            for e in g.examples {
                code_block(&mut out, e);
            }
        }
    }
    out.push_str("\n## On every command\n\n| Option | What it does |\n| --- | --- |\n");
    for (k, v) in COMMON {
        out.push_str(&format!(
            "| `{}` | {} |\n",
            k.replace('|', "\\|"),
            md_text(v)
        ));
    }
    out.push_str("\n## Inside a Claude Code session\n");
    for t in SESSION {
        out.push_str(&format!("\n### {}\n", t.title));
        for l in t.lines {
            match l.strip_prefix("$ ") {
                Some(cmd) => code_block(&mut out, cmd),
                None => out.push_str(&format!("\n{}\n", md_text(l))),
            }
        }
    }
    out
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
    fn pause_is_described_as_the_paused_lane_everywhere() {
        // mu-59 moved a paused task to its own lane; mu-68 makes every text say so.
        let summary = text()
            .lines()
            .find(|l| l.trim_start().starts_with("pause "))
            .expect("pause in the summary list")
            .to_string();
        let full = command_text("pause").expect("muster help pause");
        let (_, app) = APP
            .iter()
            .find(|(name, _)| *name == "Pause · Resume")
            .expect("the app glossary's Pause · Resume");
        for (place, said) in [
            ("summary", summary.as_str()),
            ("help pause", full.as_str()),
            ("app glossary", app),
        ] {
            assert!(
                said.contains("Paused lane"),
                "{place} should name the Paused lane: {said}"
            );
            for stale in ["where it is", "keeps its column", "keeps its place"] {
                assert!(
                    !said.contains(stale),
                    "{place} still says {stale:?}: {said}"
                );
            }
        }
    }

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
            "checkin",
            "ping",
            "reclaim",
            "ready",
            "start",
            "release",
            "pass",
            "return",
            "block",
            "unblock",
            "pause",
            "resume",
            "stage",
            "set",
            "edit",
            "note",
            "comment",
            "link",
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
            "vision",
            "review",
            "lessons",
            "check",
            "index",
            "projects",
            "init",
            "migrate",
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
        assert_eq!(COMMANDS.len(), 45);
    }

    #[test]
    fn every_command_is_on_the_commands_page_once_in_its_group() {
        let mut seen: Vec<&str> = Vec::new();
        for g in GROUPS {
            for name in g.commands {
                assert!(
                    COMMANDS.iter().any(|c| c.name == *name),
                    "{name} in {} isn't a command",
                    g.title
                );
                assert!(!seen.contains(name), "{name} is in two groups");
                seen.push(name);
            }
            for e in g.examples {
                assert!(
                    COMMANDS.iter().any(|c| c.examples.contains(e)),
                    "{e} isn't one of the commands' own examples"
                );
            }
            assert!(
                !g.commands.is_empty() || !g.examples.is_empty(),
                "{} shows nothing",
                g.title
            );
        }
        for c in COMMANDS {
            assert!(seen.contains(&c.name), "{} is in no group", c.name);
        }
        let page = commands_page();
        for c in COMMANDS {
            let heading = format!("\n### muster {}\n", c.name);
            assert_eq!(page.matches(&heading).count(), 1, "{heading}");
            for e in c.examples {
                assert!(page.contains(&format!("\n```\n{e}\n```\n")), "{e}");
            }
        }
        let titles: Vec<&str> = page.lines().filter_map(|l| l.strip_prefix("## ")).collect();
        assert_eq!(
            titles,
            [
                "Tasks",
                "Stages",
                "Claims",
                "Review",
                "Questions",
                "Backlog",
                "Levels",
                "Projects and the queue",
                "On every command",
                "Inside a Claude Code session"
            ]
        );
        assert!(page.starts_with("# Commands\n"));
        // The Help tab gets it with the rest of the help.
        assert_eq!(help().commands_page, page);
    }

    #[test]
    fn the_commands_page_keeps_the_help_text_as_text() {
        let page = commands_page();
        // `<id>` would be an HTML tag on GitHub, and `_` or `*` would start emphasis.
        assert!(page.contains("it creates \\<folder\\>/tasks"), "{page}");
        assert!(page.contains("CLAUDE\\_CODE\\_SESSION\\_ID"), "{page}");
        // A `|` inside an option's name would end its table cell.
        assert!(page.contains("| `--queue <name\\|folder>` |"), "{page}");
        // Command lines stay as typed, in their own blocks.
        assert!(page.contains("\n```\nmuster new \"<title>\" [--type"));
        assert!(page.contains("\n```\n! muster list\n```\n"));
    }

    #[test]
    fn pass_and_stage_say_the_tester_isnt_the_builder() {
        let pass = command_text("pass").unwrap();
        for words in [
            "didn't build it",
            "own --as",
            "owner's override",
            "builder's session",
        ] {
            assert!(pass.contains(words), "{words}: {pass}");
        }
        assert!(command_text("stage").unwrap().contains("skip its tester"));
        assert!(command_text("move")
            .unwrap()
            .contains("whoever built the task"));
    }

    #[test]
    fn the_commands_page_in_the_docs_is_what_muster_makes() {
        // docs/manual/05-commands.md, so the manual reads the same on GitHub as in the app.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("docs")
            .join("manual")
            .join("05-commands.md");
        let page = commands_page();
        if std::env::var_os("MUSTER_WRITE_MANUAL").is_some() {
            std::fs::write(&path, &page).unwrap();
        }
        let file = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            file == page,
            "docs/manual/05-commands.md isn't what help.rs makes: run `MUSTER_WRITE_MANUAL=1 cargo test commands_page` in app/src-tauri"
        );
    }

    #[test]
    fn types_test_time_links_and_comments_are_documented() {
        let new = command_text("new").unwrap();
        for words in [
            "code|bug|chore|tech-debt|spike|design|docs|research",
            "--test-time",
            "muster add",
        ] {
            assert!(new.contains(words), "{words}: {new}");
        }
        assert!(!new.contains("idea]"), "{new}");
        assert!(command_text("file").unwrap().contains("--test-time"));
        assert!(command_text("set").unwrap().contains("test-time"));
        let link = command_text("link").unwrap();
        for words in [
            "https only",
            "--remove",
            "2,000",
            "<, > or ;",
            "links: title <https://…>",
        ] {
            assert!(link.contains(words), "{words}: {link}");
        }
        let comment = command_text("comment").unwrap();
        for words in [
            "## Comments",
            "--body",
            "4,000",
            "stage change",
            "someone else holds",
        ] {
            assert!(comment.contains(words), "{words}: {comment}");
        }
        assert!(command_text("check").unwrap().contains("https"));
        let app: String = APP.iter().map(|(k, v)| format!("{k} {v}\n")).collect();
        for words in ["Links", "Comments", "the comment", "Type"] {
            assert!(app.contains(words), "{words}: {app}");
        }
    }

    #[test]
    fn vision_says_what_it_reads_records_and_holds_back() {
        let t = command_text("vision").unwrap();
        for words in [
            "docs/VISION.md",
            "docs/decisions/",
            "docs/DESIGN.md",
            "--json",
            "--record -",
            "--resolve V2",
            "--all",
            "## Vision",
            "vision: <n> open",
            "escalated to the owner",
            "cost of pivoting",
            "Shaping → Ready",
            "Writes nothing",
        ] {
            assert!(t.contains(words), "{words}: {t}");
        }
        assert!(command_text("check").unwrap().contains("no vision check"));
        assert!(command_text("pass").unwrap().contains("vision conflict"));
        assert!(text().contains("      muster vision <id>\n"), "{}", text());
        let app: String = APP.iter().map(|(k, v)| format!("{k} {v}\n")).collect();
        for words in ["⚠ 2 vision", "Resolve", "Copy vision brief"] {
            assert!(app.contains(words), "{words}: {app}");
        }
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
    fn human_tasks_are_documented() {
        // 0022: the owner's own tasks, made by the owner, never taken by agents.
        let new = command_text("new").unwrap();
        for words in [
            "--human",
            "only the owner",
            "muster add",
            "muster new \"Buy the domain\" --human --as owner",
        ] {
            assert!(new.contains(words), "{words}: {new}");
        }
        let set = command_text("set").unwrap();
        for words in ["|for>", "human or ai", "muster set 0041 for human"] {
            assert!(set.contains(words), "{words}: {set}");
        }
        for name in ["claim", "next"] {
            let c = command_text(name).unwrap();
            assert!(c.contains("human task"), "{name}: {c}");
        }
        let pass = command_text("pass").unwrap();
        for words in ["human task", "To do", "Doing", "Done", "no gate line"] {
            assert!(pass.contains(words), "{words}: {pass}");
        }
        assert!(command_text("list").unwrap().contains("[you]"));
        assert!(command_text("check").unwrap().contains("for"));
        let t = text();
        assert!(t.contains("never claim a human task"), "{t}");
        let app: String = APP.iter().map(|(k, v)| format!("{k} {v}\n")).collect();
        // One board (owner, 2026-10-13): the owner's own tasks hatched among the agents', the Who
        // chips, and what waits on them as needs-you cards; no Human board or lane of its own.
        for words in [
            "Your own tasks",
            "same board",
            "hatched",
            "All · You · Agents",
            "Start",
            "You",
            "needs you",
            "Your check",
            "escalated",
            "blocked on you",
        ] {
            assert!(app.contains(words), "{words}: {app}");
        }
        for gone in ["Human board", "AI board", "From the agents"] {
            assert!(!app.contains(gone), "{gone}: {app}");
            assert!(!text().contains(gone), "{gone} in the commands' help");
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
        assert!(t.contains(
            "      muster checkin 0007 \"gate green, writing tests\" --pct 60 --as <your branch>\n"
        ));
        assert!(command_text("checkin").unwrap().contains("--pct"));
        assert!(!t.contains("$USER"), "the login name is never who's acting");
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
        // list's search, filters and sort (task 0032), and the app's search key.
        let list = command_text("list").unwrap();
        for opt in [
            "--search",
            "--stage",
            "--type",
            "--tag",
            "--holder",
            "--blocked",
            "--sort",
        ] {
            assert!(list.contains(opt), "{opt}: {list}");
        }
        assert!(APP.iter().any(|(k, _)| *k == "⌘F"));
        assert!(command_text("frob").is_none());
        assert!(command_text("pass").unwrap().contains("--from"));
        // Levels: the flags of new, the keys of set, and what check looks for.
        let new = command_text("new").unwrap();
        for flag in ["--level", "--parent", "--due"] {
            assert!(new.contains(flag), "{flag}: {new}");
        }
        let set = command_text("set").unwrap();
        assert!(set.contains("level|parent|due"), "{set}");
        assert!(set.contains("muster set 0031 parent \"\""), "{set}");
        assert!(command_text("check").unwrap().contains("parent"));
        // The master list (0018): what index does, its --check, and that check reports a stale one.
        let index = command_text("index").unwrap();
        for words in ["INDEX.md", "--check", "never edit", "exit 1"] {
            assert!(index.contains(words), "{words}: {index}");
        }
        assert!(command_text("check").unwrap().contains("INDEX.md"));
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
            "Accept all",
        ] {
            assert!(app.contains(word), "{word}: {app}");
        }
    }
}
