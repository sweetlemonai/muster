#!/usr/bin/env python3
"""Tests for the /muster and /mu slash commands (commands/muster.md, commands/mu.md) against
docs/COMMANDS.md and the `muster` they run.
Run: python3 -I bin/test_commands.py

The routes are read from the tables in both files: every row whose first cell starts with `/mu`.
Each `muster <cmd>` they name must be in `muster help --json` of the debug build of this checkout
(`cargo build --bin muster` first, quick when it's up to date), unless a row naming it is marked
"(task NNNN)": not built yet. $MUSTER_BIN names another muster to check against."""
# Annotations stay text, so this runs on macOS's own python3 (3.9) too.
from __future__ import annotations

import json
import os
import plistlib
import re
import shlex
import shutil
import subprocess
import tempfile
import time
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent
ROUTER = REPO / "commands" / "muster.md"
ALIAS = REPO / "commands" / "mu.md"
DOC = REPO / "docs" / "COMMANDS.md"
CRATE = REPO / "app" / "src-tauri"
MUSTER = Path(os.environ.get("MUSTER_BIN") or
              Path(os.environ.get("CARGO_TARGET_DIR") or CRATE / "target") / "debug" / "muster")
# The status line's lean reader (0113), built beside it.
STATUSLINE = MUSTER.parent / "muster-statusline"

# The routes task 0003 asks for, by first word, and the `muster` command each one runs.
# "" is a bare /mu, "<id>" a task id, "<other>" anything else (captured as a backlog one-liner).
ROUTES = {
    "": {"list", "show"},
    "<id>": {"show"},
    "waiting": {"list", "show"},
    "list": {"list"},
    "add": {"add"},
    "ask": {"ask"},
    "answer": {"answer"},
    "claim": {"claim"},
    "next": {"next"},
    "pass": {"pass"},
    "return": {"return"},
    "block": {"block"},
    "unblock": {"unblock"},
    "heartbeat": {"heartbeat"},
    "note": {"note"},
    "check": {"check"},
    "help": {"help"},
    "<other>": {"add"},
}
AUDIENCES = {"owner", "agent", "system"}
# docs/COMMANDS.md's tables, by heading.
HEADINGS = {
    "## For the owner": "owner",
    "## For agents doing the work": "agent",
    "## For running the system": "system",
}
READ_ONLY = {"Read", "Glob", "Grep"}
UNBUILT = re.compile(r"\(task \d{4}\)")
MUSTER_CMD = re.compile(r"`muster ([a-z][a-z-]*)")


def route_key(cell: str) -> str:
    """The first word of a route cell like "`/mu claim <id>`": "claim"; "" for a bare `/mu`."""
    m = re.match(r"`/mu(?: ([^`]*))?`", cell.strip())
    if not m:
        raise AssertionError(f"not a route: {cell!r}")
    first = (m.group(1) or "").split(" ")[0]
    if first in ("<id>", "<anything"):
        return "<id>" if first == "<id>" else "<other>"
    return first


def rows(text: str) -> list[dict]:
    """Every route row: its key, cells, the `muster` commands it names, whether it's marked
    not built yet, and the heading it's under."""
    out = []
    heading = ""
    for line in text.splitlines():
        if line.startswith("## "):
            heading = line.strip()
        if not line.startswith("| `/mu"):
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        out.append({
            "key": route_key(cells[0]),
            "cells": cells,
            "line": line,
            "muster": set(MUSTER_CMD.findall(line)),
            "unbuilt": bool(UNBUILT.search(line)),
            "heading": heading,
        })
    return out


def front_matter(text: str) -> dict[str, str]:
    m = re.match(r"---\n(.*?)\n---\n", text, re.S)
    if not m:
        raise AssertionError("no front matter")
    fields = {}
    for line in m.group(1).splitlines():
        k, _, v = line.partition(":")
        fields[k.strip()] = v.strip()
    return fields


def help_commands() -> set[str]:
    r = subprocess.run([str(MUSTER), "help", "--json"], capture_output=True, text=True)
    if r.returncode != 0:
        raise AssertionError(f"{MUSTER} help --json: {r.stderr}")
    return {c["name"] for c in json.loads(r.stdout)["commands"]}


def setUpModule():
    if not os.environ.get("MUSTER_BIN"):
        subprocess.run(["cargo", "build", "--quiet", "--bin", "muster", "--bin", "muster-statusline",
                        "--manifest-path", str(CRATE / "Cargo.toml")], check=True)


class CommandFiles(unittest.TestCase):
    def setUp(self):
        self.router = ROUTER.read_text(encoding="utf-8")
        self.doc = DOC.read_text(encoding="utf-8")

    def test_mu_is_an_identical_copy(self):
        # Claude Code has no alias for a command: /mu is its own file, kept byte for byte equal.
        self.assertEqual(ALIAS.read_bytes(), ROUTER.read_bytes())

    def test_front_matter(self):
        fm = front_matter(self.router)
        for k in ("description", "argument-hint", "allowed-tools"):
            v = fm.get(k, "")
            self.assertTrue(v, f"{k} is missing or empty")
            # Plain YAML: an unquoted value can't hold ": " or " #" or start with a special
            # character; a quoted one can't hold its own quote.
            if v.startswith('"'):
                self.assertTrue(v.endswith('"') and '"' not in v[1:-1], f"{k}: {v}")
            else:
                self.assertNotIn(": ", v, k)
                self.assertNotIn(" #", v, k)
                self.assertNotIn(v[0], "[]{}*&!|>'%@`,?:-", k)
        # One prefix per routed muster command, never Bash(muster:*): attach, init, set, move
        # and stage stay the owner's.
        tools = [t.strip() for t in fm["allowed-tools"].split(",")]
        self.assertEqual(len(tools), len(set(tools)), "a tool twice")
        routed = set().union(*ROUTES.values())
        self.assertEqual(set(tools), {f"Bash(muster {c}:*)" for c in routed} | READ_ONLY)
        for c in ("attach", "init", "set", "move", "stage"):
            self.assertNotIn(c, routed)
        self.assertIn("$ARGUMENTS", self.router)

    def test_router_has_exactly_the_routes(self):
        got = rows(self.router)
        keys = [r["key"] for r in got]
        self.assertEqual(len(keys), len(set(keys)), f"a route twice: {keys}")
        self.assertEqual(set(keys), set(ROUTES))
        for r in got:
            self.assertEqual(r["muster"], ROUTES[r["key"]], r["line"])
            self.assertIn(r["cells"][1], AUDIENCES, r["line"])

    def test_doc_lists_exactly_the_router_routes(self):
        router = {r["key"]: r for r in rows(self.router)}
        got = rows(self.doc)
        keys = [r["key"] for r in got]
        self.assertEqual(len(keys), len(set(keys)), f"a route twice: {keys}")
        self.assertEqual(set(keys), set(router), "COMMANDS.md and the router list the same routes")
        for r in got:
            mine = router[r["key"]]
            self.assertEqual(r["muster"], mine["muster"], r["line"])
            self.assertIn(r["heading"], HEADINGS, r["line"])
            self.assertEqual(HEADINGS[r["heading"]], mine["cells"][1],
                             f"same audience in both: {r['line']}")
            self.assertEqual(r["unbuilt"], mine["unbuilt"], f"marked the same: {r['line']}")

    def test_every_muster_command_exists_or_is_marked(self):
        built = help_commands()
        both = rows(self.router) + rows(self.doc)
        planned = set().union(*(r["muster"] for r in both if r["unbuilt"]))
        named = set(MUSTER_CMD.findall(self.router)) | set(MUSTER_CMD.findall(self.doc))
        missing = named - built - planned
        self.assertEqual(missing, set(), f"not in `muster help --json`: {sorted(missing)}")
        for r in both:
            if r["muster"] - built:
                self.assertTrue(r["unbuilt"], f"say which task builds it: {r['line']}")

    def test_every_muster_command_is_routed_or_not_routed(self):
        # The "Not routed" line: the built commands /mu doesn't route, then the ones a task is
        # building, which may not be in this muster yet.
        m = re.search(r"^Not routed: (.+?)(?:; being built \(task \d{4}\): (.+))?$", self.router, re.M)
        self.assertIsNotNone(m, "the Not routed line")
        built = set(re.findall(r"`([a-z][a-z-]*)`", m.group(1)))
        planned = set(re.findall(r"`([a-z][a-z-]*)`", m.group(2) or ""))
        self.assertTrue(built)
        cmds = help_commands()
        routed = set().union(*ROUTES.values())
        self.assertEqual(built - cmds, set(), "not routed but not in muster either")
        self.assertEqual((built | planned) & routed, set(), "a routed command isn't 'not routed'")
        self.assertEqual(cmds - routed, (built | planned) & cmds,
                         "every muster command is routed or listed as not routed")

    def test_owner_actions_say_owner(self):
        # What's waiting tells the owner how to act; those commands act as the owner, with the
        # options right after the id.
        part = self.router.split("### What's waiting", 1)[1].split("\n### ", 1)[0]
        acts = re.findall(r"`(/mu (?:pass|unblock|return) <id>[^`]*)`", part)
        self.assertEqual({a.split()[1] for a in acts}, {"pass", "unblock", "return"}, acts)
        for a in acts:
            self.assertRegex(a, r"^/mu \w+ <id>( --[a-z-]+ [^ ]+)* --as owner\b", a)


