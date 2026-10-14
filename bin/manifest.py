#!/usr/bin/env python3
"""Build tasks.json from the task files and check the queue.

The task files are the source of truth; tasks.json is generated and never edited by hand.
Run: MUSTER_ROOT=<queue> python3 -I bin/manifest.py            (writes tasks.json, prints problems)
     python3 -I bin/manifest.py --check    (prints problems only; exit 1 if any)
"""
import json
import os
import re
import sys
from datetime import datetime
from pathlib import Path

_root = os.environ.get("MUSTER_ROOT") or os.environ.get("TASKS_ROOT")
if not _root:
    sys.exit("muster: set MUSTER_ROOT to the task queue folder")
ROOT = Path(_root).resolve()
FOLDERS = ["inbox", "todo", "doing", "blocked", "done"]
# The new layout (task 0005, layout.rs): a repo's `.muster/`, task files flat in `tasks/` (never
# moved), `stage:` the only truth and no `status:` written, notes in `backlog/`. Otherwise the old
# `<project>/tasks`, where the status folder is the status.
MUSTER = ROOT.name == ".muster"
# The stages (decision 0003). `stage:` is the truth; an old file's status says it otherwise.
# The rules live in the app's core (ops.rs); this only reads them.
STAGES = ["backlog", "shaping", "ready", "building", "testing", "your-check", "done"]
LEGACY_STAGE = {"inbox": "backlog", "todo": "ready", "doing": "building", "blocked": "ready", "done": "done"}
# Goal → Feature → Task (decision 0003); a file without `level:` is a task.
LEVELS = ["goal", "feature", "task"]
# Who does it (0022): `for:` is ai (an agent; also a file without one) or human (the owner's own).
FOR_WHO = ["ai", "human"]
# A heartbeat older than this is stale: the default claim lease, 12 h. The lease set in the app's
# Settings (0036, settings.json) is not read here; `muster check` and `muster reclaim` follow it.
STALE_HOURS = 12


def front_matter(text):
    m = re.match(r"---\n(.*?)\n---\n", text, re.S)
    if not m:
        return None, text
    fields = {}
    for line in m.group(1).splitlines():
        k, sep, v = line.partition(":")
        if sep and not line.startswith(" "):
            v = v.split("  #")[0].strip()
            fields[k.strip()] = v
    return fields, text[m.end():]


def parse_depends(raw: str) -> tuple[list[str], list[str]]:
    """`depends:` as `[0014, 0016]` or `0014, 0016`: the ids (each once) and anything else."""
    ids: list[str] = []
    bad: list[str] = []
    # Same rules as the core (queue.rs parse_depends): split on comma, space or tab only, and an
    # id is four ASCII digits.
    for tok in re.split(r"[, \t]+", raw.strip().lstrip("[").rstrip("]")):
        if not tok:
            continue
        if re.fullmatch(r"[0-9]{4}", tok):
            if tok not in ids:
                ids.append(tok)
        else:
            bad.append(tok[:24])
    return ids, bad


def dependency_loops(edges: dict[str, list[str]]) -> list[list[str]]:
    """Sets of two or more tasks that wait on each other, ids sorted (as the core's check)."""
    index: dict[str, int] = {}
    low: dict[str, int] = {}
    stack: list[str] = []
    on_stack: set[str] = set()
    loops: list[list[str]] = []
    for start in sorted(edges):
        if start in index:
            continue
        work = [(start, 0)]
        while work:
            v, i = work[-1]
            if i == 0 and v not in index:
                index[v] = low[v] = len(index)
                stack.append(v)
                on_stack.add(v)
            out = edges.get(v, [])
            if i < len(out):
                work[-1] = (v, i + 1)
                w = out[i]
                if w not in edges:
                    continue
                if w not in index:
                    work.append((w, 0))
                elif w in on_stack:
                    low[v] = min(low[v], index[w])
                continue
            work.pop()
            if work:
                p = work[-1][0]
                low[p] = min(low[p], low[v])
            if low[v] == index[v]:
                group = []
                while True:
                    w = stack.pop()
                    on_stack.discard(w)
                    group.append(w)
                    if w == v:
                        break
                if len(group) > 1:
                    loops.append(sorted(group))
    return sorted(loops)


