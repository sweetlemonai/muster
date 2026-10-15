// The speed tests' timer (mu-220): one untimed warm-up, then the fastest of the timed runs.
import { afterEach, describe, expect, it, vi } from "vitest";
import { fastest } from "./fastest";

afterEach(() => vi.restoreAllMocks());

// A clock where each run takes the next of `costs` ms; the warm-up costs nothing and isn't read.
const clock = (costs: number[]) => {
  let now = 0;
  let i = 0;
  let started = false;
  vi.spyOn(performance, "now").mockImplementation(() => {
    if (started) now += costs[i++];
    started = !started;
    return now;
  });
};

describe("fastest", () => {
  it("is the fastest of the timed runs, whatever order they come in", () => {
    clock([40, 9, 30, 7, 80, 12, 20]);
    expect(fastest(() => 0, 7).ms).toBe(7);
  });
  it("runs once untimed, then `runs` timed, and gives the last run's result", () => {
    let calls = 0;
    clock(Array(7).fill(1));
    const { out } = fastest(() => ++calls, 7);
    expect(calls).toBe(8);
    expect(out).toBe(8);
    expect(performance.now).toHaveBeenCalledTimes(14);
  });
  it("is slow when every run is slow, so a real slowdown still fails", () => {
    clock(Array(15).fill(60));
    expect(fastest(() => 0).ms).toBe(60);
  });
  it("takes 15 timed runs by default", () => {
    let calls = 0;
    clock(Array(15).fill(1));
    fastest(() => calls++);
    expect(calls).toBe(16);
  });
});
