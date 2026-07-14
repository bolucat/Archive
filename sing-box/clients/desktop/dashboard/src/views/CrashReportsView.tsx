import { useCallback, useEffect, useState } from "react";

import { formatDateTime } from "../api/format";
import { navigate } from "../app/context";
import {
  useLocalDesktopHost,
  type DesktopCrashReport,
  type DesktopCrashReportExportOptions,
  type DesktopCrashReportFile,
  type DesktopHost,
} from "../app/desktop";
import { showError } from "../app/errorStore";
import { useI18n } from "../app/i18n";
import { Icon } from "../components/Icon";
import { PageHeader } from "../components/PageHeader";
import {
  Button,
  Card,
  DataLine,
  Dialog,
  EmptyState,
  IconButton,
  MenuItem,
  MenuLabel,
  OthersMenu,
  Spinner,
  Toggle,
} from "../components/ui";
import { cx } from "../lib/cx";
import styles from "./CrashReportsView.module.css";
import { ToolsPageHeader } from "./ToolsView";
import { crashReportFileDisplayName, crashReportTitle } from "./reportFormat";

function reportPath(name: string, crashedAt: number | null, file?: string): string {
  const path = `tools/crash-reports/${encodeURIComponent(name)}${file === undefined ? "" : `/${encodeURIComponent(file)}`}`;
  return crashedAt !== null ? `${path}?at=${crashedAt}` : path;
}

export function CrashReportListView() {
  const host = useLocalDesktopHost();
  if (host === null) {
    return null;
  }
  return <CrashReportListContent host={host} />;
}

