#!/usr/bin/env python3
"""Claude Code hook: deliver task-queue answers into the session that asked.

Wire it to PostToolUse, UserPromptSubmit and SessionStart. It reads the hook's JSON from stdin,
looks in mail/<first 8 chars of session_id>/ and, if answers are waiting, returns them as
additionalContext and marks them delivered. With nothing waiting it prints nothing and exits 0,
after one directory listing. It never blocks the tool call.
"""
import importlib.util
import json
import sys
from importlib.machinery import SourceFileLoader
from pathlib import Path

HERE = Path(__file__).resolve().parent


def load_task_module():
    loader = SourceFileLoader("taskcli", str(HERE / "task"))
    spec = importlib.util.spec_from_loader("taskcli", loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


def main():
    try:
        data = json.load(sys.stdin)
        taskcli = load_task_module()  # exits when MUSTER_ROOT isn't set; a hook must never fail
    except (Exception, SystemExit):
        return 0
    session = str(data.get("session_id") or "")
    event = str(data.get("hook_event_name") or "PostToolUse")
    if not session:
        return 0
    try:
        mail = taskcli.take_mail(session)
    except Exception:
        return 0
    if not mail:
        return 0
    text = (
        "Answers from the task queue to questions this session asked "
        "(already written in the task files; act on them, don't ask again):\n\n" + "\n".join(mail)
    )
    print(json.dumps({"hookSpecificOutput": {"hookEventName": event, "additionalContext": text}}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
