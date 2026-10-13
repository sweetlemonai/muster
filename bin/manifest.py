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
# Goal → Feature → Task (decision 0003); a file without `level:` is a task.
LEVELS = ["goal", "feature", "task"]
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


def level_of(fields: dict) -> str:
    """goal, feature or task: `level:`, a task when it has none or one that isn't a level."""
    level = fields.get("level", "").lower()
    return level if level in LEVELS else "task"


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
    tasks, problems = [], []
    for folder in FOLDERS:
        for path in sorted((ROOT / folder).glob("*.md")):
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
                "level": level_of(fields),
                "parent": fields.get("parent", ""),
                "due": fields.get("due", ""),
            }
            raw_level = fields.get("level", "").lower()
            t["bad_level"] = raw_level[:40] if raw_level and raw_level not in LEVELS else ""
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
    level_problems, warnings = level_checks(tasks, now.strftime("%Y-%m-%d"))
    problems.extend(level_problems)
    for t in tasks:
        del t["bad_level"]
    inbox = sorted(p.name for p in (ROOT / "inbox").glob("*.md"))
    if not check_only:
        manifest = {
            "version": 1,
            "generated": now.strftime("%Y-%m-%d %H:%M"),
            "next_id": f"{max((int(i) for i in known if i.isdigit()), default=0) + 1:04d}",
            "counts": {f: sum(t["folder"] == f for t in tasks) for f in FOLDERS},
            "inbox": inbox,
            "waiting_for_owner": [
                t["id"] for t in tasks
                if t["escalated_to_owner"] or t["stage"] == "your-check" or t["waiting_on"].startswith("owner")
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
