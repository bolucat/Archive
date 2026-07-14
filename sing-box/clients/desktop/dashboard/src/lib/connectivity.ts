export type ConnectionDiagnosis =
  | "offline"
  | "cors-blocked"
  | "mixed-content"
  | "mixed-content-or-unreachable"
  | "unreachable";

export function isOpaqueNetworkError(message: string): boolean {
  return (
    message.includes("Failed to fetch") || // Chromium
    message.includes("Load failed") || // WebKit
    message.includes("NetworkError when attempting to fetch resource") // Firefox
  );
}

export function isUnknownServiceError(message: string): boolean {
  return message.includes("unimplemented") && message.includes("StartedService");
}

export function guessApiBaseUrl(href: string): string | null {
  let dir: URL;
  try {
    dir = new URL(".", href);
  } catch {
    return null;
  }
  if (dir.pathname === "/") {
    return null;
  }
  return new URL("..", dir).href;
}

export function isLoopbackHost(hostname: string): boolean {
  return (
    hostname === "localhost" ||
    hostname.endsWith(".localhost") ||
    hostname === "[::1]" ||
    /^127(\.\d{1,3}){3}$/.test(hostname)
  );
}

const PROBE_TIMEOUT_MS = 5000;

export async function diagnoseConnection(
  serverUrl: string,
  signal: AbortSignal,
): Promise<ConnectionDiagnosis> {
  if (!navigator.onLine) {
    return "offline";
  }
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), PROBE_TIMEOUT_MS);
  const onAbort = () => controller.abort();
  signal.addEventListener("abort", onAbort, { once: true });
  try {
    await fetch(serverUrl, { mode: "no-cors", cache: "no-store", signal: controller.signal });
    return "cors-blocked";
  } catch {
    if (location.protocol === "https:" && serverUrl.startsWith("http:")) {
      try {
        return isLoopbackHost(new URL(serverUrl).hostname)
          ? "mixed-content-or-unreachable"
          : "mixed-content";
      } catch {
        return "mixed-content";
      }
    }
    return "unreachable";
  } finally {
    clearTimeout(timer);
    signal.removeEventListener("abort", onAbort);
  }
}
