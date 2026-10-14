# Muster: as built (2026-10-12, updated with the app)

What exists, how it fits together, how it was checked, and what's left. Newest state, not
history (history is in `DOGFOOD.md` and the queue's git log).

## In one paragraph

A **queue** is a folder of Markdown task files, one per task: the repo's `.muster/` (task 0005),
where files sit in `tasks/` and never move and `stage:` says where each task is, or, for the
queues not moved yet, the old `<project>/tasks` beside the repo, where the subfolder is the
status (`inbox/ todo/ doing/ blocked/ done/`). A **capture** session (PM + tech lead) writes the
owner's words up as tasks, answers technical questions, and escalates product questions to the
owner in plain terms. **Coder** sessions claim a task by moving its file, work in their repo,
log, and ask questions with `task ask` without stopping. The answer is written into the task and
pushed into the asking session by a Claude Code hook. A generated **manifest** (`tasks.json`)
gives one view of everything and checks the queue for problems. The owner reads
`WAITING-FOR-OWNER.md` (and, in Drover, the waiting list on Home).

## Parts

| Part | Where | State |
|---|---|---|
| Protocol v2.1 | `docs/PROTOCOL.md`; each queue's `README.md` | In use on Drover |
| File contract v1 (for tools that write answers without `bin/task`) | `docs/PROTOCOL.md`, "File contract" | In use: Drover's waiting list (Drover task 0015, `327af59`) writes against it |
| `task` CLI: ask / answer / escalate / inbox | `bin/task` | A stand-in since task 0002: it runs the same `muster` commands |
| Answer push | `bin/answer-hook.py` | Live in Drover's checkouts (`drover/main`, `agent-controls`, `cleanup`, `docs-wt`), verified end to end on 2026-10-12. Reads `$MUSTER_ROOT`'s mailbox and the session folder's repo's `.muster/` one (0005), finding the main checkout from `.git` files, without running git; sessions of two projects at once each get only their own repo's mail (0004, tested). For every repo it waits on the personal layer's change (below, "Other projects") |
| Manifest + check | `bin/manifest.py` → `tasks.json` | Built, tested, both layouts (0005: `MUSTER_ROOT=<repo>/.muster`). Capture rebuilds it on every commit |
| Queue in the repo | `.muster/` (`layout.rs`, `migrate.rs`); `muster init [<repo>] [--local\|--committed]`, the app's New queue, `muster migrate` | Step 1 built and tested (task 0005): layout, local mode, one queue for every worktree, discovery, migrate. No queue moved yet. Committed mode with live state in `.git/muster/` is the next step |
| Master list | `INDEX.md` per queue, written by `muster` after every change; `muster index` | Built, tested (task 0018). `muster check` and `manifest.py --check` report a stale one. No app link yet: the app has no open-a-file path |
| Owner's waiting list | `WAITING-FOR-OWNER.md` per queue (by hand); Drover's Home (task 0015) | By hand here. Built in Drover |
| Attachments | `attachments/<id>/` per queue | In use (screenshots, mockups) |
| `muster` command line | the app's binary, `bin/muster` shim | Built: new, list, show, move, set, note, attach, answer, check, index, projects, init, the backlog commands add, shape, file, drop, the stage commands pass, return, block, unblock, stage, the claims claim, next, heartbeat, checkin, reclaim, and questions ask, escalate, inbox (same rules as the app) |
| Commands `/muster`, `/mu` | `docs/COMMANDS.md` | Designed, not built |
| Desktop app | `app/` (Tauri 2 + React 19, decision 0001) | Built and running: board, new task, task management, waiting for you with answering, files, activity, problems, projects, the AI helper |

## The app

`app/` is a Tauri 2 + React desktop app on Drover's stack (decision 0001), so it can move into
Drover later. Open it with `open app/src-tauri/target/release/bundle/macos/Muster.app` (build:
`cd app && pnpm install && pnpm release:build`; develop: `pnpm app`).

- **Projects sidebar** (left): All and every queue (found at `~/playground/*/tasks` and one level deeper,
  `~/playground/*/*/tasks`, and the repos' `.muster/` at `~/playground/*/.muster` and
  `~/playground/*/*/.muster` in a main checkout, a project with both showing its `.muster/` under
  its name; plus ones added by hand), each with its task count, how many need you (orange) and how many are in progress
  (the Building colour), and **+ Project** at the bottom (Settings, at Projects). The header keeps only the views, New task and
  the display mode chip (0038), on one line. Under each name, quietly (0037, `activity.ts`): the Building-colour
  dot and "2 working" (tasks with a live session, or a check-in or heartbeat in the last 30
  minutes; never in Done, and in Your check only while live), or **stalled** in the attention
  colour (work in Building, not blocked and not the owner's, and nothing in the project for over
  30 minutes), and the last activity ("now", "12 min ago", "3 h ago", "yesterday", "5 days ago":
  the newest stage change, check-in, heartbeat, file change, queue commit or live transcript).
  The All row sums projects: "3 working · 1 stalled". Chips above the list order it: **Working**
  (default: working projects first, then by last activity), **A–Z**, **Needs you** (most waiting
  first, then as Working), remembered in `localStorage` (`muster.sidebarOrder`).
- **Labels and categories** (task 0044, `labels.rs`, `labels.ts`, `ProjectName.tsx`,
  `InlineField.tsx`): `projects.json` keeps `labels` and `categories` by queue folder, beside the
  gate and the limit; each entry is cleaned when read and set (control characters stripped,
  trimmed, at most 40 and 24 characters), and one that isn't text or is empty is left out without
  losing the rest of the file (`labels::read_labels`, `read_categories`). `Project` carries them
  as set (`label`, `category`, "" for none) and `QueueInfo` as shown (`labels::apply`): the label,
  else the name; the category, else the project's group (`persianUtils`), else "Other" ("other"
  in any case is Other). The queue's `name` stays what commands, `--queue`, task keys' links and
  the window's own bookkeeping use; `muster projects` adds `label:` and `category:` after the rest
  of a line when set. The window calls a project by its label everywhere it names one (the
  `Labels` context, read through its Consumer by `ProjectName` and `TaskKey`, so rows stay
  hook-free): the sidebar, cards and lists with All, Waiting for you, the task pane, New task's and
  the Helper's project pickers, the backlog's quick add, the Graph, the Your check limit's words,
  and each task key's tooltip ("Muster · mu-12", Markdown links too); the real name is the tooltip.
  The window's title is "<label> — Muster" for one project and "Muster" for All
  (`show_project_title`, by the project's name, `labels::window_title`). The sidebar groups its
  rows under category headings (A–Z, Other last; `byCategory`), each with its needs-you count,
  its working tasks and its tasks ("3 tasks in 2 projects"), always shown, nothing folded; the
  order chips order within each heading, and A–Z goes by the label. Double-clicking a row's name
  puts a field in its place (`SideLabelEdit`): ↩ or leaving it saves, Esc keeps it, the folder's
  name typed back is no label. Settings › Projects has a label field (the real name small under
  it) and a category field per row, saved as you type (`InlineField`, its timing in `fieldSteps.ts`:
  after 400 ms, at once on ↩ or blur; Esc puts back what it held and saves that; left, it shows
  what it sent until the save settles), the other projects' categories as chips under the field
  while it has focus. The sidebar's order holds while a label is edited in place. `set_project_label` and `set_project_category` replace
  `rename_project`: the old `names` are still read, so a project renamed before keeps that name in
  commands, but the window no longer sets them. `carry_over` (0005's migrate) carries both.
- **Arranged in the sidebar** (task 0057, `arrange.rs`, `projectOrder.ts`, `SidebarProjects.tsx`,
  `ProjectSwatches.tsx`, `projectColours.ts`): projects and category headings drag to reorder (a
  heading takes its projects), and ⌥↑/⌥↓ moves a focused row; the first move picks the fourth order
  chip, **Mine**, built from the order on screen plus that move. Heading order holds under every
  chip; the chips sort within a heading. Dropping a project on another heading moves it there (at
  the top), "+ Category" makes an empty heading kept only in the window until a project goes in,
  double-click or ↩ renames a heading (a name another heading has merges the two), and empty
  headings go. Each project can have a colour, a left-edge dot on its row: clicking it shows an
  inline row of five swatches (Lime, Orchid, Pink, Tan, Mint, a shade per theme; `PROJECT_COLOURS`,
  CIE76 ≥ 30 from every stage, blocked, needs-you, priority and gold colour in every theme and
  accent, and from each other). The dot shows wherever `ProjectName` names the project. Settings ›
  Projects has a colour cell and ↑/↓ place cells, in Mine order. `projects.json` keeps `colours`,
  `order` and `categoryOrder` under its lock (`set_project_colour`, `arrange_projects`,
  `rename_category`; unknown entries are dropped when read, a refused change writes nothing);
  `remove_project` clears them and `carry_over` carries them.
- **Headings fold, and a group can be named** (task 0070, `arrange.rs`, `projectOrder.ts`,
  `SidebarProjects.tsx`): a heading's chevron, or ← and → on a focused heading, folds it to its name,
  "N projects", what needs you, what's being worked on and its tasks, in every display mode and
  under every chip; the selected project and every one that needs you still show under it as chips
  that pick them, and a project moved under a folded heading unfolds it. Headings are
  `role="heading"` (focusable, with `aria-keyshortcuts`); the chevron is a `Pill` with
  `aria-expanded`. `projects.json` keeps `folded` (headings, read leniently, `fold_category`;
  `QueueInfo.categoryFolded`) and `groupNames` (a group folder's display name). Renaming a group's
  heading (`rename_category`) sets the group's display name, so its projects, and any it gains,
  go under it with no category of their own; projects whose own category was the old name take the
  new one, as before; renaming it back to the folder's name clears it; a fold and a place move with
  a rename, and a merge is folded if either heading was. A migrated queue outside its group keeps
  the group's heading as its category (`carry_over`).
- **Reaching a working session** (0039, `ping.rs`, `Ping.tsx`, `reach.ts`): **Ask for a
  check-in** under a quiet card ("no check-in for N min"; a strip joined to the card's bottom,
  since the card is a button), in the task pane's **Session** strip (Building and Testing, an
  agent's task the owner doesn't hold) beside **Message the session working on this** (one line,
  500 characters), and **Ask all quiet sessions for a check-in (n)** under a stalled project in
  the sidebar. Each goes through the core to the holder's session mailbox
  (`mail/<session8>/<id>-checkin.md`, `<id>-note-<nnnn>.md`) and is logged on the task; the answer
  hook hands it over after that session's next tool call, check-in requests first, messages
  quoted as the owner's note. The card shows "asked 2 min ago" in the check-in's place until a
  check-in of that Building stage comes (`checkinAsked`, from the log's ask line). A holder
  without a session stamp, or in Testing for a check-in, shows the button disabled with the
  reason. Not built: the optional auto-nudge after 45 quiet minutes, off by default; it can now
  go in Settings → Agents (0036).
- **Send-back comes first** (mu-66, `ops.rs` `change_locked`, `block.rs` `put_sent_back_first`): a
  Testing → Building change (a review round, `muster return`) skips the one-claim-per-holder check,
  so a review can always send a task back to its builder; the sent-back task comes first (`muster
  next` refuses with "fix mu-59 first", its card says so, the holder's other card says it waits
  behind it). Claiming another task stays refused while you hold one.
- **Pause and Resume** (0045, `pause.rs`, `pause_cmds.rs`, `Pause.tsx`): **Pause** or **Resume**
  in the strip under a card (Backlog through Testing; the strip a quiet card's Ask for a check-in
  shares) and beside the stage in the pane's status line, with an optional why typed in place. A
  paused task keeps its stage and claim but leaves its column for the **Paused lane** (0059,
  `PausedLane.tsx`, `lib.ts` `columns()`): the board's first column, before Backlog, always shown
  with its count (a thin labelled strip when empty, folded in Focus), a Paused band first under
  Group by; its card says "⏸ was Building · paused 2 h ago · why", and Resume brings it back to its
  column on the next poll. The stage columns' counts and the sidebar's in-progress count leave
  paused tasks out; the sidebar's "⏸ 2" stays. Elsewhere it shows the grey **⏸ Paused** chip with "2 h ago
  · why" (never red, never a stage colour); its claim isn't quiet and its lease doesn't run (the
  card's flags, Release, the sidebar's stalled), Pass is off with why, the timeline hatches its
  paused stretches grey (`pauseHistory`), the Flags filter has **paused**, and the sidebar row
  counts them ("⏸ 2"); the table has a grey ⏸ Paused pill and a "was Building" pill (sorted first by
  Stage), the timeline bar says "Paused · was Building", the graph a paused badge (and never
  "ready"), Waiting for you shows the chip, the owner's own paused task doesn't need them, the
  stage track is off, and the pane's time so far leaves paused time out. In Building or Testing the holder's session is told through its mailbox
  (`<id>-pause.md`, `<id>-resume.md`); the pane says whether it was.
- **Old pauses** (0059, `old_pause.rs`, `OldPauses.tsx`): a block from before 0045 whose reason says
  paused ("owner (paused)") is found in every queue and turned into a pause with its reason and the
  time it was blocked (`muster check --fix-pauses`, below). Once, on load, the board shows an
  inline notice listing them with **Make them pauses** and **Not now**
  (`old_pauses`, `fix_old_pauses`, `set_old_pauses_offered`; the setting `old-pauses-offered`).
- **Claude Code's session states** (0048, `agents.rs`, `sessionState.ts`): the core runs
  `claude agents --json --all` (argument array, `CLAUDE_CONFIG_DIR` per account the stamps in
  Building and Testing name, usage reporting off, in a process group of its own that a 10 s timeout stops whole, 1 MB cap) on a thread of its own,
  started by the window's state poll (or the Dock badge's own read while the window is hidden,
  0006) at most every 15 s and never while nothing has a stamped
  holder; the poll uses the latest result and never waits for a run. Parsed with serde into the
  session id and state only (`cwd` and `name` are never kept); an interactive session's `status`
  stands in for its missing `state`, working with an idle `status` is unknown, and a repeated id
  is its latest entry. `agents::holds` counts each session's claims and Testing holds across every
  queue (the app's poll, and `muster list --json` through `projects::active`), and
  `agents::apply_with` gives a task its holder's state when the id and account match and the
  session holds nothing else (in Testing: no claim, one Testing hold); a paused task (0045) gets none and isn't counted as a hold. The card's who lines add
  "working" (accent, with the Building dot), "waiting for input in Claude Code" (orange pill, and
  the task counts as needs you: the core sets `waitingForOwner`) or "session ended, task still in
  Building" (Building only), with "no check-in for N min" kept under it in muted text; the pane's status
  line says the same and "Waiting on you: input in Claude Code"; the sidebar's working now takes
  Claude Code's word over the transcript and heartbeat; Ask for a check-in is disabled with
  "waiting for input in Claude Code: answer it there first", and Ask all leaves those out and
  says so. Unknown, unmatched or unreadable: the card reads as before. Settings › Agents shows
  one line under the LIVE window from `sessionStates` ("Claude Code's session states: on (2
  accounts)" / "not available: <why>"). One real run (2.1.296, 2026-10-13,
  this Mac, 2 and 6 sessions listed): 0.13–0.18 s, 0.5 and 1.5 KB of output; the task's note had
  seen several seconds on one run, which the 10 s timeout and the background thread cover.
- **Projects** (in Settings, or **+ Project** under the project list): every project with its folder, task count and where it came from (found
  in `~/playground`, or added). Discovery (`queue::discover`) looks at `<project>/tasks` and one
  level deeper, `<group>/<project>/tasks`, and no further, and at a repo's `<checkout>/.muster`
  at the same two levels, in main checkouts only (see `.muster/` below); it passes over folders named `.*`, `_*`,
  `node_modules`, `worktrees`, `*.worktrees`, `target` and `dist`, follows a symlink only while it
  stays inside `~/playground`, and keeps only queues: a `.muster/` with its `config.toml`, or a
  folder with a `README.md` and `todo/` (or `tasks.json`). A nested queue is named by its own project folder and carries its group (the
  folder above it). A queue both found and added shows once, as found. `muster projects` marks
  each line found or added. **New queue…** picks the project's repo (its main checkout or any
  worktree of it) and makes a local `.muster/` in the main checkout, as `muster init` does
  (`projects::new_queue`, task 0004); it refuses if `.muster/` is there already ("To show it, use
  Add existing queue.") and in a repo without a main checkout. A folder outside any repo still
  gets the older `tasks/`: status folders, README with the rules, `WAITING-FOR-OWNER.md`, the
  tools' stand-ins in `bin/`, and its own git with a first commit, never over an existing
  `tasks/`. **Add existing queue…** picks any queue folder. The label and category fields
  (0044), Hide/Show, and Remove (added ones only) never touch files. Each project's **Prefix** (0033) shows with **Set** beside
  it: stored in its queue (a `.muster/`'s `config.toml`, an old queue's README.md), refused (why shows under the field) when it's bad or
  another project's; New queue stores the default. Saved in `~/Library/Application Support/ai.sweetlemon.muster/projects.json`.
  Folders are picked by the core's own dialog, so the window never hands the core a path.
- **Settings** (task 0036, `Settings.tsx`, `settingsData.ts`, `settings.rs`): a view in the header's
  row (never a window), its groups docked on the left (Appearance, Display, Projects, Agents,
  About; `SETTINGS_GROUPS` is where later groups such as notifications go), the open
  group's settings on the right. Appearance is only here and Help only in the header: on the
  owner's word (2026-10-13) the header's one-click Appearance button and Settings' Help group
  (links into the manual, which the Help tab and every ? already open) were removed; the old
  Appearance pane and Projects tab are gone, their contents moved here. Paths under the home
  folder (About, the Agents settings file, Projects' folders) show as `~/…` (`homePath` in
  `lib.ts`, `HomePath.tsx`; the core sends `home` in `settings_info`), the real path in the
  tooltip and on the clipboard when copied. **Agents** has the
  helper's account and model, the **claim lease** (default 12 h, 1 h to 7 d) and the **LIVE
  window** (default 5 min, 1 min to 1 h), typed as `30m`, `2h`, `1d` and checked by the core.
  Both are saved per machine in `settings.json` beside `projects.json`; `muster reclaim` (with
  no `--after`) and `muster check` read the same file, the app reads it on every poll (Release,
  the card's "quiet" and dot, the pane's LIVE), and a missing, broken or out-of-bounds value is
  the default, field by field. **Projects** also shows each project's gate command (0023),
  read-only, with the `muster projects set` line when none is set. Your check's view buttons
  (0043) open Settings for Appearance, Projects and Settings. **About** shows the version, the
  settings folder and its files, and every folder the app reads: the queues found on their own,
  then the ones added by you (or that there are none), with paths under the home folder as `~/`.
- **Display modes** (task 0038, `modes.ts`, `ModeViews.tsx`, `Display.tsx`): **ADHD** (the default:
  today's screens), **Standard** and **Focus**, picked in Settings › Display (three chips and a
  table of every element by mode, the current column lit), shown as a chip at the end of the
  header that opens that page, and toggled by ⌘. (Focus from any mode, back to the one before).
  Kept in `localStorage` (`muster.displayMode`, `muster.displayMode.before`). `modes.ts` is one
  table of the elements a mode could change (filter bar, Who and Group by, card lines, the Done
  line, board columns, Recently finished, task pane sections, sidebar), each with its behaviour,
  words and how to get back what it folds;
  components read it through `useLook` only, and `modes.test.ts` fails on a read of an element
  the table lacks, on a mode read around it, and on a row nothing reads. Standard: the filter
  groups and Sort behind **Filters (n)**, opened in place; Who and Group by as their value; brief
  cards (who, check-in, branch, tags, parent, estimate and files on the selected card); Vision,
  Review, Work, and the Log and check-ins folded to a header each. Focus: Building, Testing and Your check
  open, the other columns strips with counts, needs-you and blocked counts (a click opens one until
  the mode changes, in the plain and the grouped board, whose Backlog band folds to its head the
  same way); cards are the title, one status line and blocked, quiet, needs a tester or needs you; the filter bar is a **Search and filters ⌘F** button until ⌘F, and
  stays while a search, filter or Who hides tasks; the sidebar is 150 px of category headings with their
  needs-you count, names, a working dot,
  stalled (with its Ask all quiet sessions) and needs-you counts; Recently finished (0051) is a strip with its name and count that opens in
  place (Fold puts it back), and reads no costs while folded; the Done card's "done 4 min ago" line
  shows in all three; the pane folds as Standard. Needs-you counts and badges show in every mode. A pane fold keeps its
  section mounted in every mode (ADHD's wrapper is `display: contents`), so typed text survives a
  fold or a mode switch; the folded Log's header counts its lines and check-ins.
- **Themes and accents** (Settings' Appearance): 7 themes (Violet night, Violet day, Graphite,
  Midnight, Paper, Snow, High contrast), with Match the Mac picking one dark and one light, and 6
  accents (Violet, Teal, Rose, Gold, Green, Graphite). No blue, no orange. They apply live and are
  remembered. Default: Muster's violet, following the Mac. A test checks every theme and accent
  pair: text 7:1, secondary text 4.5:1, accent-coloured words 4.5:1 (they use the strong shade),
  button text 4.5:1, fills 3:1.
- **Drover's names** (task 0053): the CSS variables carry Drover's token names (`--panel`, `--line`,
  `--hover`, `--selected`, `--accent-ink`, `--focus`, `--bad`, `--warn`; table in DESIGN.md), with
  Muster's values; the old names stay as aliases in `styles.css`'s `:root`. Buttons, chips, stage
  pills, badges and the pane's value picker are atoms in `app/src/ui` shaped like Drover's (`Button`,
  `Pill`, `StatusPill`, `Badge`, `Segmented`/`Segment`, classes `ui-<name>`), drawn in Muster's own
  look. `ui/adoption.test.ts` fails on a screen using the old classes or a plain `<button>`.
- **Look** (Muster task 0010): a violet palette (accent `#8B6CF0` dark and `#6D4AE0` light, with
  violet-tinted backgrounds and surfaces), chosen because orange belongs to Drover and Braindump
  and blue to 127. Stage colours (task 0034, colours people know from boards): Backlog grey,
  Shaping blue, Ready cyan, Building the accent (violet when the accent would pass for
  another stage), Testing amber, Your check and "needs you" orange, Done green, Blocked red. No
  two stages are closer than a colour distance of 30. Contrast is checked to WCAG AA. Dark
  buttons use dark text on violet. In the light theme, status colours are fills behind dark
  text, with darker variants for coloured words.
- **Icon:** a bot face with a visor eye and a check, drawn by capture as a vector
  (`muster/tasks/attachments/0010/`). In use: **B**, the lineup's style (violet bot on near-black
  `#16131F`, eye and check cut out), like Drover's orange crook on navy. **A** (light face on a
  `#2E1F66`→`#170F38` gradient) is ready if the owner prefers it. `dock-preview-drover-a-drover-b.png`
  shows both next to Drover's. The earlier dog-face mark and its recolourings are in
  `attachments/0006/`.