def stage_of(fields: dict, folder: str) -> str:
    stage = fields.get("stage", "").lower()
    if stage in STAGES:
        return stage
    return LEGACY_STAGE.get(fields.get("status") or folder, "backlog")


def kind_of(raw: str) -> str:
    """A type as muster reads it (ops.rs read_kind): lowercase, the old `task` as `code`, anything
    else (an old `idea`) as it is."""
    kind = raw.strip().lower()
    return "code" if kind == "task" else kind


# Why a task is blocked (task 0024, the app core's block.rs): `blocked-kind:` holds one of these,
# next to `waiting_on` (the detail). A block on tasks is never written: it's read from depends.
TYPED_KINDS = ["you", "permission", "question", "outside"]


def after_word(text: str, word: str) -> bool:
    """Does `text` start with `word` (any case), not followed by a letter or digit? (block.rs)"""
    return text[:len(word)].lower() == word and not (text[len(word):len(word) + 1].isalnum())


def guess_kind(waiting_on: str) -> str:
    """The kind an old `waiting_on` reads as (block.rs guess): owner → you, capture → question,
    a task id → tasks, anything else → outside."""
    w = waiting_on.strip()
    if after_word(w, "owner"):
        return "you"
    if after_word(w, "capture"):
        return "question"
    if w[:4].isascii() and w[:4].isdigit() and len(w) >= 4 and not w[4:5].isalnum():
        return "tasks"
    return "outside"


# The stages a task can be paused in (0045, pause.rs PAUSABLE), and "paused" as a word in any case
# (0059, old_pause.rs says_paused).
PAUSABLE = ["backlog", "shaping", "ready", "building", "testing"]
PAUSED_WORD = re.compile(r"(?<![^\W_])paused(?![^\W_])", re.I)


def old_pause(t: dict, blocked: bool) -> bool:
    """A pause from before `muster pause` (0059, old_pause.rs is_old_pause): a task in a stage it can
    be paused in, flagged blocked (not by an old block on a task id) with a reason that says paused,
    and not paused already. `muster check --fix-pauses` makes it a pause."""
    return (blocked and not t["paused"] and t["level"] == "task" and t["blocked_kind"] != "tasks"
            and t["stage"] in PAUSABLE and bool(PAUSED_WORD.search(t["waiting_on"])))


def mirror_status(stage: str, blocked: bool) -> str:
    """The old status a stage and the blocked flag mean (ops.rs/queue.rs mirror_status)."""
    if blocked:
        return "blocked"
    return {"backlog": "inbox", "shaping": "todo", "ready": "todo", "done": "done"}.get(stage, "doing")


# A stage line muster writes in a task's log (queue.rs stage_line): who, from, to, maybe a detail.
STAGE_LINE = re.compile(r"- \d{4}-\d{2}-\d{2} \d{2}:\d{2} (.+?): stage (\S+) → (\S+)(?: \(.*\))?")


def stages_reached(body: str) -> list[str]:
    """The stages the log's stage lines moved the task into, oldest first (its last `## Log`)."""
    log = body.rsplit("\n## Log\n", 1)[1].split("\n## ", 1)[0] if "\n## Log\n" in body else ""
    out = []
    for line in log.splitlines():
        m = STAGE_LINE.fullmatch(line)
        if m and m.group(1).strip() and m.group(2) in STAGES and m.group(3) in STAGES and m.group(2) != m.group(3):
            out.append(m.group(3))
    return out


def met(stage: str, human: bool) -> bool:
    """Is a dependency in `stage` met (0042, block.rs met_at)? An agent's task once it's merged, in
    Your check or Done; the owner's own (for: human) only once it's Done: nothing merges for it."""
    return stage == "done" or (stage == "your-check" and not human)


def started(t: dict) -> bool:
    """Under way: held in Building, or past it in Testing or Your check (block.rs started)."""
    return (t["stage"] == "building" and bool(t["claimed_by"])) or t["stage"] in ("testing", "your-check")


def holds(dep: dict, under_way: bool) -> bool:
    """Does `dep` hold back a task that depends on it (block.rs holds)? Until it's met. One merged
    once and sent back holds back only tasks not started yet."""
    human = dep["for"] == "human"
    merged_once = any(met(s, human) for s in dep["_reached"])
    return not met(dep["stage"], human) and not (under_way and merged_once)


