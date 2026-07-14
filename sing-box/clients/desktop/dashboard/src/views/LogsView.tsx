import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";

import type { LogEntry } from "../api/daemon";
import { pad2 } from "../api/format";
import { isTerminalCode, useStream } from "../api/stream";
import { useApi } from "../app/context";
import { showError } from "../app/errorStore";
import { useStreamOutage } from "../app/hooks";
import { useI18n } from "../app/i18n";
import { Icon } from "../components/Icon";
import { PageHeader } from "../components/PageHeader";
import { StreamErrorBanner } from "../components/StreamBanner";
import { EmptyState, IconButton, MenuItem, OthersMenu, SearchInput, Spinner, SubMenu } from "../components/ui";
import { LogLevel, ServiceStatus_Type } from "../gen/daemon/started_service_pb";
import { ansiColorCss, parseAnsi, parseCssColor, stripAnsi, type Rgb } from "../lib/ansi";
import styles from "./LogsView.module.css";

const MAX_VISIBLE_LOGS = 1000;

const LEVEL_OPTIONS: { value: LogLevel; label: string }[] = [
  { value: LogLevel.ERROR, label: "Error" },
  { value: LogLevel.WARN, label: "Warn" },
  { value: LogLevel.INFO, label: "Info" },
  { value: LogLevel.DEBUG, label: "Debug" },
  { value: LogLevel.TRACE, label: "Trace" },
];

function useLogBackground(): Rgb {
  const [background, setBackground] = useState<Rgb>(() => resolveBackground());
  useEffect(() => {
    const observer = new MutationObserver(() => setBackground(resolveBackground()));
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
    return () => observer.disconnect();
  }, []);
  return background;
}

function resolveBackground(): Rgb {
  const value = getComputedStyle(document.documentElement).getPropertyValue("--card");
  return parseCssColor(value) ?? [255, 255, 255];
}

function logFileName(): string {
  const now = new Date();
  const timestamp = `${now.getFullYear()}-${pad2(now.getMonth() + 1)}-${pad2(now.getDate())}-${pad2(now.getHours())}.${pad2(now.getMinutes())}.${pad2(now.getSeconds())}`;
  return `logs-${timestamp}.txt`;
}

function onShareError(error: unknown): void {
  if (error instanceof DOMException && error.name === "AbortError") {
    return;
  }
  showError(error);
}

