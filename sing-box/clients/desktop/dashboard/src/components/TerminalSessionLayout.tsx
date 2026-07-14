import type { CSSProperties, ReactNode, RefObject } from "react";

import type { ModKey, Modifiers, TerminalKey } from "../lib/terminalKeys";
import { cx } from "../lib/cx";
import { Spinner } from "./ui";
import { TerminalSymbolBar } from "./TerminalSymbolBar";
import styles from "../views/TerminalView.module.css";

const BANNER_URL_REGEX = /https?:\/\/[^\s]+/g;

function linkifyBanner(text: string): ReactNode[] {
  const content: ReactNode[] = [];
  let plainTextStart = 0;
  for (const match of text.matchAll(BANNER_URL_REGEX)) {
    const offset = match.index;
    if (offset > plainTextStart) {
      content.push(text.slice(plainTextStart, offset));
    }
    const url = match[0];
    content.push(
      <a key={`${offset}:${url}`} href={url} target="_blank" rel="noreferrer">
        {url}
      </a>,
    );
    plainTextStart = offset + url.length;
  }
  if (plainTextStart < text.length) {
    content.push(text.slice(plainTextStart));
  }
  return content;
}

export function TerminalSessionLayout(props: {
  active: boolean;
  hostRef: RefObject<HTMLDivElement | null>;
  hostStyle?: CSSProperties;
  connecting: boolean;
  banner: string | null;
  connectingLabel: string;
  barVisible: boolean;
  keyboardInset: number;
  modifiers: Modifiers;
  onModifier: (modifier: ModKey) => void;
  onKey: (key: TerminalKey) => void;
  onPaste: () => void;
}) {
  return (
    <>
      <div className={styles.terminalHostWrap} style={!props.active ? { display: "none" } : undefined}>
        <div className={styles.terminalHost} style={props.hostStyle} ref={props.hostRef} />
        {props.active && props.connecting && (
          <div className={styles.terminalConnecting}>
            <Spinner className={styles.terminalConnectingSpinner} />
            {props.banner ? (
              <div className={cx("card", styles.terminalBanner)}>{linkifyBanner(props.banner)}</div>
            ) : (
              <span className={styles.terminalConnectingLabel}>{props.connectingLabel}</span>
            )}
          </div>
        )}
      </div>
      {props.barVisible && (
        <TerminalSymbolBar
          modifiers={props.modifiers}
          onModifier={props.onModifier}
          onKey={props.onKey}
          onPaste={props.onPaste}
          style={{ bottom: props.keyboardInset }}
        />
      )}
    </>
  );
}
