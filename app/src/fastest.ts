// For tests only (mu-220): how long `run` takes on its own. One warm-up, then the fastest of
// `runs` timed runs. Other sessions' load only ever adds time, so the fastest run is the code's
// own cost and a busy machine can't fail a speed budget; a real slowdown is slow every run.
export function fastest<T>(run: () => T, runs = 15): { ms: number; out: T } {
  let out = run();
  let ms = Infinity;
  for (let i = 0; i < runs; i++) {
    const t0 = performance.now();
    out = run();
    ms = Math.min(ms, performance.now() - t0);
  }
  return { ms, out };
}
