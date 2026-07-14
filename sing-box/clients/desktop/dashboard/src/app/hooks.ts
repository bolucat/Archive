import { useEffect, useRef, useState, type RefObject } from "react";

import type { StreamSnapshot } from "../api/stream";
import { loadTerminalConfig, TERMINAL_CONFIG_EVENT, type TerminalConfig } from "../lib/tailscaleSSH";
import { useLatestRef } from "./useLatest";

export const RECONNECT_GRACE_MS = 6500;

export function useStreamOutage(
  snapshot: StreamSnapshot<unknown>,
  immediate: boolean,
  graceMs = RECONNECT_GRACE_MS,
): string | null {
  const [outage, setOutage] = useState<string | null>(null);
  const lastError = useRef("");
  const timer = useRef<number | null>(null);
  const phase = snapshot.phase;
  const error = snapshot.error;
  useEffect(() => {
    const cancel = () => {
      if (timer.current !== null) {
        clearTimeout(timer.current);
        timer.current = null;
      }
    };
    if (phase === "active") {
      cancel();
      setOutage((prev) => (prev === null ? prev : null));
    } else if (phase === "error") {
      lastError.current = error ?? "";
      if (immediate) {
        cancel();
      } else if (timer.current === null) {
        timer.current = window.setTimeout(() => {
          timer.current = null;
          setOutage(lastError.current);
        }, graceMs);
      }
    }
  }, [phase, error, immediate, graceMs]);
  useEffect(() => {
    const pending = timer;
    return () => {
      if (pending.current !== null) {
        clearTimeout(pending.current);
      }
    };
  }, []);
  return phase === "error" && immediate ? (error ?? "") : outage;
}

// Mobile visualViewport height stays fixed while the page rubber-band scrolls;
// offsetTop changes even though the keyboard remains anchored to the layout viewport.
export function useKeyboardInset(): number {
  const [inset, setInset] = useState(0);
  useEffect(() => {
    const viewport = window.visualViewport;
    if (!viewport) {
      return;
    }
    const update = () => {
      const height = window.innerHeight - viewport.height;
      setInset(height > 1 ? Math.round(height) : 0);
    };
    update();
    viewport.addEventListener("resize", update);
    return () => viewport.removeEventListener("resize", update);
  }, []);
  return inset;
}

export function useTerminalConfig(): TerminalConfig {
  const [config, setConfig] = useState<TerminalConfig>(loadTerminalConfig);
  useEffect(() => {
    const update = () => setConfig(loadTerminalConfig());
    window.addEventListener("storage", update);
    window.addEventListener(TERMINAL_CONFIG_EVENT, update);
    return () => {
      window.removeEventListener("storage", update);
      window.removeEventListener(TERMINAL_CONFIG_EVENT, update);
    };
  }, []);
  return config;
}

const escapeStack: (() => void)[] = [];

function useEscapeEntry(active: boolean, onDismiss: () => void) {
  const dismissRef = useLatestRef(onDismiss);
  useEffect(() => {
    if (!active) {
      return;
    }
    const entry = () => dismissRef.current();
    escapeStack.push(entry);
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape" && escapeStack[escapeStack.length - 1] === entry) {
        event.preventDefault();
        entry();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => {
      const index = escapeStack.indexOf(entry);
      if (index >= 0) {
        escapeStack.splice(index, 1);
      }
      window.removeEventListener("keydown", onKeyDown);
    };
  }, [active, dismissRef]);
}

export function useDismiss(
  ref: RefObject<HTMLElement | null>,
  open: boolean,
  onDismiss: () => void,
) {
  useEscapeEntry(open, onDismiss);
  const dismissRef = useLatestRef(onDismiss);
  useEffect(() => {
    if (!open) {
      return;
    }
    const onPointerDown = (event: PointerEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) {
        dismissRef.current();
      }
    };
    window.addEventListener("pointerdown", onPointerDown);
    return () => window.removeEventListener("pointerdown", onPointerDown);
  }, [ref, open, dismissRef]);
}

export function usePendingValue<T>(serverValue: T): [T, (pending: T | null) => void] {
  const [pending, setPending] = useState<T | null>(null);
  if (pending !== null && serverValue === pending) {
    setPending(null);
  }
  return [pending ?? serverValue, setPending];
}

// Failures are ignored: daemons predating the method reject with Unimplemented.
export function useUnaryOnce<T>(call: () => Promise<T>, enabled = true): T | null {
  const [value, setValue] = useState<T | null>(null);
  const callRef = useLatestRef(call);
  useEffect(() => {
    if (!enabled || value !== null) {
      return;
    }
    let stale = false;
    callRef.current().then(
      (result) => {
        if (!stale) {
          setValue(() => result);
        }
      },
      () => {},
    );
    return () => {
      stale = true;
    };
  }, [enabled, value, callRef]);
  return value;
}

export function useStreamingAction(): {
  running: boolean;
  error: string;
  reportError: (message: string) => void;
  start: (run: (signal: AbortSignal) => Promise<void>) => void;
  stop: () => void;
} {
  const [running, setRunning] = useState(false);
  const [error, setError] = useState("");
  const controllerRef = useRef<AbortController | null>(null);

  useEffect(() => () => controllerRef.current?.abort(), []);

  const start = (run: (signal: AbortSignal) => Promise<void>) => {
    const controller = new AbortController();
    controllerRef.current = controller;
    setRunning(true);
    setError("");
    void run(controller.signal)
      .catch((streamError: unknown) => {
        if (!controller.signal.aborted) {
          setError(String(streamError));
        }
      })
      .finally(() => setRunning(false));
  };

  const stop = () => {
    controllerRef.current?.abort();
    setRunning(false);
  };

  return { running, error, reportError: setError, start, stop };
}
