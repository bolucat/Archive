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
import { MenuItem, OthersMenu, Spinner } from "../components/ui";
import { ServiceStatus_Type } from "../gen/daemon/started_service_pb";
import { cx } from "../lib/cx";
import crashStyles from "./CrashReportsView.module.css";
import styles from "./OOMReportsView.module.css";
import { powerReportPath } from "./reportFormat";
import { ToolsPageHeader } from "./ToolsView";

export function PowerReportListContent({ host }: { host: DesktopHost }) {
  const api = useApi();
  const { t, language } = useI18n();
  const serviceStatus = useStream(api.serviceStatus);
  const started = serviceStatus.data.status?.status === ServiceStatus_Type.STARTED;
  const [reports, setReports] = useState<DesktopOOMReport[] | null>(null);
  const [settings, setSettings] = useState<DesktopSettingsState | null>(null);

  const reload = useCallback(() => {
    host.reports.power
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
    host.reports.power.removeAll().then(reload).catch(showError);
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
        title={t("Power Report")}
        actions={
          reports !== null && reports.length > 0 ? (
            <OthersMenu>
              <MenuItem danger icon="delete" onSelect={deleteAll}>
                {t("Delete All")}
              </MenuItem>
            </OthersMenu>
          ) : undefined
        }
      />
      <div className="settings-stack">
        {reports === null || settings === null ? (
          <Spinner />
        ) : (
          <>
            <div>
              <div className="list-section-title">{t("Settings")}</div>
              <div className={styles.settingsList}>
                <div className="settings-row">
                  <div className={styles.rowText}>
                    <span className="settings-row-label">{t("Enable Power Report")}</span>
                    <span className="hint">
                      {t("A report is saved for each service run")}
                    </span>
                  </div>
                  <button
                    type="button"
                    className={settings.powerReportEnabled ? "switch on" : "switch"}
                    role="switch"
                    aria-checked={settings.powerReportEnabled}
                    aria-label={t("Enable Power Report")}
                    onClick={() => {
                      const value = !settings.powerReportEnabled;
                      applySettings({ powerReportEnabled: value }, () =>
                        host.settings.setPowerReportEnabled(value),
                      );
                    }}
                  />
                </div>
              </div>
            </div>
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
                      onClick={() => navigate(powerReportPath(report.name, report.recordedAt))}
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
            </div>
          </>
        )}
      </div>
    </div>
  );
}