def is_task_name(name: str) -> bool:
    """`NNNN-slug.md`, not a date-named note (queue.rs is_task_name, on the same bytes)."""
    b = name.encode("utf-8")

    def digits(s: bytes) -> bool:
        return len(s) > 0 and all(48 <= c <= 57 for c in s)

    task = len(b) > 8 and digits(b[:4]) and b[4:5] == b"-" and name.endswith(".md")
    dated = len(b) > 11 and digits(b[5:7]) and b[7:8] == b"-" and digits(b[8:10]) and b[10:11] == b"-"
    return task and not dated


def backlog_dir() -> Path:
    """Where backlog notes live (backlog.rs backlog_dir): inbox/ in the v2 layout, else backlog/
    (`.muster/backlog/` too)."""
    return ROOT / ("inbox" if (ROOT / "todo").is_dir() and not MUSTER else "backlog")


def task_dirs() -> list:
    """The folders task files are read from, each with the status folder it stands for ("" when
    the file alone says): `.muster/tasks/`, or the old status folders (layout.rs task_dirs)."""
    if MUSTER:
        return [("", ROOT / "tasks")]
    return [(f, ROOT / f) for f in FOLDERS]


def is_note_file(name: str) -> bool:
    """A backlog note: `.md` in any case, not a dot-file, not a task file (backlog.rs is_note_file)."""
    return len(name) > 3 and name[-3:].lower() == ".md" and not name.startswith(".") and not is_task_name(name)


NOTE_TEXT_BYTES = 20 * 1024
MAX_NOTE_BYTES = 1024 * 1024
# Control characters and the Unicode line/paragraph separators: anything that can break a line.
CONTROL = re.compile("[\x00-\x1f\x7f-\x9f\u2028\u2029]")


def after_front_matter(text: str) -> str:
    for start in ("---\n", "---\r\n"):
        if text.startswith(start):
            rest, pos = text[len(start):], 0
            for line in re.split(r"(?<=\n)", rest):
                pos += len(line)
                if line.rstrip() == "---":
                    return rest[pos:]
    return text


def heading_text(line: str) -> str:
    """A line as plain words, heading marks off (backlog.rs heading_text)."""
    s = CONTROL.sub(" ", line).strip()
    marks = len(s) - len(s.lstrip("#"))
    after = s[marks:]
    if 1 <= marks <= 6 and (after == "" or after[0].isspace()):
        rest = after.strip()
        opened = rest.rstrip("#")
        s = opened.strip() if opened == "" or opened[-1].isspace() else rest
    return " ".join(s.split())


def note_title(text: str) -> str:
    """The note's first non-empty line, as the app shows it (backlog.rs note_title); "" for none."""
    if text.startswith("﻿"):
        text = text[1:]
    for line in after_front_matter(text).split("\n"):
        t = heading_text(line[:-1] if line.endswith("\r") else line)
        if t:
            return t if len(t) <= 200 else t[:199] + "…"
    return ""


def backlog_notes():
    """The backlog notes (file, title, added), newest first, and what's wrong with any of them."""
    notes, problems = [], []
    folder = backlog_dir()
    if not folder.is_dir():
        return notes, problems
    for p in sorted(folder.iterdir()):
        if not is_note_file(p.name):
            continue
        rel = p.relative_to(ROOT).as_posix()
        if p.is_symlink():
            problems.append(f"{rel}: a symlink; a backlog note must be a plain file")
            continue
        if not p.is_file():
            continue
        if CONTROL.search(p.name):
            problems.append(f"{rel}: control characters in a backlog note's name; rename it")
            continue
        if "\\" in p.name:
            problems.append(f"{rel}: a \\ in a backlog note's name; rename it")
            continue
        st = p.stat()
        if st.st_size > MAX_NOTE_BYTES:
            problems.append(f"{rel}: a backlog note over 1 MB, not read")
            continue
        with p.open("rb") as f:
            raw = f.read(NOTE_TEXT_BYTES)
        text = re.sub(r"[\x00-\x08\x0b-\x1f\x7f-\x9f]", "", raw.decode("utf-8", errors="replace"))
        # Line and paragraph separators are line breaks, as in backlog.rs clean_text.
        text = re.sub("[\u2028\u2029]", "\n", text)
        notes.append({"file": p.name, "title": note_title(text), "added": int(st.st_mtime)})
    notes.sort(key=lambda n: (-n["added"], n["file"]))
    return notes, problems