- **Layout: nothing covers the board.** A task, a backlog note and New task open in one pane docked
  beside the content, with the board still visible and live beside it (the owner: "need to have
  everything in front of my eyes"). Drag the edge to resize it; the width is remembered. Esc
  closes it. The projects sidebar resizes the same way (0058): one splitter (`ui/Splitter.tsx`,
  rules in `splitter.ts`) on both, drag, double-click for the default, ← → (Shift for 120 px).
- **Columns always fit:** an empty column becomes a slim strip with its name written vertically,
  so all five stay in view, even with a task open beside the board.
- **Columns scroll without scrollbars** (owner, 2026-10-13: "hide the scrollbar, in the columns
  doesn't feel good"): `.board .cards` hides its scrollbar (`scrollbar-width: none` and
  `::-webkit-scrollbar`), plain and grouped board; other scroll areas keep theirs. `ColumnCards.tsx`
  draws every column's list: while it overflows, a strip above and below it (18 px, fixed, so
  nothing moves) holds "+N above" and "+N below" (muted text; a click scrolls one screen, smooth
  unless reduced motion), and the edge with more fades (a mask, so it takes the column's own
  background). `scrollMore.ts` counts the cards (task and note cards) wholly or partly out of
  sight (`moreAround`, a pure function); `watchMore` measures on a passive scroll listener and a
  ResizeObserver on the list and its children (a MutationObserver follows children), one
  measure per frame, all released on unmount. Grouped bands grow to fit, so their columns never
  scroll and show neither.