export function LogsView() {
  const api = useApi();
  const { t } = useI18n();
  const logs = useStream(api.logs);
  const outage = useStreamOutage(logs, isTerminalCode(logs.errorCode));
  const serviceStatus = useStream(api.serviceStatus);
  const [level, setLevel] = useState<LogLevel | null>(null);
  const [paused, setPaused] = useState(false);
  const [frozen, setFrozen] = useState<LogEntry[]>([]);
  const [search, setSearch] = useState("");
  const viewRef = useRef<HTMLDivElement>(null);
  const background = useLogBackground();

  const started = serviceStatus.data.status?.status === ServiceStatus_Type.STARTED;
  const effectiveLevel = level ?? logs.data.defaultLevel ?? LogLevel.INFO;

  const togglePause = () => {
    if (paused) {
      setPaused(false);
    } else {
      setFrozen(logs.data.entries);
      setPaused(true);
    }
  };

  const sourceEntries = paused ? frozen : logs.data.entries;

  const filtered = useMemo(() => {
    let entries = sourceEntries.filter((entry) => entry.level <= effectiveLevel);
    const query = search.trim().toLowerCase();
    if (query !== "") {
      entries = entries.filter((entry) => stripAnsi(entry.message).toLowerCase().includes(query));
    }
    return entries;
  }, [sourceEntries, effectiveLevel, search]);

  const visible = useMemo(
    () =>
      filtered.length > MAX_VISIBLE_LOGS
        ? filtered.slice(filtered.length - MAX_VISIBLE_LOGS)
        : filtered,
    [filtered],
  );

  const logsText = () => filtered.map((entry) => stripAnsi(entry.message)).join("\n");
  const canShare = typeof navigator.share === "function";

  const saveLogs = () => {
    const url = URL.createObjectURL(new Blob([logsText()], { type: "text/plain" }));
    const anchor = document.createElement("a");
    anchor.href = url;
    anchor.download = logFileName();
    anchor.click();
    URL.revokeObjectURL(url);
  };

  const shareLogs = () => {
    const text = logsText();
    const file = new File([text], logFileName(), { type: "text/plain" });
    if (navigator.canShare?.({ files: [file] })) {
      void navigator.share({ files: [file] }).catch(onShareError);
    } else {
      void navigator.share({ text }).catch(onShareError);
    }
  };

  useLayoutEffect(() => {
    if (paused) {
      return;
    }
    const view = viewRef.current;
    if (view) {
      view.scrollTop = view.scrollHeight;
    }
  }, [visible, paused]);

  let body: ReactNode;
  if (sourceEntries.length > 0 && outage === null) {
    body = (
      <div className={styles.logView} ref={viewRef}>
        {visible.map((entry) => (
          <LogLine
            key={entry.id}
            message={entry.message}
            highlight={search.trim()}
            background={background}
          />
        ))}
      </div>
    );
  } else if (outage !== null) {
    body = null;
  } else if (!started && serviceStatus.phase === "active") {
    body = <EmptyState icon="text_snippet">{t("Service not started")}</EmptyState>;
  } else if (logs.phase === "connecting") {
    body = (
      <EmptyState>
        <Spinner />
      </EmptyState>
    );
  } else {
    body = <EmptyState icon="text_snippet">{t("Empty logs")}</EmptyState>;
  }

  return (
    <div className="page page-full">
      <PageHeader
        title={t("Logs")}
        actions={
          <>
            <IconButton
              active={paused}
              title={paused ? t("Resume scrolling") : t("Pause scrolling")}
              onClick={togglePause}
            >
              <Icon name={paused ? "play_arrow" : "pause"} />
            </IconButton>
            <OthersMenu>
              <SubMenu label={t("Log Level")} icon="filter_list">
                <MenuItem checked={level === null} onSelect={() => setLevel(null)}>
                  {t("Default")}
                </MenuItem>
                {LEVEL_OPTIONS.map((option) => (
                  <MenuItem
                    key={option.value}
                    checked={level === option.value}
                    onSelect={() => setLevel(option.value)}
                  >
                    {option.label}
                  </MenuItem>
                ))}
              </SubMenu>
              <SubMenu label={t("Save")} icon="save">
                <MenuItem
                  icon="content_copy"
                  onSelect={() => void navigator.clipboard.writeText(logsText()).catch(showError)}
                >
                  {t("To Clipboard")}
                </MenuItem>
                <MenuItem icon="save" onSelect={saveLogs}>
                  {t("To File")}
                </MenuItem>
                {canShare && (
                  <MenuItem icon="share" onSelect={shareLogs}>
                    {t("Share")}
                  </MenuItem>
                )}
              </SubMenu>
              <div className="menu-divider" />
              <MenuItem
                danger
                icon="delete"
                onSelect={() => {
                  void api.clearLogs().catch(showError);
                }}
              >
                {t("Clear Logs")}
              </MenuItem>
            </OthersMenu>
          </>
        }
      />
      <div className="field">
        <SearchInput value={search} onChange={setSearch} />
      </div>
      <StreamErrorBanner error={outage} />
      {body}
    </div>
  );
}

const LogLine = memo(function LogLine(props: {
  message: string;
  highlight: string;
  background: Rgb;
}) {
  const segments = parseAnsi(props.message);
  const query = props.highlight.toLowerCase();

  const ranges: [number, number][] = [];
  if (query !== "") {
    const plain = segments.map((segment) => segment.text).join("");
    const lower = plain.toLowerCase();
    let index = lower.indexOf(query);
    while (index !== -1) {
      ranges.push([index, index + query.length]);
      index = lower.indexOf(query, index + query.length);
    }
  }

  const parts: ReactNode[] = [];
  let offset = 0;
  let key = 0;
  for (const segment of segments) {
    const start = offset;
    const end = offset + segment.text.length;
    offset = end;

    let style: CSSProperties | undefined;
    if (segment.style) {
      style = {};
      if (segment.style.color) {
        style.color = ansiColorCss(segment.style.color, props.background);
      }
      if (segment.style.bold) {
        style.fontWeight = 700;
      }
      if (segment.style.italic) {
        style.fontStyle = "italic";
      }
      if (segment.style.underline) {
        style.textDecoration = "underline";
      }
    }

    const overlapping = ranges.filter(([from, to]) => to > start && from < end);
    let content: ReactNode;
    if (overlapping.length === 0) {
      content = segment.text;
    } else {
      const pieces: ReactNode[] = [];
      let cursor = 0;
      for (const [from, to] of overlapping) {
        const localFrom = Math.max(0, from - start);
        const localTo = Math.min(segment.text.length, to - start);
        if (localFrom > cursor) {
          pieces.push(segment.text.slice(cursor, localFrom));
        }
        pieces.push(<mark key={key++}>{segment.text.slice(localFrom, localTo)}</mark>);
        cursor = localTo;
      }
      if (cursor < segment.text.length) {
        pieces.push(segment.text.slice(cursor));
      }
      content = pieces;
    }

    if (style) {
      parts.push(
        <span key={key++} style={style}>
          {content}
        </span>,
      );
    } else {
      parts.push(<span key={key++}>{content}</span>);
    }
  }

  return <span className={styles.logLine}>{parts}</span>;
});
