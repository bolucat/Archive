import { Fragment, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import type { DaemonApi } from "../api/daemon";
import { formatBytes, formatRelativeTime } from "../api/format";
import { isTerminalCode, StreamStore, useStream } from "../api/stream";
import { navigate, useApi, useNow } from "../app/context";
import {
  useDesktopHost,
  useLocalDesktopHost,
  type DesktopHost,
  type DesktopTaildropDownloadAction,
  type DesktopTaildropFile,
} from "../app/desktop";
import { showError } from "../app/errorStore";
import { useI18n } from "../app/i18n";
import { Icon } from "../components/Icon";
import { PageHeader } from "../components/PageHeader";
import {
  Badge,
  Button,
  Dialog,
  EmptyState,
  IconButton,
  MenuItem,
  OthersMenu,
  Spinner,
  StateDot,
} from "../components/ui";
import {
  ServiceStatus_Type,
  type TaildropFile,
  type TaildropReceivingFile,
  type TailscalePeer,
} from "../gen/daemon/started_service_pb";
import { canShareFiles, shareError, shareFile } from "../lib/sharing";
import {
  dismissTaildropSend,
  startDesktopTaildropSend,
  startTaildropSend,
  useTaildropSendFailures,
  useTaildropSendSessions,
} from "../lib/taildropSend";
import { allPeers, peerDisplayName } from "../lib/tailscaleSSH";
import styles from "./TaildropView.module.css";

const OBJECT_URL_LIFETIME = 60_000;
const BLOB_FLUSH_THRESHOLD = 8 * 1024 * 1024;

const MEDIA_TYPES: Record<string, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  jpeg: "image/jpeg",
  gif: "image/gif",
  webp: "image/webp",
  bmp: "image/bmp",
  pdf: "application/pdf",
  txt: "text/plain",
  log: "text/plain",
  json: "application/json",
  mp3: "audio/mpeg",
  wav: "audio/wav",
  mp4: "video/mp4",
  webm: "video/webm",
};

function mediaType(name: string): string {
  const dot = name.lastIndexOf(".");
  const known = dot >= 0 ? MEDIA_TYPES[name.slice(dot + 1).toLowerCase()] : undefined;
  return known ?? "application/octet-stream";
}

export function taildropPath(tag: string): string {
  return `tools/tailscale/${encodeURIComponent(tag)}/taildrop`;
}

export function TaildropNavBadge() {
  const api = useApi();
  const tailscale = useStream(api.tailscale);
  const failed = useTaildropSendFailures(api);
  const count = tailscale.data.endpoints.reduce(
    (total, endpoint) => total + endpoint.unreadFileCount,
    0,
  );
  if (failed.length > 0) {
    return <Badge tone="danger">!</Badge>;
  }
  if (count === 0) {
    return null;
  }
  return <Badge tone="accent">{count}</Badge>;
}

interface TaildropInboxData {
  files: TaildropFile[];
  receiving: TaildropReceivingFile[];
  loaded: boolean;
}

