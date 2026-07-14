import { useLayoutEffect, useRef, type RefObject } from "react";

/** Keeps callbacks used by long-lived subscriptions current without mutating refs during render. */
export function useLatestRef<T>(value: T): RefObject<T> {
  const ref = useRef(value);
  useLayoutEffect(() => {
    ref.current = value;
  }, [value]);
  return ref;
}