function CrashReportListContent({ host }: { host: DesktopHost }) {
  const { t, language } = useI18n();
  const [reports, setReports] = useState<DesktopCrashReport[] | null>(null);

  const reload = useCallback(() => {
    host.reports.crash
      .list()
      .then(setReports)
      .catch((error) => {
        showError(error);
        setReports([]);
      });
  }, [host]);

  useEffect(() => {
    reload();
  }, [reload]);

  const deleteAll = () => {
    host.reports.crash
      .removeAll()
      .then(reload)
      .catch(showError);
  };

  return (
    <div className="page">
      <ToolsPageHeader
        title={t("Crash Report")}
        actions={
          <>
            {import.meta.env.DEV && <CrashTriggerMenu host={host} />}
            {reports !== null && reports.length > 0 && (
              <OthersMenu>
                <MenuItem danger icon="delete" onSelect={deleteAll}>
                  {t("Delete All")}
                </MenuItem>
              </OthersMenu>
            )}
          </>
        }
      />
      <div className="settings-stack">
        {reports === null ? (
          <Spinner />
        ) : (
          <div>
            <div className="list-section-title">{t("Reports")}</div>
            <div className="nav-list">
              {reports.length === 0 ? (
                <div className={styles.emptyRow}>{t("Empty")}</div>
              ) : (
                reports.map((report) => (
                  <button
                    type="button"
                    key={report.name}
                    className={cx("nav-row", styles.reportRow)}
                    onClick={() => navigate(reportPath(report.name, report.crashedAt))}
                  >
                    <span className={cx(styles.reportDot, !report.isRead && styles.unread)} />
                    <span className={styles.reportText}>
                      <span className={cx(styles.reportDate, !report.isRead && styles.unread)}>
                        {formatDateTime(report.crashedAt, language)}
                      </span>
                      <span className={styles.reportOrigin}>
                        <Icon name="computer" size={12} />
                        {t("Local")}
                      </span>
                    </span>
                    <Icon name="keyboard_arrow_right" size={14} />
                  </button>
                ))
              )}
            </div>
            <div className={cx("hint", styles.sectionFooter)}>
              {t("You will receive a report when a crash occurs.")}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function CrashTriggerMenu({ host }: { host: DesktopHost }) {
  const triggerApp = (type: "js" | "native") => {
    host.reports.triggerAppCrash(type).catch(showError);
  };
  const triggerDaemon = (type: "go" | "native") => {
    host.reports.triggerDebugCrash(type).catch(showError);
  };
  return (
    <OthersMenu icon="bug_report" title="Crash Trigger">
      <MenuLabel>Application</MenuLabel>
      <MenuItem onSelect={() => triggerApp("js")}>JS Crash</MenuItem>
      <MenuItem onSelect={() => triggerApp("native")}>Native Crash</MenuItem>
      <MenuLabel>Daemon</MenuLabel>
      <MenuItem onSelect={() => triggerDaemon("go")}>Go Crash</MenuItem>
      <MenuItem onSelect={() => triggerDaemon("native")}>Native Crash</MenuItem>
    </OthersMenu>
  );
}

export function CrashReportDetailView(props: { name: string; crashedAt: number | null }) {
  const host = useLocalDesktopHost();
  if (host === null) {
    return null;
  }
  return <CrashReportDetailContent host={host} name={props.name} crashedAt={props.crashedAt} />;
}

function CrashReportDetailContent({
  host,
  name,
  crashedAt,
}: {
  host: DesktopHost;
  name: string;
  crashedAt: number | null;
}) {
  const { t, language } = useI18n();
  const [files, setFiles] = useState<DesktopCrashReportFile[] | null>(null);
  const [sharing, setSharing] = useState(false);

  useEffect(() => {
    let stale = false;
    host.reports.crash
      .read(name)
      .then((value) => {
        if (!stale) {
          setFiles(() => value);
        }
        host.reports.crash.markRead(name).catch(showError);
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
    host.reports.crash
      .remove(name)
      .then(() => navigate("tools/crash-reports"))
      .catch(showError);
  };

  return (
    <div className="page">
      <PageHeader
        title={crashReportTitle(name, crashedAt, language)}
        back={{ label: t("Crash Report"), onClick: () => navigate("tools/crash-reports") }}
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
                file.isBinary ? (
                  <div key={file.name} className={cx("nav-row", styles.staticRow)}>
                    <span>{crashReportFileDisplayName(file.name, t)}</span>
                  </div>
                ) : (
                  <button
                    type="button"
                    key={file.name}
                    className="nav-row"
                    onClick={() => navigate(reportPath(name, crashedAt, file.name))}
                  >
                    <span>{crashReportFileDisplayName(file.name, t)}</span>
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
          hasConfiguration={files.some((file) => file.name === "configuration.json")}
          hasLog={false}
          onSave={(options) => {
            host.reports.crash.exportFile(name, options).catch(showError);
          }}
          onClose={() => setSharing(false)}
        />
      )}
    </div>
  );
}

export function ReportShareDialog(props: {
  hasConfiguration: boolean;
  hasLog: boolean;
  onSave: (options: DesktopCrashReportExportOptions) => void;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [withConfiguration, setWithConfiguration] = useState(false);
  const [withLog, setWithLog] = useState(true);
  const [encrypt, setEncrypt] = useState(false);

  const save = () => {
    props.onClose();
    props.onSave({ withConfiguration, withLog: props.hasLog ? withLog : true, encrypt });
  };

  return (
    <Dialog onClose={props.onClose}>
      <h3>{t("Share")}</h3>
      {(props.hasConfiguration || props.hasLog) && (
        <div className={styles.shareSection}>
          {props.hasLog && <Toggle label={t("With Log")} value={withLog} onChange={setWithLog} />}
          {props.hasConfiguration && (
            <Toggle
              label={t("With Configuration")}
              value={withConfiguration}
              onChange={setWithConfiguration}
            />
          )}
          <div className="hint">
            {props.hasLog
              ? t("Logs and configuration files may contain private content and should not be made public.")
              : t("Configuration files may contain private content and should not be made public.")}
          </div>
        </div>
      )}
      <div className={styles.shareSection}>
        <Toggle label={t("Encrypt with age for Project S")} value={encrypt} onChange={setEncrypt} />
        <div className="hint">
          <MarkdownLinkText
            text={t(
              "[age](https://github.com/filosottile/age) is a modern and secure asymmetric encryption tool. When enabled, the zip file is encrypted with this project's public key so it can be posted publicly, e.g. in GitHub issues.",
            )}
          />
        </div>
      </div>
      <div className="row-actions dialog-actions">
        <Button onClick={props.onClose}>{t("Cancel")}</Button>
        <Button variant="primary" onClick={save}>
          <Icon name="save" size={13} />
          {t("Save")}
        </Button>
      </div>
    </Dialog>
  );
}

function MarkdownLinkText(props: { text: string }) {
  const match = /\[([^\]]+)\]\(([^)]+)\)/.exec(props.text);
  if (match === null) {
    return <>{props.text}</>;
  }
  return (
    <>
      {props.text.slice(0, match.index)}
      <a href={match[2]} target="_blank" rel="noreferrer">
        {match[1]}
      </a>
      {props.text.slice(match.index + match[0].length)}
    </>
  );
}

export function CrashReportFileView(props: { name: string; file: string; crashedAt: number | null }) {
  const host = useLocalDesktopHost();
  if (host === null) {
    return null;
  }
  return (
    <CrashReportFileContent
      host={host}
      name={props.name}
      file={props.file}
      crashedAt={props.crashedAt}
    />
  );
}

function CrashReportFileContent({
  host,
  name,
  file,
  crashedAt,
}: {
  host: DesktopHost;
  name: string;
  file: string;
  crashedAt: number | null;
}) {
  const { t, language } = useI18n();
  const [content, setContent] = useState<string | null>(null);

  useEffect(() => {
    let stale = false;
    host.reports.crash
      .read(name)
      .then((files) => {
        if (!stale) {
          setContent(files.find((entry) => entry.name === file)?.content ?? "");
        }
      })
      .catch((error) => {
        showError(error);
        if (!stale) {
          setContent("");
        }
      });
    return () => {
      stale = true;
    };
  }, [host, name, file]);

  return (
    <div className="page">
      <PageHeader
        title={crashReportFileDisplayName(file, t)}
        back={{
          label: crashReportTitle(name, crashedAt, language),
          onClick: () => navigate(reportPath(name, crashedAt)),
        }}
      />
      <div className="settings-stack">
        {content === null ? (
          <Spinner />
        ) : content === "" ? (
          <EmptyState>{t("Empty")}</EmptyState>
        ) : file === "metadata.json" ? (
          <MetadataCard content={content} />
        ) : (
          <pre className={styles.fileContent}>{content}</pre>
        )}
      </div>
    </div>
  );
}

export function MetadataCard(props: { content: string }) {
  const { t } = useI18n();
  let entries: [string, string][] = [];
  try {
    const parsed: unknown = JSON.parse(props.content);
    if (parsed !== null && typeof parsed === "object" && !Array.isArray(parsed)) {
      entries = Object.entries(parsed).reduce<[string, string][]>((result, [key, value]) => {
        const text = value === null ? "" : String(value);
        if (text !== "") {
          result.push([key, text]);
        }
        return result;
      }, []);
    }
  } catch {
    return <pre className={styles.fileContent}>{props.content}</pre>;
  }
  if (entries.length === 0) {
    return <EmptyState>{t("Empty")}</EmptyState>;
  }
  return (
    <Card>
      {entries.map(([key, value]) => (
        <DataLine key={key} label={key} value={value} />
      ))}
    </Card>
  );
}
