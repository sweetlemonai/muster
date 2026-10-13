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
# The stages (decision 0003). `stage:` is the truth; an old file's status says it otherwise.
# The rules live in the app's core (ops.rs); this only reads them.
STAGES = ["backlog", "shaping", "ready", "building", "testing", "your-check", "done"]
LEGACY_STAGE = {"inbox": "backlog", "todo": "ready", "doing": "building", "blocked": "ready", "done": "done"}
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


def stage_of(fields: dict, folder: str) -> str:
    stage = fields.get("stage", "").lower()
    if stage in STAGES:
        return stage
    return LEGACY_STAGE.get(fields.get("status") or folder, "backlog")


def mirror_status(stage: str, blocked: bool) -> str:
    """The old status a stage and the blocked flag mean (ops.rs/queue.rs mirror_status)."""
    if blocked:
        return "blocked"
    return {"backlog": "inbox", "shaping": "todo", "ready": "todo", "done": "done"}.get(stage, "doing")


def is_task_name(name: str) -> bool:
    """`NNNN-slug.md`, not a date-named note (queue.rs is_task_name, on the same bytes)."""
    b = name.encode("utf-8")

    def digits(s: bytes) -> bool:
        return len(s) > 0 and all(48 <= c <= 57 for c in s)

    task = len(b) > 8 and digits(b[:4]) and b[4:5] == b"-" and name.endswith(".md")
    dated = len(b) > 11 and digits(b[5:7]) and b[7:8] == b"-" and digits(b[8:10]) and b[10:11] == b"-"
    return task and not dated


def backlog_dir() -> Path:
    """Where backlog notes live (backlog.rs backlog_dir): inbox/ in the v2 layout, else backlog/."""
    return ROOT / ("inbox" if (ROOT / "todo").is_dir() else "backlog")


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
    tasks, problems = [], []
    backlog, note_problems = backlog_notes()
    problems.extend(note_problems)
    for folder in FOLDERS:
        for path in sorted((ROOT / folder).glob("*.md")):
            if ROOT / folder == backlog_dir() and not is_task_name(path.name):
                continue  # a backlog note (task 0015), listed under "backlog"
            rel = path.relative_to(ROOT).as_posix()
            fields, body = front_matter(path.read_text(encoding="utf-8"))
            if fields is None:
                if folder != "inbox":
                    problems.append(f"{rel}: no front matter")
                continue
            title = next((l[2:].strip() for l in body.splitlines() if l.startswith("# ")), "")
            questions = body.split("## Questions", 1)[1].split("\n## ", 1)[0] if "## Questions" in body else ""
            t = {
                "id": fields.get("id", ""),
                "title": title,
                "type": fields.get("type", ""),
                "priority": fields.get("priority", ""),
                "status": fields.get("status", ""),
                "stage": stage_of(fields, folder),
                "folder": folder,
                "path": rel,
                "claimed_by": fields.get("claimed-by", ""),
                "heartbeat": fields.get("heartbeat", ""),
                "waiting_on": fields.get("waiting_on", ""),
                "commit": fields.get("commit", ""),
                "tags": [t.strip() for t in fields.get("tags", "").split(",") if t.strip()],
                "est_time": fields.get("est-time", ""),
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
            }
            tasks.append(t)
            if t["status"] != folder:
                problems.append(f"{rel}: status '{t['status']}' but it's in {folder}/")
            raw_stage = fields.get("stage", "").lower()
            blocked = bool(t["waiting_on"]) or t["status"] == "blocked"
            if raw_stage in STAGES and mirror_status(raw_stage, blocked) != t["status"]:
                problems.append(f"{rel}: says stage {raw_stage} but status {t['status']} (moved by hand? use `muster stage`)")
            elif raw_stage and raw_stage not in STAGES:
                problems.append(f"{rel}: stage '{raw_stage[:40]}' isn't a stage (read as {t['stage']})")
            if not re.fullmatch(r"\d{4}", t["id"]) or not path.name.startswith(t["id"] + "-"):
                problems.append(f"{rel}: id '{t['id']}' doesn't match the file name")
            # Only Building is held by someone; Testing and Your check also sit in doing/.
            building = folder == "doing" and t["stage"] == "building"
            if building and not t["claimed_by"]:
                problems.append(f"{rel}: in doing/ with no claimed-by")
            if folder == "blocked" and not t["waiting_on"]:
                problems.append(f"{rel}: blocked with no waiting_on")
            if building and t["heartbeat"]:
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
    for t in tasks:
        ref = re.match(r"(\d{4})\b", t["waiting_on"])
        if ref and ref.group(1) not in known:
            problems.append(f"{t['path']}: waiting_on {ref.group(1)}, which doesn't exist")
    inbox = sorted(p.name for p in (ROOT / "inbox").glob("*.md"))
    if not check_only:
        manifest = {
            "version": 1,
            "generated": now.strftime("%Y-%m-%d %H:%M"),
            "next_id": f"{max((int(i) for i in known if i.isdigit()), default=0) + 1:04d}",
            "counts": {f: sum(t["folder"] == f for t in tasks) for f in FOLDERS},
            "inbox": inbox,
            "backlog": backlog,
            "waiting_for_owner": [
                t["id"] for t in tasks
                if t["escalated_to_owner"] or t["stage"] == "your-check" or t["waiting_on"].startswith("owner")
            ],
            "tasks": tasks,
            "problems": problems,
        }
        # Write a temp file and rename it, so a reader never sees a half-written manifest.
        tmp = ROOT / f".tasks.json.{os.getpid()}.tmp"
        tmp.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
        os.replace(tmp, ROOT / "tasks.json")
    for p in problems:
        print("problem:", p)
    if check_only and problems:
        sys.exit(1)


if __name__ == "__main__":
    main()