def quote(text: str) -> str:
    """Free text the way the router quotes it: single quotes, each ' written as '\\''."""
    return "'" + text.replace("'", "'\\''") + "'"


# Text that would break a careless command line: a quote, a leading --, $, backticks, Unicode.
TEXT = "--it's a \"quoted\" $HOME `uname` note: ünïcode ✓"


def commands_of(row: dict) -> list[str]:
    """The `muster …` command lines in a route row's Runs cell, as written."""
    return re.findall(r"`(muster [^`]+)`", row["cells"][2])


def render(cmd: str, id_: str, text: str = TEXT) -> list[str]:
    """A Runs command made concrete, then split as the shell would split it."""
    s = re.sub(r" \[[^\]]*\]", "", cmd)  # optional parts left out
    s = re.sub(r"'<[^>]*>'", lambda _m: quote(text), s)  # free text, quoted the router's way
    s = (s.replace("<id>", id_).replace("Q<n>", "Q1")
         .replace("…", "--gate " + quote("gate full: green (1 step)")
                  + " --summary " + quote("A synthetic summary: what was built")))
    return shlex.split(s)


def ident(key: str) -> str:
    """A task's file id from the key muster prints (0033): tc-12 is 0012."""
    return key.rsplit("-", 1)[-1].zfill(4)


