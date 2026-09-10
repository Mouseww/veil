import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { DryRun } from "./DryRun";

vi.mock("./api", () => ({ api: { dryRun: vi.fn() } }));

describe("DryRun", () => {
  it("shows type and snippet of the sample without fetching", () => {
    const sample = "call 13800138000 now";
    render(
      <DryRun
        sample={sample}
        hits={[{ type_prefix: "PHONE", start: 5, end: 16, matched: "13800138000" }]}
      />
    );
    expect(screen.getByText("PHONE")).toBeInTheDocument();
    expect(screen.getByText("13800138000")).toBeInTheDocument();
  });
});