INDEX = "INDEX.md"
INDEX_STALE = "INDEX.md doesn't match the task files (rebuild with `muster index`)"
INDEX_REFRESH = ("INDEX.md: refresh due (the files changed outside muster; the next muster write "
                 "or `muster index` refreshes it)")
INDEX_SOURCES = re.compile(r"<!-- muster sources: ([0-9a-f]{16}) -->")
MAX_INDEX_BYTES = 16 * 1024 * 1024


def fnv1a(h: int, data: bytes) -> int:
    """64-bit FNV-1a over `data`, from `h` (index.rs fnv1a)."""
    for b in data:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def utf8_names(folder: Path) -> list:
    """The names in a folder that are UTF-8 (the core leaves the others out of the sources too)."""
    try:
        names = os.listdir(folder)
    except OSError:
        return []
    out = []
    for name in names:
        try:
            name.encode("utf-8")
        except UnicodeEncodeError:
            continue
        out.append(name)
    return out


def index_sources() -> str:
    """What INDEX.md is made from, worked out as the app's core does (index.rs sources): every
    task file in the queue folder and its status folders (symlinks followed) and every backlog
    note (plain files only), by relative path, each as its path, a NUL, its size, a NUL and its
    bytes (none over 1 MB), hashed with 64-bit FNV-1a."""
    files = []
    dirs = [("tasks/", ROOT / "tasks")] if MUSTER else [("", ROOT)] + [(f + "/", ROOT / f) for f in FOLDERS]
    for prefix, folder in dirs:
        for name in utf8_names(folder):
            if is_task_name(name) and (folder / name).is_file():
                files.append((prefix + name, folder / name))
    notes = backlog_dir()
    for name in utf8_names(notes):
        p = notes / name
        if is_note_file(name) and p.is_file() and not p.is_symlink():
            files.append((notes.name + "/" + name, p))
    files.sort(key=lambda f: f[0].encode("utf-8"))
    h = 0xCBF29CE484222325
    for rel, p in files:
        try:
            size = p.stat().st_size
        except OSError:
            size = 0
        data = b""
        if size <= MAX_NOTE_BYTES:
            try:
                data = p.read_bytes()
            except OSError:
                data = b""
        for part in (rel.encode("utf-8"), b"\0", str(size).encode("ascii"), b"\0", data):
            h = fnv1a(h, part)
    return f"{h:016x}"


def index_check():
    """What's wrong with INDEX.md (task 0018), as (problem, warning), each None or a line. Only
    muster writes it; this never does, and there being none yet is fine.
    - A problem: it isn't a plain file, or has no sources line, so muster didn't write it.
    - A warning: its sources line doesn't name what the files are now. The files changed outside
      muster (Drover's answers, a hand edit), and the next muster write or `muster index`
      refreshes it. Only muster can tell whether what the list shows changed too
      (`muster check`)."""
    p = ROOT / INDEX
    if not p.is_symlink() and not p.exists():
        return None, None
    if p.is_symlink() or not p.is_file() or p.stat().st_size > MAX_INDEX_BYTES:
        return INDEX_STALE, None
    lines = p.read_text(encoding="utf-8", errors="replace").rstrip("\n").split("\n")
    m = INDEX_SOURCES.fullmatch(lines[-1])
    if not m:
        return INDEX_STALE, None
    return None, (None if m.group(1) == index_sources() else INDEX_REFRESH)


def level_of(fields: dict) -> str:
    """goal, feature or task: `level:`, a task when it has none or one that isn't a level."""
    level = fields.get("level", "").lower()
    return level if level in LEVELS else "task"


def for_of(fields: dict) -> str:
    """ai or human: `for:`, ai when it has none or one that isn't either (as the core reads it)."""
    who = fields.get("for", "").lower()
    return who if who in FOR_WHO else "ai"


def is_date(value: str) -> bool:
    """A `YYYY-MM-DD` date that exists."""
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", value):
        return False
    try:
        datetime.strptime(value, "%Y-%m-%d")
    except ValueError:
        return False
    return True


