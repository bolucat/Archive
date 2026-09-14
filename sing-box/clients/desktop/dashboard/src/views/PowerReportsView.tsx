import { useLocalDesktopHost } from "../app/desktop";
import { PowerReportDetailContent } from "./PowerReportDetailContent";
import { PowerReportFileContent } from "./PowerReportFileContent";
import { PowerReportListContent } from "./PowerReportListContent";

export function PowerReportListView() {
  const host = useLocalDesktopHost();
  return host === null ? null : <PowerReportListContent host={host} />;
}

export function PowerReportDetailView(props: { name: string; recordedAt: number | null }) {
  const host = useLocalDesktopHost();
  return host === null ? null : (
    <PowerReportDetailContent host={host} name={props.name} recordedAt={props.recordedAt} />
  );
}

export function PowerReportFileView(props: { name: string; file: string; recordedAt: number | null }) {
  const host = useLocalDesktopHost();
  return host === null ? null : (
    <PowerReportFileContent
      host={host}
      name={props.name}
      file={props.file}
      recordedAt={props.recordedAt}
    />
  );
}
