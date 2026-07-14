import { Code, ConnectError } from "@connectrpc/connect";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { StreamStore } from "./stream";

describe("StreamStore", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("starts the stream on the first subscriber and publishes updates", async () => {
    const store = new StreamStore<number[]>(
      () => [],
      async ({ update }) => {
        update(() => [1]);
        await new Promise(() => {});
      },
    );
    let notified = 0;
    const unsubscribe = store.subscribe(() => {
      notified += 1;
    });

    expect(store.getSnapshot().phase).toBe("active");
    expect(store.getSnapshot().data).toEqual([1]);
    await vi.advanceTimersByTimeAsync(0);
    expect(notified).toBeGreaterThan(0);
    unsubscribe();
  });

  it("coalesces a burst of updates into a single notification", async () => {
    const store = new StreamStore<number>(
      () => 0,
      async ({ update }) => {
        for (let i = 1; i <= 100; i += 1) {
          update(() => i);
        }
        await new Promise(() => {});
      },
    );
    let notified = 0;
    const unsubscribe = store.subscribe(() => {
      notified += 1;
    });

    expect(store.getSnapshot().data).toBe(100);
    expect(notified).toBe(0);

    await vi.advanceTimersByTimeAsync(0);
    expect(notified).toBe(1);
    expect(store.getSnapshot().data).toBe(100);
    unsubscribe();
  });

  it("publishes updates while rendering frames are suspended", async () => {
    const requestFrame = vi.fn(() => 1);
    vi.stubGlobal("requestAnimationFrame", requestFrame);
    vi.resetModules();
    const { StreamStore: FrameIndependentStreamStore } = await import("./stream");
    const store = new FrameIndependentStreamStore<number>(
      () => 0,
      async ({ update }) => {
        update(() => 1);
        await new Promise(() => {});
      },
    );
    let notified = 0;
    const unsubscribe = store.subscribe(() => {
      notified += 1;
    });

    await vi.advanceTimersByTimeAsync(0);
    expect(notified).toBe(1);
    expect(requestFrame).not.toHaveBeenCalled();
    unsubscribe();
  });

  it("aborts the stream when the last subscriber leaves", () => {
    let aborted = false;
    const store = new StreamStore<null>(
      () => null,
      ({ signal }) => {
        signal.addEventListener("abort", () => {
          aborted = true;
        });
        return new Promise(() => {});
      },
    );
    const unsubscribe = store.subscribe(() => {});
    expect(aborted).toBe(false);
    unsubscribe();
    expect(aborted).toBe(true);
  });

  it("reconnects with backoff after a retryable error", async () => {
    let calls = 0;
    const store = new StreamStore<null>(
      () => null,
      async () => {
        calls += 1;
        throw new Error("boom");
      },
    );
    const unsubscribe = store.subscribe(() => {});

    await vi.advanceTimersByTimeAsync(0);
    expect(calls).toBe(1);
    expect(store.getSnapshot().phase).toBe("error");
    expect(store.getSnapshot().error).toBe("boom");

    await vi.advanceTimersByTimeAsync(1000);
    expect(calls).toBe(2);
    expect(store.getSnapshot().phase).toBe("error");
    expect(store.getSnapshot().error).toBe("boom");
    await vi.advanceTimersByTimeAsync(1999);
    expect(calls).toBe(2);
    expect(store.getSnapshot().phase).toBe("error");
    await vi.advanceTimersByTimeAsync(1);
    expect(calls).toBe(3);
    expect(store.getSnapshot().phase).toBe("error");
    unsubscribe();
  });

  it("keeps the last error visible while a reconnect attempt is pending", async () => {
    let calls = 0;
    let rejectFirst: (error: Error) => void = () => {};
    const store = new StreamStore<null>(
      () => null,
      () => {
        calls += 1;
        if (calls === 1) {
          return new Promise((_, reject) => {
            rejectFirst = reject;
          });
        }
        return new Promise(() => {});
      },
    );
    const unsubscribe = store.subscribe(() => {});

    rejectFirst(new Error("offline"));
    await vi.advanceTimersByTimeAsync(0);
    expect(store.getSnapshot()).toMatchObject({ phase: "error", error: "offline" });

    await vi.advanceTimersByTimeAsync(1000);
    expect(calls).toBe(2);
    expect(store.getSnapshot()).toMatchObject({ phase: "error", error: "offline" });
    unsubscribe();
  });

  it("stops permanently on errors a retry cannot fix", async () => {
    let calls = 0;
    const store = new StreamStore<null>(
      () => null,
      async () => {
        calls += 1;
        throw new ConnectError("bad secret", Code.Unauthenticated);
      },
    );
    const unsubscribe = store.subscribe(() => {});

    await vi.advanceTimersByTimeAsync(0);
    expect(store.getSnapshot().phase).toBe("error");
    expect(store.getSnapshot().errorCode).toBe(Code.Unauthenticated);

    await vi.advanceTimersByTimeAsync(60_000);
    expect(calls).toBe(1);
    unsubscribe();
  });

  it("restarts a fatally stopped stream when a subscriber returns", async () => {
    let calls = 0;
    const store = new StreamStore<null>(
      () => null,
      async () => {
        calls += 1;
        throw new ConnectError("bad secret", Code.Unauthenticated);
      },
    );
    const first = store.subscribe(() => {});
    await vi.advanceTimersByTimeAsync(0);
    expect(calls).toBe(1);
    first();

    const second = store.subscribe(() => {});
    await vi.advanceTimersByTimeAsync(0);
    expect(calls).toBe(2);
    second();
  });

  it("retryNow cuts the backoff short and restarts the attempt counter", async () => {
    let calls = 0;
    const store = new StreamStore<null>(
      () => null,
      async () => {
        calls += 1;
        throw new Error("boom");
      },
    );
    const unsubscribe = store.subscribe(() => {});

    await vi.advanceTimersByTimeAsync(0);
    await vi.advanceTimersByTimeAsync(1000);
    await vi.advanceTimersByTimeAsync(2000);
    expect(calls).toBe(3);

    store.retryNow();
    await vi.advanceTimersByTimeAsync(0);
    expect(calls).toBe(4);

    await vi.advanceTimersByTimeAsync(999);
    expect(calls).toBe(4);
    await vi.advanceTimersByTimeAsync(1);
    expect(calls).toBe(5);
    unsubscribe();
  });

  it("skips the next backoff when retryNow arrives before the failure", async () => {
    let calls = 0;
    let rejectCurrent: (error: Error) => void = () => {};
    const store = new StreamStore<null>(
      () => null,
      () => {
        calls += 1;
        return new Promise((_, reject) => {
          rejectCurrent = reject;
        });
      },
    );
    const unsubscribe = store.subscribe(() => {});
    expect(calls).toBe(1);

    store.retryNow();
    rejectCurrent(new Error("connection lost"));
    await vi.advanceTimersByTimeAsync(0);
    expect(calls).toBe(2);
    unsubscribe();
  });

  it("reconnects an active stream immediately", async () => {
    let calls = 0;
    let aborted = 0;
    const store = new StreamStore<number>(
      () => 0,
      ({ signal, update }) => {
        calls += 1;
        update(() => calls);
        signal.addEventListener("abort", () => {
          aborted += 1;
        });
        return new Promise(() => {});
      },
    );
    const unsubscribe = store.subscribe(() => {});
    expect(calls).toBe(1);

    store.reconnectNow();
    expect(aborted).toBe(1);
    expect(calls).toBe(2);
    expect(store.getSnapshot()).toMatchObject({ phase: "active", data: 2 });
    unsubscribe();
  });

  it("resets data on reconnect when configured to", async () => {
    let calls = 0;
    const store = new StreamStore<number[]>(
      () => [],
      async ({ update }) => {
        calls += 1;
        update((data) => data.concat(calls));
        throw new Error("boom");
      },
      true,
    );
    const unsubscribe = store.subscribe(() => {});

    await vi.advanceTimersByTimeAsync(1000);
    expect(calls).toBe(2);
    expect(store.getSnapshot().data).toEqual([2]);
    unsubscribe();
  });
});
