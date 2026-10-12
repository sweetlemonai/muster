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
                "folder": folder,
                "path": rel,
                "claimed_by": fields.get("claimed-by", ""),
                "heartbeat": fields.get("heartbeat", ""),
                "waiting_on": fields.get("waiting_on", ""),
                "commit": fields.get("commit", ""),
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
            if not re.fullmatch(r"\d{4}", t["id"]) or not path.name.startswith(t["id"] + "-"):
                problems.append(f"{rel}: id '{t['id']}' doesn't match the file name")
            if folder == "doing" and not t["claimed_by"]:
                problems.append(f"{rel}: in doing/ with no claimed-by")
            if folder == "blocked" and not t["waiting_on"]:
                problems.append(f"{rel}: blocked with no waiting_on")
            if folder == "doing" and t["heartbeat"]:
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
            "waiting_for_owner": [t["id"] for t in tasks if t["escalated_to_owner"] or t["waiting_on"].startswith("owner")],
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
