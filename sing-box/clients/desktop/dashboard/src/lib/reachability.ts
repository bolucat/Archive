import { normalizeServerUrl, type Server } from "../api/config";
import { DaemonApi } from "../api/daemon";

export async function probeServerReachable(
  server: Server,
  language: string,
  signal: AbortSignal,
): Promise<void> {
  const api = new DaemonApi(server, language);
  for await (const _ of api.client.subscribeServiceStatus({}, { signal })) {
    void _;
    return;
  }
  throw new Error("Stream ended without a status message");
}

export async function checkServerReachable(
  url: string,
  secret: string,
  language: string,
  signal: AbortSignal,
): Promise<boolean> {
  try {
    await probeServerReachable(
      { id: "", name: "", url: normalizeServerUrl(url), secret },
      language,
      signal,
    );
    return true;
  } catch {
    return false;
  }
}
