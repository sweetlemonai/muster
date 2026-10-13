# What OpenSpec and similar tools do, and where Muster stands (2026-10-13)

**Answer:** The tools fall into four groups, and none of them combines what Muster does. The first
group is spec-driven planners (OpenSpec, Spec Kit, Kiro, BMAD). They are good at turning intent
into a spec and keeping a record of changes, but none of them has claims between agents [1][4][6].
The second is agent task files (Task Master, Backlog.md, Beads). Beads is the closest to Muster on
coordination: a claim with a lease, a heartbeat and a dependency-gated ready list [16][17]. The
third is hosted trackers (Linear, GitHub, Jira). They have the best protocols for an agent asking
a person something and the strictest review gates, but everything lives in their cloud [19][20][22][24].
The fourth is parallel-worktree boards (Conductor, Orca, Claude Squad, Claude Code's agent view).
They isolate each task in a worktree and coordinate nothing beyond that [28][30][32].

Muster's differences hold up against all four. No tool found does all of these: refuse a stage
move without proof, push answers back into the session that asked, check a new task against one
project picture, and show the token cost of each task locally. What Muster lacks is mainly three
things. It has no "what's true now" spec that outlives a task (OpenSpec's archive). It doesn't
force the tester to be someone other than the builder (GitHub, Factory). And it doesn't read
Claude Code's own session states.

**Confidence:** medium-high. Most claims rest on the vendors' own docs and repos, read in
October 2026. The weak spots are user criticism (often from blogs) and claims that something is
absent, such as "no claiming". Those come from what the docs leave out, so they are marked.

## Findings

### 1. Spec-driven planners: OpenSpec, Spec Kit, Kiro, BMAD

| | OpenSpec | Spec Kit | Kiro | BMAD |
|---|---|---|---|---|
| Model | living specs in `openspec/specs/`; each change is a folder with ADDED/MODIFIED/REMOVED deltas and a checkbox `tasks.md`; archive merges the deltas and dates the change [1][2] | constitution → specify → clarify → plan → tasks → analyze → implement, in `specs/NNN-feature/` [4][5] | `.kiro/specs/<f>/requirements.md` (EARS) + `design.md` + `tasks.md`; steering files; hooks on file save and tool use [7][8][9] | persona skills (analyst, PM, architect, developer, UX); story files + `sprint-status.yaml` [12][13] |
| Coordination between agents | none documented; changes can run in parallel [2] | none; agents in one checkout race on `.specify/feature.json` [5]; native worktrees declined [6] | runs independent tasks in waves within one spec; nothing across specs [8] | an unattended dev loop driven by spec status [13] |
| Verification | `validate --strict` checks structure only; `verify` is advisory [2][3] | `/analyze` is read-only and flags conflicts between spec, plan and tasks [4] | "Sync Files" marks tasks done by scanning the code [8] | multi-reviewer code review [13] |
| Licence / telemetry | MIT; anonymous telemetry **on by default**, opt-out with `OPENSPEC_TELEMETRY=0` or `DO_NOT_TRACK=1` [3] | MIT; no statement found (gap) [4] | proprietary; opt-out telemetry; inputs kept for abuse checks regardless [10] | MIT [12] |
| Maturity | ~72k stars, roughly weekly releases (v1.14.1, Oct) [3] | ~141k stars, releases every few days (1.1.3, Oct) [4] | IDE 1.2.56 and CLI 2.30.0, Oct 2026; credits from $20/month [10][11] | ~54k stars; v7 due 2026-10-16 [12] |

**What practitioners say:** Böckeler tried Kiro, Spec Kit and Tessl. She found the markdown
verbose and tedious to review: "To be honest, I'd rather review code than all these markdown files" [36]. Beck
and Fowler object to writing the whole spec before building, because that rules out learning
along the way [37]. Thoughtworks put the technique at "Assess" and said some tools generate spec
files that are hard to review [38]. BMAD users reported 80–100k tokens per story step in the v6
alpha [14].

### 2. Task files for agents: Task Master, Backlog.md, Beads

- **Task Master** turns a PRD into `tasks.json`. `next` picks a task whose dependencies are done,
  and since 0.42 the file has locking [15]. Its licence is MIT with the Commons Clause, so its
  code can't go into a product that is sold [15]. Sentry telemetry is on by default, and an open
  issue says it records full prompts and responses [15]. Releases have slowed since March 2026 [15].
- **Backlog.md** keeps one markdown file per task, with a terminal board, a local web board and
  an MCP server. It is MIT and says "no telemetry" [18]. Its agent guide has the agent assign
  itself, write a plan, and give evidence for each criterion before finishing [18]. It has no
  locking, and task claims stored in git are an open request (#1063, 2026-10-06) [18].
- **Beads** is the nearest thing to Muster's coordination model. `bd update --claim` is atomic.
  Since v1.3.0 (2026-09-15) a claim has a 5-minute lease and `bd heartbeat`, `bd reclaim` returns
  expired claims, and updates can be conditional with `--if-assignee` and `--if-status` [16].
  `bd ready` lists the tasks with no open blockers [17]. `bd close` records the session that
  closed the task [17]. It has no cost tracking [17]. Any caller, including the agent that is
  gated, can resolve a "human gate" (open issue #7370) [16]. Licence MIT, ~28k stars [17].

### 3. Hosted trackers with agents: Linear, GitHub, Jira

- **Linear** makes the agent the issue's *delegate*, so a person stays the assignee [19]. Agents
  report through five typed activities, and the session has six states, including `awaitingInput`
  and `stale` [20]. A person can send a **stop** signal, and an agent can ask with a **select**
  (a set of options to pick from) [21]. Coding sessions are billed at the providers' token rates,
  with no markup, plus $0.25 per 20 minutes of sandbox, and each session's cost can be broken
  down [22].
- **GitHub** cloud agent (Copilot, and Claude and Codex through Agent HQ) works on one branch and
  opens one PR. The agent can't approve or merge, **the person who asked can't approve it**, and
  Actions don't run until someone reviews [23][24]. Mission control shows the sessions and lets a
  person steer mid-run [25]. Billing moved to AI credits on 2026-06-01 [26].
- **Jira/Rovo** shows the agent as the assignee on the board, but only the person who started an
  agent can see its output [27]. Remote agents can enter `INPUT_REQUIRED` to ask a question [27].
- All three keep the data in their cloud and run the code in their own sandboxes [22][23][27].

### 4. Hosted agents and parallel-worktree boards

- **Devin** splits large tasks across child Devins, each in its own VM. Since Aug–Sep 2026 it
  groups sessions as Working / Ready / Blocked / Inactive, and it shows the cost in ACUs per
  session [33][34]. **Factory Missions** plan features in milestones with the person, then start
  one worker per feature, in order, and run **validation with fresh agents** when each milestone is
  done. One run took 16.5 h and 778M tokens (chart labels in Factory's post), and 34% of its features were fixes [35]. **Codex cloud** gives each task its own
  workspace and hands back a diff [39]. None of the three has a backlog board across tasks (from
  the docs read).
- **Boards.** Each task gets an agent in its own worktree, then diff review, a PR and archive.
  This is how Conductor, Orca, Claude Squad and Nimbalyst work [28][29][30][31]. None of them
  detects conflicts between agents before they happen [28]. Several shut down in 2026: Terragon
  (Feb), Vibe Kanban's company bloop (Apr), Crystal (replaced by Nimbalyst) and Roo Code
  (May) [29][31][14]. Telemetry differs. Conductor sends PostHog analytics with no documented
  opt-out [28], and Orca can be turned off with `DO_NOT_TRACK=1` [30].
- **Claude Code itself.** *Agent teams* (experimental, off by default) have a shared task list
  with dependencies, claims by file lock and JSON mailboxes, but no lease [32]. Their known limit:
  "teammates sometimes fail to mark tasks as completed" [32]. *Agent view* (`claude agents`,
  research preview) groups sessions as Ready for review / Needs input / Working / Completed, puts
  each session in its own worktree under `.claude/worktrees/`, opens a draft PR but never merges,
  and has `--json` output [32].

### 5. Running agents in parallel, in practice

- Review is the bottleneck, not writing the code [40]. Osmani recommends limits on work in
  progress, a second agent that reviews, and the agent's final message including the test
  output [41].
- Anthropic's 16-agent compiler run used a lock file per task in git, tests as the verifier, and
  CI against regressions. "It is easy to see tests pass and assume the job is done, when this is
  rarely the case" [42].
- On false "done": one self-reported record found 24 of 265 "done, tests pass" claims failed when
  re-run (one machine, not a benchmark; single source) [43].
- Cost per task: Claude Code's telemetry gives cost per session and per prompt [44]. ccusage
  reports by day and by session [45]. No tracker found attributes tokens to tasks, so people map
  sessions to tasks by hand (vendor blog, single source) [46].

## Where Muster stands

**What Muster has that the others don't** (from the findings above; the judgement is mine):
1. **Stage moves enforced by rules, with proof.** Only GitHub's PR gate comes close [24]. OpenSpec's
   `verify` is advisory [2], and Backlog.md asks for evidence by convention only [18].
2. **Answers pushed into the session that asked.** Linear and Jira have an "awaiting input" state,
   but they live in the cloud [20][27]. Claude Code's agent view only notifies you [32].
3. **A vision check and the cost of changing course.** Spec Kit's `/analyze` checks one feature's
   files against each other, not the whole project [4].
4. **Token and time cost per task, from local transcripts.** Linear shows cost per session in its
   cloud [22]. Elsewhere the mapping is done by hand [46].
5. **Owner attention as the thing to protect:** Your check, needs-you and the Human board. Of the
   tools above, only Linear's delegate model keeps a person on the issue [19].
6. **Local, no telemetry, plain files with their own git.** OpenSpec, Task Master, Conductor and Kiro
   all report by default or keep data [3][15][28][10]. Four tools in this space shut down in
   2026 [29][31][14]. Plain files outlive the tool.

**What the others do well that Muster lacks** (my reading):
1. **A spec that outlives the task.** OpenSpec's archive merges each change's deltas into living
   specs, so `openspec/specs/` is always the current truth [1]. A Muster task ends at Done, and
   its decisions stay in the task file and the work log.
2. **A tester other than the builder.** GitHub doesn't let the person who asked approve the PR
   [24]. Factory validates each milestone with fresh agents [35]. Osmani recommends a separate reviewing agent
   [41]. Muster has a Testing stage, but doesn't stop the building session from testing its own
   work.
3. **Reading the agent tool's own session states.** Claude Code's agent view already knows Needs
   input / Working / Completed and the worktree of each session [32]. Muster infers this from
   heartbeats, which is why its cards said "no check-in for 36 min".
4. **Stop and steer** (Linear's stop signal, select questions [21]; GitHub's steering [25]).
   0039 (ping a session) covers the message part, not stop or the pick-one questions.
5. **A worktree per task with diff review** (every board in group 4). Already queued as 0023.
6. **A clarify step before planning** (Spec Kit's `/clarify` and `[NEEDS CLARIFICATION]` [4][5];
   Factory's read-only spec mode [35]).
7. **Limits on work in progress**, for the review bottleneck [40][41].
8. **An MCP server for agents outside Claude Code** (Backlog.md, Task Master [18][15]). This
   matters for selling Muster, not for the owner today.

**What I'd adopt, in order** (my recommendations; each would be a queue task, not built):
1. **The tester isn't the builder:** Testing → Your check is refused when the session that ticks
   the criteria is the one that built the task. It is cheap, it sits on the session stamps
   Muster already has, and it answers the false-"done" problem [24][35][43].
2. **Read Claude Code's agent states** (`claude agents --json`) to tell working from waiting from
   finished, and fall back to heartbeats. Pairs with 0023, 0030, 0037 and 0039. Caveat: research
   preview, so the format may change [32].
3. **Stop and pick-one questions in the mailbox:** extend 0039 with "stop" and answers given as
   options [21].
4. **"What's true now" pages:** on Done, a task's decisions update one short page per area,
   which the vision check reads. This is OpenSpec's archive idea [1]. Keep it short, given the
   complaints about verbose specs [36][38].
5. **A clarify pass in Shaping:** shaping agents mark the unknowns and send them to the owner
   in one batch [4].
6. **A work-in-progress limit per project** on Your check, warned about on the board [41].
7. **Later, for selling:** an MCP server, and importing from Spec Kit's `tasks.md`, OpenSpec
   changes and Beads [18][4][1][17].

**Don't adopt:** heavy up-front spec suites and persona zoos (token cost and review burden
[14][36][37]), cloud sandboxes (against the privacy rule), or anything with default telemetry.

## Trade-offs / what would change the answer
- **Anthropic is building the basics into Claude Code:** agent teams have claims, dependencies
  and mailboxes, and agent view has states and worktrees [32]. If agent teams leave experimental
  and get leases, Muster's coordination layer becomes less unique. Its rules, owner attention,
  cost and vision features would still be its own. That argues for building *on* agent view's
  states (item 2) rather than competing with them.
- **Beads keeps moving towards Muster's model** (leases since Sep 2026 [16]). It is worth
  watching as a possible interop target or competitor.
- If the owner wants Muster to work with Codex, Cursor and others, the MCP server moves up the
  list.

## Gaps and caveats
- A citation audit fetched the sources behind 14 key claims. 12 were supported as written, and the
  Factory wording was corrected ("in order", not "serially"). Quotes came from a summarising fetch.
- Spec Kit's telemetry and Kiro's licence terms were not established [4][10].
- "No claiming" for OpenSpec, Spec Kit and Kiro is inferred from what their docs leave out [2][5][8].
- Release dates on GitHub pages showed no year. 2026 was inferred from issue dates.
- Criticism of Devin, Jira/Rovo and Kiro came from weak sources and was left out.
- The false-done figure [43] and the claim that no tool tracks cost per task [46] are single,
  secondary sources.
- Not covered in depth: Amp, Cursor's agents window (secondary sources only), Sculptor and Jules.
- No prompt-injection attempts were seen in the fetched pages.

## Sources
[1] OpenSpec concepts — Fission-AI, n.d. (read 2026-10) — https://raw.githubusercontent.com/Fission-AI/OpenSpec/main/docs/concepts.md (primary)
[2] OpenSpec CLI reference — Fission-AI, n.d. — https://raw.githubusercontent.com/Fission-AI/OpenSpec/main/docs/cli.md (primary)
[3] OpenSpec CLI reference (telemetry), repo and releases — Fission-AI, v1.14.1 Oct 2026 — https://raw.githubusercontent.com/Fission-AI/OpenSpec/main/docs/cli.md (primary)
[4] Spec Kit quickstart and releases — GitHub, 1.1.3 2026-10-09 — https://raw.githubusercontent.com/github/spec-kit/main/docs/quickstart.md ; https://github.com/github/spec-kit/releases (primary)
[5] Spec-driven development (spec-kit); issue #4128 — GitHub — https://raw.githubusercontent.com/github/spec-kit/main/spec-driven.md ; https://github.com/github/spec-kit/issues/4128 (primary)
[6] Native git worktree support #1476 — GitHub, closed 2026-04-14 — https://github.com/github/spec-kit/issues/1476 (primary)
[7] Kiro feature specs — AWS — https://kiro.dev/docs/specs/feature-specs/ (primary)
[8] Kiro spec best practices — AWS — https://kiro.dev/docs/specs/best-practices/ (primary)
[9] Kiro hooks; Kiro steering — AWS — https://kiro.dev/docs/hooks/ ; https://kiro.dev/docs/steering/ (primary)
[10] Kiro data protection — AWS — https://kiro.dev/docs/privacy-and-security/data-protection/ (primary)
[11] Kiro pricing — AWS, 2026-10-12 — https://kiro.dev/pricing/ (primary)
[12] BMAD-METHOD README — bmad-code-org, 2026-10-11 — https://github.com/bmad-code-org/BMAD-METHOD (primary)
[13] BMAD CHANGELOG — bmad-code-org, 2026 — https://github.com/bmad-code-org/BMAD-METHOD/blob/main/CHANGELOG.md (primary)
[14] BMAD issue #1235 (token use), 2026-01-01; Roo Code repo archived 2026-05-15 — https://github.com/bmad-code-org/BMAD-METHOD/issues/1235 ; https://github.com/RooCodeInc/Roo-Code (primary/secondary)
[15] Task Master task structure, LICENSE, releases, issue #1681 — eyaltoledano, 0.43.1 2026-03-31 — https://raw.githubusercontent.com/eyaltoledano/claude-task-master/main/docs/task-structure.md ; https://github.com/eyaltoledano/claude-task-master/blob/main/LICENSE ; https://github.com/eyaltoledano/claude-task-master/issues/1681 (primary/secondary)
[16] Beads v1.3.0 release notes, 2026-09-15; issue #7370, 2026-10-07 — https://github.com/steveyegge/beads/releases/tag/v1.3.0 ; https://github.com/gastownhall/beads/issues/7370 (primary)
[17] Beads README — 2026-10 — https://github.com/steveyegge/beads (primary)
[18] Backlog.md README, MCP task-execution guide, issues — MrLesk, v1.53.0 2026-09-24 — https://github.com/MrLesk/Backlog.md ; https://raw.githubusercontent.com/MrLesk/Backlog.md/main/src/guidelines/mcp/task-execution.md ; https://github.com/MrLesk/Backlog.md/issues (primary)
[19] Linear Developers: Agents — Linear — https://linear.app/developers/agents (primary)
[20] Linear Developers: Agent interaction — Linear — https://linear.app/developers/agent-interaction (primary)
[21] Linear Developers: Agent signals — Linear — https://linear.app/developers/agent-signals (primary)
[22] Linear AI Credits — Linear, 2026-08-20 — https://linear.app/docs/ai-credits (primary)
[23] About Copilot cloud agent — GitHub Docs — https://docs.github.com/en/copilot/concepts/agents/coding-agent/about-coding-agent (primary)
[24] Risks and mitigations for Copilot cloud agent — GitHub Docs — https://docs.github.com/en/copilot/concepts/agents/coding-agent/risks-and-mitigations (primary)
[25] Agent management — GitHub Docs — https://docs.github.com/en/enterprise-cloud@latest/copilot/concepts/agents/coding-agent/agent-management (primary)
[26] GitHub Copilot is moving to usage-based billing — GitHub Blog, 2026-04-27 — https://github.blog/news-insights/company-news/github-copilot-is-moving-to-usage-based-billing/ (primary)
[27] Collaborate on work items with AI agents; Remote agents in Jira — Atlassian — https://support.atlassian.com/jira-software-cloud/docs/collaborate-on-work-items-with-ai-agents/ ; https://developer.atlassian.com/platform/forge/remote-agents-in-jira/ (primary)
[28] Conductor privacy and docs — Conductor; Conductor vs Intent — Augment Code (competitor), 2026 — https://www.conductor.build/docs/reference/privacy ; https://www.augmentcode.com/tools/intent-vs-conductor-macos-agent-orchestrators (primary/secondary)
[29] Vibe Kanban shutdown, 2026-04-10; terragon-oss — https://www.vibekanban.com/blog/shutdown ; https://github.com/terragon-labs/terragon-oss (primary)
[30] Orca privacy and telemetry — Stably AI, 2026 — https://www.onorca.dev/docs/telemetry (primary)
[31] Claude Squad repo (v1.0.20, 2026-08-20); stravu/crystal; Nimbalyst — https://github.com/smtg-ai/claude-squad ; https://github.com/stravu/crystal ; https://nimbalyst.com (primary)
[32] Claude Code agent teams; agent view — Anthropic, 2026 — https://code.claude.com/docs/en/agent-teams ; https://code.claude.com/docs/en/agent-view (primary)
[33] Devin advanced capabilities — Cognition — https://docs.devin.ai/work-with-devin/advanced-capabilities (primary)
[34] Devin 2026 release notes; plans and usage — Cognition, 2026-10-09 — https://docs.devin.ai/release-notes/2026 ; https://docs.devin.ai/desktop/accounts/usage (primary)
[35] Factory Missions overview, planning and validation; How Missions Work — Factory, 2026-04-10; spec mode — https://docs.factory.com/missions/overview ; https://docs.factory.com/missions/planning ; https://factory.com/news/missions-architecture ; https://docs.factory.com/cli/user-guides/specification-mode (primary)
[36] Understanding Spec-Driven Development: Kiro, spec-kit and Tessl — Birgitta Böckeler, martinfowler.com, 2025-10-15 (12 months old) — https://martinfowler.com/articles/exploring-gen-ai/sdd-3-tools.html (primary)
[37] Fragments: January 8 — Martin Fowler quoting Kent Beck, 2026-01-08 — https://martinfowler.com/fragments/2026-01-08.html (primary)
[38] Spec-driven development — Thoughtworks Technology Radar, 2025-11-05 — https://www.thoughtworks.com/radar/techniques/spec-driven-development (primary)
[39] Codex Cloud — OpenAI — https://learn.chatgpt.com/docs/cloud (primary)
[40] Embracing the parallel coding agent lifestyle — Simon Willison, 2025-10-05 (12 months old) — https://simonwillison.net/2025/Oct/5/parallel-coding-agents/ (primary)
[41] Your AI coding agents need a manager — Addy Osmani, 2026-01-08 — https://addyosmani.com/blog/coding-agents-manager/ (primary)
[42] Building a C compiler with a team of parallel Claudes — Anthropic Engineering, 2026-02-05 — https://www.anthropic.com/engineering/building-c-compiler (primary)
[43] When a Coding Agent Says Done — Remy.Blog, 2026-10-11 — https://redreamality.com/blog/coding-agent-done-claims-verification-survey/ (secondary, single source)
[44] Claude Code monitoring — Anthropic — https://code.claude.com/docs/en/monitoring-usage (primary)
[45] ccusage — ryoppippi — https://github.com/ryoppippi/ccusage (primary)
[46] AI Agent Cost Monitoring — amux (vendor blog), 2026 — https://amux.io/guides/ai-agent-cost-monitoring/ (secondary, single source)
