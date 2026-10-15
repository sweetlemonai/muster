// No UI text outside the translation files (Drover's task 0007, mu-146
// here): the gate fails when someone writes English straight into a screen.
// It reads every source file with the TypeScript parser and flags
//   - text between JSX tags,
//   - text props (title, label, placeholder, aria-label…),
//   - text fields in objects (label:, hint:, title:…),
//   - any other string that reads like a sentence ("Couldn't save.").
// What isn't UI text (a log line, a command, a key name that looks like
// prose) is marked `i18n-ignore` in a comment on its line or the line above.
//
// Muster's screens move into the layer area by area, so until they all
// have (mu-155 removes the list), the files still holding text are listed in
// ALLOWED, each with the task that moves it. A file not on the list fails,
// and so does a listed one that has none left (it leaves the list).
import ts from "typescript";
import { describe, expect, it } from "vitest";
import untranslated from "./fixtures/untranslated.tsx?raw";

const sources = import.meta.glob(["../**/*.{ts,tsx}", "!../**/*.test.{ts,tsx}", "!../i18n/**", "!../**/*.d.ts"], {
  query: "?raw",
  import: "default",
  eager: true,
}) as Record<string, string>;

/** The task that moves a file's text into the layer. The shell's (mu-146) and the board's
 *  (mu-147) have moved. */
type Owner = "mu-148";

/** The files that still have text outside the layer, by the task that moves it. Files the shell
 *  shows a part of (the filter bar's Who and Group by, Settings' card previews and helper account,
 *  the sidebar's check-in button) have that part in the layer already; the rest is their owner's. */
const ALLOWED: Record<string, Owner> = {
  // ── task pane, comments, review, help — mu-148 ──
  "src/TaskPanel.tsx": "mu-148",
  // The task pane's right column (next step, priority, type, estimate, files), not the window's
  // sidebar, whatever its name: mu-146's list named it, but it's the pane's text.
  "src/Sidebar.tsx": "mu-148",
  "src/PaneHeader.tsx": "mu-148",
  "src/PaneMsg.tsx": "mu-148",
  "src/paneData.ts": "mu-148",
  "src/AnswerBox.tsx": "mu-148",
  "src/PickOne.tsx": "mu-148",
  "src/Branch.tsx": "mu-148",
  "src/Links.tsx": "mu-148",
  "src/OwnerChecklist.tsx": "mu-148",
  "src/StageContent.tsx": "mu-148",
  "src/StageForms.tsx": "mu-148",
  "src/StageTrack.tsx": "mu-148",
  "src/Tags.tsx": "mu-148",
  "src/EditBody.tsx": "mu-148",
  "src/writeUp.ts": "mu-148",
  "src/useTaskActions.ts": "mu-148",
  "src/Vision.tsx": "mu-148",
  "src/VisionPanel.tsx": "mu-148",
  "src/Comments.tsx": "mu-148",
  "src/Review.tsx": "mu-148",
  "src/ReviewPanel.tsx": "mu-148",
  "src/Helper.tsx": "mu-148", // the Helper view; mu-148 lists it under talking and checking
  "src/Help.tsx": "mu-148",
  "src/manual.ts": "mu-148",
  "src/shortcuts.ts": "mu-148",
  "src/shortcutsPage.ts": "mu-148",
};

const TEXT_ATTRS = new Set(["title", "aria", "label", "placeholder", "aria-label", "aria-description", "aria-roledescription", "aria-valuetext", "alt", "hint", "heading", "description", "tooltip", "message", "empty", "confirm", "intro", "note", "caption", "summary"]);
const TEXT_KEYS = new Set(["label", "name", "title", "hint", "text", "description", "placeholder", "message", "heading", "tooltip", "empty", "help", "summary", "caption", "confirm", "intro", "note", "verb", "blurb", "detail", "lead"]);
/** Names that are the same in every language, alone in a tag. */
const NAMES = new Set(["Muster", "Drover", "GitHub", "Git", "Claude", "Claude Code", "Codex", "Jira", "macOS", "Monaco", "OpenCode", "Gemini CLI", "Qwen Code", "Antigravity", "MCP", "PR", "CI", "SSH", "API", "JSON", "URL"]);