class RoutesRunOnARealQueue(unittest.TestCase):
    """Each route's command, as the router writes it, run by this checkout's muster on a
    throwaway queue: the forms parse, the text arrives intact, and an unbuilt command is
    reported the way the router expects."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name) / "tasks"
        for f in ("inbox", "todo", "doing", "blocked", "done"):
            (self.root / f).mkdir(parents=True)
        # Its prefix stored (0033): muster names its tasks tc-1, tc-2…, whatever projects this Mac has.
        (self.root / "README.md").write_text("---\nprefix: tc\n---\nA Muster queue, for the tests.\n",
                                              encoding="utf-8")
        # A home of its own (task 0048): `muster list --json` asks Claude Code about a claim
        # stamped with a session, in its account's folder (cc-hg here, which is there). These
        # claims aren't stamped (the id below isn't a session id, so the labels stay as typed);
        # one stamped by a route would be read through this `claude`, which records every call.
        # None is expected.
        self.home = Path(self.tmp.name) / "home"
        bin_dir = self.home / ".local" / "bin"
        bin_dir.mkdir(parents=True)
        (self.home / ".claude-hg").mkdir()
        self.claude = bin_dir / "claude"
        self.claude.write_text("#!/bin/sh\necho \"$@\" >> \"$0.calls\"\necho '[]'\n", encoding="utf-8")
        self.claude.chmod(0o755)
        self.env = {**os.environ, "MUSTER_ROOT": str(self.root), "MUSTER_AS": "wt-a",
                    "CLAUDE_CODE_SESSION_ID": "aaaa1111-test",
                    "CLAUDE_CONFIG_DIR": str(self.home / ".claude-hg"), "HOME": str(self.home)}
        for k in ("TASKS_ROOT", "MUSTER_BASE", "MUSTER_AGENT", "MUSTER_LANG"):
            self.env.pop(k, None)
        self.routes = {r["key"]: r for r in rows(ROUTER.read_text(encoding="utf-8"))}
        self.a = self.new("Synthetic first")
        self.b = self.new("Synthetic second")

    def tearDown(self):
        calls = Path(f"{self.claude}.calls")
        ran = calls.read_text(encoding="utf-8") if calls.exists() else ""
        self.tmp.cleanup()
        self.assertEqual(ran, "", "a route ran Claude Code's agents listing")

    def muster(self, argv: list[str], **env: str) -> subprocess.CompletedProcess:
        self.assertEqual(argv[0], "muster")
        return subprocess.run([str(MUSTER), *argv[1:]], cwd=self.root, capture_output=True,
                              text=True, env={**self.env, **env})

    def new(self, title: str) -> str:
        body = "## Acceptance criteria\\n- [x] AC1: synthetic, ticked\\n"  # muster reads \n
        r = self.muster(["muster", "new", title, "--body", body])
        self.assertEqual(r.returncode, 0, r.stderr)
        return r.stdout.split()[0]

    def route(self, key: str, text: str = TEXT, **env: str) -> list[subprocess.CompletedProcess]:
        out = []
        for cmd in commands_of(self.routes[key]):
            r = self.muster(render(cmd, self.a, text), **env)
            self.assertEqual(r.returncode, 0, f"{key}: {cmd}\n{r.stdout}{r.stderr}")
            out.append(r)
        return out

    def task(self, key: str) -> str:
        return next(self.root.glob(f"*/{ident(key)}-*.md")).read_text(encoding="utf-8")

    def test_every_built_route_runs(self):
        self.assertIn("Synthetic first", self.route("")[0].stdout)
        self.route("waiting")
        self.assertIn("# Synthetic first", self.route("<id>")[0].stdout)
        self.route("list")
        self.route("claim")
        self.route("heartbeat")
        self.route("note")
        self.assertIn(TEXT, self.task(self.a))
        self.route("ask")
        self.route("answer")
        self.assertIn("**A (owner, ", self.task(self.a))
        # The kind, then what it waits on (0024); the old "owner …" is a block on you.
        self.route("block", "owner: " + TEXT)
        self.assertIn(f"waiting_on: owner: {TEXT}\nblocked-kind: you\n", self.task(self.a))
        self.route("unblock")
        self.route("block", "permission " + TEXT)
        self.assertIn(f"waiting_on: {TEXT}\nblocked-kind: permission\n", self.task(self.a))
        self.route("unblock")
        self.route("pass")
        self.assertIn("stage: testing", self.task(self.a))
        self.route("return")
        self.assertIn("stage: building", self.task(self.a))
        self.assertIn(f"claimed {self.b}", self.route("next", MUSTER_AS="wt-b")[0].stdout)
        # Task ids go into depends, with no flag: it shows blocked by them until they're done.
        out = self.route("block", self.b)[0].stdout
        self.assertIn(f"After {self.b} (Building, wt-b)", out)
        self.assertIn(f"depends: [{ident(self.b)}]", self.task(self.a))
        self.assertNotIn("blocked-kind: tasks", self.task(self.a))
        self.route("check")
        self.route("help")

    def test_owner_actions_run(self):
        # The owner's act hints: the route's command with the options right after the id.
        def act(key: str, opts: str, text: str = TEXT) -> str:
            [cmd] = commands_of(self.routes[key])
            # The options are the act's own: pass's "…" is them, not a gate line.
            argv = render(cmd.replace(" …", "").replace("<id>", "<id> " + opts, 1), self.a, text)
            r = self.muster(argv)
            self.assertEqual(r.returncode, 0, f"{argv}\n{r.stdout}{r.stderr}")
            return r.stdout

        self.route("claim")
        self.route("block", "owner: which colour?")
        act("unblock", "--as owner")
        self.route("pass")
        # The builder (wt-a) can't pass it out of Testing (0047); a QA session does.
        test = ["muster", "pass", self.a, "--from", "testing"]
        self.assertEqual(self.muster(test).returncode, 1)
        self.assertEqual(self.muster(test, MUSTER_AS="qa").returncode, 0)
        self.assertIn("stage: your-check", self.task(self.a))
        act("return", "--as owner")
        self.assertIn(f"owner: stage your-check → testing (returned: {TEXT})", self.task(self.a))
        self.assertEqual(self.muster(test, MUSTER_AS="qa").returncode, 0)
        act("pass", "--from your-check --as owner", "")
        self.assertIn("stage: done", self.task(self.a))

    def test_a_task_of_yours_is_the_owners_to_start_and_finish(self):
        # 0022: a human task, made by the owner. Agents can't claim it; What's waiting's two act
        # hints start it and finish it, as the owner.
        r = self.muster(["muster", "new", "Synthetic task of yours", "--human", "--as", "owner"])
        self.assertEqual(r.returncode, 0, r.stderr)
        mine = r.stdout.split()[0]
        listed = self.muster(["muster", "list"]).stdout
        self.assertIn(f"  {mine} code      normal  Synthetic task of yours  [you]\n", listed)
        [claim] = commands_of(self.routes["claim"])
        r = self.muster(render(claim, mine))
        self.assertEqual(r.returncode, 1, r.stdout)
        self.assertIn(f"{mine} is the owner's to do", r.stderr)
        part = ROUTER.read_text(encoding="utf-8").split("### What's waiting", 1)[1].split("\n### ", 1)[0]
        self.assertIn("`[you]`", part)
        hints = re.findall(r"`/mu pass <id> (--from (?:ready|building) --as owner)`", part)
        self.assertEqual(hints, ["--from ready --as owner", "--from building --as owner"])
        [cmd] = commands_of(self.routes["pass"])
        for opts, stage in zip(hints, ["building", "done"]):
            argv = render(cmd.replace(" …", "").replace("<id>", "<id> " + opts, 1), mine, "")
            r = self.muster(argv)
            self.assertEqual(r.returncode, 0, f"{argv}\n{r.stdout}{r.stderr}")
            self.assertIn(f"stage: {stage}", self.task(mine))

    def test_a_question_with_options_is_answered_by_its_pick(self):
        # 0049: What's waiting's act hint for a question with options, through the answer route.
        r = self.muster(["muster", "ask", self.a, "--option", "Violet", "--option", "Orange",
                         "--pick", "a", "--", "Which colour?"])
        self.assertEqual(r.returncode, 0, r.stderr)
        part = ROUTER.read_text(encoding="utf-8").split("### What's waiting", 1)[1].split("\n### ", 1)[0]
        self.assertIn("`/mu answer <id> Q<n> --pick <letter>`", part)
        [cmd] = commands_of(self.routes["answer"])
        argv = render(cmd.replace("<id>", "<id> --pick b", 1), self.a, "")
        r = self.muster(argv)
        self.assertEqual(r.returncode, 0, f"{argv}\n{r.stdout}{r.stderr}")
        self.assertIn("):** (b) Orange\n", self.task(self.a))

    def test_the_ask_route_passes_options_through_before_the_question(self):
        # 0049 review: --option, --pick and --no-answer are options, never the question's words.
        text = ROUTER.read_text(encoding="utf-8")
        listed = text.split("pass through unchanged", 1)[0].rsplit("There they", 1)[1]
        for flag in ("--option", "--pick", "--no-answer"):
            self.assertIn(f"`{flag}`", listed)
        self.assertIn("--option '<text>'", self.routes["ask"]["cells"][3])
        [cmd] = commands_of(self.routes["ask"])
        opts = "--option " + quote("A window, it's own") + " --option " + quote("A panel") + " --pick b --no-answer a"
        argv = render(cmd.replace("<id>", "<id> " + opts, 1), self.a, "Where does it open?")
        r = self.muster(argv)
        self.assertEqual(r.returncode, 0, f"{argv}\n{r.stdout}{r.stderr}")
        self.assertIn(":** Where does it open?\n  - (a) A window, it's own\n  - (b) A panel (my pick)\n"
                      "  With no answer: (a)\n", self.task(self.a))
        self.assertNotIn("--option", self.task(self.a))

    def test_add_runs_or_says_it_isnt_built(self):
        # The router stops on `unknown command "add"`: that must be what muster says without it.
        for key in ("add", "<other>"):
            [cmd] = commands_of(self.routes[key])
            r = self.muster(render(cmd, self.a))
            if "add" in help_commands():
                self.assertEqual(r.returncode, 0, r.stderr)
            else:
                self.assertEqual(r.returncode, 2)
                self.assertIn('unknown command "add"', r.stderr)
                self.assertIn('unknown command "add"', ROUTER.read_text(encoding="utf-8"))


class Hermetic(unittest.TestCase):
    """A git repo with one work log commit on main, in a temp folder with a space in its path,
    and a home of its own: the `muster` the tests run never sees the real queues."""

    PRE = "# Work log\n\nNewest first.\n\n"
    OLD = "## 2026-10-12 — Old\n**Did:** old\n**Verified:** `gate: green (pnpm check)`\n\n"

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name) / "the playground"
        home = Path(self.tmp.name) / "home"
        bin_dir = home / "bin"
        bin_dir.mkdir(parents=True)
        (bin_dir / "muster").symlink_to(MUSTER)
        self.env = {**os.environ, "HOME": str(home), "XDG_CONFIG_HOME": str(home / ".config"),
                    "MUSTER_BASE": str(base), "GIT_CONFIG_NOSYSTEM": "1", "MUSTER_AS": "wt-a",
                    "PATH": f"{bin_dir}{os.pathsep}{os.environ.get('PATH', '')}"}
        for k in ("MUSTER_ROOT", "TASKS_ROOT", "CLAUDE_CODE_SESSION_ID", "GIT_DIR", "GIT_WORK_TREE", "MUSTER_AGENT", "MUSTER_LANG"):
            self.env.pop(k, None)
        # The muster the tests run: the one on the PATH, unless a test names another.
        self.exe = "muster"
        self.repo = base / "log proj" / "main"
        self.repo.mkdir(parents=True)
        self.git(self.repo, "init", "-q", "-b", "main")
        hooks = Path(self.tmp.name) / "no hooks"
        hooks.mkdir()
        for k, v in (("user.name", "Test Person"), ("user.email", "test@example.com"),
                     ("commit.gpgsign", "false"), ("core.hooksPath", str(hooks))):
            self.git(self.repo, "config", k, v)
        (self.repo / "docs").mkdir()
        (self.repo / ".gitattributes").write_text("docs/WORKLOG.md merge=muster-log\n", encoding="utf-8")
        self.log(self.repo, self.OLD, "base")

    def tearDown(self):
        self.tmp.cleanup()

    def git(self, cwd: Path, *args: str) -> str:
        r = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True, env=self.env)
        self.assertEqual(r.returncode, 0, f"git {args}: {r.stdout}{r.stderr}")
        return r.stdout.strip()

    def muster(self, *args: str, input: str | None = None) -> subprocess.CompletedProcess:
        r = subprocess.run([self.exe, *args], cwd=self.repo, capture_output=True, text=True,
                           env=self.env, input=input)
        self.assertEqual(r.returncode, 0, f"muster {args}: {r.stdout}{r.stderr}")
        return r

    def entry(self, what: str) -> str:
        return f"## 2026-10-14 — {what}\n**Did:** {what}\n**Verified:** `gate: green (pnpm check)`\n\n"

    def log(self, cwd: Path, entries: str, msg: str) -> None:
        (cwd / "docs" / "WORKLOG.md").write_text(self.PRE + entries, encoding="utf-8")
        self.git(cwd, "add", "-A")
        self.git(cwd, "commit", "-qm", msg)

    def local_config(self) -> list[str]:
        return self.git(self.repo, "config", "--local", "--list").splitlines()

    def start(self) -> Path:
        key = self.muster("new", "Synthetic logged task", "--touches", "src/").stdout.split()[0]
        out = self.muster("start", key).stdout
        m = re.search(r"^worktree: (.+)$", out, re.M)
        self.assertIsNotNone(m, out)
        return Path(m.group(1))



class WorkLogMergeDriver(Hermetic):
    """0067: after `muster start`, a hand `git rebase main` in the new worktree merges two work
    log entries added on top with no other setup, through the `muster` on the PATH; and start
    leaves a merge.muster-log config that's already there, and every other key, as they were."""

    def test_a_hand_rebase_in_a_started_worktree_merges_the_work_log(self):
        self.muster("init")
        driver = ("merge.muster-log.driver=muster merge-log %O %A %B; s=$?; [ $s -le 1 ] && exit $s; "
                  "git merge-file -L ours -L base -L theirs %A %O %B")
        self.assertIn(driver, self.local_config(), "init sets it")
        # Gone again: start puts it back, and nothing else changes.
        self.git(self.repo, "config", "--local", "--remove-section", "merge.muster-log")
        before = self.local_config()
        wt = self.start()
        after = self.local_config()
        self.assertEqual([k for k in after if not k.startswith("merge.muster-log.")], before)
        self.assertIn(driver, after)
        self.assertTrue(any(k.startswith("merge.muster-log.name=") for k in after), after)
        self.assertIn(" ", str(wt), "a path with a space")
        self.log(wt, self.entry("The task") + self.OLD, "the task's entry")
        self.log(self.repo, self.entry("Landed before") + self.OLD, "another task's entry")
        self.git(wt, "rebase", "-q", "main")
        self.assertEqual((wt / "docs" / "WORKLOG.md").read_text(encoding="utf-8"),
                         self.PRE + self.entry("The task") + self.entry("Landed before") + self.OLD)
        self.assertEqual(self.git(wt, "status", "--porcelain"), "")

    def test_musters_own_merge_runs_this_binary_as_the_driver(self):
        # The merge (pass --from testing) of a started task whose only clash with main is the log:
        # it lands, both entries whole, the task's on top, with no driver in the repo's config.
        # Muster by its absolute path, and a PATH (the one git gets too) with git and no muster:
        # a driver that looked muster up on the PATH would fail to start, and the merge with it.
        self.exe = str(MUSTER.resolve())
        git_bin = shutil.which("git", path=os.environ.get("PATH", ""))
        self.assertIsNotNone(git_bin)
        self.env["PATH"] = os.pathsep.join([str(Path(git_bin).parent), "/usr/bin", "/bin"])
        self.assertIsNone(shutil.which("muster", path=self.env["PATH"]))
        self.muster("init")
        self.git(self.repo, "config", "--local", "--remove-section", "merge.muster-log")
        [project] = [p["name"] for p in json.loads(self.muster("projects", "--json").stdout)
                     if Path(p["path"]).resolve() == (self.repo / ".muster").resolve()]
        self.muster("projects", "set", project, "gate", "true")
        body = "## Acceptance criteria\\n- [x] AC1: synthetic, ticked\\n"
        key = self.muster("new", "Synthetic merged task", "--touches", "src/",
                          "--body", body).stdout.split()[0]
        wt = Path(re.search(r"^worktree: (.+)$", self.muster("start", key).stdout, re.M).group(1))
        self.git(self.repo, "config", "--local", "--remove-section", "merge.muster-log")
        self.log(wt, self.entry("The task") + self.OLD, "the task's entry")
        self.muster("pass", key, "--from", "building", "--gate", "gate: green (synthetic)",
                    "--summary", "A synthetic merged task")
        tip = self.git(wt, "rev-parse", "HEAD")
        self.muster("review", key, "--record", "-", "--as", "qa",
                    input=json.dumps({"head": tip, "tier": "light"}))
        # Started since gate records (mu-131): the merge needs its criteria gate green at the tip.
        self.muster("gate", key, "criteria", "--record", "-", "--as", "qa",
                    input=json.dumps({"head": tip, "verdict": "green",
                                      "items": [{"id": "AC1", "verdict": "met"}]}))
        self.log(self.repo, self.entry("Landed before") + self.OLD, "another task's entry")
        out = self.muster("pass", key, "--from", "testing", "--as", "qa").stdout
        self.assertIn("(fast-forward, not pushed: no origin)", out)
        self.assertEqual((self.repo / "docs" / "WORKLOG.md").read_text(encoding="utf-8"),
                         self.PRE + self.entry("The task") + self.entry("Landed before") + self.OLD)
        self.assertFalse(any(k.startswith("merge.muster-log.") for k in self.local_config()))

    def test_start_leaves_a_driver_already_set_and_every_other_key_alone(self):
        self.git(self.repo, "config", "merge.muster-log.name", "the owner's own")
        self.git(self.repo, "config", "merge.muster-log.driver", "true")
        self.git(self.repo, "config", "some.other", "kept")
        self.muster("init")
        before = self.local_config()
        self.start()
        self.assertEqual(self.local_config(), before)
        self.assertIn("merge.muster-log.driver=true", before)
        self.assertIn("some.other=kept", before)

    NOT_SET = "Couldn't set the work log's merge driver in .git/config: File exists."

    def driver_block(self, out: str) -> list[str]:
        """The block that ends `out`: from the line saying the driver isn't set to the end."""
        lines = out.rstrip("\n").splitlines()
        at = next(i for i, l in enumerate(lines) if "merge driver" in l)
        return lines[at:]

    def check_warning(self) -> str:
        """How check's merge-driver warning starts (mu-166): the queue's prefix, as its config
        stores it, then "warning: ", the form every other check warning has."""
        cfg = (self.repo / ".muster" / "config.toml").read_text(encoding="utf-8")
        m = re.search(r'^prefix = "([a-z]+)"$', cfg, re.M)
        self.assertIsNotNone(m, cfg)
        return f"{m.group(1)}: warning: the work log's merge driver isn't in the repo's git config"

    def test_start_and_init_say_plainly_when_the_driver_cant_be_set(self):
        # A config git can't lock, as in a sandboxed session (a read-only .git/config isn't
        # enough: git replaces the file through its lock). init and start still succeed.
        lock = self.repo / ".git" / "config.lock"
        lock.touch()
        for out in (self.muster("init").stdout, self.muster("start", self.muster(
                "new", "Synthetic task", "--touches", "src/").stdout.split()[0]).stdout):
            block = self.driver_block(out)
            self.assertEqual(block[0], self.NOT_SET, out)
            self.assertIn("a clash in docs/WORKLOG.md is a conflict instead of merging by itself", block[1])
            self.assertEqual([l.split()[:4] for l in block[3:]],
                             [["git", "config", "--local", "merge.muster-log.name"],
                              ["git", "config", "--local", "merge.muster-log.driver"]], out)
            self.assertNotIn(self.tmp.name, "\n".join(block))
        self.assertFalse(any(k.startswith("merge.muster-log.") for k in self.local_config()))
        # check says so too, as a "<prefix>: warning:" line (mu-166) with the same lines two
        # spaces further in under it; the git lines, run as printed, set it.
        r = subprocess.run([self.exe, "check"], cwd=self.repo, capture_output=True, text=True, env=self.env)
        block = self.driver_block(r.stdout)
        self.assertTrue(block[0].startswith(self.check_warning()), r.stdout)
        self.assertTrue(all(l.startswith("  ") for l in block[1:]), r.stdout)
        self.assertEqual([l.split()[:4] for l in block[3:]],
                         [["git", "config", "--local", "merge.muster-log.name"],
                          ["git", "config", "--local", "merge.muster-log.driver"]], r.stdout)
        self.assertTrue(all(l.startswith("    git ") for l in block[3:]), r.stdout)
        self.assertNotIn(self.tmp.name, "\n".join(block))
        lock.unlink()
        for line in block[3:]:
            subprocess.run(["sh", "-c", line], cwd=self.repo, check=True, env=self.env)
        self.assertIn(("merge.muster-log.driver=muster merge-log %O %A %B; s=$?; [ $s -le 1 ] && exit $s; "
                       "git merge-file -L ours -L base -L theirs %A %O %B"), self.local_config())
        self.assertNotIn("merge driver", subprocess.run(
            [self.exe, "check"], cwd=self.repo, capture_output=True, text=True, env=self.env).stdout)

    def test_check_warns_until_both_keys_are_set_and_only_where_gitattributes_uses_the_driver(self):
        self.muster("init")

        def check() -> subprocess.CompletedProcess:
            return subprocess.run([self.exe, "check"], cwd=self.repo, capture_output=True,
                                  text=True, env=self.env)

        self.assertNotIn("merge driver", check().stdout, "init set both")
        before = check().returncode
        self.git(self.repo, "config", "--local", "--unset", "merge.muster-log.name")
        r = check()
        self.assertIn("\n" + self.check_warning(), "\n" + r.stdout, r.stdout)
        self.assertEqual(r.returncode, before, "a warning: the exit code ignores it")
        self.git(self.repo, "config", "--local", "--remove-section", "merge.muster-log")
        r = check()
        self.assertIn("\n" + self.check_warning(), "\n" + r.stdout, r.stdout)
        self.assertEqual(r.returncode, before, "neither key set: still only a warning")
        # A repo whose .gitattributes doesn't use it isn't told to set it.
        (self.repo / ".gitattributes").write_text("# docs/WORKLOG.md merge=muster-log\n", encoding="utf-8")
        self.assertNotIn("merge driver", check().stdout)
        (self.repo / ".gitattributes").write_text("docs/WORKLOG.md merge=muster-log\n", encoding="utf-8")
        self.git(self.repo, "config", "merge.muster-log.name", "the owner's own")
        self.git(self.repo, "config", "merge.muster-log.driver", "true")
        self.assertNotIn("merge driver", check().stdout, "both set, whatever they say")



