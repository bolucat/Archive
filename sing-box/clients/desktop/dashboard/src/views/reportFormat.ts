import { formatAbbreviatedDateTime } from "../api/format";
import type { Translate } from "../app/i18n";

export function crashReportTitle(
  name: string,
  crashedAt: number | null,
  language: string,
): string {
  return crashedAt !== null ? formatAbbreviatedDateTime(crashedAt, language) : name;
}

export function crashReportFileDisplayName(name: string, t: Translate): string {
  switch (name) {
    case "metadata.json":
      return t("Metadata");
    case "native.log":
      return t("Crash Report");
    case "go.log":
      return t("Go Crash Log");
    case "configuration.json":
      return t("Configuration");
    default:
      return name;
  }
}

export function oomReportTitle(
  name: string,
  recordedAt: number | null,
  language: string,
): string {
  return recordedAt !== null ? formatAbbreviatedDateTime(recordedAt, language) : name;
}

export function oomReportPath(name: string, recordedAt: number | null): string {
  const path = `tools/oom-reports/${encodeURIComponent(name)}`;
  return recordedAt !== null ? `${path}?at=${recordedAt}` : path;
}

export function oomReportFilePath(
  name: string,
  file: string,
  recordedAt: number | null,
): string {
  const path = `${oomReportPath(name, null)}/${encodeURIComponent(file)}`;
  return recordedAt !== null ? `${path}?at=${recordedAt}` : path;
}

export function oomReportFileDisplayName(name: string, t: Translate): string {
  switch (name) {
    case "metadata.json":
      return t("Metadata");
    case "configuration.json":
      return t("Configuration");
    case "go.log":
      return t("Log");
    default:
      return name;
  }
}