const hasLetter = (s: string) => /\p{L}/u.test(s);
/** A key ("standardHint", "general.off"), a colour or a CSS value, not text. */
const codeLike = (s: string) => /^[a-z][\w.-]*$/.test(s.trim()) || /^#[0-9a-fA-F]{3,8}$/.test(s.trim()) || /^(var|rgba?|hsla?)\(/.test(s.trim());
/** Reads like a sentence or a label made of words. */
function prose(s: string): boolean {
  const t = s.trim();
  if (!hasLetter(t)) return false;
  if (/^[\p{Lu}][\p{Ll}'’]+[\s,]+[\p{L}"“(]/u.test(t)) return true;
  return /\s/.test(t) && /\p{L}{2,}/u.test(t) && /(…|\.|\?|!)$/.test(t) && !/^[\w./-]+$/.test(t);
}

function ignored(lines: string[], line: number): boolean {
  return /i18n-ignore/.test(lines[line] ?? "") || /i18n-ignore/.test(lines[line - 1] ?? "");
}

const NON_UI_CALLS = /^(console\.\w+|invoke|listen|emit|Error|TypeError|RangeError|require|import|matchMedia|querySelector(All)?|getElementById|addEventListener|removeEventListener|setAttribute|getItem|setItem|removeItem|test|match|replace|split|startsWith|endsWith|includes|indexOf|execCommand|postMessage|RegExp|describe|it)$/;

/** Where a string is code, not text: comparisons, cases, keys, types, imports, certain calls. */
function codeContext(node: ts.Node): boolean {
  const p = node.parent;
  if (!p) return false;
  if (ts.isImportDeclaration(p) || ts.isExportDeclaration(p) || ts.isLiteralTypeNode(p) || ts.isExternalModuleReference(p)) return true;
  if (ts.isCaseClause(p) || ts.isElementAccessExpression(p)) return true;
  if (ts.isPropertyAssignment(p) && p.name === node) return true;
  if (ts.isBinaryExpression(p) && [ts.SyntaxKind.EqualsEqualsEqualsToken, ts.SyntaxKind.ExclamationEqualsEqualsToken, ts.SyntaxKind.EqualsEqualsToken, ts.SyntaxKind.ExclamationEqualsToken].includes(p.operatorToken.kind)) return true;
  if ((ts.isCallExpression(p) || ts.isNewExpression(p)) && p.arguments?.includes(node as ts.Expression)) {
    const callee = p.expression.getText();
    const last = callee.split(".").pop() ?? callee;
    if (NON_UI_CALLS.test(callee) || NON_UI_CALLS.test(last)) return true;
  }
  return false;
}

/** Attributes that hold class names, ids or values, never text. */
const CLASS_ATTRS = new Set(["className", "class", "id", "key", "style", "href", "src", "role", "type", "htmlFor", "name", "value", "defaultValue", "accept", "autoComplete", "inputMode", "lang", "dir"]);

/** A template that puts words around values: `${n} files`, `in ${weeks} weeks`. */
function fragment(node: ts.TemplateExpression): boolean {
  const parts = [node.head.text, ...node.templateSpans.map((s) => s.literal.text)];
  return parts.some((p) => /\s/.test(p) && /(^|\s)\p{Ll}{2,}(\s|$)/u.test(p));
}

function textOf(node: ts.Node): string | null {
  if (ts.isStringLiteral(node) || ts.isNoSubstitutionTemplateLiteral(node)) return node.text;
  if (ts.isTemplateExpression(node)) return [node.head.text, ...node.templateSpans.map((s) => s.literal.text)].join(" ");
  return null;
}

/** Each offending line in one file, as "line: text". */
export function scan(file: string, source: string): string[] {
  const sf = ts.createSourceFile(file, source, ts.ScriptTarget.Latest, true, file.endsWith(".tsx") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  const lines = source.split("\n");
  const out: string[] = [];
  const flag = (node: ts.Node, text: string) => {
    const line = sf.getLineAndCharacterOfPosition(node.getStart(sf)).line;
    if (!ignored(lines, line)) out.push(`${line + 1}: ${text.trim().slice(0, 80)}`);
  };
  /** Inside a JSX child expression, a text prop or a text field: any
   *  string with a letter is text. */
  const inText = (node: ts.Node, label: string): void => {
    const text = textOf(node);
    if (text !== null) {
      if (hasLetter(text) && !codeContext(node) && !codeLike(text) && !NAMES.has(text.trim())) flag(node, label + text);
      if (ts.isTemplateExpression(node)) node.templateSpans.forEach((s) => visit(s.expression));
      return;
    }
    // Calls (t(…), fmt.time(…)), elements and functions inside are code;
    // their own strings get the plain checks.
    if (ts.isCallExpression(node) || ts.isJsxElement(node) || ts.isJsxSelfClosingElement(node) || ts.isJsxFragment(node) || ts.isArrowFunction(node) || ts.isFunctionExpression(node)) return visit(node);
    ts.forEachChild(node, (c) => inText(c, label));
  };
  const visit = (node: ts.Node): void => {
    if (ts.isJsxText(node)) {
      const t = node.text.replace(/\s+/g, " ").trim();
      if (hasLetter(t) && !NAMES.has(t) && !/^[⌘⇧⌥⌃]+\s*\S?$/u.test(t)) flag(node, t);
      return;
    }
    if (ts.isJsxExpression(node) && node.expression && (ts.isJsxElement(node.parent) || ts.isJsxFragment(node.parent))) return inText(node.expression, "");
    if (ts.isJsxAttribute(node)) {
      const name = node.name.getText(sf);
      const init = node.initializer;
      const value = init && (ts.isStringLiteral(init) ? init : ts.isJsxExpression(init) && init.expression ? init.expression : undefined);
      if (!value) return;
      if (TEXT_ATTRS.has(name)) return inText(value, `${name}=`);
      if (CLASS_ATTRS.has(name) || name.startsWith("data-")) return;
      const text = textOf(value);
      if (text !== null && prose(text) && !NAMES.has(text.trim())) return flag(node, `${name}=${text}`);
      return visit(value);
    }
    if (ts.isPropertyAssignment(node) && TEXT_KEYS.has(node.name.getText(sf).replace(/["']/g, ""))) return inText(node.initializer, `${node.name.getText(sf)}: `);
    const text = textOf(node);
    if (text !== null) {
      if (!codeContext(node) && (prose(text) || (ts.isTemplateExpression(node) && fragment(node))) && !NAMES.has(text.trim())) flag(node, text);
      if (ts.isTemplateExpression(node)) node.templateSpans.forEach((s) => visit(s.expression));
      return;
    }
    ts.forEachChild(node, visit);
  };
  visit(sf);
  return out;
}

const found = Object.entries(sources)
  .map(([path, text]) => [path.replace(/^\.\.\//, "src/"), scan(path, text)] as const)
  .filter(([, hits]) => hits.length > 0);

/** What the guard fails on: each file with text that isn't on `allowed`, with its lines. */
export function unlisted(hits: readonly (readonly [string, readonly string[]])[], allowed: Record<string, string>): string[] {
  return hits.filter(([f]) => !(f in allowed)).map(([f, lines]) => `${f}:\n  ${lines.join("\n  ")}`);
}

describe("UI text goes through the translation layer", () => {
  it("reads the sources", () => {
    expect(Object.keys(sources).length).toBeGreaterThan(100);
  });

  it("flags text, text props, text fields and sentences, and nothing marked i18n-ignore", () => {
    const hits = scan(
      "x.tsx",
      [
        'const a = <p title="Open it" className="row">Hello <b>{name}</b> ·</p>;',
        'const b = { label: "Standard", mode: "standard", id: "not-this" };',
        'toast("Couldn\'t save the file.");',
        'if (e.key === "Arrow Up") go();',
        "console.warn(`Something went wrong here.`);",
        'const c = <span>Drover</span>; // the name',
        'const e = <i aria-label={`Close ${x}`} />;',
        'const d = "Save changes"; // i18n-ignore: a commit message',
        'import x from "Some module";',
        'type T = "Two words";',
        'const f = <p className={`row ${x}`}>{busy ? "Saving" : t("save")}</p>;',
        "const g = `${n} files`;",
        'const h = { hint: cond ? "Off" : `${a}-${b}` };',
        'const i = { name: "Ocean", id: "ocean" };',
        'const j = <ChoiceChips aria="Who" value={who} />;',
      ].join("\n"),
    );
    expect(hits).toEqual(["1: title=Open it", "1: Hello", "2: label: Standard", "3: Couldn't save the file.", "7: aria-label=Close", "11: Saving", "12: files", "13: hint: Off", "14: name: Ocean", "15: aria=Who"]);
  });

  it("fails on a screen with English written into it", () => {
    const hits = scan("src/i18n/fixtures/untranslated.tsx", untranslated);
    expect(hits).toEqual(["4: aria-label=Waiting tasks", "5: Nothing waits for you", "6: tasks"]);
    // The repo's check below, given that file: it isn't on the list, so it fails.
    expect(unlisted([["src/Untranslated.tsx", hits]], ALLOWED)).toEqual(["src/Untranslated.tsx:\n  4: aria-label=Waiting tasks\n  5: Nothing waits for you\n  6: tasks"]);
    // And passes once its text has an owner, as the listed files have.
    expect(unlisted([["src/Untranslated.tsx", hits]], { ...ALLOWED, "src/Untranslated.tsx": "mu-148" })).toEqual([]);
  });

  it("finds no UI text outside the layer but in the files listed, each with its owner", () => {
    expect(unlisted(found, ALLOWED), "Move these into src/i18n/locales/en/*.json and use t()").toEqual([]);
  });

  it("lists only files that still have text: one that's clean leaves the list", () => {
    const left = new Set(found.map(([f]) => f));
    expect(Object.keys(ALLOWED).filter((f) => !left.has(f))).toEqual([]);
  });

  it("names the task that moves each listed file: mu-148 (the shell's, mu-146, and the board's, mu-147, have moved)", () => {
    expect(Object.entries(ALLOWED).filter(([, owner]) => owner !== "mu-148")).toEqual([]);
    expect(Object.keys(ALLOWED).length).toBeGreaterThan(0);
  });

  it("lists none of the board's files: the board, views, backlog and sessions are all in the layer (mu-147)", () => {
    const board = ["Backlog.tsx", "NotePanel.tsx", "NewTask.tsx", "BlockWhy.tsx", "blockWords.ts", "Card.tsx", "CardLook.tsx", "CardStatus.tsx", "HolderRibbon.tsx", "ColumnCards.tsx", "ColumnRow.tsx", "rowCue.ts", "EffortGroups.tsx", "FeatureGrid.tsx", "featureGridData.ts", "GraphView.tsx", "Groups.tsx", "ModeViews.tsx", "modes.ts", "PausedLane.tsx", "Views.tsx", "viewData.ts", "Run.tsx", "runData.ts", "SessionRecap.tsx", "sessionsData.ts", "sessionState.ts", "Ping.tsx", "Pause.tsx", "OldPauses.tsx", "Lessons.tsx", "oldPauseData.ts", "reach.ts", "levels.ts", "lib.ts", "human.ts", "yourCheck.ts", "yourCheckLimit.ts", "CheckLimit.tsx", "YouRibbon.tsx", "Legend.tsx", "Work.tsx"];
    expect(board.map((f) => `src/${f}`).filter((f) => f in ALLOWED)).toEqual([]);
    expect(board.every((f) => `../${f}` in sources), "each of them is still a source file").toBe(true);
    // And none of them has text the guard finds (the list above is the proof, not a hope).
    expect(found.filter(([f]) => board.includes(f.replace(/^src\//, ""))).map(([f]) => f)).toEqual([]);
  });

  it("lists none of the shell's files: the window's frame is all in the layer (mu-146)", () => {
    const shell = ["App.tsx", "SidebarProjects.tsx", "ProjectList.tsx", "ProjectName.tsx", "ProjectSwatches.tsx", "FilterBar.tsx", "filters.ts", "Settings.tsx", "settingsData.ts", "Appearance.tsx", "themes.ts", "Display.tsx", "Notifications.tsx", "Projects.tsx", "WaitingList.tsx", "Recent.tsx", "recentData.ts", "activity.ts", "projectOrder.ts"];
    expect(shell.map((f) => `src/${f}`).filter((f) => f in ALLOWED)).toEqual([]);
    expect(shell.every((f) => `../${f}` in sources), "each of them is still a source file").toBe(true);
  });
});