class StrayFenceSections(Hermetic):
    """mu-131 R2: a fence the write-up leaves open hides neither the tool's ## Review nor its
    ## Gates, whatever fence lines the tool's own text below it holds (a comment's code block, an
    answer's), so recording again replaces each section instead of adding a copy, and the
    write-up an editor opens leaves both out."""

    def test_records_after_fenced_comments_and_answers_replace_their_sections(self):
        self.muster("init")
        body = ("## Acceptance criteria\\n- [x] AC1: synthetic, ticked\\n\\n"
                "```md\\n```rust\\nx\\n```\\n```\\nleft open\\n")
        key = self.muster("new", "Synthetic fenced task", "--touches", "src/",
                          "--body", body).stdout.split()[0]
        wt = Path(re.search(r"^worktree: (.+)$", self.muster("start", key).stdout, re.M).group(1))
        self.log(wt, self.entry("The task") + self.OLD, "the task's entry")
        self.muster("pass", key, "--from", "building", "--gate", "gate: green (synthetic)",
                    "--summary", "A synthetic fenced task")
        tip = self.git(wt, "rev-parse", "HEAD")

        def record() -> None:
            self.muster("review", key, "--record", "-", "--as", "qa",
                        input=json.dumps({"head": tip, "tier": "light"}))
            self.muster("gate", key, "criteria", "--record", "-", "--as", "qa",
                        input=json.dumps({"head": tip, "verdict": "green",
                                          "items": [{"id": "AC1", "verdict": "met"}]}))

        record()
        self.muster("comment", key, "--body", "see:\\n```\\nlet x = 1;\\n```", "--as", "qa")
        self.muster("ask", key, "Which one?", "--as", "qa", "--session", "aaaa1111")
        self.muster("answer", key, "Q1", "this:\n```\nend", "--as", "owner")
        record()
        record()
        [task] = (self.repo / ".muster" / "tasks").glob("*-synthetic-fenced-task.md")
        text = task.read_text(encoding="utf-8")
        self.assertIn("\n  ```\n  let x = 1;\n  ```\n", text)
        lines = text.splitlines()
        self.assertEqual(lines.count("## Review"), 1, text)
        self.assertEqual(lines.count("## Gates"), 1, text)
        self.assertLess(text.index("## Gates"), text.index("## Questions"), text)
        # The editor gets the write-up: it says what it was given and changes nothing.
        seen = Path(self.tmp.name) / "seen.md"
        editor = Path(self.tmp.name) / "editor"
        editor.write_text(f"#!/bin/sh\ncat \"$1\" > {shlex.quote(str(seen))}\n", encoding="utf-8")
        editor.chmod(0o755)
        self.env["EDITOR"] = str(editor)
        self.env.pop("VISUAL", None)
        self.assertIn("no change", self.muster("edit", key, "--as", "owner").stdout)
        wu = seen.read_text(encoding="utf-8")
        self.assertIn("left open", wu)
        self.assertNotIn("## Review", wu)
        self.assertNotIn("## Gates", wu)

