/** Sidebar extras (Todos, Scratchpad) register a flush so pending text is saved
 * before the workspace changes: closing tabs, projects, committing or quitting. */
export function createExtrasFlush(fail: (e: unknown) => void) {
  const flushers = new Set<() => Promise<void>>();
  async function flushExtras() {
    await Promise.all([...flushers].map((flush) => flush()));
  }
  function registerExtrasFlush(flush: () => Promise<void>) {
    flushers.add(flush);
    return () => {
      queueMicrotask(() => {
        void flush().then(
          () => flushers.delete(flush),
          (error) => {
            flushers.delete(flush);
            fail(error);
          },
        );
      });
    };
  }
  return { flushExtras, registerExtrasFlush };
}
