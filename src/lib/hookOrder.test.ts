import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * No hook may be called after an early return.
 *
 * React counts hooks per render. A component that returns early on one render and
 * then reaches a `useEffect` on the next has changed its hook count, and React
 * responds by tearing the tree down - the whole console goes blank with no clue on
 * screen as to why.
 *
 * That happened: a `useEffect` added to PreviewPane landed below its
 * `if (!staged) return null`, and previewing a video blanked the app. TypeScript is
 * perfectly happy with it, the tests all passed, and the production build succeeded.
 * The usual guard is eslint's react-hooks/rules-of-hooks, which this project does not
 * have, so this stands in for it: cheap, offline, and aimed at exactly the mistake
 * that got through.
 *
 * Deliberately a simple reading of the source rather than a parse. It looks only at
 * top-level statements inside a component - two spaces of indent - which is where
 * both hooks and guard clauses live, and where the bug was.
 */

const HOOK = /^ {2}(?:const|let)?\s*[\w{},[\]\s:]*=?\s*use[A-Z]\w*\(/;
const HOOK_BARE = /^ {2}use[A-Z]\w*\(/;
const EARLY_RETURN = /^ {2}if \(.*\)\s*return\b/;
const COMPONENT_END = /^\}/;

function tsxFiles(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const full = join(dir, name);
    if (statSync(full).isDirectory()) return tsxFiles(full);
    return name.endsWith(".tsx") ? [full] : [];
  });
}

/** Lines where a hook is called after a guard clause, with the guard's line. */
function offenders(source: string): string[] {
  const out: string[] = [];
  let guardedAt: number | null = null;
  source.split("\n").forEach((line, i) => {
    if (COMPONENT_END.test(line)) {
      guardedAt = null; // left the component; the next one starts clean
      return;
    }
    if (EARLY_RETURN.test(line)) {
      guardedAt = i + 1;
      return;
    }
    if (guardedAt !== null && (HOOK.test(line) || HOOK_BARE.test(line))) {
      out.push(`line ${i + 1} (${line.trim().slice(0, 48)}) after the guard on line ${guardedAt}`);
    }
  });
  return out;
}

describe("hooks are never called after an early return", () => {
  it("holds for every component in the app", () => {
    const problems = tsxFiles("src").flatMap((file) =>
      offenders(readFileSync(file, "utf8")).map((p) => `${file}: ${p}`),
    );
    expect(problems).toEqual([]);
  });

  it("catches the shape of the bug it was written for", () => {
    // PreviewPane, as it was: the guard, then a hook below it.
    const broken = [
      "export function Thing() {",
      "  const [a, setA] = useState(1);",
      "  if (!a) return null;",
      "  useEffect(() => {}, [a]);",
      "  return null;",
      "}",
    ].join("\n");
    expect(offenders(broken)).toHaveLength(1);
  });

  it("does not complain when every hook comes first", () => {
    const fine = [
      "export function Thing() {",
      "  const [a, setA] = useState(1);",
      "  useEffect(() => {}, [a]);",
      "  if (!a) return null;",
      "  return null;",
      "}",
    ].join("\n");
    expect(offenders(fine)).toEqual([]);
  });
});