def level_checks(tasks: list, today: str) -> tuple:
    """(problems, warnings) of the queue's levels and parent links, as the core's
    queue::level_problems finds them: a level that isn't one, a parent that doesn't exist or is at
    the wrong level, parents in a loop, a due that isn't a date, and (a warning) a goal past its
    due date with tasks open. `today` is YYYY-MM-DD."""
    problems, warnings = [], []
    by_id = {}
    for t in tasks:
        by_id.setdefault(t["id"], t)
    kids = {}
    for t in tasks:
        if t["parent"] and t["parent"] in by_id and t["parent"] != t["id"]:
            kids.setdefault(t["parent"], []).append(t)
    for t in tasks:
        rel, level, parent = t["path"], t["level"], t["parent"]
        if t["bad_level"]:
            problems.append(f"{rel}: level '{t['bad_level']}' isn't a level (read as task)")
        if parent:
            above = by_id.get(parent)
            if level == "goal":
                problems.append(f"{rel}: a goal with a parent ({parent}); a goal has none")
            elif above is None:
                problems.append(f"{rel}: parent {parent}, which doesn't exist")
            elif level == "feature" and above["level"] != "goal":
                problems.append(f"{rel}: a feature under {above['level']} {parent}; a feature's parent is a goal")
            elif level == "task" and above["level"] == "task":
                problems.append(f"{rel}: a task under task {parent}; a task's parent is a feature or a goal")
        due = t["due"]
        if due and not is_date(due):
            problems.append(f"{rel}: due '{due}' isn't a date (YYYY-MM-DD)")
        elif level == "goal" and due and due < today:
            # Its tasks: every descendant of level task, each once.
            seen, stack, total, done = {t["id"]}, list(kids.get(t["id"], [])), 0, 0
            while stack:
                c = stack.pop()
                if c["id"] in seen:
                    continue
                seen.add(c["id"])
                stack.extend(kids.get(c["id"], []))
                if c["level"] == "task":
                    total += 1
                    done += c["stage"] == "done"
            if done < total:
                warnings.append(f"{rel}: past its due date ({due}) with {total - done} of {total} tasks open")
    loops = set()
    for start in tasks:
        path, cur = [start["id"]], start
        while cur["parent"] and cur["parent"] in by_id:
            up = by_id[cur["parent"]]
            if up["id"] in path:
                ring = path[path.index(up["id"]):]
                low = ring.index(min(ring))
                ring = ring[low:] + ring[:low]
                if tuple(ring) not in loops:
                    loops.add(tuple(ring))
                    problems.append(f"parents go round in a loop: {' → '.join(ring)} → {ring[0]}")
                break
            path.append(up["id"])
            cur = up
    return problems, warnings


def heartbeat_age(value, now):
    for fmt in ("%Y-%m-%d %H:%M", "%Y-%m-%dT%H:%M", "%Y-%m-%d"):
        try:
            return (now - datetime.strptime(value, fmt)).total_seconds() / 3600
        except ValueError:
            pass
    return None


