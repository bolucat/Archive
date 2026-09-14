export interface CheckScheduler {
  schedule(): void;
  cancel(): void;
  unblocked(): void;
}

export function createCheckScheduler(options: {
  delayMs: number;
  isBlocked: () => boolean;
  run: () => void;
}): CheckScheduler {
  let timer: ReturnType<typeof setTimeout> | null = null;
  let deferred = false;
  const scheduler: CheckScheduler = {
    schedule() {
      scheduler.cancel();
      timer = setTimeout(() => {
        timer = null;
        if (options.isBlocked()) {
          deferred = true;
          return;
        }
        options.run();
      }, options.delayMs);
    },
    cancel() {
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
      }
      deferred = false;
    },
    unblocked() {
      if (timer !== null || deferred) {
        scheduler.schedule();
      }
    },
  };
  return scheduler;
}