- **Search, filters and sort** (task 0032): one bar above Board, Table, Timeline and Graph; ⌘F
  stays in whichever of them is showing. Group by (0016) sits on the same bar.
  - **Search** (always there, ⌘F, Esc clears): words in any order, case and accents aside, in the
    id, title, tags, text and holder; the matches are marked on cards, backlog notes and rows. A
    word of digits only (0–9) is an id (19 finds 0019) and never matches text, so dates in logs
    don't swamp it; a key (mu-19, MU-19) finds that project's task (0033). The core builds each task's search text when it reads the file (title, tags,
    body without front matter, folded, each word once, digit-only words out, 4 KB at most),
    cached with the task, so the window filters in memory (1,000 tasks well under a frame,
    measured in a test). The text isn't in the 2 s `state` poll or `--json`: the window asks for
    it (`search_index`, one project or all) only while the search has words, and again only when
    a task in view is new or its `modified` changed. ⌘F only (Ctrl+F stays a text field's
    "forward a character"). Esc in the search box stays there: it clears the words, or leaves an
    empty box, and never closes the pane beside the board or throws away a New task draft.
  - **Filters** as chip groups: Stage, Type, Priority, Tags, Held by, Flags (blocked, needs you,
    open questions), Captured / Changed / Done (today, this week, this month). Held by is whoever
    holds the task in its stage (0030: the claim in Ready and Building, `held-by` in Testing and
    Your check, the owner there when it names nobody), shown as the stamp's label: `wt-a (wt-0030
    · session 7b5e8856 · cc-hg)` is `wt-a`, in the app and in `muster list`. Each group shows
    only the values the project has. Any chip within a group, every group together. The line
    under the chips reads what's active ("“flicker” · Building · high · #design"), each with ✕,
    then "36 hidden by filters" and Clear all; it keeps its place when nothing is active. Backlog
    notes follow the search and hide under any filter but a Stage one with Backlog.
  - **Sort:** the board's Sort chips order every column (priority, the default: needs you first,
    then high to low, Done newest finished first by `doneAt`, else `doneOn`, then latest changed,
    0051, in `muster list --sort priority` too; newest, oldest, changed, estimate, id), inside
    each Group by band too. The Table sorts by its columns; the Timeline and Graph keep their own
    order.
  - **Saved views:** + Save view keeps the search, filters and sort under a name, per project, in
    local storage, as a chip; ✕ deletes it.
  - **`muster list --search --stage --type --tag --holder --blocked --sort`** gives the same
    tasks (`search.rs`; values joined by commas mean any of them). The rules are written once in
    `search.rs` and `search.ts`, and `app/src/search-cases.json` holds both to the same answers
    (`tests/search_cases.rs` runs the real command on it, `search-cases.test.ts` the app's filter).
- **Three views of a project:** **Board** (the kanban below), **Table** (every task as a row:
  project, id, title, type, priority, status, who's on it or what it waits on, captured, changed,
  tags; click a header to sort) and **Timeline** (0031: scale chips Day · Week · Month, Week by
  default and remembered, with ◀ ▶ and a way back to the current period. Day is one day in hours,
  Week seven days in quarter days, Month the calendar month in days. Each task's bar is waiting
  (captured → first claim) in a quiet tint, then one segment per stage it held, in the stage
  colour, from the stage history's stamps; tasks with dates only fall back to whole days with a
  faded start. A now line runs across the chart and open tasks run to it. Bars are worked out in
  `viewData.ts` in wall-clock minutes, so a clock change doesn't bend them). All three, and the
  Graph (0019), share the search and filters and open tasks in the docked pane.
- **Help** (tab, task 0035): the manual in `docs/manual/`, eight pages (Start here, Concepts,
  Tutorials, In the app, Commands, Shortcuts, For agents, FAQ), bundled at build time and rendered
  with the same sanitised Markdown as task text. A docked sidebar lists every page and its
  sections, the current one lit, under a search box over all pages (an in-memory match); ◀ ▶ step
  between pages; links between pages and sections move within the tab. Commands is made from
  `help.rs` (`help::commands_page`, grouped: tasks, stages, claims, questions, backlog, levels,
  projects), and a Rust test keeps `docs/manual/05-commands.md` equal to it
  (`MUSTER_WRITE_MANUAL=1 cargo test commands_page` rewrites it). Shortcuts is made from one table,
  `app/src/shortcuts.ts`, which the key handlers read (`pressed`; the window's Esc, ⌘N and ⌘F
  and the search box's Esc in `keys.ts`; ⌘F takes ⌘ only), and a test fails when a handler
  acts on a key the table doesn't list for its file; `pnpm test -u` rewrites
  `docs/manual/06-shortcuts.md`. Another test fails on a link to a page, section or repo file that
  doesn't exist. A small **?** beside each pane's title and in each view's toolbar opens its
  section, with the pane left in place. Click a command line to copy it. The manual has no
  screenshots yet.
- **Stages** (task 0014, decision 0003): Backlog, Shaping (product, tech and UI agents), Ready,
  Building (code agent), Testing (QA agent), Your check (the owner), Done. `stage:` in the file is
  the truth, written only by `muster`. A stage change is logged as `<time> <who>: stage <from> → <to>`,
  and those lines are the stage history. Blocked is a flag (`waiting_on`, with its kind in
  `blocked-kind:`, task 0024) that keeps the task in its
  stage. `pass` moves it on, refusing when the task is blocked, when someone else holds it, or when it
  has moved since you looked (`--from`). It also refuses Building → Testing without the gate's line,
  and Testing → Your check with any criterion unticked. `return` sends it back with a reason and
  hands Building back to its last builder. In the old layout `status:` and the status folder are a
  mirror of the stage for older readers (Drover), and `check` reports a stage whose status doesn't
  match, which is what a hand move leaves. In `.muster/` (0005) no `status:` is written, and one
  left in a file isn't read: the status the window and `muster list --status` use is worked out
  from the stage.
- **The tester isn't the builder** (task 0047): `pass_change` (so `pass`, the merge's
  `check_merge_pass` before any rebase or gate, and `pass_merged`), `set_stage` and `move_task`
  (past Testing) refuse the builder of an agent's task, matched by `session::same_holder` against
  whoever last claimed it (`ready → building (claimed)`) and whoever last passed it into Testing
  with a gate line. The refusal names the builder and suggests a QA label (`muster/qa-0042`). The
  owner is always let through, logged `(owner's override: no separate tester)`; another label in
  a builder's session is let through, logged `(tested under another label in the builder's
  session)`. The task carries `builtBy`, `gatedBy` and `testedBy` (the last pass into Your check),
  and the app counts builders as the core does. A Testing card says "needs a tester" on a line of
  its own while a builder holds it; the Your-check card says "built by … · tested by …", wrapped,
  never cut; the task pane's status line says the same from Testing on.
- **Depends, met once merged** (tasks 0019, 0024, 0042): a task's `depends:` hold it back until
  each is merged, which for an agent's task is Your check or Done (work merges into main after
  Testing passes) and for a human task only Done. One rule in `block.rs` (`met_at`, `holds`)
  feeds every reader: `claim` and `next` (`waiting_on_deps`), the derived "After 0014" block
  (`block::derive`) that the card, table, timeline, pane, `muster list`, INDEX.md's "waits on"
  and `check` show, the Graph's "ready" (`readyToStart` reads the core's verdict), and
  `manifest.py`, which mirrors it. A change that merges a task logs `unblocked: 0014 done; 0017 in
  Your check` on each task it frees (Done after Your check writes nothing more). Sent back from
  Your check (`return`, or a `stage` override), it blocks again the tasks that depend on it and
  haven't started; each one under way (held in Building, or in Testing or Your check) keeps its
  stage and claim, gets `0017 went back to Building: <why>` on its log, and isn't held by that
  task again. A shared fixture, `app/src/deps-cases.json`, holds the CLI
  (`tests/deps_cases.rs`), the app (`deps-cases.test.ts`) and `manifest.py` (`test_task.py`) to
  the same answers.
- **Backlog** (task 0015): any `.md` in the queue's `inbox/` that isn't a numbered task is a
  backlog note, a one-liner or a whole page, however it got there. Add one with `muster add` or the
  quick-add field at the top of the Backlog column. Open a note in the docked pane to:
  - **File as task:** `muster file` makes a numbered task in Shaping and moves the note into its
    attachments.
  - **Drop** it: `muster drop`, kept in `inbox/dropped/` with the reason.
  - **Copy shaping brief:** `muster shape` gives the note, quoted as data, with the roles and the docs
    to read, related tasks, and the command to file it.

  `muster list` shows notes too. They live in `.muster/backlog/` (0005), and in the old layout's
  `inbox/`.
- **Goals, features and tasks** (task 0016): `level: goal | feature | task`, `parent:` and a goal's
  `due:` set with `muster new --level/--parent/--due` and `muster set`. A task sits under a feature
  or goal, a feature under a goal. Goals and features are never claimed or built: their stage comes
  from their children (Done only when every child is), and they roll up "n of m done" and the open
  estimate. **Group by** None · Feature · Goal (chips above the board and table) gives one band per
  parent, with the stage columns inside, after a Backlog band (0046) with the quick add, the backlog
  notes and the Backlog tasks under no parent. Cards show their parent. `check` reports missing parents,
  wrong levels and loops, and warns about late goals.
- **Editing** (task 0020, part 1): **Edit** beside the description turns it into a text box in place.
  ⌘S saves, Esc twice discards, and nothing typed is lost when you switch cards. `muster edit` does
  the same with `$VISUAL` or `$EDITOR`. A save is refused if the file changed since it was opened,
  or while someone else holds the task in Building (add a note instead). Comments come with 0017.
- **Slash commands** (task 0003): `commands/muster.md` and `commands/mu.md` route `/mu <word>` to
  one `muster` command each. They may run only the commands they route. Anything else is captured
  as a backlog one-liner. They're installed into the personal layer only with the owner's yes.
- **The task pane** (task 0021, the owner's design D2):
  - The title (rename in place) and status line come first: stage, who's on it and since when,
    LIVE, blocked, and what waits on you.
  - Then the seven-stage track: click to move, with undo. It refuses if the task moved since.
  - Then two columns. On the left, what the stage needs: work so far, your checklist and the
    history, or the criteria. On the right, a sidebar with the one next step, Send back,
    Block/Unblock, priority and type as one chip each, estimates, tags, files and links.
  - Under the write-up come the vision check, the review of record and the comment thread. The
    note bar stays at the bottom.
- **Fields** (task 0017):
  - Types are code, bug, chore, tech-debt, spike, design, docs, research.
  - Estimates are agent time, test time and tokens.
  - `links:` hold https links, opened only if the task has them.
  - Comments live in a tool-written `## Comments` section (`muster comment`), and Edit never
    rewrites them.
