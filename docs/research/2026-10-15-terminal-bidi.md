# Persian in macOS terminals (mu-152, for mu-154's font guide)

Researched 2026-10-15 for `muster --lang fa`. The question: what does each terminal do with a
right-to-left line that `muster` prints in logical order, with no bidi control characters?

## Findings

| Terminal | Bidi (reorders RTL runs) | Arabic-script joining | Source, date |
|---|---|---|---|
| iTerm2 3.7+ | Yes, on by default | Yes | 3.7.0 changelog: "Right-to-left script support (bidi reordering plus Arabic and Persian cursive joining) is now enabled by default." Built 2026-06-08, announced 2026-09-08. |
| iTerm2 3.6 | Opt-in: Settings › General › Experimental › "Enable support for right-to-left scripts" | Yes, when on | 3.6.0 changelog, 2025-09-16 |
| Terminal.app | "Some essential BiDi (apparently implicit LTR, at least level 1, without autodetection)" | Users report it shows Arabic correctly | freedesktop BiDi spec, Prior Work, 2019-01-29 (judged from screenshots); claude-code issue #34134, 2026-03-13 (anecdote). No Apple document found. |
| VS Code (xterm.js) | No | No (letters disjointed) | xterm.js #701 "Support RTL languages", open since 2017-06-13, last comment 2026-10-11; VS Code #271615, closed as tracked upstream, 2025-10-18 |

iTerm2 3.7.5 (2026-10-14) adds the BDSM and SCP escape sequences, so apps can control reordering.

## What muster does with this

- It prints logical order, one paragraph per line, with no bidi control characters. This is the
  freedesktop spec's implicit mode, which is the default: "doing BiDi and shaping are the sole
  responsibilities of the terminal emulator … On level 1, BiDi control characters are discarded."
  (basic-modes page, draft v0.1, 2019-01-29)
- A Persian line starts with a Persian word. Terminal.app's paragraph direction is always LTR, so
  there the line runs left to right, with its Persian runs in the right order. iTerm2 3.7 reads the
  line right to left.
- It sends no SCP/BDSM sequences. The spec warns that apps "mustn't rely on the default being LTR",
  and the CLI's output is also piped and read by scripts.
- In VS Code's terminal (`TERM_PROGRAM=vscode`) under `--lang fa`, it prints one stderr line saying
  that VS Code's terminal can't show right-to-left text but Terminal and iTerm2 can.

## For mu-154 (fonts)

- Terminal.app and iTerm2 fall back to a system font with Persian glyphs when the profile's font
  has none. A Persian-capable monospace font, such as Vazir Code, is the font guide's call.
- These need a check on the owner's Mac. They were never tested here, because no terminal window
  was opened: Terminal.app's shaping, and how iTerm2 3.7 handles a line mixing Persian with a long
  Latin key.

## Sources

- https://iterm2.com/downloads/stable/iTerm2-3_7_0.changelog
- https://iterm2.com/downloads/stable/iTerm2-3_6_0.changelog
- https://iterm2.com/downloads.html (3.7.5 notes)
- https://iterm2.com/documentation-one-page.html (Experimental settings; undated)
- https://terminal-wg.pages.freedesktop.org/bidi/recommendation/basic-modes.html
- https://terminal-wg.pages.freedesktop.org/bidi/recommendation/escape-sequences.html
- https://terminal-wg.pages.freedesktop.org/bidi/prior-work/terminals.html
- https://github.com/xtermjs/xterm.js/issues/701
- https://github.com/microsoft/vscode/issues/271615
- https://github.com/anthropics/claude-code/issues/34134
