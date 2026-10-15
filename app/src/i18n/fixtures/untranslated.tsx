// A screen with English written straight into it: literals.test.ts proves its guard fails on this.
export function Untranslated({ count }: { count: number }) {
  return (
    <section aria-label="Waiting tasks">
      <h2>Nothing waits for you</h2>
      <p>{count} tasks</p>
    </section>
  );
}
