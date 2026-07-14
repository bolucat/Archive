import { useCallback, useEffect, useState } from "react";

import { formatDateTime } from "../api/format";
import { useStream } from "../api/stream";
import { navigate, useApi } from "../app/context";
import type {
  DesktopHost,
  DesktopOOMReport,
  DesktopSettingsState,
} from "../app/desktop";
import { showError } from "../app/errorStore";
import { useI18n } from "../app/i18n";
import { Icon } from "../components/Icon";
import { MenuItem, OthersMenu, Select, Spinner } from "../components/ui";
import { ServiceStatus_Type } from "../gen/daemon/started_service_pb";
import { cx } from "../lib/cx";
import crashStyles from "./CrashReportsView.module.css";
import styles from "./OOMReportsView.module.css";
import { oomReportPath } from "./reportFormat";
import { ToolsPageHeader } from "./ToolsView";

const MEMORY_LIMIT_OPTIONS_MB = [50, 100, 200, 300, 500, 750, 1024];

export function OOMReportListContent({ host }: { host: DesktopHost }) {
  const api = useApi();
  const { t, language } = useI18n();
  const serviceStatus = useStream(api.serviceStatus);
  const started = serviceStatus.data.status?.status === ServiceStatus_Type.STARTED;
  const [reports, setReports] = useState<DesktopOOMReport[] | null>(null);
  const [settings, setSettings] = useState<DesktopSettingsState | null>(null);

  const reload = useCallback(() => {
    host.reports.oom
      .list()
      .then(setReports)
      .catch((error) => {
        showError(error);
        setReports([]);
      });
  }, [host]);

  useEffect(() => {
    reload();
    host.settings.get().then(setSettings).catch(showError);
  }, [host, reload]);

  const deleteAll = () => {
    host.reports.oom.removeAll().then(reload).catch(showError);
  };

  const fetchReport = () => {
    if (!started) {
      showError(new Error(t("Service not started")));
      return;
    }
    host.reports
      .triggerOOMReport()
      .then(() => {
        setTimeout(reload, 1000);
      })
      .catch(showError);
  };

  const applySettings = (update: Partial<DesktopSettingsState>, save: () => Promise<void>) => {
    if (settings === null) {
      return;
    }
    setSettings({ ...settings, ...update });
    save()
      .then(() => (started ? host.service.start() : undefined))
      .catch(showError);
  };

  return (
    <div className="page">
      <ToolsPageHeader
        title={t("OOM Report")}
        actions={
          <OthersMenu>
            <MenuItem icon="memory" onSelect={fetchReport}>
              {t("Fetch Memory Report")}
            </MenuItem>
            {reports !== null && reports.length > 0 && (
              <MenuItem danger icon="delete" onSelect={deleteAll}>
                {t("Delete All")}
              </MenuItem>
            )}
          </OthersMenu>
        }
      />
      <div className="settings-stack">
        {reports === null || settings === null ? (
          <Spinner />
        ) : (
          <>
            <div>
              <div className="list-section-title">{t("Reports")}</div>
              <div className="nav-list">
                {reports.length === 0 ? (
                  <div className={crashStyles.emptyRow}>{t("Empty")}</div>
                ) : (
                  reports.map((report) => (
                    <button
                      type="button"
                      key={report.name}
                      className={cx("nav-row", crashStyles.reportRow)}
                      onClick={() => navigate(oomReportPath(report.name, report.recordedAt))}
                    >
                      <span
                        className={cx(crashStyles.reportDot, !report.isRead && crashStyles.unread)}
                      />
                      <span className={crashStyles.reportText}>
                        <span
                          className={cx(crashStyles.reportDate, !report.isRead && crashStyles.unread)}
                        >
                          {formatDateTime(report.recordedAt, language)}
                        </span>
                        <span className={crashStyles.reportOrigin}>
                          <Icon name="computer" size={12} />
                          {t("Local")}
                        </span>
                      </span>
                      <Icon name="keyboard_arrow_right" size={14} />
                    </button>
                  ))
                )}
              </div>
              <div className={cx("hint", crashStyles.sectionFooter)}>
                {t(
                  "When memory limit is enabled, you will receive a report if the service memory exceeds the limit. You can also manually trigger report collection.",
                )}
              </div>
            </div>
            <div>
              <div className="list-section-title">{t("Settings")}</div>
              <div className={styles.settingsList}>
                <div className="settings-row">
                  <div className={styles.rowText}>
                    <span className="settings-row-label">{t("Enable Memory Limit")}</span>
                    <span className="hint">
                      {t(
                        "Provide a soft memory limit for the service. The service will perform multiple processes to try to stay within this memory limit.",
                      )}
                    </span>
                  </div>
                  <button
                    type="button"
                    className={settings.oomKillerEnabled ? "switch on" : "switch"}
                    role="switch"
                    aria-checked={settings.oomKillerEnabled}
                    aria-label={t("Enable Memory Limit")}
                    onClick={() => {
                      const value = !settings.oomKillerEnabled;
                      applySettings({ oomKillerEnabled: value }, () =>
                        host.settings.setOOMKillerEnabled(value),
                      );
                    }}
                  />
                </div>
                {settings.oomKillerEnabled && (
                  <>
                    <div className="settings-row">
                      <span className="settings-row-label">{t("Memory Limit")}</span>
                      <Select<number>
                        inline
                        options={MEMORY_LIMIT_OPTIONS_MB.map((value) => ({
                          value,
                          label: `${value} MB`,
                        }))}
                        value={
                          MEMORY_LIMIT_OPTIONS_MB.includes(settings.oomMemoryLimitMB)
                            ? settings.oomMemoryLimitMB
                            : MEMORY_LIMIT_OPTIONS_MB[0]
                        }
                        onChange={(value) => {
                          applySettings({ oomMemoryLimitMB: value }, () =>
                            host.settings.setOOMMemoryLimitMB(value),
                          );
                        }}
                      />
                    </div>
                    <div className="settings-row">
                      <div className={styles.rowText}>
                        <span className="settings-row-label">{t("Kill Connections")}</span>
                        <span className="hint">
                          {t(
                            "Kill all connections to free memory when the service memory exceeds the limit.",
                          )}
                        </span>
                      </div>
                      <button
                        type="button"
                        className={settings.oomKillerKillConnections ? "switch on" : "switch"}
                        role="switch"
                        aria-checked={settings.oomKillerKillConnections}
                        aria-label={t("Kill Connections")}
                        onClick={() => {
                          const value = !settings.oomKillerKillConnections;
                          applySettings({ oomKillerKillConnections: value }, () =>
                            host.settings.setOOMKillerKillConnections(value),
                          );
                        }}
                      />
                    </div>
                  </>
                )}
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}
