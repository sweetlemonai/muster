// Settings › Projects' Run line (0077): each project's run command under its gate, read-only.
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { GateRow, ProjectGateRow, RunCell } from "./Projects";
import type { Project } from "./types";

const text = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&quot;/g, '"').replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/\s+/g, " ").trim();
const row = (gate: string, run: string, name = "muster") =>
  renderToStaticMarkup(
    <table>
      <tbody>
        <GateRow name={name} gate={gate} run={run} hidden={false} />
      </tbody>
    </table>,
  );

describe("the Run line", () => {
  it("shows the run command under the gate, with its label, in the gate line's look", () => {
    const html = row("bin/gate", "bin/run --fresh");
    expect(text(html)).toBe("Gate bin/gate Run bin/run --fresh");
    // Its own line after the gate's, both in the one row that spans the table.
    expect(html).toContain('<div><b class="gate-label">Gate</b> <code class="path">bin/gate</code></div><div><b class="gate-label">Run</b> <code class="path" dir="ltr">bin/run --fresh</code></div>');
  });

  it("says how to set one when there's none, beside the gate's own hint", () => {
    expect(text(row("", ""))).toBe('Gate none: merges wait for one. muster projects set muster gate "<command>" Run none: say what starts it, to try it. muster projects set muster run "<command>"');
    expect(text(row("bin/gate", ""))).toBe('Gate bin/gate Run none: say what starts it, to try it. muster projects set muster run "<command>"');
  });

  it("is filled from the project the core listed: its own run command, or the hint", () => {
    const p: Project = { key: "/q", name: "muster", path: "/q", source: "found", group: "", hidden: false, ok: true, gate: "bin/gate", run: "bin/run" };
    const of = (q: Project) => text(renderToStaticMarkup(<table><tbody><ProjectGateRow p={q} /></tbody></table>));
    expect(of(p)).toBe("Gate bin/gate Run bin/run");
    expect(of({ ...p, run: undefined })).toBe('Gate bin/gate Run none: say what starts it, to try it. muster projects set muster run "<command>"');
    expect(renderToStaticMarkup(<table><tbody><ProjectGateRow p={{ ...p, hidden: true }} /></tbody></table>)).toContain('<tr class="gate-row dim">');
  });

  it("keeps the command left to right, so right-to-left text around it can't reorder it", () => {
    expect(renderToStaticMarkup(<RunCell name="muster" run="swift run --product نگار" />)).toBe('<code class="path" dir="ltr">swift run --product نگار</code>');
    expect(renderToStaticMarkup(<RunCell name="negar" run="" />)).toContain('<code dir="ltr">muster projects set negar run &quot;&lt;command&gt;&quot;</code>');
  });
});
