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
import re
import shlex
import subprocess
import tempfile
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
        subprocess.run(["cargo", "build", "--quiet", "--bin", "muster",
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
        built = set(re.findall(r"`([a-z]+)`", m.group(1)))
        planned = set(re.findall(r"`([a-z]+)`", m.group(2) or ""))
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
         .replace("…", "--gate " + quote("gate full: green (1 step)")))
    return shlex.split(s)


class RoutesRunOnARealQueue(unittest.TestCase):
    """Each route's command, as the router writes it, run by this checkout's muster on a
    throwaway queue: the forms parse, the text arrives intact, and an unbuilt command is
    reported the way the router expects."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name) / "tasks"
        for f in ("inbox", "todo", "doing", "blocked", "done"):
            (self.root / f).mkdir(parents=True)
        (self.root / "README.md").write_text("A Muster queue, for the tests.\n", encoding="utf-8")
        self.env = {**os.environ, "MUSTER_ROOT": str(self.root), "MUSTER_AS": "wt-a",
                    "CLAUDE_CODE_SESSION_ID": "aaaa1111-test"}
        self.env.pop("TASKS_ROOT", None)
        self.routes = {r["key"]: r for r in rows(ROUTER.read_text(encoding="utf-8"))}
        self.a = self.new("Synthetic first")
        self.b = self.new("Synthetic second")

    def tearDown(self):
        self.tmp.cleanup()

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

    def task(self, id_: str) -> str:
        return next(self.root.glob(f"*/{id_}-*.md")).read_text(encoding="utf-8")

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
        self.assertIn(f"Blocked by tasks: {self.b} (Building, wt-b)", out)
        self.assertIn(f"depends: [{self.b}]", self.task(self.a))
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


if __name__ == "__main__":
    unittest.main(verbosity=1)