export function TaildropView(props: { tag: string }) {
  const api = useApi();
  const { t } = useI18n();
  const now = useNow(30_000);
  const store = useMemo(
    () =>
      new StreamStore<TaildropInboxData>(
        () => ({ files: [], receiving: [], loaded: false }),
        async ({ signal, update }) => {
          for await (const message of api.client.subscribeTaildropInbox(
            { endpointTag: props.tag },
            { signal },
          )) {
            update(() => ({
              files: message.files,
              receiving: message.receiving,
              loaded: true,
            }));
          }
        },
      ),
    [api, props.tag],
  );
  const inbox = useStream(store);
  const sending = useTaildropSendSessions(api, props.tag);
  const endpointPath = `tools/tailscale/${encodeURIComponent(props.tag)}`;
  const entered = useRef(false);
  const empty =
    sending.length === 0 && inbox.data.files.length === 0 && inbox.data.receiving.length === 0;

  useEffect(() => {
    if (entered.current || !inbox.data.loaded || empty) {
      return;
    }
    entered.current = true;
    void api.client.markTaildropInboxRead({ endpointTag: props.tag }).catch(showError);
  }, [api, props.tag, inbox.data.loaded, empty]);

  useEffect(() => {
    if (!inbox.data.loaded || !empty) {
      return;
    }
    navigate(endpointPath);
  }, [inbox.data.loaded, empty, endpointPath]);

  const terminalError =
    inbox.phase === "error" && isTerminalCode(inbox.errorCode) ? (inbox.error ?? "") : null;
  useEffect(() => {
    if (terminalError === null) {
      return;
    }
    showError(terminalError);
    navigate(endpointPath);
  }, [terminalError, endpointPath]);

  useEffect(() => {
    for (const session of sending) {
      if (session.error !== "") {
        showError(session.error);
        dismissTaildropSend(session.id);
      }
    }
  }, [sending]);

  const deleteAll = () => {
    for (const file of inbox.data.files) {
      void api.client
        .deleteTaildropFile({ endpointTag: props.tag, name: file.name })
        .catch(showError);
    }
  };

  const header = (
    <PageHeader
      title="Taildrop"
      back={{ label: "Tailscale", onClick: () => navigate(endpointPath) }}
      actions={
        inbox.data.files.length > 0 ? (
          <OthersMenu>
            <MenuItem danger icon="delete" onSelect={deleteAll}>
              {t("Delete All")}
            </MenuItem>
          </OthersMenu>
        ) : undefined
      }
    />
  );

  return (
    <div className="page">
      {header}
      {!inbox.data.loaded && (
        <EmptyState>
          <Spinner />
        </EmptyState>
      )}
      <div className="settings-stack">
        {sending.length > 0 && (
          <div>
            <div className="list-section-title">{t("Send")}</div>
            <div className={styles.fileList}>
              {sending.map((session) => (
                <Fragment key={session.id}>
                  {session.files.map((file, index) => (
                    <TransferRow
                      key={`${session.id}:${index}`}
                      name={file.name}
                      peer={t("To {name}", { name: session.peerName })}
                      transferred={file.completed ? file.size : file.sentBytes}
                      size={file.size}
                      finished={session.finished}
                      onCancel={() => dismissTaildropSend(session.id)}
                    />
                  ))}
                </Fragment>
              ))}
            </div>
          </div>
        )}
        {inbox.data.receiving.length > 0 && (
          <div>
            <div className="list-section-title">{t("Receive")}</div>
            <div className={styles.fileList}>
              {inbox.data.receiving.map((file) => (
                <TransferRow
                  key={`${file.senderID}:${file.name}`}
                  name={file.name}
                  peer={file.senderName !== "" ? t("From {name}", { name: file.senderName }) : ""}
                  transferred={Number(file.receivedBytes)}
                  size={Number(file.size)}
                  onCancel={() => {
                    void api.client
                      .cancelTaildropReceiving({
                        endpointTag: props.tag,
                        senderID: file.senderID,
                        name: file.name,
                      })
                      .catch(showError);
                  }}
                />
              ))}
            </div>
          </div>
        )}
        {inbox.data.files.length > 0 && (
          <div>
            <div className="list-section-title">{t("Files")}</div>
            <div className={styles.fileList}>
              {inbox.data.files.map((file) => (
                <InboxRow key={file.name} endpointTag={props.tag} file={file} now={now} />
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function ProgressBar(props: { value: number }) {
  return (
    <div className={styles.progressTrack}>
      <div
        className={styles.progressFill}
        style={{ width: `${Math.round(Math.min(1, Math.max(0, props.value)) * 100)}%` }}
      />
    </div>
  );
}

function TransferRow(props: {
  name: string;
  peer: string;
  transferred: number;
  size: number;
  finished?: boolean;
  onCancel: () => void;
}) {
  const { t } = useI18n();
  const finished = props.finished === true;
  const sizeText =
    props.size < 0
      ? formatBytes(props.transferred)
      : finished && props.transferred >= props.size
        ? formatBytes(props.size)
        : `${formatBytes(props.transferred)} / ${formatBytes(props.size)}`;
  const meta = [sizeText, props.peer].filter((part) => part !== "");
  return (
    <div className={styles.transferItem}>
      <div className={styles.transferHead}>
        <div className={styles.transferMain}>
          <span className={styles.fileName}>{props.name}</span>
          <span className={styles.fileMeta}>{meta.join(" · ")}</span>
        </div>
        <IconButton
          className={styles.fileAction}
          danger={!finished}
          title={finished ? t("Close") : t("Cancel")}
          onClick={props.onCancel}
        >
          <Icon name="close" />
        </IconButton>
      </div>
      {!finished && props.size > 0 && <ProgressBar value={props.transferred / props.size} />}
    </div>
  );
}

function onShareError(error: unknown): void {
  const reportableError = shareError(error);
  if (reportableError !== null) {
    showError(reportableError);
  }
}

async function receiveTaildropFile(
  api: DaemonApi,
  endpointTag: string,
  name: string,
  type: string,
  signal: AbortSignal,
  onProgress: (transferred: number, size: number) => void,
): Promise<Blob> {
  let content = new Blob([], { type });
  let pending: BlobPart[] = [];
  let pendingBytes = 0;
  let transferred = 0;
  let size = -1;
  const flush = () => {
    if (pendingBytes === 0) {
      return;
    }
    content = new Blob([content, ...pending], { type });
    pending = [];
    pendingBytes = 0;
  };
  for await (const chunk of api.client.downloadTaildropFile({ endpointTag, name }, { signal })) {
    if (size < 0) {
      size = Number(chunk.size);
    }
    if (chunk.data.length === 0) {
      continue;
    }
    pending.push(new Uint8Array(chunk.data).buffer);
    pendingBytes += chunk.data.length;
    transferred += chunk.data.length;
    onProgress(transferred, size);
    if (pendingBytes >= BLOB_FLUSH_THRESHOLD) {
      flush();
    }
  }
  flush();
  if (transferred !== size) {
    throw new Error(`incomplete download: ${transferred} of ${size} bytes`);
  }
  return content;
}

function saveInBrowser(name: string, content: Blob) {
  const url = URL.createObjectURL(content);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = name;
  anchor.click();
  URL.revokeObjectURL(url);
}

function openInBrowser(content: Blob) {
  const url = URL.createObjectURL(content);
  window.open(url, "_blank", "noreferrer");
  setTimeout(() => URL.revokeObjectURL(url), OBJECT_URL_LIFETIME);
}

let downloadSequence = 0;

function InboxRow(props: { endpointTag: string; file: TaildropFile; now: number }) {
  const api = useApi();
  const host = useDesktopHost();
  const localHost = useLocalDesktopHost();
  const { t, language } = useI18n();
  const [busy, setBusy] = useState(false);
  const [progress, setProgress] = useState<{ transferred: number; size: number } | null>(null);
  const [confirmingDelete, setConfirmingDelete] = useState(false);
  const name = props.file.name;
  const abortRef = useRef<AbortController | null>(null);
  const downloadRef = useRef(0);
  useEffect(() => () => abortRef.current?.abort(), []);
  useEffect(() => {
    if (localHost === null) {
      return;
    }
    return localHost.taildrop.onDownloadProgress((event) => {
      if (event.downloadID === downloadRef.current) {
        setProgress({ transferred: event.transferred, size: event.size });
      }
    });
  }, [localHost]);

  const download = (action: DesktopTaildropDownloadAction) => {
    if (busy) {
      return;
    }
    setBusy(true);
    setProgress(null);
    const settle = () => {
      setBusy(false);
      setProgress(null);
    };
    if (localHost !== null) {
      downloadSequence += 1;
      downloadRef.current = downloadSequence;
      void localHost.taildrop
        .download({
          downloadID: downloadRef.current,
          endpointTag: props.endpointTag,
          name,
          action,
        })
        .catch(action === "share" ? onShareError : showError)
        .finally(settle);
      return;
    }
    const controller = new AbortController();
    abortRef.current = controller;
    const type = action === "open" ? mediaType(name) : "application/octet-stream";
    void receiveTaildropFile(
      api,
      props.endpointTag,
      name,
      type,
      controller.signal,
      (transferred, size) => setProgress({ transferred, size }),
    )
      .then(async (content) => {
        if (action === "save") {
          saveInBrowser(name, content);
          return;
        }
        if (action === "open") {
          openInBrowser(content);
          return;
        }
        await shareFile(host, name, content, type).catch(onShareError);
      })
      .catch((error: unknown) => {
        if (!controller.signal.aborted) {
          showError(error);
        }
      })
      .finally(settle);
  };

  const meta = [
    formatBytes(props.file.size),
    props.file.senderName !== "" ? t("From {name}", { name: props.file.senderName }) : "",
    props.file.modifiedAt > 0n
      ? formatRelativeTime(Number(props.file.modifiedAt) * 1000, props.now, language)
      : "",
  ].filter((part) => part !== "");

  const primaryAction: DesktopTaildropDownloadAction =
    localHost !== null || host === null ? "open" : "save";

  return (
    <div className={styles.fileItem}>
      <button type="button" className={styles.fileMain} onClick={() => download(primaryAction)}>
        <span className={styles.fileName}>{name}</span>
        <span className={styles.fileMeta}>{meta.join(" · ")}</span>
        {progress !== null && progress.size > 0 && (
          <ProgressBar value={progress.transferred / progress.size} />
        )}
      </button>
      {busy && (
        <div className={styles.fileBusy}>
          <Spinner />
        </div>
      )}
      <OthersMenu className={styles.fileMenu} icon="more_horiz">
        <MenuItem icon="download" onSelect={() => download("save")}>
          {t("Save")}
        </MenuItem>
        {canShareFiles(host) && (
          <MenuItem icon="share" onSelect={() => download("share")}>
            {t("Share")}
          </MenuItem>
        )}
        <MenuItem danger icon="delete" onSelect={() => setConfirmingDelete(true)}>
          {t("Delete")}
        </MenuItem>
      </OthersMenu>
      {confirmingDelete && (
        <Dialog onClose={() => setConfirmingDelete(false)}>
          <h3>{t("Delete")}</h3>
          <div className="dialog-message">{t("Delete this received file?")}</div>
          <div className="dialog-message">{name}</div>
          <div className="row-actions dialog-actions">
            <Button onClick={() => setConfirmingDelete(false)}>{t("Cancel")}</Button>
            <Button
              variant="danger"
              onClick={() => {
                setConfirmingDelete(false);
                void api.client
                  .deleteTaildropFile({ endpointTag: props.endpointTag, name })
                  .catch(showError);
              }}
            >
              {t("Delete")}
            </Button>
          </div>
        </Dialog>
      )}
    </div>
  );
}

export function useTaildropSend(tag: string) {
  const api = useApi();
  const host = useLocalDesktopHost();
  const inputRef = useRef<HTMLInputElement>(null);
  const targetRef = useRef<TailscalePeer | null>(null);

  const send = (peer: TailscalePeer, files: File[]) => {
    if (files.length === 0) {
      return;
    }
    if (host !== null) {
      const local = files
        .map((file) => ({
          name: file.name,
          size: file.size,
          path: host.taildrop.pathForFile(file),
        }))
        .filter((file) => file.path !== "");
      startDesktopTaildropSend(api, host, tag, peer.stableID, peerDisplayName(peer), local);
    } else {
      startTaildropSend(api, tag, peer.stableID, peerDisplayName(peer), files);
    }
    navigate(taildropPath(tag));
  };

  const pick = (peer: TailscalePeer) => {
    const input = inputRef.current;
    if (!input) {
      return;
    }
    targetRef.current = peer;
    input.value = "";
    input.click();
  };

  const element = (
    <input
      ref={inputRef}
      className={styles.fileInput}
      type="file"
      multiple
      onChange={(event) => {
        const peer = targetRef.current;
        targetRef.current = null;
        if (peer) {
          send(peer, Array.from(event.target.files ?? []));
        }
      }}
    />
  );

  return { send, pick, element };
}

export function TaildropSendRequestDialog(props: { host: DesktopHost }) {
  const host = props.host;
  const [files, setFiles] = useState<DesktopTaildropFile[] | null>(null);

  useEffect(() => host.onTaildropSendRequested((requested) => setFiles(() => requested)), [host]);

  if (files === null) {
    return null;
  }
  return <SendRequestFlow host={host} files={files} onClose={() => setFiles(null)} />;
}

function SendRequestFlow(props: {
  host: DesktopHost;
  files: DesktopTaildropFile[];
  onClose: () => void;
}) {
  const api = useApi();
  const { t } = useI18n();
  const serviceStatus = useStream(api.serviceStatus);
  const tailscale = useStream(api.tailscale);

  const started = serviceStatus.data.status?.status === ServiceStatus_Type.STARTED;
  const targets = tailscale.data.endpoints
    .filter((endpoint) => endpoint.canShareFiles)
    .map((endpoint) => ({
      endpoint,
      peers: allPeers(endpoint).filter((peer) => peer.online && peer.canReceiveFiles),
    }))
    .filter((target) => target.peers.length > 0);

  const startSending = (tag: string, peer: TailscalePeer) => {
    startDesktopTaildropSend(
      api,
      props.host,
      tag,
      peer.stableID,
      peerDisplayName(peer),
      props.files,
    );
    navigate(taildropPath(tag));
    props.onClose();
  };

  let body: ReactNode;
  if (!started) {
    body = <div className={styles.emptyHint}>{t("The sing-box service is not running")}</div>;
  } else if (!tailscale.data.loaded) {
    body = (
      <div className={styles.dialogBusy}>
        <Spinner />
      </div>
    );
  } else if (targets.length === 0) {
    body = <div className={styles.emptyHint}>{t("No devices can receive files")}</div>;
  } else {
    body = targets.map(({ endpoint, peers }) => (
      <Fragment key={endpoint.endpointTag}>
        {targets.length > 1 && (
          <div className="list-section-title">{endpoint.networkName || endpoint.endpointTag}</div>
        )}
        {peers.map((peer) => (
          <button
            type="button"
            className="peer-row"
            key={`${endpoint.endpointTag}:${peer.stableID}`}
            onClick={() => startSending(endpoint.endpointTag, peer)}
          >
            <StateDot tone="good" />
            <span className="peer-name">{peerDisplayName(peer)}</span>
            <span className="peer-address">{peer.tailscaleIPs[0] ?? ""}</span>
          </button>
        ))}
      </Fragment>
    ));
  }

  return (
    <Dialog onClose={props.onClose}>
      <h3>Taildrop</h3>
      {body}
      <div className="row-actions dialog-actions">
        <Button onClick={props.onClose}>{t("Cancel")}</Button>
      </div>
    </Dialog>
  );
}