class RunScript(unittest.TestCase):
    """0077, 0079: bin/run, Muster's run command, builds the release app, then becomes that
    build's own binary (`exec`), never `open`, so the second Muster stays in the process group
    Muster's Run started and its Stop ends it. `pnpm` is a stand-in first on the PATH that logs
    each argument in brackets, so a path split at its space shows; its release:build "builds" a
    stand-in .app whose binary logs how it was started instead of being Muster. The target folder
    is a temp one with a space in its path."""

    RUN = REPO / "bin" / "run"
    LOG = 'printf %s "$0" >> "$CALLS"\nfor a; do printf "[%s]" "$a" >> "$CALLS"; done\necho >> "$CALLS"\n'

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        tmp = Path(self.tmp.name)
        stubs = tmp / "stubs"
        stubs.mkdir()
        self.calls = tmp / "calls.log"
        self.pid = tmp / "exe.pid"
        self.target = tmp / "the target"
        # The .app a build leaves: an Info.plist naming its binary, and the binary, which logs
        # its own path and arguments and its process id.
        self.template = tmp / "template.app"
        self.bundle(self.template, "muster")
        # pnpm: logs; its release:build then fails with $STUB_FAIL, or "builds" the .app at
        # $STUB_BUNDLE when that's set. An install, in a checkout with no node_modules, just succeeds.
        self.stub(stubs / "pnpm", self.LOG.replace("$0", '$(basename "$0")')
                  + 'for a; do [ "$a" != release:build ] || build=1; done\n'
                  '[ -n "${build:-}" ] || exit 0\n'
                  '[ -z "${STUB_FAIL:-}" ] || exit "$STUB_FAIL"\n'
                  '[ -z "${STUB_BUNDLE:-}" ] || { mkdir -p "$(dirname "$STUB_BUNDLE")" && '
                  'cp -R "$STUB_TEMPLATE" "$STUB_BUNDLE"; }\n')
        self.env = {**os.environ, "CALLS": str(self.calls), "PIDFILE": str(self.pid),
                    "CARGO_TARGET_DIR": str(self.target), "STUB_TEMPLATE": str(self.template),
                    "PATH": f"{stubs}{os.pathsep}{os.environ.get('PATH', '')}"}
        self.app = self.target / "release" / "bundle" / "macos" / "Muster.app"

    def tearDown(self):
        self.tmp.cleanup()

    @staticmethod
    def stub(path: Path, body: str) -> None:
        path.write_text("#!/bin/sh\n" + body, encoding="utf-8")
        path.chmod(0o755)

    def bundle(self, app: Path, exe: str | None) -> None:
        """A stand-in .app at `app` whose Info.plist names `exe` (None: names none)."""
        (app / "Contents" / "MacOS").mkdir(parents=True)
        info = {"CFBundleIdentifier": "ai.sweetlemon.muster"}
        if exe is not None:
            info["CFBundleExecutable"] = exe
        (app / "Contents" / "Info.plist").write_bytes(plistlib.dumps(info))
        self.stub(app / "Contents" / "MacOS" / "muster", self.LOG + 'echo $$ > "$PIDFILE"\n')

    def run_it(self, run: Path | None = None, env: dict[str, str] | None = None,
               **more: str) -> tuple[subprocess.CompletedProcess, int]:
        """bin/run (or `run`) as Muster's Run starts it; with the process id it ran as."""
        p = subprocess.Popen(["sh", str(run or self.RUN)], cwd=Path(self.tmp.name),
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                             env={**(env or self.env), **more})
        out, err = p.communicate(timeout=60)
        return subprocess.CompletedProcess(p.args, p.returncode, out, err), p.pid

    @staticmethod
    def call(*argv: object) -> str:
        """A call as the stand-ins log it: the program, then each argument in brackets."""
        return str(argv[0]) + "".join(f"[{a}]" for a in argv[1:])

    BUILD = call.__func__("pnpm", "--dir", REPO / "app", "release:build")

    def logged(self) -> list[str]:
        """What ran, without the install a checkout with no node_modules does first."""
        lines = self.calls.read_text(encoding="utf-8").splitlines() if self.calls.exists() else []
        return [line for line in lines if not line.endswith("[install][--frozen-lockfile]")]

    def test_it_builds_then_becomes_that_builds_binary_in_its_own_place(self):
        r, pid = self.run_it(STUB_BUNDLE=str(self.app))
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn(" ", str(self.app))
        exe = self.app / "Contents" / "MacOS" / "muster"
        self.assertEqual(self.logged(), [self.BUILD, self.call(exe)])
        # exec: the binary is the process Run started, so it's in Run's group and Stop reaches it.
        self.assertEqual(int(self.pid.read_text(encoding="utf-8")), pid)
        self.assertIn(f"starting {self.app} beside the Muster already running", r.stdout)

    def test_a_failed_build_starts_nothing(self):
        r, _ = self.run_it(STUB_FAIL="3", STUB_BUNDLE=str(self.app))
        self.assertEqual(r.returncode, 3)
        self.assertEqual(self.logged(), [self.BUILD])

    def test_a_build_that_leaves_no_app_starts_nothing_and_says_where_it_looked(self):
        r, _ = self.run_it()
        self.assertEqual(r.returncode, 1)
        self.assertIn(f"the build finished, but {self.app} is not there", r.stderr)
        self.assertEqual(self.logged(), [self.BUILD])

    def test_an_app_whose_info_plist_names_no_binary_starts_nothing(self):
        for exe in (None, "", "../muster", "."):
            with self.subTest(exe=exe):
                shutil.rmtree(self.template)
                self.bundle(self.template, exe)
                shutil.rmtree(self.target, ignore_errors=True)
                self.calls.unlink(missing_ok=True)
                r, _ = self.run_it(STUB_BUNDLE=str(self.app))
                self.assertEqual(r.returncode, 1)
                self.assertIn(f"{self.app}/Contents/Info.plist names no executable", r.stderr)
                self.assertEqual(self.logged(), [self.BUILD])

    def test_the_app_is_in_the_checkouts_own_target_folder_by_default(self):
        # A copy of the script in a checkout of its own, so the default target is a temp folder
        # and no real build of this checkout is ever started.
        repo = Path(self.tmp.name) / "a checkout"
        (repo / "bin").mkdir(parents=True)
        (repo / "app" / "node_modules").mkdir(parents=True)
        shutil.copy(self.RUN, repo / "bin" / "run")
        repo = repo.resolve()  # as the script's `pwd -P` sees it (/tmp is /private/tmp)
        own =repo / "app" / "src-tauri" / "target" / "release" / "bundle" / "macos" / "Muster.app"
        env = {k: v for k, v in self.env.items() if k != "CARGO_TARGET_DIR"}
        r, _ = self.run_it(repo / "bin" / "run", env, STUB_BUNDLE=str(own))
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(self.logged(), [self.call("pnpm", "--dir", repo / "app", "release:build"),
                                         self.call(own / "Contents" / "MacOS" / "muster")])

    def test_a_relative_target_folder_is_refused_before_anything_runs(self):
        r, _ = self.run_it(CARGO_TARGET_DIR="target")
        self.assertEqual(r.returncode, 2)
        self.assertIn("give it as an absolute path", r.stderr)
        self.assertFalse(self.calls.exists())