- **INDEX.md** (task 0018): one generated list per queue, regenerated after every write. Goals,
  features and tasks by stage, each with its estimate, what it waits on, comments and open vision
  conflicts, then backlog notes and Done. `muster index` rebuilds it, and `check` flags a stale one.
- **Vision check** (task 0029):
  - `muster vision <id>` prints the brief.
  - `--record -` stores an agent's findings as a `## Vision` section with V-numbered conflicts, and
    the section isn't editable once the task is checked.
  - `--resolve Vn` settles one.
  - Shaping → Ready waits while a conflict is open.
  - A change of direction becomes one owner question with the cost of pivoting.
- **Review of record** (task 0027): the card says where it stands in Building and Testing
  (`review 1/3 · 2 to fix`, `review 2/3 · 1 to re-check`, `review clean · round 2`, `not reviewed
  yet` on a started task in Testing; quiet, bolder while findings wait for the builder,
  `Review.tsx`); the pane's **Review** fold (`ReviewPanel.tsx`) shows the status, **Copy review
  brief** (the core's brief, loaded on open, a started task only) and the section as sanitized
  Markdown. **Lessons to approve** leads Waiting for you (`Lessons.tsx`): each proposal in full as
  plain text, **Reject…** with why, **Accept all** (the core's `decide_lessons` as the owner);
  the queue's waiting lessons count in the tab's needs-you count, the sidebar's and the Dock's.
- **Board:** one column per stage, with its role under the name. Blocked cards stay in their stage
  with a badge that says plainly what it waits for (task 0024; each kind its words and look, task
  0060: "Your answer: Q2 …", "Your action: …", "Permission: …" in needs-you orange, "After mu-14",
  "Outside: …", "Question: Q2 …, waiting on capture" neutral, each with a red mark), and Your check
  cards count as "needs you". The task pane has a
  stage picker, Pass to <next> (with the gate line out of Building), Send back with a reason,
  Block/Unblock, and Accept / Send back in Your check. Only one action runs at a time. Cards show the id, title, type, priority (on every card: HIGH filled magenta, normal outlined, low faint), tags,
  "needs you", open questions, files, who's on it (flagged when quiet for 12 h), what a blocked
  task waits on, and when it last changed. Tasks waiting on the owner sort first. Done shows the
  latest 8.
- **Human tasks, on the one board** (task 0022; the owner, 2026-10-13: "human board is taking
  space from the ai board", so one board, human cards patterned): `for: human` makes a task the
  owner's own (`muster new --human --as owner`, the Human/AI chips in New task, the For chip in
  the task pane, `muster set <id> for human|ai`). It goes To do (Ready) → Doing (Building) → Done
  with the pane's Start and Done, no gate line, no Testing, no Your check, no lease. Agents never
  take one: `claim` refuses it and `next` passes it by; an agent proposes one with `muster add`,
  and only the owner makes one. Its card sits in the same stage columns as the agents' tasks
  (`lib.ts` `boardColumn`: To do in Ready, Doing in Building, Done in Done; never Backlog,
  Shaping, Testing or Your check), drawn so it can't be mistaken: a faint diagonal hatch in the
  human gold (`.card.human`, CSS only: stripes of `--st-human` mixed by `--human-hatch`, 14%,
  `themes.ts` `HUMAN_HATCH`, into the card's own background, so the Tint look's blocked or
  needs-you wash stays under them) plus the gold "You" ribbon (`#A08428`, never a stage colour),
  in every card look. `themes.test.ts` holds every card word on a stripe over the plain card to
  its contrast in every theme and accent (text 7:1; muted, project name and warnings 4.5:1), the
  title (7:1) and the muted, project-name and warning words (4.5:1) on a stripe over every wash the
  Tint look can give the card (each stage, priority, blocked, needs you; mu-65). The project name and
  the quiet and vision flags sit just under 4.5:1 there in their plain colours, so `.card.human`
  takes `--human-accent-text` and `--human-bad` (`themes.ts` `wordOnStripes`: the colour mixed toward
  the theme's text until every stripe reads), the stripes and wash untouched. The hatch
  visible (CIE76 8+ from the surface), and the gold a hue apart from blocked red and needs-you
  orange. The ribbon marks human tasks in Table, Timeline, Graph and the pane too; the pane's
  stage track shows a human task only To do · Doing · Done (`paneData.ts` `trackSteps`), and the
  Stage filter chips go by the board column. The **Who** chips (All · You · Agents, remembered)
  sit above the Board, Table and Timeline: You shows only the owner's tasks, Agents only the
  agents'; backlog notes show under every Who (nobody's yet).
  What the agents wait on the owner for has no lane of its own: Your check is a column, and
  escalated questions and blocks on you or by a permission are needs-you cards where they are.
  `needsYouCount` (`human.ts`: the owner's own tasks to do and doing, plus those) is the one
  count: sidebar, Waiting for you, the needs-you filter chip, and the Dock badge (0006, counted
  in the core by `badge.rs`, which `needs-cases.json` holds to the same answers). An
  agent's task that depends on an unfinished human task says "Blocked by tasks: 0041" (0024's
  sentence; derived, so the blocked chip, `muster list --blocked`, the graph and the activity
  feed count it as blocked too) and can't be claimed until it's done. `muster list` marks human
  tasks `[you]`, INDEX.md "you", and `manifest.py` reads `for`.
- **Recently finished** (task 0051, `recentData.ts`, `Recent.tsx`): docked at the board's right
  edge (plain or grouped), always in view, its own scroll; about a quarter of the board's row,
  200 to 272 px. When the row can't hold the board's 210 px and the rail's 200 px (a narrow
  window, or a wide pane), the rail goes under the board, full width, at most a third of the
  window high (`paneSize.ts` `railBeside`, the `stacked` row): moved, never hidden. The pane's
  widest leaves the content 210 px in every view, counted in the page's pixels under ⌘+ zoom
  (`clampPane`, `layoutWidth`), so it doesn't change width with the view. In a narrow page it
  gives way first, below the 360 px you can drag it to, down to 240 px (`PANE_FLOOR`, the `.pane`
  min-width): the board keeps 210 px while the page is at least 660 px wide (an 820 px window up
  to 115% zoom, 1000 px at 150%); narrower than that the board gets less (97 px at 820 × 150%),
  and the pane never runs past the window. What landed on main (the latest
  Testing → Your check stage line, **merged**) and what moved into Done (`doneAt`, **accepted**;
  the owner's own task's Done too), each a row of its own, for the project in view (every project
  with All, each row with its project), whatever the filter bar says. Only what's still finished:
  a task sent back has no row; goals and features have none. Newest first, grouped by day (Today,
  Yesterday, then "Sun 11 Oct"), each day with its count; when in words ("4 min ago", "2 h ago",
  "yesterday 18:20", then the date), the stamp in the tooltip. Chips All · Merged · Accepted,
  remembered (`muster.recentKind`). The last 7 days, **Show older · n more** adds a week. Each row
  opens its task in the pane, and its second line says who built it (the session that took it
  into Building) and its time and tokens (0025's totals, `task_sessions`, read one task at a time
  for the rows in view and again only when a file changes). Done cards say "done 4 min ago"
  (`CardStatus.tsx`).
- **Waiting for you:** what the agents wait on the owner for (escalated questions, Your check,
  blocked on you or by a permission, `queue::needs_owner`), then the owner's own tasks to do and
  doing, each opening its task (`human.ts` `waitingForYou`: the needs-you count's tasks). Your check comes first, by
  effort (`WaitingList.tsx`, task 0043).
- **Your check says what to do** (task 0043, `yourCheck.ts`, `EffortGroups.tsx`):
  - Each Your check card says **Your turn** (not "on it: owner"; the pane's status line too) and
    its next step: the first open owner criterion as an instruction ("Try: …", or "Check: …"
    when it isn't written as something the owner does), cut to 90 characters with the whole in
    its tooltip and the pane, "· n more", or "Nothing to try: Accept".
  - The next step is, in order: an open question escalated to the owner ("Answer Q2: …"), a
    block on them or by a permission ("Unblock: …"), the first open owner criterion, else
    "Nothing to try: Accept".
  - Effort is derived from the open owner criteria: any mentioning a session, terminal or command
    (whole words) → Needs a session; any other open one, an open escalated question or a block on
    the owner → One look; else Just accept. `— effort: look|session`
    on a criterion overrides it, and its words leave it out.
  - The board's Your check column (each band's too, with Group by) and Waiting for you group by
    effort, in that order, each with its count; an empty group keeps its heading.
  - **Accept all** in Just accept: the first click freezes the tasks shown (tasks only, not
    blocked) and names them in place, with the focus on Cancel; the second sends them to the
    core's `accept_all` (lib.rs, one call per project), which checks every id and the stamp first, then passes each from Your check as the
    owner, as Accept does (`ops::pass_task` with `from: your-check`, so the commit the merge
    recorded stays). The core's rules refuse an open owner criterion, a block or a task that moved
    on, and `accept_all_in` also refuses a goal or feature and an open escalated question (read
    just before each pass), with the reason, and the rest go on. Each accepted task's log gets its
    stage line. Esc cancels the confirm (a capture listener, so it never closes the pane), and
    the focus goes back to the button. The result line is kept by place (board, band, Waiting for
    you) outside the column, so it outlives an emptied column, and goes on the next change
    (`staleResult`). Refusals read without the leading id or `muster` hints.
  - The pane's checklist lists the open owner criteria first, each with its effort, **Tick** and
    a button per view it names (Board, Table, Timeline, Graph, Appearance in any case; Helper,
    Waiting for you, Activity, Problems, Projects, Help only capitalized). Ticked ones follow with
    **Untick**.
- **A limit on Your check per project** (task 0050, `check_limit.rs`, `yourCheckLimit.ts`,
  `CheckLimit.tsx`): `projects.json`'s `yourCheck` maps a queue folder (the project's key, as
  `gate`, `names` and `hidden` do) to 1–50 or `"off"`, so a rename keeps it and two projects shown
  under one name keep their own; a folder not in it, or with a bad value, has 5, and a bad value
  never stops the rest of the file loading (`check_limit::read_limits`). The core counts each
  queue's waiting tasks (`check_limit::waiting`: stage your-check, level task, not `for: human`)
  and sends it with the limit in the state (`QueueInfo.yourCheck`, `yourCheckLimit`, null for off;
  the `state` command fills the limit from projects.json with `check_limit::apply`). Over the limit
  (more than it, not equal), the Your check header reads "7 of 5" (the project's whole pile,
  whatever the filters show) in `--warn` (the orange on dark themes, #A84300 on light ones:
  4.5:1) on the plain board. A Group by band's Your check keeps its own count, orange (with the
  pile in its title) when it holds tasks of a project over its limit. Under the plain board's
  header a short line ("Over your limit of 5", or "2 projects over limit" in All; at most 24
  characters) has a reserved two-line place; above the board, plain or grouped, a full-width
  line with a reserved one-line place says "7 of 5 in Your check: check these before more
  start." or, in All, "Over your limit in 2 projects: …"; it never wraps (nowrap, cut with an
  ellipsis at the narrowest) and its title has it whole, with each project over in All. Over, the column stays open even when a filter
  empties it. The sidebar row of a project over says "7 in Your check, limit 5" under its activity
  line. Settings › Projects has a Your check column: "limit 5 · 7 waiting", an **Off** chip and a
  1–50 field with **Set** (`set_your_check_limit` by the project's key, checked by the core; the
  board refreshes at once); a hidden project shows "—". On the command line `next`, `start` and
  `ready --parallel` print `muster: 7 tasks wait in Your check (limit 5): the owner's review is the
  bottleneck.` on stderr before their work, and `list` prints it between its Your check and Done
  groups (stdout is cut there, `list_parts`); with `--json` it goes on stderr after the JSON. Exit
  codes don't change. The CLI finds the queue's entry by folder, through symlinks too.
  Every change to projects.json, the app's and `muster projects set`'s, goes through
  `projects::edit`: an exclusive flock on `projects.json.lock` held across load, change and
  save, and a temporary file per writer (process id and counter) renamed over it, so concurrent
  writers keep each other's edits and a reader never sees half a file.
