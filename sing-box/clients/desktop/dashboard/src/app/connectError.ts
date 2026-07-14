import { useEffect, useState } from "react";

import { serverConnectUrl } from "../api/config";
import {
  diagnoseConnection,
  isOpaqueNetworkError,
  isUnknownServiceError,
  type ConnectionDiagnosis,
} from "../lib/connectivity";
import { useI18n, type MessageKey } from "./i18n";

const DIAGNOSIS_MESSAGES: Record<ConnectionDiagnosis, MessageKey> = {
  "offline": "The browser is offline; check your network connection.",
  "cors-blocked":
    "The server is reachable, but the browser blocked the request: the server does not allow this origin (CORS).",
  "mixed-content":
    "The browser blocked the request: an HTTPS page cannot access an HTTP server. Open the dashboard over HTTP, or serve the API over HTTPS.",
  "mixed-content-or-unreachable":
    "The server is unreachable — or, if it is running, the browser blocked the HTTPS page from accessing the HTTP server; try opening the dashboard over HTTP.",
  "unreachable":
    "The server is unreachable; check that the address is correct and the service is running.",
};

export function useDiagnosedConnectError(
  message: string | null,
  serverUrl: string,
): string | null {
  const { t } = useI18n();
  const opaque = message !== null && isOpaqueNetworkError(message);
  const [diagnosis, setDiagnosis] = useState<ConnectionDiagnosis | null>(null);
  useEffect(() => {
    setDiagnosis(null);
    if (!opaque) {
      return;
    }
    const controller = new AbortController();
    void diagnoseConnection(serverConnectUrl(serverUrl), controller.signal).then((result) => {
      if (!controller.signal.aborted) {
        setDiagnosis(() => result);
      }
    });
    return () => controller.abort();
  }, [opaque, message, serverUrl]);
  if (message === null) {
    return null;
  }
  if (isUnknownServiceError(message)) {
    return `${message} — ${t("This is not a sing-box API service, or the path is incorrect.")}`;
  }
  return diagnosis === null ? message : `${message} — ${t(DIAGNOSIS_MESSAGES[diagnosis])}`;
}