class StatusLineShim(unittest.TestCase):
    """0113: bin/statusline-usage, chained before the owner's own status line, passes Claude
    Code's status-line JSON through byte for byte, always, and records only its rate_limits in
    usage/<label>.jsonl, for an allowed account's CLAUDE_CONFIG_DIR and never the company
    ~/.claude. The shim is copied into a temp checkout of its own, so this checkout's release
    build isn't what runs; a home of its own, with ~/.claude, ~/.claude-mg and ~/.claude-ih
    (off the allowlist)."""

    SHIM = REPO / "bin" / "statusline-usage"
    FIXTURES = CRATE / "tests" / "fixtures"

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        tmp = Path(self.tmp.name)
        self.home = tmp / "home"
        for d in (".claude", ".claude-mg", ".claude-hg", ".claude-ih"):
            (self.home / d).mkdir(parents=True)
        self.app = self.home / "Library" / "Application Support" / "ai.sweetlemon.muster"
        # muster on the PATH: this checkout's debug build.
        self.on_path = tmp / "path"
        self.on_path.mkdir()
        (self.on_path / "muster").symlink_to(MUSTER)
        self.checkout = tmp / "a checkout"
        (self.checkout / "bin").mkdir(parents=True)
        self.shim = self.checkout / "bin" / "statusline-usage"
        shutil.copy2(self.SHIM, self.shim)
        self.env = {"HOME": str(self.home), "PATH": f"{self.on_path}{os.pathsep}/usr/bin:/bin",
                    "CLAUDE_CONFIG_DIR": str(self.home / ".claude-mg")}
        # mu-111's readings with their resets moved to an hour and three days from now: a reset
        # more than a year off isn't a reading, and one past is stale.
        now = int(time.time())
        self.resets = {"1792035600": str(now + 3600), "1792346400": str(now + 3 * 86400)}
        rl = json.loads(self.moved((self.FIXTURES / "statusline-rate-limits.json").read_text(encoding="utf-8")))
        # The status line's stdin as Claude Code sends it: private keys beside rate_limits,
        # Unicode, and no newline at the end.
        self.stdin = json.dumps({
            "cwd": "/Users/someone/playground/پروژه naïve 🍋", "session_id": "session-1",
            "transcript_path": "/Users/someone/.claude-mg/projects/x/session-1.jsonl",
            "model": {"id": "claude-opus-5-5", "display_name": "Opus"},
            "cost": {"total_cost_usd": 1.25}, "workspace": {"current_dir": "/Users/someone"},
            "rate_limits": rl["after_first_response"]["rate_limits"],
        }, ensure_ascii=False).encode("utf-8")

    def moved(self, text: str) -> str:
        for old, new in self.resets.items():
            text = text.replace(old, new)
        return text

    def tearDown(self):
        self.tmp.cleanup()

    def run_shim(self, data: bytes, **env: str | None) -> subprocess.CompletedProcess:
        e = {**self.env, **env}
        e = {k: v for k, v in e.items() if v is not None}
        return subprocess.run([str(self.shim)], input=data, capture_output=True, env=e, timeout=30)

    def usage(self) -> Path:
        return self.app / "usage" / "mg.jsonl"

    def test_stdin_passes_through_byte_for_byte_and_only_rate_limits_are_kept(self):
        r = self.run_shim(self.stdin)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stdout, self.stdin)
        self.assertEqual(r.stderr, b"")
        lines = self.usage().read_text(encoding="utf-8").splitlines()
        self.assertEqual(len(lines), 1)
        reading = json.loads(lines[0])
        self.assertEqual(sorted(reading), ["account", "at", "five_hour", "seven_day", "source"])
        self.assertEqual(reading["account"], "mg")
        self.assertEqual(reading["source"], "statusline")
        self.assertEqual(reading["five_hour"],
                         {"used_percentage": 35.0, "resets_at": int(self.resets["1792035600"])})
        self.assertEqual(reading["seven_day"],
                         {"used_percentage": 41.0, "resets_at": int(self.resets["1792346400"])})
        for private in ("cwd", "session", "transcript", "model", "cost", "someone", "Opus"):
            self.assertNotIn(private, lines[0])
        # The status line runs after every message: the same windows aren't written again.
        self.assertEqual(self.run_shim(self.stdin).stdout, self.stdin)
        self.assertEqual(len(self.usage().read_text(encoding="utf-8").splitlines()), 1)

    def test_what_isnt_a_reading_still_passes_through_and_records_nothing(self):
        big = b'{"cwd": "' + b"x" * (2 << 20) + b'"}'
        for data in (b"", b"\n", b"not json", b"\xff\xfe\x00{\"rate_limits\": 1}\r\n", big,
                     json.dumps({"cwd": "/x", "session_id": "s"}).encode()):
            r = self.run_shim(data)
            self.assertEqual((r.returncode, r.stdout, r.stderr), (0, data, b""), data[:40])
        self.assertFalse((self.app / "usage").exists())

    def test_the_company_account_and_one_off_the_allowlist_record_nothing(self):
        for config in (str(self.home / ".claude"), str(self.home / ".claude-ih"), "", None,
                       str(Path(self.tmp.name) / ".claude-mg")):
            r = self.run_shim(self.stdin, CLAUDE_CONFIG_DIR=config)
            self.assertEqual((r.returncode, r.stdout), (0, self.stdin), config)
        (Path(self.tmp.name) / ".claude-mg").mkdir()
        r = self.run_shim(self.stdin, CLAUDE_CONFIG_DIR=str(Path(self.tmp.name) / ".claude-mg"))
        self.assertEqual(r.stdout, self.stdin)
        self.assertFalse((self.app / "usage").exists(), "nothing recorded")

    def test_without_muster_it_is_cat(self):
        r = self.run_shim(self.stdin, PATH="/usr/bin:/bin")
        self.assertEqual((r.returncode, r.stdout), (0, self.stdin))
        self.assertFalse((self.app / "usage").exists())

    def release(self) -> Path:
        release = self.checkout / "app" / "src-tauri" / "target" / "release"
        release.mkdir(parents=True, exist_ok=True)
        return release

    def stub(self, path: Path, body: str) -> Path:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text("#!/bin/sh\n" + body + "\n", encoding="utf-8")
        path.chmod(0o755)
        return path

    # A muster that refuses before it reads stdin, as bin/muster does with no release build and a
    # release build from before 0113 does on `accounts record`.
    REFUSES = 'echo "muster: build it first" >&2; exit 2'

    def path_with(self, body: str) -> str:
        """A PATH whose muster is a stub running `body`."""
        self.stub(Path(self.tmp.name) / "stub" / "muster", body)
        return f"{Path(self.tmp.name) / 'stub'}{os.pathsep}/usr/bin:/bin"

    def test_this_checkouts_release_build_comes_before_the_path(self):
        (self.release() / "muster").symlink_to(MUSTER)
        # The PATH's muster passes stdin through and records nothing: only the release build can.
        r = self.run_shim(self.stdin, PATH=self.path_with("exec cat"))
        self.assertEqual((r.returncode, r.stdout, r.stderr), (0, self.stdin, b""))
        self.assertTrue(self.usage().exists(), "the release build recorded")

    def test_the_lean_reader_comes_first_and_records(self):
        self.assertTrue(STATUSLINE.exists(), f"{STATUSLINE}: cargo build --bin muster-statusline")
        (self.release() / "muster-statusline").symlink_to(STATUSLINE)
        self.stub(self.release() / "muster", "exec cat")
        r = self.run_shim(self.stdin, PATH=self.path_with("exec cat"))
        self.assertEqual((r.returncode, r.stdout, r.stderr), (0, self.stdin, b""))
        reading = json.loads(self.usage().read_text(encoding="utf-8"))
        self.assertEqual((reading["account"], reading["source"]), ("mg", "statusline"))
        # What isn't a reading, or the company account, passes through and records nothing.
        self.usage().unlink()
        for data, config in ((b"not json", str(self.home / ".claude-mg")),
                             (self.stdin, str(self.home / ".claude")), (self.stdin, None)):
            r = self.run_shim(data, CLAUDE_CONFIG_DIR=config)
            self.assertEqual((r.returncode, r.stdout, r.stderr), (0, data, b""), config)
        self.assertFalse(self.usage().exists())

    def test_the_lean_reader_links_no_app_frameworks(self):
        # Only a release build drops them (LTO leaves nothing that uses them, then build.rs's
        # -dead_strip_dylibs); the debug build keeps every reference.
        lean = MUSTER.parent.parent / "release" / "muster-statusline"
        if shutil.which("otool") is None or not lean.exists():
            self.skipTest("no release build of muster-statusline, or no otool")
        r = subprocess.run(["otool", "-L", str(lean)], capture_output=True, text=True, check=True)
        for framework in ("AppKit", "WebKit", "Carbon", "QuartzCore", "CoreVideo"):
            self.assertNotIn(framework, r.stdout)

    def test_a_muster_that_refuses_falls_through_to_cat(self):
        # The PATH's muster refuses: cat passes stdin through, nothing on stderr.
        r = self.run_shim(self.stdin, PATH=self.path_with(self.REFUSES))
        self.assertEqual((r.returncode, r.stdout, r.stderr), (0, self.stdin, b""))
        # A release build too old for `accounts record`, and the PATH's refusing as well.
        self.stub(self.release() / "muster", self.REFUSES)
        r = self.run_shim(self.stdin, PATH=self.path_with(self.REFUSES))
        self.assertEqual((r.returncode, r.stdout, r.stderr), (0, self.stdin, b""))
        self.assertFalse((self.app / "usage").exists())
        # The release build refuses and the PATH's muster records: it gets all of stdin.
        r = self.run_shim(self.stdin)
        self.assertEqual((r.returncode, r.stdout, r.stderr), (0, self.stdin, b""))
        self.assertTrue(self.usage().exists())

    def test_a_reading_that_cant_be_written_still_passes_through(self):
        # app/usage is a plain file: the write fails, the status line doesn't. Through the PATH's
        # muster and through the lean reader.
        self.app.mkdir(parents=True)
        (self.app / "usage").write_text("not a folder", encoding="utf-8")
        r = self.run_shim(self.stdin)
        self.assertEqual((r.returncode, r.stdout, r.stderr), (0, self.stdin, b""))
        (self.release() / "muster-statusline").symlink_to(STATUSLINE)
        r = self.run_shim(self.stdin, PATH="/usr/bin:/bin")
        self.assertEqual((r.returncode, r.stdout, r.stderr), (0, self.stdin, b""))
        r = subprocess.run([str(MUSTER), "accounts", "record", "--from", "statusline"], input=self.stdin,
                           capture_output=True, env=self.env, timeout=30)
        self.assertEqual((r.returncode, r.stdout, r.stderr), (0, self.stdin, b""))
        self.assertEqual((self.app / "usage").read_text(encoding="utf-8"), "not a folder")

    def test_a_runs_stream_is_recorded_and_muster_accounts_shows_it(self):
        stream = self.moved((self.FIXTURES / "stream" / "claude-rate-limit-over-cap.jsonl")
                            .read_text(encoding="utf-8")).encode("utf-8")
        r = subprocess.run([str(MUSTER), "accounts", "record", "--from", "stream", "--account", "cc-mg"],
                           input=stream, capture_output=True, env=self.env, timeout=30)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertEqual(r.stdout.decode(), "recorded 1 reading for cc-mg\n")
        reading = json.loads(self.usage().read_text(encoding="utf-8"))
        self.assertEqual((reading["source"], reading["five_hour"]["used_percentage"]), ("stream", 104.0))
        r = subprocess.run([str(MUSTER), "accounts", "--json"], capture_output=True, text=True,
                           env=self.env, timeout=30)
        self.assertEqual(r.returncode, 0, r.stderr)
        info = json.loads(r.stdout)
        mg = info["accounts"][0]
        self.assertEqual((mg["name"], mg["source"], mg["fiveHour"]["usedPercentage"]),
                         ("cc-mg", "stream", 104.0))
        self.assertEqual(mg["fiveHour"]["headroom"], -20.0)
        r = subprocess.run([str(MUSTER), "accounts"], capture_output=True, text=True,
                           env=self.env, timeout=30)
        self.assertEqual(r.returncode, 0, r.stderr)
        self.assertIn("cc-mg (claude)\n  5 h: 104% used (past the cap)", r.stdout)
        self.assertIn("cc-hg (claude)\n  no reading yet\n", r.stdout)
        # Off the allowlist: refused, nothing recorded.
        r = subprocess.run([str(MUSTER), "accounts", "record", "--from", "stream", "--account", "ih"],
                           input=stream, capture_output=True, env=self.env, timeout=30)
        self.assertNotEqual(r.returncode, 0)
        self.assertFalse((self.app / "usage" / "ih.jsonl").exists())

    def test_it_adds_little_to_the_status_line(self):
        # 100 runs of the shim, through the lean reader, against 100 of cat. The budget (under
        # 5 ms) is the release build's, which is what the owner runs; this is the debug build on a
        # Mac that may be busy, so the number is printed and the bound only catches something gone
        # badly wrong (a wait, a network call). The release figures are in the WORKLOG.
        (self.release() / "muster-statusline").symlink_to(STATUSLINE)
        def loop(cmd: list[str]) -> float:
            start = time.perf_counter()
            for _ in range(100):
                subprocess.run(cmd, input=self.stdin, capture_output=True, env=self.env)
            return (time.perf_counter() - start) / 100
        cat = loop(["cat"])
        shim = loop([str(self.shim)])
        added_ms = (shim - cat) * 1000
        print(f"\nstatusline-usage adds {added_ms:.1f} ms a run ({STATUSLINE})")
        self.assertLess(added_ms, 200)


if __name__ == "__main__":
    unittest.main(verbosity=1)
