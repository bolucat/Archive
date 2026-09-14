import { useEffect, useState, useSyncExternalStore } from "react";

import { formatBytes } from "../api/format";
import type {
  DesktopHost,
  DesktopUpdateInfo,
  DesktopUpdateInstallResult,
  DesktopUpdatesState,
} from "../app/desktop";
import { showError } from "../app/errorStore";
import { useI18n } from "../app/i18n";
import { GitHubMarkdown } from "../components/GitHubMarkdown";
import { Button, Dialog } from "../components/ui";
import styles from "./UpdateViews.module.css";

let updateDialogVisible = false;
const updateDialogListeners = new Set<() => void>();

function notifyUpdateDialogListeners() {
  for (const listener of updateDialogListeners) {
    listener();
  }
}

export function openUpdateDialog() {
  if (!updateDialogVisible) {
    updateDialogVisible = true;
    notifyUpdateDialogListeners();
  }
}

function closeUpdateDialog() {
  if (updateDialogVisible) {
    updateDialogVisible = false;
    notifyUpdateDialogListeners();
  }
}

function subscribeUpdateDialog(listener: () => void): () => void {
  updateDialogListeners.add(listener);
  return () => {
    updateDialogListeners.delete(listener);
  };
}

export function useUpdatesState(host: DesktopHost): DesktopUpdatesState | null {
  const [state, setState] = useState<DesktopUpdatesState | null>(null);

  useEffect(() => {
    let stale = false;
    let pushed = false;
    const unsubscribe = host.updates.onStateChanged((value) => {
      pushed = true;
      setState(() => value);
    });
    host.updates
      .state()
      .then((value) => {
        if (!stale && !pushed) {
          setState(() => value);
        }
      })
      .catch(showError);
    return () => {
      stale = true;
      unsubscribe();
    };
  }, [host]);

  return state;
}

export function UpdatesGate(props: { host: DesktopHost }) {
  const host = props.host;
  const state = useUpdatesState(host);
  const dialogVisible = useSyncExternalStore(subscribeUpdateDialog, () => updateDialogVisible);

  useEffect(() => host.updates.onPresentRequested(openUpdateDialog), [host]);

  const currentVersion = state?.info?.versionName;
  useEffect(() => {
    if (dialogVisible && currentVersion !== undefined) {
      void host.updates.markShown().catch(() => {});
    }
  }, [dialogVisible, currentVersion, host]);

  if (state === null || !state.supported) {
    return null;
  }

  return (
    <>
      {!state.prompted && <UpdateCheckPromptDialog host={host} />}
      {dialogVisible && state.info !== null && (
        <UpdateDialog host={host} info={state.info} state={state} />
      )}
    </>
  );
}

function UpdateCheckPromptDialog(props: { host: DesktopHost }) {
  const host = props.host;
  const { t } = useI18n();
  const [busy, setBusy] = useState(false);

  const decline = () => {
    void host.updates.setPrompted().catch(showError);
  };

  const accept = () => {
    setBusy(true);
    host.updates
      .setCheckUpdateEnabled(true)
      .then(() => host.updates.check())
      .then((info) => {
        if (info !== null) {
          openUpdateDialog();
        }
      })
      .catch(showError)
      .finally(() => setBusy(false));
  };

  return (
    <Dialog onClose={decline}>
      <h3>{t("Check Update")}</h3>
      <p className="dialog-message">
        {t("Would you like to enable automatic update checking from GitHub?")}
      </p>
      <div className="row-actions dialog-actions">
        <Button disabled={busy} onClick={decline}>
          {t("No, thanks")}
        </Button>
        <Button variant="primary" disabled={busy} onClick={accept}>
          {t("Ok")}
        </Button>
      </div>
    </Dialog>
  );
}

function UpdateDialog(props: {
  host: DesktopHost;
  info: DesktopUpdateInfo;
  state: DesktopUpdatesState;
}) {
  const host = props.host;
  const { t } = useI18n();
  const [elevationReason, setElevationReason] = useState<
    Exclude<DesktopUpdateInstallResult, "started"> | null
  >(null);
  const [elevating, setElevating] = useState(false);
  const info = props.info;
  const busy = props.state.downloading || props.state.installing || elevating;
  const progressIndeterminate = props.state.installing || props.state.downloadProgress <= 0;
  const downloadProgress = Math.round(props.state.downloadProgress * 100);

  const close = () => {
    if (!busy) {
      closeUpdateDialog();
    }
  };

  const install = () => {
    void host.updates
      .downloadAndInstall()
      .then((result) => {
        if (result !== "started") {
          setElevationReason(result);
        }
      })
      .catch(showError);
  };

  const installWithElevation = () => {
    setElevating(true);
    void host.updates
      .installWithElevation()
      .then((launched) => {
        if (!launched) {
          setElevationReason(null);
        }
      })
      .catch(showError)
      .finally(() => setElevating(false));
  };

  if (elevationReason !== null) {
    const signerMismatch = elevationReason === "signer-mismatch";
    return (
      <Dialog
        onClose={() => {
          if (!elevating) {
            setElevationReason(null);
          }
        }}
      >
        <h3>{t(signerMismatch ? "Signature mismatch" : "Not a newer version")}</h3>
        <p className="dialog-message">
          {t(
            signerMismatch
              ? "The new version is signed differently from the current application. Do you want to continue installing?"
              : "This installer is not newer than the current application. Do you want to continue installing?",
          )}
        </p>
        <div className="row-actions dialog-actions">
          <Button disabled={elevating} onClick={() => setElevationReason(null)}>
            {t("Cancel")}
          </Button>
          <Button variant="danger" disabled={elevating} onClick={installWithElevation}>
            {t("Continue")}
          </Button>
        </div>
      </Dialog>
    );
  }

  return (
    <Dialog onClose={close} className={styles.updateDialog}>
      <h3>{t("Check Update")}</h3>
      <p className="dialog-message">
        {t("New version available: {version}", { version: info.versionName })}
        {info.fileSize > 0 && ` (${formatBytes(info.fileSize)})`}
      </p>
      {info.releaseNotes !== "" && (
        <div className={styles.releaseNotes}>
          <GitHubMarkdown text={info.releaseNotes} />
        </div>
      )}
      {busy && (
        <div className={styles.progressRow}>
          <div
            className={styles.progressTrack}
            role="progressbar"
            aria-label={t(props.state.installing ? "Installing..." : "Downloading...")}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={progressIndeterminate ? undefined : downloadProgress}
          >
            <div
              className={progressIndeterminate ? styles.progressIndeterminate : styles.progressFill}
              style={progressIndeterminate ? undefined : { width: `${downloadProgress}%` }}
            />
          </div>
        </div>
      )}
      <div className="row-actions dialog-actions">
        <Button href={info.releaseURL} target="_blank" rel="noreferrer">
          {t("View Release")}
        </Button>
        <span className={styles.actionsSpacer} />
        <Button disabled={busy} onClick={close}>
          {t("Cancel")}
        </Button>
        <Button variant="primary" disabled={busy} onClick={install}>
          {t("Update")}
        </Button>
      </div>
    </Dialog>
  );
}
