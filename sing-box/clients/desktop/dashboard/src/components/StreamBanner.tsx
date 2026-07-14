import { isTerminalCode, type StreamSnapshot } from "../api/stream";
import { useStreamOutage } from "../app/hooks";
import { useI18n } from "../app/i18n";
import { Icon, type IconName } from "./Icon";
import { EmptyState } from "./ui";

export function StreamErrorBanner(props: { error: string | null }) {
  if (props.error === null) {
    return null;
  }
  return (
    <div className="banner error">
      <Icon name="warning_amber" />
      <div>{props.error}</div>
    </div>
  );
}

export function StreamBanner(props: { snapshot: StreamSnapshot<unknown> }) {
  const outage = useStreamOutage(props.snapshot, isTerminalCode(props.snapshot.errorCode));
  return <StreamErrorBanner error={outage} />;
}

export function StreamStates(props: {
  snapshot: StreamSnapshot<unknown>;
  loaded: boolean;
  empty: boolean;
  emptyIcon?: IconName;
  emptyMessage: string;
}) {
  const { t } = useI18n();
  const outage = useStreamOutage(props.snapshot, isTerminalCode(props.snapshot.errorCode));
  return (
    <>
      <StreamErrorBanner error={outage} />
      {!props.loaded && outage === null && <EmptyState>{t("Loading...")}</EmptyState>}
      {props.loaded && props.empty && (
        <EmptyState icon={props.emptyIcon}>{props.emptyMessage}</EmptyState>
      )}
    </>
  );
}