- **Task panel** (loads on first open): the task rendered as Markdown with a Raw switch,
  open questions with an answer box (⌘↩), the files as thumbnails that open large (← → between
  them, Esc to close), who's on it, what it waits on, and the commit.
- **Pick-one questions** (task 0049, `choices.rs`, `PickOne.tsx`): a question asked with options
  shows them as chips in the pane's open questions and in its Waiting for you entry, every option
  on screen, the asker's pick marked **pick** and "with no answer: a" after them. One click
  answers (`answer_pick`): the answer is `(b) <the option's text>`, plus the answer box's words
  after " — " when there are any; the box stays below for something else. Escalated ones keep
  the orange edge, as before.
- **Work** (task 0025, below the description and check-ins in the task panel): every Claude Code
  session that worked on the task, found by the `session <8 hex>` in its claim or `held-by` and in
  the who of its log lines (the older `label (session 7b5e8856)` and 0030's stamped
  `label (checkout · session 7b5e8856 · cc-hg)`). One row each: short id and label, the stage it
  held, whether it holds the task now (0030's holder: the claim, or `held-by` in Testing and Your
  check), started, ended or **live**, minutes and tokens. Then the totals against the estimates ("Tokens 70k of 100k (70%)"), the tokens by kind,
  and a progress guess from ticked criteria and the stage, labelled as a guess. **View** opens that
  session's transcript below the rows as Markdown: one line per prompt, reply and tool call, tool
  output and thinking left out, the latest 1,500 entries, every punctuation mark escaped so nothing
  in it renders as a link, image or HTML. Only a session the task names can be viewed.
  - **Where the numbers come from** (`transcripts.rs`, the one reader 0017's actuals will use too:
    `transcripts::actuals`): `~/.claude*/projects/<folder key>/<session id>.jsonl`, every account
    folder once (`.claude-hg/projects` linking to `.claude/projects` isn't counted twice), plus the
    session's `subagents/*.jsonl` for tokens. Usage counts once per reply (Claude Code repeats it
    on each content block); minutes are first to last time stamp. Read on this Mac and sent nowhere;
    the window gets counts, ids and times, never a path. A session not found reads "not on this
    Mac". Transcripts over 128 MB and lines over 8 MB are skipped, and summaries are cached by
    size and modified time (512 files).
  - **Live:** a transcript (or one of its subagents') changed in the last 5 minutes. The card shows
    a small dot by its id while any of the task's sessions is live, worked out in the state read
    from file times only (20 cards × 50 sessions: about 0.5 ms). Done tasks aren't checked.
  - **Speed:** 50 transcripts of 1,000 lines each read in about 0.3 s cold in a debug build and
    0.5 ms warm. On the owner's Mac, five real sessions (up to 60 MB) read in 0.15 s cold in a
    release build, with no line skipped.
- **New task** (`+ New task`, ⌘N, in the docked pane): project, type, priority, tags, a one-line title, a Markdown
  description, and files by paste, drop or picking. The id comes from the core under the queue's
  lock. The task lands in To do and opens.
- **Tags:** a `tags:` list in the front matter. Suggested: spike, chore, tech-debt, code, docs,
  design, research, test, perf, security, and any other word works. They're cleaned the same way
  everywhere (lowercase, hyphens, 24 characters, 8 at most). On cards, in the filter, in New task
  and on each task. `muster new --tags`, `muster set <id> tags`, `muster list --tag`.
- **Estimates:** `est-time` (45m, 2h, 1.5d; a day is 8 agent hours) and `est-tokens` (150k, 1.2M)
  on each task, cleaned the same way everywhere. On cards (`~2h · 500k`), in the table (sortable),
  in New task and on each task. The filter bar totals what's shown and still open ("≈ 10 d · 20.1M
  tokens left, 11 of 13 open tasks estimated"). The helper estimates every task it files with a size
  guide (S 30m/150k, M 2h/500k, L 1d/2M, XL: split). `muster new --time --tokens`, `muster set <id>
  time|tokens`. Actual time and tokens per task can come later from Drover (Drover task 0018).
- **Managing a task** (top of its panel): status (inbox, to do, blocked with what it waits on,
  done; moving a claimed task releases the claim; the app never claims), priority, type, rename,
  a note to the log, and files by paste, drop or **Attach…**. Every change is logged with the
  time and "owner".
- **Answering** writes file contract v1, so the asking agent gets it pushed by the answer hook.
- **What the app writes**, all through the core's rules, under the queue's lock, logged with the
  time and "owner": new tasks (and their ids), status moves, priority, type and title, log notes,
  attachments, answers to questions, and new queues (`New queue…`). Its own settings go in
  `projects.json` and `settings.json`, and the helper's transcripts in its settings folder. **The app never claims a
  task.** Claims belong to the agents doing the work.
- **Activity:** queue commits and changed task files, newest first. **Problems:** the same
  checks as `manifest.py --check`, done natively.
- **Live:** refreshes every 2 s while the window is visible; the window never refreshes while
  hidden (the Dock badge reads on its own, below).
  On the real queues (21 tasks): 19 ms for the first read, 0.6 ms after that. Startup code is
  229 KB (72 KB gzipped). The task panel is a separate 102 KB chunk. The app is 5.2 MB.
  **2026-10-13 (0006):** on the real queues (662 tasks), 180 ms cold and 7 ms warm (release
  build, `examples/probe.rs`). Startup code is 329 KB (106 KB gzipped), the task panel 42 KB.
  The app is 8.2 MB (`pnpm release:build`), the binary 8.4 MB.
- **Dock badge** (task 0006): Muster's Dock icon shows how many tasks need the owner in every
  shown project (the needs-you count; no badge at 0), so it's seen without opening the window.
  `badge.rs` counts in the core and sets it from Rust (`set_badge_count`), so the window has no
  permission for it (still `core:default` only). Each window poll sets it when the count
  changes; while the window is hidden and doesn't poll, a thread reads the queues every 15 s for
  the badge alone (7 ms each on the real queues), and applies Claude Code's session states as the
  poll does (0048), so a session waiting for input stays on the badge. That starts at most one
  `claude agents` run per 15 s, on its own thread and never waited on, and only while some task
  in Building or Testing has a stamped holder. `needs_you` mirrors `human.ts`'s `needsYou`
  (the owner's own paused task left out, 0045);
  `app/src/needs-cases.json` holds both to the same answers. It adds 16 KB to the binary.
- **Mac notifications** (task 0052): one notification when something new waits for the owner.
  `notify.rs` splits what the badge counts into items, one per reason a task waits (Your check,
  keyed by when it came in, so a second arrival is new; each escalated question; blocked on you
  or by a permission; its session waiting for input; the owner's own task) and one per lesson to
  approve, and keeps the keys it has seen. Each window poll and each hidden read (above) passes
  its state through it (`notify::after_read`), so there are no extra reads: the lessons' rules
  come from the same `(len, mtime)` cache as their count (`lessons::proposed`), and while
  Settings › Notifications is Off (`settings.json` `notify`, default on) nothing is read for them.
  New items are posted once, several in one notification (the first three named, the rest
  counted; each line at most 200 characters). Taken as seen without a notification: everything
  on the first read after start, after Off, and in a project the last read didn't have (no
  flood); a session waiting for input until a read before knew the sessions (`agents::known`:
  some account read, or none wanted), so one waiting at launch isn't posted when the first
  `claude agents` read lands; and what the window shows while it has the focus (its project, or
  All; the window tells the core with `show_project_title`). News from another project is
  neither posted nor seen while the window has the focus (`mac-notification-sys` shows no banner
  while the app is frontmost) and is posted at the first read once it hasn't. The keys are kept
  in `notified.json` beside it: the last 500, plus every one still waiting, so nothing comes up
  twice. Permission: asked at the first thing to say, never at start
  (`tauri-plugin-notification` 2.6.0 on desktop grants it itself; macOS asks the owner on the
  first post; no usage string exists for notifications). Posted from Rust through the plugin, so
  the window has no notification permission (still `core:default` only); local only, the plugin
  has no network code. Under `tauri dev` they show as Terminal's (the plugin's rule for unbundled
  builds); the built app shows them as Muster's.

## The AI helper

The **Helper** tab: a chat per project that does what the capture session does. You write ideas,
bugs and tasks (and paste screenshots). It checks the queue for duplicates, reads the project to
understand the code, files one task per point with your words, details and acceptance criteria,
and asks in plain words when something's unclear, with its own pick. You see its replies and a
folded "steps" line, never file edits.

- It runs your own `claude` with the account you pick once (decision 0002), in restricted mode:
  only `muster` and read-only tools, file reading inside the project folder, and anything else
  refused. Router and token variables are stripped. Model: Sonnet, or Haiku or Opus picked in
  Settings (0036; saved in `helper/helper.json`, and anything else there reads as Sonnet).
- **Accounts:** the picker shows each `~/.claude*` folder's login (from `claude auth status`, no model
  call), with **Change account** any time. An account that only works through a shell command
  with a token (like `cc-hg`) isn't logged in on its own: the helper says so in plain words and
  offers another account or **Log in…** (Terminal on that account's own `claude auth login`).
- Conversations resume per project. **New conversation** starts fresh, and **Stop** ends a turn.
  The transcript lives in the app's settings folder (`helper/<project>.jsonl`).
- Checked end to end on a throwaway project: a two-item message filed both items (a high-priority
  bug and an idea) through `muster new`, for $0.05–0.07 a turn. Writes, deletes and reads outside the
  project were refused. The queue check was clean afterwards.

## The command line

The app's binary is also the `muster` command (`bin/muster` runs it). It uses the same Rust code as
the app (`ops.rs`), so the app and the command line can't disagree. It picks the queue from
`--queue <name|folder>`, then `$MUSTER_ROOT`, then the current folder's: its repo's `.muster/`
(the main checkout's, found through git's common folder from any worktree, 0005), else the nearest
old queue (a queue, or a `tasks/` beside or above it); a task named by another project's key
(`br-7`) picks that project's queue instead.

`muster` is linked into `~/.local/bin`, so it works in any terminal and in any Claude Code
session as `! muster …`. `muster help` lists everything, and `muster help <command>` shows usage,
options and examples (`--json` for scripts).

```
muster new "Board flickers on resize" --type bug --priority high --body "Steps: …"
muster list [--status todo] [--json]      muster show 0007 [--json]
muster block 0007 --on you "pick a colour" muster set 0007 priority low
muster note 0007 "Checked on the Mac"     muster attach 0007 shot.png
muster answer 0007 Q2 "Use 5173"          muster check      muster projects      muster init ~/playground/x
```

**Claims** (task 0002): `muster claim <id>` or `muster next` takes a Ready task for you, one
per holder (`--as`), and says which queue it claimed in. A claim made in a queue that came from
`MUSTER_ROOT` but isn't the current folder's own queue is refused unless `--queue` says so. The
claim has a **lease**: `heartbeat`, and the holder's notes, passes and questions, renew it, and
`muster reclaim` (or **Release** on a stale card) returns a claim quiet past the lease to Ready:
12 h, or this Mac's lease from Settings (0036).
**Questions:** `muster ask`, `escalate`, `inbox` and `answer` follow the file contract. `bin/task`
is a stand-in that runs them. The answer hook still reads the mailbox itself, for speed.
**Pick-one questions** (task 0049): `muster ask <id> "<question>" --option "<text>"…
[--pick <a|1>] [--no-answer <a|1>]` writes 2 to 6 options (one line, 200 characters each) under
the question as plain indented lines (`- (a) … (my pick)`, `With no answer: (a)`), so file
contract v1 readers see text; `muster answer <id> Qn --pick b ["<more>"]` answers with the
option's text. Refusals (count, length, a pick or fallback that isn't an option, a question
without options) write nothing. `muster escalate` prints the options for `WAITING-FOR-OWNER.md`.
`bin/task` passes no options: it asks in words and answers any question in words.
**Ping** (task 0039): `muster ping <id>` asks the holder's session for a check-in (Building),
`muster ping <id> "<message>"` sends it a message (Building or Testing), the same rules as the
app's buttons (`ping.rs`): only the session in the holder's stamp, refused with why otherwise,
logged on the task.
**Pause** (task 0045): `muster pause <id> ["<why>"]` and `muster resume <id>`, the app's rules
(`pause.rs`): Backlog through Testing; while paused, `claim`, `next`, `start`, `ready` and every
pass refuse it and `reclaim` skips it; the holder's session in Building or Testing gets a stop
request, and on resume a carry-on (and in Building a fresh lease). `muster list` marks it
`[paused since …]`, INDEX.md "paused". `muster check` warns about old paused blocks (0059), and
`muster check --fix-pauses [--dry-run]` lists them in every queue Muster knows before changing
anything, then unblocks each (an old-layout file moves back to `todo/`, stage Ready) and pauses it
from the original time: a `paused (why)` and `unblocked` line with that stamp, then a line stamped
now saying it was converted. A dry run changes nothing; a task it can't unblock is named, left as
it was, and the command exits 1.
**Session states** (task 0048): `muster list --json` gives each task `sessionState` (`working`,
`needs-input`, `ended`, `unknown`), read from `claude agents --json` for that listing only, under
the caller's `$HOME`; no other command runs it, a write never does.
**The review of record** (task 0027, `review.rs`): `muster review <id>` prints the brief for the
next round of a started task (the range: merge base..tip the first time, then the last reviewed
commit..tip while the branch still has it; the task quoted as data, the builder's answers to
re-check, the owner's lessons, the JSON shape) and runs no agent: a session that didn't build it
runs `/deep-review` on the range and records the result with `--record -` (strict JSON:
`deny_unknown_fields`, every text one line and capped, `head` must be the branch's tip, every
answered finding needs an answer, findings under confidence 75 refused). The truth is
`reviews/<id>.json` in the queue; the task gets a `## Review` section rendered from it (above
Comments and Log; `muster edit` leaves it out and refuses a new `## Review` line), `review: round
<n> · <open> open · <answered> answered` in the front matter (`Task::review_round`,
`review_open`, `review_answered`; a malformed one is a `check` problem) and a log line per change.
After a record, open findings (not nits) return it to Building (rounds 1 and 2, `ops::return_task`)
or block it on you (round 3 on, `ops::block_as`), each after the lock is let go. `--fixed` and
`--dispute` are the holder's in Building (keep the claim alive); `--settle` is the owner's.
`parallel::merge` calls `review::merge_check` before its lock, rebase and gate (clean at the
tip, or the owner's override, said in the report) and `review::carry` after each rebase.
**Lessons** (`lessons.rs`): `reviews/lessons.json` (truth) and `lessons.md` (rendered) in the
queue (`layout::reviews_dir`, `layout::lessons_file`: `.muster/reviews/` and `.muster/lessons.md`
in a repo's queue; `muster migrate` copies both, never moves them). `propose_locked` runs under the lock of each record and settle: a `kind` confirmed on 2+
tasks proposes one for builders, false-positive on 2+ one for reviewers, never one already
proposed, accepted or rejected. `muster lessons accept|reject` is the owner's (`--as owner`);
over 40 live, the least recently mattered expire; 60 days without a finding of its kind or an
acceptance, an accepted one reads as expired. `muster start` prints the live builders' lessons
whose areas overlap the task's `touches:` (`parallel::touches_overlap`; all of them without
touches). `QueueInfo::lessons_waiting` feeds the window's needs-you count, the sidebar, Waiting
for you's Lessons block and the Dock badge (`badge::dock_count`); `muster check` lists it.

## How the pieces talk

```
owner ──chat──▶ capture ──writes──▶ queue/todo/NNNN-*.md ◀──mv──  coder (claims)
                   ▲                         │  ## Questions  ◀── task ask (session recorded in mail/asks.json)
                   │ escalates (plain terms) │
owner ◀────────────┘ WAITING-FOR-OWNER.md    └─ task answer ─▶ mail/<session8>/NNNN-Qn.md ─▶ answer-hook ─▶ coder's context
```

- **Keys** (task 0033, `key.rs` and `taskKey.ts`, held to `app/src/key-cases.json`): a task is
  shown and typed as `mu-12`, its project's prefix and its number; files, `depends:` and ids in
  JSON keep `0012`. The prefix is 2 to 4 lowercase letters in the queue's README.md front matter
  (`prefix: mu`, read only by `key::stored_prefix`), else the first 2 letters of the project's
  name (3 or 4 when another project Muster knows starts the same), stored by the first write
  under the queue's lock (`key::pin_prefix`) so it stays. Every command takes `mu-12`,
  `MU-12`, `12` or `0012`; another project's key runs in its queue (refused against a `--queue`
  that names another). Cards, rows, the timeline, the graph, the pane, the parent line, the
  activity feed, `muster list`/`show`, what commands print, branches (`task/mu-12-slug`) and new
  log lines, INDEX.md, `muster check` and the core's refusals (`key::name_tasks`, at the command
  line's and the window's edge) name tasks by key, and a key in a task's text, log, comments or
  questions links to it. `manifest.py` and `tasks.json`, which scripts read, keep ids.