def main():
    check_only = "--check" in sys.argv
    now = datetime.now()
    tasks, problems, block_warnings = [], [], []
    backlog, note_problems = backlog_notes()
    problems.extend(note_problems)
    for folder, folder_path in task_dirs():
        for path in sorted(folder_path.glob("*.md")):
            if (MUSTER or folder_path == backlog_dir()) and not is_task_name(path.name):
                continue  # a backlog note (task 0015), listed under "backlog"; in .muster/tasks/, not a task
            rel = path.relative_to(ROOT).as_posix()
            fields, body = front_matter(path.read_text(encoding="utf-8"))
            if fields is None:
                if folder != "inbox":
                    problems.append(f"{rel}: no front matter")
                continue
            if MUSTER:
                # The stage is the only truth there: a `status:` left in the file isn't read.
                fields.pop("status", None)
            title = next((l[2:].strip() for l in body.splitlines() if l.startswith("# ")), "")
            questions = body.split("## Questions", 1)[1].split("\n## ", 1)[0] if "## Questions" in body else ""
            t = {
                "id": fields.get("id", ""),
                "title": title,
                "type": kind_of(fields.get("type", "")),
                "priority": fields.get("priority", ""),
                "status": fields.get("status", ""),
                "stage": stage_of(fields, folder),
                "folder": folder,
                "path": rel,
                "claimed_by": fields.get("claimed-by", ""),
                # Who holds Testing or Your check (muster writes it, 0030); Building has claimed_by.
                "held_by": fields.get("held-by", ""),
                "heartbeat": fields.get("heartbeat", ""),
                # Paused by the owner (0045): when, else "". Its lease doesn't run meanwhile.
                "paused": fields.get("paused", ""),
                "waiting_on": fields.get("waiting_on", ""),
                "depends": parse_depends(fields.get("depends", ""))[0],
                "_bad_depends": parse_depends(fields.get("depends", ""))[1],
                "commit": fields.get("commit", ""),
                "tags": [t.strip() for t in fields.get("tags", "").split(",") if t.strip()],
                "est_time": fields.get("est-time", ""),
                "est_test_time": fields.get("est-test-time", ""),
                "est_tokens": fields.get("est-tokens", ""),
                "open_questions": len(re.findall(r"^- \[ \] \*\*Q", questions, re.M)),
                "escalated_to_owner": any(
                    q.startswith("[ ]") and "Escalated to owner" in q
                    for q in re.split(r"^- ", questions, flags=re.M)
                ),
                "attachments": sorted(
                    p.relative_to(ROOT).as_posix()
                    for p in (ROOT / "attachments" / fields.get("id", "-")).glob("*.*")
                ) if (ROOT / "attachments" / fields.get("id", "-")).is_dir() else [],
                "level": level_of(fields),
                "parent": fields.get("parent", ""),
                "due": fields.get("due", ""),
                "for": for_of(fields),
                "_reached": stages_reached(body),
            }
            raw_level = fields.get("level", "").lower()
            t["bad_level"] = raw_level[:40] if raw_level and raw_level not in LEVELS else ""
            raw_for = fields.get("for", "").lower()
            if raw_for and raw_for not in FOR_WHO:
                problems.append(f"{rel}: for '{raw_for[:40]}' isn't ai or human (read as ai)")
            tasks.append(t)
            if not MUSTER and t["status"] != folder:
                problems.append(f"{rel}: status '{t['status']}' but it's in {folder}/")
            raw_stage = fields.get("stage", "").lower()
            blocked = bool(t["waiting_on"]) or t["status"] == "blocked"
            if MUSTER:
                # Worked out from the stage, for the readers that still use it (queue.rs parse_task).
                t["status"] = mirror_status(t["stage"], blocked)
            # Why it's blocked (0024): the typed kind, or what an old waiting_on reads as (warned
            # about until it's blocked again with one); "tasks" from depends is filled in below.
            typed = fields.get("blocked-kind", "").lower()
            t["blocked_kind"] = (typed if typed in TYPED_KINDS else guess_kind(t["waiting_on"])) if blocked else ""
            t["_kind_guessed"] = blocked and typed not in TYPED_KINDS
            if old_pause(t, blocked):
                block_warnings.append(
                    f"{rel}: paused the old way, as a block on {t['waiting_on'][:80]!r};"
                    f" make it a pause: muster check --fix-pauses")
            elif t["_kind_guessed"]:
                block_warnings.append(
                    f"{rel}: blocked on {t['waiting_on'][:80]!r} without a kind, read as {t['blocked_kind']};"
                    f" block it again with one to confirm")
            if raw_stage and raw_stage not in STAGES:
                problems.append(f"{rel}: stage '{raw_stage[:40]}' isn't a stage (read as {t['stage']})")
            elif not MUSTER and raw_stage in STAGES and mirror_status(raw_stage, blocked) != t["status"]:
                problems.append(f"{rel}: says stage {raw_stage} but status {t['status']} (moved by hand? use `muster stage`)")
            if not re.fullmatch(r"\d{4}", t["id"]) or not path.name.startswith(t["id"] + "-"):
                problems.append(f"{rel}: id '{t['id']}' doesn't match the file name")
            # Only Building is held by someone; Testing and Your check also sit in doing/. In
            # .muster/ a blocked task isn't being worked on (queue.rs problems).
            building = t["stage"] == "building" and (not blocked if MUSTER else folder == "doing")
            if building and not t["claimed_by"]:
                problems.append(f"{rel}: in Building with no claimed-by" if MUSTER
                                else f"{rel}: in doing/ with no claimed-by")
            if folder == "blocked" and not t["waiting_on"]:
                problems.append(f"{rel}: blocked with no waiting_on")
            # The owner doing their own task (for: human) has no lease, so no stale heartbeat, and
            # a paused one's doesn't run (0045).
            if building and t["heartbeat"] and t["for"] != "human" and not t["paused"]:
                age = heartbeat_age(t["heartbeat"], now)
                if age is not None and age > STALE_HOURS:
                    problems.append(f"{rel}: heartbeat {age:.0f} h old (stale after {STALE_HOURS} h)")
    ids = {}
    for t in tasks:
        ids.setdefault(t["id"], []).append(t["path"])
    for i, paths in ids.items():
        if len(paths) > 1:
            problems.append(f"id {i} used twice: {', '.join(paths)}")
    known = set(ids)
    # Any file with that id not merged yet holds a task that depends on it (as `muster claim` reads
    # it): per id, for a task not started yet and for one under way.
    open_ids = {False: {t["id"] for t in tasks if holds(t, False)},
                True: {t["id"] for t in tasks if holds(t, True)}}
    for t in tasks:
        # Only an old block names a task in waiting_on; a typed one's words are just words.
        ref = re.match(r"(\d{4})\b", t["waiting_on"]) if t["_kind_guessed"] else None
        if ref and ref.group(1) not in known:
            problems.append(f"{t['path']}: waiting_on {ref.group(1)}, which doesn't exist")
        del t["_kind_guessed"]
        # Blocked by tasks (0024): not flagged, not done, and something it depends on isn't merged.
        waiting = open_ids[started(t)]
        if not t["blocked_kind"] and t["stage"] != "done" and any(
                d != t["id"] and d in waiting for d in t["depends"]):
            t["blocked_kind"] = "tasks"
        # Dependencies (0019), as muster check reports them.
        for b in t.pop("_bad_depends"):
            problems.append(f"{t['path']}: depends has {b!r}, which isn't a task id")
        for d in t["depends"]:
            if d == t["id"]:
                problems.append(f"{t['path']}: depends on itself")
            elif d not in known:
                problems.append(f"{t['path']}: depends on {d}, which doesn't exist")
    edges: dict[str, list[str]] = {}
    for t in tasks:  # a duplicate id merges its edges, as the core does
        edges.setdefault(t["id"], []).extend(d for d in t["depends"] if d != t["id"])
    for loop in dependency_loops(edges):
        problems.append(f"{', '.join(loop)} depend on each other in a loop")
    level_problems, warnings = level_checks(tasks, now.strftime("%Y-%m-%d"))
    warnings.extend(block_warnings)
    problems.extend(level_problems)
    index_problem, index_warning = index_check()
    if index_problem:
        problems.append(index_problem)
    if index_warning:
        warnings.append(index_warning)
    for t in tasks:
        del t["bad_level"]
        del t["_reached"]
    inbox = sorted(p.name for p in (ROOT / "inbox").glob("*.md"))
    if not check_only:
        manifest = {
            "version": 1,
            "generated": now.strftime("%Y-%m-%d %H:%M"),
            "next_id": f"{max((int(i) for i in known if i.isdigit()), default=0) + 1:04d}",
            # By status folder; in .muster/, by the status the stage means.
            "counts": {f: sum(t["status" if MUSTER else "folder"] == f for t in tasks) for f in FOLDERS},
            "inbox": inbox,
            "backlog": backlog,
            "waiting_for_owner": [
                t["id"] for t in tasks
                if t["escalated_to_owner"] or t["stage"] == "your-check"
                or t["blocked_kind"] in ("you", "permission")
            ],
            "tasks": tasks,
            "problems": problems,
            "warnings": warnings,
        }
        # Write a temp file and rename it, so a reader never sees a half-written manifest.
        tmp = ROOT / f".tasks.json.{os.getpid()}.tmp"
        tmp.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        os.replace(tmp, ROOT / "tasks.json")
    for p in problems:
        print("problem:", p)
    # Warnings (a goal past its due date) are shown but don't fail --check.
    for w in warnings:
        print("warning:", w)
    if check_only and problems:
        sys.exit(1)


if __name__ == "__main__":
    main()
