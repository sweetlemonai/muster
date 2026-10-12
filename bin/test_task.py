#!/usr/bin/env python3
"""Tests for bin/task and bin/answer-hook.py. Run: python3 -I bin/test_task.py"""
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
TASK = HERE / "task"
HOOK = HERE / "answer-hook.py"

TASK_FILE = """---
id: 0042
type: bug
priority: normal
status: doing
captured: 2026-10-11
claimed-by: wt-a (session aaaa1111)
heartbeat:
waiting_on:
commit:
---

# Something is broken

**Details:** synthetic test task.

## Log
- 2026-10-11 captured
"""

OLD_STYLE = """---
id: 0043
type: idea
priority: normal
status: todo
captured: 2026-10-11
claimed-by:
---

# Old style questions

## Questions
- [ ] **Q (drover/main, session bbbb2222, 2026-10-11):** an unnumbered question?

## Log
- 2026-10-11 captured
"""


class TaskTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        for f in ["inbox", "todo", "doing", "blocked", "done"]:
            (self.root / f).mkdir()
        (self.root / "doing" / "0042-something.md").write_text(TASK_FILE)
        (self.root / "todo" / "0043-old.md").write_text(OLD_STYLE)
        self.env = {**os.environ, "MUSTER_ROOT": str(self.root)}
        for k in ("TASKS_ROOT", "CLAUDE_CODE_SESSION_ID"):
            self.env.pop(k, None)

    def tearDown(self):
        self.tmp.cleanup()

    def run_task(self, *args, ok=True):
        r = subprocess.run([sys.executable, "-I", str(TASK), *args], env=self.env, capture_output=True, text=True)
        if ok:
            self.assertEqual(r.returncode, 0, r.stderr)
        return r

    def hook(self, session, event="PostToolUse"):
        r = subprocess.run(
            [sys.executable, "-I", str(HOOK)],
            input=json.dumps({"session_id": session, "hook_event_name": event}),
            env=self.env, capture_output=True, text=True,
        )
        self.assertEqual(r.returncode, 0, r.stderr)
        return r.stdout

    def text(self, rel):
        return (self.root / rel).read_text()

    def test_ask_answer_deliver(self):
        self.run_task("ask", "0042", "Which port?", "--from", "wt-a", "--session", "aaaa1111-full-id")
        t = self.text("doing/0042-something.md")
        self.assertIn("## Questions\n- [ ] **Q1 (wt-a, session aaaa1111, ", t)
        self.assertLess(t.index("## Questions"), t.index("## Log"))
        self.assertIn("wt-a: asked Q1", t)

        self.run_task("answer", "0042", "Q1", "Use 5173.")
        t = self.text("doing/0042-something.md")
        self.assertIn("- [x] **Q1 (wt-a", t)
        self.assertIn("**A (capture, ", t)
        self.assertIn("Use 5173.", t)

        self.assertEqual(self.hook("bbbb9999-0ther"), "")  # another session gets nothing
        out = json.loads(self.hook("aaaa1111-full-id"))
        ctx = out["hookSpecificOutput"]["additionalContext"]
        self.assertEqual(out["hookSpecificOutput"]["hookEventName"], "PostToolUse")
        self.assertIn("Which port?", ctx)
        self.assertIn("Use 5173.", ctx)
        self.assertEqual(self.hook("aaaa1111-full-id"), "")  # delivered once

    def test_hook_event_name_passes_through(self):
        self.run_task("ask", "0042", "Q?", "--session", "cccc3333")
        self.run_task("answer", "0042", "Q1", "A.")
        out = json.loads(self.hook("cccc3333", "UserPromptSubmit"))
        self.assertEqual(out["hookSpecificOutput"]["hookEventName"], "UserPromptSubmit")

    def test_numbers_increase(self):
        self.run_task("ask", "0042", "First?", "--session", "aaaa1111")
        self.run_task("ask", "0042", "Second?", "--session", "aaaa1111")
        t = self.text("doing/0042-something.md")
        self.assertIn("**Q1 (", t)
        self.assertIn("**Q2 (", t)
        self.assertEqual(t.count("## Questions"), 1)

    def test_refuses_already_answered(self):
        self.run_task("ask", "0042", "Which port?", "--session", "aaaa1111")
        self.run_task("answer", "0042", "Q1", "5173")
        r = self.run_task("ask", "0042", "which port?", "--session", "dddd4444", ok=False)
        self.assertEqual(r.returncode, 1)
        self.assertIn("already answered", r.stderr)

    def test_ask_lists_earlier_questions(self):
        self.run_task("ask", "0042", "First?", "--session", "aaaa1111")
        self.run_task("answer", "0042", "Q1", "yes")
        r = self.run_task("ask", "0042", "Second?", "--session", "aaaa1111")
        self.assertIn("[answered]", r.stdout)

    def test_old_style_question_delivers_by_session_in_text(self):
        # Unnumbered questions can't be answered by number; a numbered one added later can.
        self.run_task("ask", "0043", "New one?", "--session", "eeee5555")
        t = self.text("todo/0043-old.md")
        self.assertIn("**Q2 (", t)
        self.run_task("answer", "0043", "Q2", "ok")
        self.assertIn("ok", json.loads(self.hook("eeee5555"))["hookSpecificOutput"]["additionalContext"])

    def test_answer_without_registry_uses_session_in_question(self):
        self.run_task("ask", "0042", "Q?", "--session", "ffff6666")
        (self.root / "mail" / "asks.json").unlink()
        self.run_task("answer", "0042", "Q1", "from text")
        self.assertIn("from text", self.hook("ffff6666"))

    def test_refusals(self):
        self.assertIn("no task", self.run_task("ask", "0099", "x", "--session", "aaaa1111", ok=False).stderr)
        self.assertIn("four digits", self.run_task("ask", "42", "x", "--session", "aaaa1111", ok=False).stderr)
        self.assertIn("no session", self.run_task("ask", "0042", "x", ok=False).stderr)
        self.assertIn("8 hex", self.run_task("ask", "0042", "x", "--session", "codex-ru", ok=False).stderr)
        self.assertIn("8 hex", self.run_task("ask", "0042", "x", "--session", "a1", ok=False).stderr)
        self.assertIn("empty", self.run_task("ask", "0042", "  ", "--session", "aaaa1111", ok=False).stderr)
        self.assertIn("longer", self.run_task("ask", "0042", "x" * 5000, "--session", "aaaa1111", ok=False).stderr)
        self.assertIn("no Q7", self.run_task("answer", "0042", "Q7", "x", ok=False).stderr)
        self.assertIn("look like", self.run_task("answer", "0042", "seven", "x", ok=False).stderr)
        (self.root / "todo" / "0042-dup.md").write_text(TASK_FILE)
        self.assertIn("twice", self.run_task("ask", "0042", "x", "--session", "aaaa1111", ok=False).stderr)

    def test_control_characters_stripped(self):
        self.run_task("ask", "0042", "bad\x1b[31m text\x07\tend?", "--session", "aaaa1111")
        t = self.text("doing/0042-something.md")
        self.assertNotIn("\t", t)
        self.assertNotIn("\x1b", t)
        self.assertNotIn("\x07", t)

    def test_session_from_environment(self):
        self.env["CLAUDE_CODE_SESSION_ID"] = "abcd1234-from-env"
        self.run_task("ask", "0042", "env?")
        self.assertIn("session abcd1234", self.text("doing/0042-something.md"))

    def test_escalate(self):
        self.run_task("ask", "0042", "Product?", "--session", "aaaa1111")
        self.run_task("escalate", "0042", "Q1")
        self.assertIn("Escalated to owner (capture", self.text("doing/0042-something.md"))
        self.assertIn("already escalated", self.run_task("escalate", "0042", "Q1", ok=False).stderr)

    def test_hook_tolerates_bad_input(self):
        for payload in ["", "not json", "{}", json.dumps({"session_id": "../../etc"})]:
            r = subprocess.run([sys.executable, "-I", str(HOOK)], input=payload, env=self.env,
                               capture_output=True, text=True)
            self.assertEqual(r.returncode, 0)
            self.assertEqual(r.stdout, "")

    def test_session_uppercase_normalized(self):
        self.run_task("ask", "0042", "Q?", "--session", "ABCD1234-5678")
        self.assertIn("session abcd1234", self.text("doing/0042-something.md"))

    def test_manifest_written_atomically(self):
        r = subprocess.run([sys.executable, "-I", str(HERE / "manifest.py")],
                           env=self.env, capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(json.loads((self.root / "tasks.json").read_text())["version"], 1)
        self.assertEqual(list(self.root.glob(".tasks.json.*")), [])

    def test_inbox_command(self):
        self.run_task("ask", "0042", "Q?", "--session", "aaaa1111")
        self.run_task("answer", "0042", "Q1", "the answer")
        self.assertIn("the answer", self.run_task("inbox", "--session", "aaaa1111", "--keep").stdout)
        self.assertIn("the answer", self.run_task("inbox", "--session", "aaaa1111").stdout)
        self.assertIn("no answers waiting", self.run_task("inbox", "--session", "aaaa1111").stdout)


if __name__ == "__main__":
    unittest.main(verbosity=1)
