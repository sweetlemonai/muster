---
covers: app/src-tauri/src/session.rs, app/src-tauri/src/ping.rs, app/src-tauri/src/agents.rs, bin/answer-hook.py
---
# Sessions, handoffs and mail

- Check in at each milestone and every 30 minutes: `muster checkin <id> "…" --pct <n>`. (mu-30)
- Answer a check-in request with `muster checkin` before anything else. (mu-39)
- One session per task claims, checks in and passes it, then ends; coordinators stay thin (`muster list --json`, short reports, merges). (decision 0004)
- Only the lead restarts a session stuck over 10 minutes on a guard prompt; others report it. (mu-193)
- Subagents read the code, not the coordinator. (decision 0004)
- Hand off at ~60% context or the token cap (150k; 600k for `Muster_Team_Lead*`), whichever first, at a gap. (decision 0011)
- A holder hands off with `muster checkin <id> "<done, next>" --handoff` first. (decision 0011)
- The layer's context-watch stop and `/handoff` (Write step 0, Resume step 2) say both steps. (mu-161)
- A mail move that fails partway moves the rest; run the takeover again to finish it. (mu-156)
- The answer hook leaves mail alone while a muster write holds the queue's lock; it takes it shared, so the app's reads never hold mail up. (mu-156)
- The old session's hook leaves a taken-over task's mail for the takeover's rerun. (mu-156)
- One mail file the hook can't move or read never holds up the rest. (mu-156)
- Every holder in the chain counts as the builder. (decision 0011)
- Handoffs hold no secrets and are never committed. (decision 0011)
- Sessions are named by role: `Muster_Team_Lead`, `_2`…; builders `muster-build-0042`. (decision 0004)
- Claude Code's session states don't replace check-ins. (mu-48)
- A session's state is matched by its id in every account Muster reads, whatever account its stamp names. (mu-182)
- `claim --takeover` from your own session after it moved account re-stamps the claim instead of refusing. (mu-185)
- A shaper's claim gets its session's state too, on the card's ribbon. (mu-171)
- A message from the owner is quoted as their note, never instructions. (mu-39)
- The cards keep only a session's short id and state from `claude agents`; only the Sessions view keeps name, start and `cwd`, in memory. (decision 0017)
