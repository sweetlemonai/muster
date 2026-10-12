# 0002 — The AI helper runs the owner's own `claude`, restricted to `muster` and reading

Date: 2026-10-12 · Status: accepted (owner asked for "an AI helper like what you do here, without
seeing you write to files")

## Decision

- **Runtime:** the owner's own Claude Code CLI, in print mode, one turn per message:
  `claude -p … --output-format stream-json --resume <session>`. It uses the account folder the owner
  picks once in the Helper tab (`CLAUDE_CONFIG_DIR`). It's never an SDK login, which isn't allowed
  for a subscription in a product. The user's own CLI and login is.
- **What it may do:** `--restricted --strict-mcp-config --tools Bash Read Grep Glob
  --permission-mode dontAsk --allowedTools "Bash(muster:*)" Read Grep Glob`. Restricted ignores the
  account's settings files (so its allow rules and auto mode don't apply) and keeps file tools in
  the project folder. `dontAsk` refuses anything not allowed, without asking. Claude Code also lets
  plain read-only shell commands (`ls`, `cat`) through on its own. Checked live: `touch`, `rm` and a
  read of `~/.zshrc` were refused, and `muster note` ran.
- **Writes go only through `muster`** (the same rules as the app and the CLI), tagged `helper` via
  `MUSTER_AS`. Commands must be one line, because a permission rule doesn't match a multi-line shell
  command. So the brief has descriptions written with `\n` for line breaks, which `muster new
  --body` turns back into line breaks. Descriptions are in single quotes, so backticks stay text.
- **Environment:** router and token variables are removed (`ANTHROPIC_BASE_URL`,
  `ANTHROPIC_CUSTOM_HEADERS`, `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `CLAUDE_CODE_OAUTH_TOKEN`), as
  the owner's own shell wrappers do, so it never goes through a third-party router. PATH is
  Muster's `bin` plus system folders.
- **Model:** Sonnet, since filing tasks doesn't need the biggest model. Measured on the same
  two-item message: about $0.26 on the default model, $0.05–0.07 on Sonnet.
- **The window** shows replies (rendered), the commands it ran folded into a "steps" line, and the
  turn's cost. Pasted screenshots are saved in the queue's `inbox/helper/` and named in the
  message, and the helper attaches them with `muster attach`.

## Options considered

- **The Claude Agent SDK with the user's login:** not allowed for a product with a subscription
  login.
- **The SDK with an API key:** works, but the owner pays per token on a separate key, and it's
  not how they use Claude.
- **Letting it edit task files directly:** that's the drift `DOGFOOD.md` warns about. Going through
  `muster` keeps one set of rules.

## Revisit when

Codex or another CLI should be a helper too, or the stream format changes (`helper::parse_line`
has the tests).
