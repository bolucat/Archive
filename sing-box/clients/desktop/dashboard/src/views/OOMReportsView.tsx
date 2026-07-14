import { useLocalDesktopHost } from "../app/desktop";
import { OOMReportDetailContent } from "./OOMReportDetailContent";
import { OOMReportFileContent } from "./OOMReportFileContent";
import { OOMReportListContent } from "./OOMReportListContent";

export function OOMReportListView() {
  const host = useLocalDesktopHost();
  return host === null ? null : <OOMReportListContent host={host} />;
}

export function OOMReportDetailView(props: { name: string; recordedAt: number | null }) {
  const host = useLocalDesktopHost();
  return host === null ? null : (
    <OOMReportDetailContent host={host} name={props.name} recordedAt={props.recordedAt} />
  );
}

export function OOMReportFileView(props: { name: string; file: string; recordedAt: number | null }) {
  const host = useLocalDesktopHost();
  return host === null ? null : (
    <OOMReportFileContent
      host={host}
      name={props.name}
      file={props.file}
      recordedAt={props.recordedAt}
    />
  );
}
