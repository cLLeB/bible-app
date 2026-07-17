import { describe, expect, it } from "vitest";
import type { UsageRow } from "../api";
import { usageCsv } from "./usageCsv";

describe("usageCsv", () => {
  it("renders a header and one row per song", () => {
    const rows: UsageRow[] = [
      { title: "Amazing Grace", author: "Newton", times: 3, lastUsed: "2026-07-12" },
      { title: "Doxology", author: null, times: 1, lastUsed: null },
    ];
    const csv = usageCsv(rows).split("\n");
    expect(csv[0]).toBe("Title,Author,Times used,Last used");
    expect(csv[1]).toBe("Amazing Grace,Newton,3,2026-07-12");
    expect(csv[2]).toBe("Doxology,,1,");
  });
  it("quotes values containing commas or quotes", () => {
    const rows: UsageRow[] = [
      { title: 'Great, Great', author: 'A "B"', times: 2, lastUsed: "2026-01-01" },
    ];
    expect(usageCsv(rows).split("\n")[1]).toBe('"Great, Great","A ""B""",2,2026-01-01');
  });
});
