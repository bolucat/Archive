import { useEffect, useState } from "react";

import { navigate } from "../app/context";
import type { DesktopHost, DesktopOOMReportFile } from "../app/desktop";
import { showError } from "../app/errorStore";
import { useI18n } from "../app/i18n";
import { Icon } from "../components/Icon";
import { PageHeader } from "../components/PageHeader";
import { EmptyState, IconButton, Spinner } from "../components/ui";
import { cx } from "../lib/cx";
import { ReportShareDialog } from "./CrashReportsView";
import { shareError, shareFile } from "../lib/sharing";
import styles from "./OOMReportsView.module.css";
import {
  powerReportFileDisplayName,
  powerReportFilePath,
  powerReportTitle,
} from "./reportFormat";

export function PowerReportDetailContent({
  host,
  name,
  recordedAt,
}: {
  host: DesktopHost;
  name: string;
  recordedAt: number | null;
}) {
  const { t, language } = useI18n();
  const [files, setFiles] = useState<DesktopOOMReportFile[] | null>(null);
  const [sharing, setSharing] = useState(false);

  useEffect(() => {
    let stale = false;
    host.reports.power
      .read(name)
      .then((value) => {
        if (!stale) {
          setFiles(() => value);
        }
        return host.reports.power.markRead(name);
      })
      .catch((error) => {
        showError(error);
        if (!stale) {
          setFiles([]);
        }
      });
    return () => {
      stale = true;
    };
  }, [host, name]);

  const remove = () => {
    host.reports.power
      .remove(name)
      .then(() => navigate("tools/power-reports"))
      .catch(showError);
  };

  return (
    <div className="page">
      <PageHeader
        title={powerReportTitle(name, recordedAt, language)}
        back={{ label: t("Power Report"), onClick: () => navigate("tools/power-reports") }}
        actions={
          files !== null && files.length > 0 ? (
            <>
              <IconButton title={t("Share")} onClick={() => setSharing(true)}>
                <Icon name="share" />
              </IconButton>
              <IconButton danger title={t("Delete")} onClick={remove}>
                <Icon name="delete" />
              </IconButton>
            </>
          ) : undefined
        }
      />
      <div className="settings-stack">
        {files === null ? (
          <Spinner />
        ) : files.length === 0 ? (
          <EmptyState>{t("Empty")}</EmptyState>
        ) : (
          <div>
            <div className="list-section-title">{t("Files")}</div>
            <div className="nav-list">
              {files.map((file) =>
                file.isProfile ? (
                  <div key={file.name} className={cx("nav-row", "static", styles.staticRow)}>
                    <span>{file.name}</span>
                  </div>
                ) : (
                  <button
                    type="button"
                    key={file.name}
                    className="nav-row"
                    onClick={() =>
                      navigate(powerReportFilePath(name, file.name, recordedAt))
                    }
                  >
                    <span>{powerReportFileDisplayName(file.name, t)}</span>
                    <Icon name="keyboard_arrow_right" size={14} />
                  </button>
                ),
              )}
            </div>
          </div>
        )}
      </div>
      {sharing && files !== null && (
        <ReportShareDialog
          host={host}
          hasConfiguration={files.some((file) => file.name === "configuration.json")}
          hasLog={files.some((file) => file.name === "go.log")}
          onSave={(options) => {
            host.reports.power.exportFile(name, options).catch(showError);
          }}
          onShare={(options) => {
            host.reports.power
              .createArchive(name, options)
              .then((archive) =>
                shareFile(host, archive.fileName, archive.data, archive.mediaType),
              )
              .catch((error) => {
                const reportableError = shareError(error);
                if (reportableError !== null) {
                  showError(reportableError);
                }
              });
          }}
          onClose={() => setSharing(false)}
        />
      )}
    </div>
  );
}