- **Ids:** four digits, taken in order under the queue's lock by whoever creates the task: `muster
  new`, the app's New task, or the helper (through `muster new`). Taking them under the lock is
  what prevents two tasks getting the same id. The Python `bin/task` doesn't create tasks yet
  (task 0002 has it allocate the same way, or hand over to `muster`). `manifest.py` reports
  `next_id` and catches duplicates left from before. A coder that finds something new drops a
  note in `inbox/`.
- **Locks:** `flock` on `mail/.lock` for every task-file write through the tools (`muster`, the
  app, `bin/task`). Claiming is still an atomic `mv` from `todo/` to `doing/`, done by hand.
- **Time:** the tools stamp times. Agents' own timestamps aren't trusted (`DOGFOOD.md` 37).
- **Git:** each queue is its own git repo, outside every code repo. Capture commits named files
  (never `git add -A` on the queue while others write, `DOGFOOD.md` 31).

## Checked

- App: `pnpm check` green (typecheck, 7 UI tests, build, `cargo fmt`, clippy with warnings as
  errors, 26 Rust tests (queue reading, contract writing; projects: list, add, create, refuse
  inside a repo, settings round trip; task operations: create with ids and template, refusals,
  flat layout, moves and claim release, fields and notes, attachments that never overwrite or
  escape, base64; the command line), plus a second interop test (tasks the app makes pass
  `manifest.py --check` and work with `bin/task`), plus `tests/interop.rs`: a question asked with `bin/task`, answered by
  the app, is read back by `bin/task inbox`, refused as already answered, and passes
  `manifest.py --check`). The real queues were read by `examples/probe.rs`. The release app was
  built and launched. Its screens haven't been looked at by a person yet.
- Hermetic tests (task 0062): every read of the projects Muster knows (`key::known`,
  `cli::base`, `cli::config_file`, the app's `base`) starts at `projects::places()`, the folder
  queues are found under and projects.json. The app and `muster` get this Mac's
  (`$MUSTER_BASE` or `~/playground`, and `~/Library/Application Support/ai.sweetlemon.muster/projects.json`).
  A lib test gets empty ones in a temp folder of its own thread, unless it sets its own with
  `projects::use_places(Places { base, config_file })` for the rest of its thread. A thread the
  test starts gets its own empty ones. In a test build, `projects::load`, `save`, `edit` and
  `queue::discover` panic on this Mac's real projects.json or `~/playground` and name
  `projects::use_places`. So `check --fix-pauses` with no `--queue`, and the paused lane's
  `find_all` and `convert_listed` over the app's queues (0059), see only a test's own queues, and
  a change that sent them to this Mac's would fail the test before reading anything (a test run
  once converted Drover's real dr-22 that way). Integration tests build the core without `cfg(test)`, so they have no
  guard: the ones that call the core in-process call `use_places` (`no_projects` in
  `tests/interop.rs`), and every `muster` they run gets a temp `HOME` of its own. Checked by
  running the whole suite with `HOME` set to an empty folder whose projects.json folder and
  `playground` can't be read.
- `bin/gate` runs every step in a home of its own (task 0062): `HOME`, `XDG_CONFIG_HOME` and
  `MUSTER_BASE` in a temporary folder it removes at the end, `MUSTER_ROOT` and `TASKS_ROOT` unset.
  `CARGO_HOME`, `RUSTUP_HOME`, `~/.cargo/bin` and pnpm's store and cache stay this Mac's, so
  `pnpm install --frozen-lockfile` in a fresh worktree still links from the real store.

- `bin/test_task.py`: 15 tests (ask/answer/deliver, numbering, refusal of an already answered
  question, earlier questions listed, old unnumbered questions, sessions from env and text,
  refusals, control characters and tabs stripped, uppercase session ids, escalate, hook given bad
  input, the inbox command, atomic manifest).
- Live: a self-test question asked and answered through `task` appeared in the asking session's
  context after its next tool call (2026-10-12).
- Drover's own implementation of the file contract found three contract bugs within an hour, all
  fixed (`DOGFOOD.md` 36).
- Hook cost: about 10 ms per tool call with an empty mailbox.

## `.muster/` in the repo (decision 0003, task 0005 step 1)

Built: the layout, local mode, one queue for every worktree, discovery and `muster migrate`. No
queue has moved yet; each queue is still `<project>/tasks` until it's migrated, and both layouts
keep working (`layout.rs` is the one place that says where a layout keeps what).

- **Layout:** `.muster/` in the repo's main checkout: `config.toml` (`local = true|false`, true
  as `init` and `migrate` write it; `prefix = "mu"`, the task keys' prefix (0033), which
  `key::stored_prefix`/`store_prefix` read and write there for a `.muster` queue, and the first
  write pins when it's missing, never in a README (an old queue keeps it in its README's front
  matter); other keys, quoted or not, and other tools' multi-line values are left alone; `init
  --prefix` sets it, and `migrate` writes the old queue's (stored, or the default its keys had),
  so `muster list` shows the same keys before and after), `tasks/NNNN-slug.md`
  (flat, never moved, `stage:` the only truth, no `status:` written or read), `backlog/` and
  `backlog/dropped/`, `attachments/NNNN/`, `mail/` (asks, mailboxes, the lock), `INDEX.md`,
  `tasks.json`, `README.md`. Every write goes through the same lock (`mail/.lock`) and refreshes
  `INDEX.md`. `muster check` also reports a `config.toml` it can't read and a local queue whose
  `.gitignore` would let it into git.
- **Local mode (the default):** `.muster/.gitignore` ignores everything but itself and
  `config.toml`. A queue is found from the repo's main checkout or any worktree (`git rev-parse
  --git-common-dir`, which must be a `.git` folder), so every session uses the main checkout's one
  copy and its one lock: two worktrees claiming one task at once, one wins (tested 10 rounds with
  real `git worktree add`). A repo without a main checkout (a bare repo's worktree, a submodule,
  `--separate-git-dir`) has no queue, and `init` refuses it; the walk up to a `.muster` folder is
  only for folders in no repo at all, so a worktree git can't read never falls back to its own
  copy. Committed mode (`--committed`) has the same single copy for now; its `.gitignore`
  leaves out `/mail/`, `/INDEX.md` and `/tasks.json`, and a merge's "is main clean" check passes
  over `.muster/`.
- **Finding it:** `queue_of` in `cli.rs` asks `layout::find` first, then the old rules; the
  `MUSTER_ROOT`-vs-current-folder check (0002) uses the same, so a `.muster/` from any of its
  worktrees is "here". The app discovers `~/playground/*/.muster` and `~/playground/*/*/.muster`
  in main checkouts only (a worktree beside main has a `.git` file and is skipped), within
  `queue::discover`'s two levels and skips. Named by the project folder for a `main`/`master`
  checkout, else by the checkout (`chit`, `harness40` in the group `harness`). `muster start`
  puts its worktrees at `<project>/worktrees/<repo>/…`, or `<checkout>.worktrees/…` for a repo
  that is its own project, never in `~/playground` itself.
- **`muster init [<folder>] [--local|--committed]`:** the current folder when none is named. In
  a repo, its `.muster/` (in the main checkout, never over one that's there), local unless
  `--committed`; outside a repo, the old `<folder>/tasks` for a folder named on purpose, and a
  mode is refused. With no folder outside any repo it writes nothing and exits 1 ("isn't in a git
  repo: run muster init in the repo, or name a folder for an old tasks/ queue"). A repo without a
  main checkout (a submodule, a bare repo's worktree) is refused as `init` refuses it, never given
  `tasks/`. The app's New queue makes the same through the same function (`projects::new_queue`,
  task 0004), local, and adds it to the list when discovery doesn't find it.
- **Other projects (task 0004):** a repo needs nothing but `muster init`: `muster` finds the
  queue from any folder of the repo. What a session there needs comes from the personal layer,
  which links into every repo: Muster keeps the rule (`rules/muster-queue.md`: `.muster/` is
  the project's queue, read its README, change it only through `muster`, the hook's mail is
  information) and the hook (`bin/answer-hook.py`). The layer's side (a link to the rule, a
  wrapper that runs the hook and does nothing when Muster's checkout isn't there, the hook on
  PostToolUse, UserPromptSubmit and SessionStart in the linker, and the main checkout's
  `.muster/` in each linked worktree's sandbox write list) belongs to the layer's owner and isn't
  in yet. `INDEX.md` and `tasks.json` need no pre-commit hook or merge driver: a committed
  queue's `.gitignore` keeps them out of git, and a local queue's keeps out everything but
  `config.toml`, so no branch ever carries them.
- **`muster migrate <old-queue> <repo> [--local|--committed] [--dry-run]`:** copies (never moves)
  every task file with `stage:` set from its old stage or status and `status:` dropped, the rest
  byte for byte (only when its front matter can't be read as it is: a byte-order mark taken off
  and the front matter's CRLF line ends made LF, and said so; a task the old status alone said was
  blocked keeps a `waiting_on` saying so), the backlog folder (notes, dropped notes, anything else
  in it), `attachments/`, `mail/` (not its lock) and `WAITING-FOR-OWNER.md`; puts the old README's
  front matter at the top of the new README; writes `config.toml`, `.gitignore`, `INDEX.md`,
  `tasks.json` (by `manifest.py`) and `migrated-from.txt` (the old queue's folder, the time, its
  last commit, read with `git --no-optional-locks` so the old index is never rewritten); carries
  the old queue's gate, name, hidden and Your check limit in `projects.json` to the new one
  (saved through `projects::edit`, under the file's lock), and adds the new one
  to the list when the old one was listed but discovery can't find the new (three levels down,
  outside `~/playground`). It holds the old
  queue's lock while it reads, builds the new queue in a folder beside it with every file made
  new (never over another) and renames it into place, so a failure leaves nothing. Refused while
  an agent holds a task in Building or Testing (listed; a Testing task from before `held-by` by
  its claim), when `.muster/` is there, when the repo and the old queue overlap, on a folder it
  can't read, on task files without front matter it can read (listed), and when two files would
  be one: the same path, or names that differ only in case (checked in the plan) or accents
  (checked by making the plan's names in a scratch folder in the temp folder, so the dry run says
  so too). Prints what it copied, notes (a stage that wasn't one, a file made plain) and what it
  didn't copy (symlinks, `bin/`, stray files). `--dry-run` prints the same plan and writes nothing
  in the repo or the old queue.
- **The tools:** `manifest.py` reads `.muster/` (`MUSTER_ROOT=<repo>/.muster`); `bin/task` runs
  muster, which finds the queue from the folder; the answer hook also reads the session
  folder's repo's `.muster/mail/` (no queue where git has no main checkout, as `layout.rs`), each
  queue's mail on its own, so one queue's problem never loses another's; the helper's brief says
  where the queue is.

The owner decided on 2026-10-12 to move each queue into the repo as `.muster/`, either committed
or local (local is what the tools write until the live state moves to `.git/muster/`). With that
came:
- A backlog of raw one-liners, which agents shape into tasks.
- Stages with roles: Backlog, Shaping, Ready, Building, Testing, Your check, Done. Blocked is a
  flag on a stage, not a stage of its own.
- Goals, features and tasks.
- Points: estimates and actuals, in minutes and tokens.
- Links, comments, and a generated `INDEX.md`.

Live claims in the repo's shared git folder (`.git/muster/`), for committed queues whose task
files have one copy per branch, are the next step of 0005. See
`docs/decisions/0003-muster-in-the-repo-backlog-and-stages.md`.

## Known limits

- **An old-layout queue needs its folder in the layer's sandbox list** (`~/playground/.claude/layer-settings.json`,
  `allowWrite`), then `link-personal-layer` re-run in that project's checkouts. Otherwise its agents
  can read tasks but not claim, log or ask (`DOGFOOD.md` 41). A `.muster/` queue is inside its
  repo: a session in the main checkout needs nothing; a worktree session needs the main checkout's
  `.muster/`, which the layer's linker is to add to each worktree it links (task 0004's layer change,
  not in yet).

- **New session id after `/clear` or a handoff:** answers to the old session's questions wait in
  its old mailbox (`task inbox --session <old>`). Fix: carry open question ids in the handoff,
  and route by task claim too.
- **The hook only runs where it's installed:** Drover's four checkouts so far. New worktrees,
  and `link-personal-layer` rewriting `settings.local.json`, can drop it. Fix: the personal
  layer's change from task 0004 (built and tested on Muster's side, waiting on the layer).
- **Capture's watcher lives in its chat session.** The app shows changes live, but nothing wakes
  a capture session when an agent asks something.
- **Files still move between status folders in the old layout** (v2), until its queue is migrated
  into `.muster/`, where they never move. There a claim is safe only because `muster claim` reads
  the stage and writes the claim under the queue's one lock (task 0002), so agents never edit the
  stage themselves.
- **A worktree session writes into the main checkout's `.muster/`.** That folder must be in the
  session's sandbox write list; a Claude Code worktree's sandbox usually allows its own folder and
  the main checkout's `.git`, not the main checkout itself (decision 0003, "Revisit when").
- **A committed queue has one copy too, for now:** every worktree writes the main checkout's
  `.muster/`, so every claim and pass rewrites tracked files there, which show up as changes to
  commit (a merge passes over them when it checks main is clean). That's why `init` and
  `migrate` write a local queue unless `--committed` is given. Live state in `.git/muster/` with
  the write-up on the claimant's branch is the next step.
- **A session counts in full on every task it names** (0025): a session that worked on three
  tasks, or a controller session that passed many, shows its whole transcript's tokens and minutes
  on each, and its live dot on each open one. Splitting a transcript by the times it held each
  task isn't built.
- **Reclaim runs when someone runs it:** `muster reclaim` or Release on a card. No scheduler runs
  it yet. A crashed session's claim waits until then.
- **A ping reaches a session only through the answer hook** (0039): a session without the hook
  installed sees a check-in request or message only when it runs `muster inbox`, and a claim
  with no session stamp can't be reached at all. Nothing asks quiet sessions on its own yet: the
  auto-nudge setting isn't built; its place is Settings → Agents (0036).
- **A pause reaches a session only through the answer hook** (0045), like a ping: a session
  that has ended its turn gets the stop or carry-on request on its next prompt. Who may pause or
  resume isn't limited to the owner, as with block and unblock.
- **Claude Code's states are a research preview** (0048): the format may change, and anything
  Muster can't read falls back to heartbeats. A coordinator's subagents share its session id, so
  their tasks never get a state; `failed`, `stopped` and an idle interactive session read as
  unknown. Only this Mac's accounts are read, and only `~/.claude-<label>` folders. When Drover
  supplies session states (Drover 0018), `agents::Fetch` is the one place the source changes.

## What's left (in order)

1. **App, next round (0006):** the Dock badge is built. Left: the owner's first look, Drover's
   look (its `src/ui`; the owner decides how far, since Muster's own palette and DESIGN.md rules
   differ). The notification is built (0052). The owner's moves (priority, unblock, a stage, Done)
   are in the task pane, through the core's rules; claims stay with agents.
2. **Files that never move:** task 0005's next step (committed mode, live state in
   `.git/muster/`), and migrating each queue (`muster migrate`). Then the `/muster` / `/mu` slash
   commands (0003).
3. **Other projects (0004):** Muster's side is built (`muster init` in a repo, New queue, the
   rule, the hook tested across two projects). Left: the personal layer's change, and a pilot on
   one more project the owner picks.
4. **Orchestration:** Muster decides who does what, Drover starts and steers the agents (owner,
   2026-10-12; Muster 0009, Drover 0018).
5. **Into Drover:** Drover task 0005 (task creator per project) and 0015 (waiting list) are the
   first pieces. The app's screens move into Drover when it's ready.
6. **Review of record, what's left (0027):** the owner's first real round (AC1). Then what needs
   the personal layer or GitHub, each the owner's call: the engine's ledgers and `/pr learn` get
   Muster's labels (they key rows on a PR number, so a task needs a place in their schema);
   `sweetlemonai/muster` in the engine's `repos` and a token budget for its light tier (no such
   key today); posting to GitHub as sweetlemonai, with a login check before every post, comes back
   when PRs do. The review's and lessons' files move into the live state with 0005's store.
