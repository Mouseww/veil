import { describe, expect, it } from "vitest";
import { COLUMNS } from "./Traffic";
import type { TrafficEvent } from "../api";

describe("Traffic table", () => {
  it("does not expose query or body columns", () => {
    expect(COLUMNS).not.toContain("query");
    expect(COLUMNS).not.toContain("body");
    const sample: TrafficEvent = {
      ts: 0,
      method: "POST",
      path_template: "/v1/messages",
      status: 200,
      streaming: true,
      hit_types: ["PHONE"],
      hit_counts: [1],
      latency_ms: 12,
      error_class: null,
      creator_prefix8: "abcd1234",
    };
    expect("query" in sample).toBe(false);
    expect("body" in sample).toBe(false);
  });
});
