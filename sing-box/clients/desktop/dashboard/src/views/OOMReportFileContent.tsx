import { useEffect, useState } from "react";

import { navigate } from "../app/context";
import type { DesktopHost } from "../app/desktop";
import { showError } from "../app/errorStore";
import { useI18n } from "../app/i18n";
import { PageHeader } from "../components/PageHeader";
import { EmptyState, Spinner } from "../components/ui";
import { MetadataCard } from "./CrashReportsView";
import crashStyles from "./CrashReportsView.module.css";
import {
  oomReportFileDisplayName,
  oomReportPath,
  oomReportTitle,
} from "./reportFormat";

export function OOMReportFileContent({
  host,
  name,
  file,
  recordedAt,
}: {
  host: DesktopHost;
  name: string;
  file: string;
  recordedAt: number | null;
}) {
  const { t, language } = useI18n();
  const [content, setContent] = useState<string | null>(null);

  useEffect(() => {
    let stale = false;
    host.reports.oom
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
        title={oomReportFileDisplayName(file, t)}
        back={{
          label: oomReportTitle(name, recordedAt, language),
          onClick: () => navigate(oomReportPath(name, recordedAt)),
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
          <pre className={crashStyles.fileContent}>{content}</pre>
        )}
      </div>
    </div>
  );
}
