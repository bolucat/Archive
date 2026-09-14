import { useEffect, useRef, useState, type CSSProperties } from "react";

import { FitAddon } from "@xterm/addon-fit";
import { Terminal, type ITheme } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";

import { useStream } from "../api/stream";
import { useApi, useIsMobile } from "../app/context";
import { useDesktopHost } from "../app/desktop";
import { showError } from "../app/errorStore";
import { useKeyboardInset, useTerminalConfig } from "../app/hooks";
import { useI18n } from "../app/i18n";
import { useLatestRef } from "../app/useLatest";
import { Icon } from "../components/Icon";
import { SYMBOL_BAR_HEIGHT } from "../components/TerminalSymbolBar";
import { TerminalSessionLayout } from "../components/TerminalSessionLayout";
import { EmptyState, IconButton, MenuItem, OthersMenu, SubMenu } from "../components/ui";
import {
  armModifier,
  consumeArmed,
  encodeSpecial,
  encodeText,
  hasActiveModifier,
  type ModKey,
  type Modifiers,
  type TerminalKey,
} from "../lib/terminalKeys";
import {
  StartedService,
  type TailscalePeer,
} from "../gen/daemon/started_service_pb";
import {
  allPeers,
  buildSSHSession,
  loadSSHPrefs,
  peerDisplayName,
  peerSSHAddress,
  peerSSHAvailable,
  SSH_DEFAULT_TERMINAL_TYPE,
  SSH_DEFAULT_USERNAME,
  sshSessionPath,
  type SSHSessionOptions,
} from "../lib/tailscaleSSH";
import {
  currentScheme,
  resolveTheme,
  resolveThemeSync,
  terminalFontFamily,
  terminalFontSize,
  type Scheme,
} from "../lib/terminalTheme";
import { cx } from "../lib/cx";
import styles from "./TerminalView.module.css";

export function TailscaleSSHView(props: {
  tag: string;
  peerID: string;
  username: string;
  terminalType: string;
}) {
  const api = useApi();
  const { t } = useI18n();
  const tailscale = useStream(api.tailscale);

  const endpoint = tailscale.data.endpoints.find((entry) => entry.endpointTag === props.tag);
  const peer = allPeers(endpoint).find((entry) => entry.stableID === props.peerID);

  if (!peer) {
    return (
      <div className="page page-full terminal-page">
        <div className="page-header">
          <h1 className="page-title">SSH</h1>
        </div>
        {tailscale.data.loaded ? (
          <EmptyState icon="terminal">{t("Peer not found")}</EmptyState>
        ) : (
          <EmptyState>{t("Loading...")}</EmptyState>
        )}
      </div>
    );
  }

  return (
    <div className="page page-full terminal-page">
      <TailscaleSSHSession
        tag={props.tag}
        peer={peer}
        username={props.username}
        terminalType={props.terminalType}
      />
    </div>
  );
}

function TailscaleSSHSession(props: {
  tag: string;
  peer: TailscalePeer;
  username: string;
  terminalType: string;
}) {
  const [initialSession] = useState(() =>
    buildSSHSession(props.tag, props.peer, props.username, props.terminalType),
  );
  return (
    <TerminalContainer
      tag={props.tag}
      initialSession={initialSession}
      setWindowTitle
      peerID={props.peer.stableID}
    />
  );
}

export function TerminalOverlay(props: {
  tag: string;
  initialSession: SSHSessionOptions;
  onClose: () => void;
}) {
  return (
    <div className={styles.terminalOverlay}>
      <div className="page page-full terminal-page">
        <TerminalContainer
          tag={props.tag}
          initialSession={props.initialSession}
          onClose={props.onClose}
        />
      </div>
    </div>
  );
}

interface ManagedSession {
  id: number;
  options: SSHSessionOptions;
  title: string;
  statusLine: string | null;
}

function sessionDisplayTitle(session: ManagedSession): string {
  const remote = session.title.trim();
  return remote !== "" ? remote : `${session.options.username}@${session.options.peerName}`;
}

function TerminalContainer(props: {
  tag: string;
  initialSession: SSHSessionOptions;
  onClose?: () => void;
  setWindowTitle?: boolean;
  peerID?: string;
}) {
  const api = useApi();
  const desktop = useDesktopHost();
  const { t } = useI18n();
  const tailscale = useStream(api.tailscale);
  const idRef = useRef(1);
  const [state, setState] = useState<{ sessions: ManagedSession[]; activeID: number }>(() => ({
    sessions: [{ id: 1, options: props.initialSession, title: "", statusLine: null }],
    activeID: 1,
  }));

  const active = state.sessions.find((session) => session.id === state.activeID);
  const activeTitle = active ? sessionDisplayTitle(active) : "SSH";

  useEffect(() => {
    if (props.setWindowTitle) {
      document.title = activeTitle;
    }
  }, [props.setWindowTitle, activeTitle]);

  const onCloseRef = useLatestRef(props.onClose);
  useEffect(() => {
    if (state.sessions.length > 0) {
      return;
    }
    if (onCloseRef.current) {
      onCloseRef.current();
    } else if (desktop && props.setWindowTitle) {
      desktop.terminal.closeWindow();
    } else {
      window.close();
    }
  }, [desktop, props.setWindowTitle, state.sessions.length, onCloseRef]);

  const addSession = (options: SSHSessionOptions) => {
    const id = ++idRef.current;
    setState((current) => ({
      sessions: [...current.sessions, { id, options, title: "", statusLine: null }],
      activeID: id,
    }));
  };

  const updateSession = (id: number, patch: Partial<ManagedSession>) => {
    setState((current) => ({
      ...current,
      sessions: current.sessions.map((session) =>
        session.id === id ? { ...session, ...patch } : session,
      ),
    }));
  };

  const prefs = loadSSHPrefs();
  const endpoint = tailscale.data.endpoints.find((entry) => entry.endpointTag === props.tag);
  const rememberedPeers = allPeers(endpoint).filter(
    (peer) =>
      prefs[peer.stableID]?.remember &&
      peerSSHAvailable(peer) &&
      peerSSHAddress(peer) !== active?.options.peerAddress,
  );

  const duplicateSession = () => {
    if (active) {
      addSession({ ...active.options });
    }
  };

  const openRememberedPeer = (stableID: string) => {
    const peer = allPeers(endpoint).find((entry) => entry.stableID === stableID);
    if (!peer) {
      return;
    }
    const peerPrefs = prefs[peer.stableID];
    addSession(
      buildSSHSession(
        props.tag,
        peer,
        peerPrefs?.username ?? SSH_DEFAULT_USERNAME,
        peerPrefs?.terminalType ?? SSH_DEFAULT_TERMINAL_TYPE,
      ),
    );
  };

  const standalone = props.setWindowTitle === true;
  const windowChrome = desktop !== null && standalone;
  const currentPeerID = props.peerID;

  const openSessionWindow = (stableID: string, username: string, terminalType: string) => {
    const path = sshSessionPath(props.tag, stableID, username, terminalType);
    if (desktop) {
      desktop.terminal.openWindow(path);
      return;
    }
    const url = new URL(location.href);
    url.hash = `#/${path}`;
    window.open(url.toString(), "_blank", "width=960,height=640");
  };

  return (
    <>
      <div className={cx("page-header", windowChrome && styles.windowHeader)}>
        {props.onClose && (
          <IconButton title={t("Close")} onClick={props.onClose}>
            <Icon name="close" size={18} />
          </IconButton>
        )}
        <h1 className="page-title">{activeTitle}</h1>
        <div className="actions">
          {active?.statusLine && <span className="hint">{active.statusLine}</span>}
          {state.sessions.length > 0 &&
            (standalone ? (
              <OthersMenu icon="add" title={t("New Session")}>
                {currentPeerID !== undefined && (
                  <MenuItem
                    icon="content_copy"
                    onSelect={() =>
                      openSessionWindow(
                        currentPeerID,
                        props.initialSession.username,
                        props.initialSession.terminalType,
                      )
                    }
                  >
                    {props.initialSession.peerName}
                  </MenuItem>
                )}
                {rememberedPeers.map((peer) => (
                  <MenuItem
                    key={peer.stableID}
                    onSelect={() =>
                      openSessionWindow(
                        peer.stableID,
                        prefs[peer.stableID]?.username ?? SSH_DEFAULT_USERNAME,
                        prefs[peer.stableID]?.terminalType ?? SSH_DEFAULT_TERMINAL_TYPE,
                      )
                    }
                  >
                    {peerDisplayName(peer)}
                  </MenuItem>
                ))}
              </OthersMenu>
            ) : (
              <OthersMenu>
                {rememberedPeers.length === 0 ? (
                  <MenuItem icon="add" onSelect={duplicateSession}>
                    {t("New Session")}
                  </MenuItem>
                ) : (
                  <SubMenu label={t("New Session")} icon="add">
                    {active && (
                      <MenuItem icon="content_copy" onSelect={duplicateSession}>
                        {active.options.peerName}
                      </MenuItem>
                    )}
                    {rememberedPeers.map((peer) => (
                      <MenuItem
                        key={peer.stableID}
                        onSelect={() => openRememberedPeer(peer.stableID)}
                      >
                        {peerDisplayName(peer)}
                      </MenuItem>
                    ))}
                  </SubMenu>
                )}
                <div className="menu-divider" />
                {state.sessions.map((session) => (
                  <MenuItem
                    key={session.id}
                    checked={session.id === state.activeID}
                    onSelect={() =>
                      setState((current) => ({ ...current, activeID: session.id }))
                    }
                  >
                    {sessionDisplayTitle(session)}
                  </MenuItem>
                ))}
              </OthersMenu>
            ))}
        </div>
      </div>
      {state.sessions.length === 0 && (
        <EmptyState icon="terminal">{t("Session closed")}</EmptyState>
      )}
      {state.sessions.map((session) => (
        <TerminalSession
          key={session.id}
          session={session.options}
          active={session.id === state.activeID}
          onStatusLine={(line) => updateSession(session.id, { statusLine: line })}
          onTitleChange={(title) => updateSession(session.id, { title })}
          onExit={(clean) => {
            if (clean) {
              window.setTimeout(() => {
                setState((current) => {
                  const sessions = current.sessions.filter((entry) => entry.id !== session.id);
                  const activeID =
                    current.activeID === session.id
                      ? (sessions[sessions.length - 1]?.id ?? 0)
                      : current.activeID;
                  return { sessions, activeID };
                });
              }, 1000);
            }
          }}
        />
      ))}
    </>
  );
}

function TerminalSession(props: {
  session: SSHSessionOptions;
  active: boolean;
  onStatusLine: (line: string | null) => void;
  onTitleChange: (title: string) => void;
  onExit: (clean: boolean) => void;
}) {
  const api = useApi();
  const desktop = useDesktopHost();
  const { t } = useI18n();
  const hostRef = useRef<HTMLDivElement>(null);
  const terminalRef = useRef<Terminal | null>(null);
  const fitRef = useRef<FitAddon | null>(null);

  const isMobile = useIsMobile();
  const keyboardInset = useKeyboardInset();
  const config = useTerminalConfig();
  const { symbolBarAlwaysShow } = config;
  const configRef = useLatestRef(config);
  const [scheme, setScheme] = useState<Scheme>(() => currentScheme());
  const [activeTheme, setActiveTheme] = useState<ITheme>(() => resolveThemeSync(config, scheme));
  const [connecting, setConnecting] = useState(true);
  const [banner, setBanner] = useState<string | null>(null);
  const [modifiers, setModifiers] = useState<Modifiers>({ ctrl: "off", alt: "off" });
  const modifiersRef = useLatestRef(modifiers);
  const armedAtRef = useRef<Record<ModKey, number>>({ ctrl: 0, alt: 0 });
  const sendRawRef = useRef<((text: string) => void) | null>(null);

  const tRef = useLatestRef(t);
  const onStatusLineRef = useLatestRef(props.onStatusLine);
  const onTitleChangeRef = useLatestRef(props.onTitleChange);
  const onExitRef = useLatestRef(props.onExit);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) {
      return;
    }
    const setStatusLine = (line: string | null) => onStatusLineRef.current(line);
    const initialConfig = configRef.current;
    const terminal = new Terminal({
      fontFamily: terminalFontFamily(initialConfig),
      fontSize: terminalFontSize(initialConfig),
      cursorBlink: true,
      theme: resolveThemeSync(initialConfig, currentScheme()),
    });
    const fit = new FitAddon();
    terminal.loadAddon(fit);
    terminal.open(host);
    fit.fit();
    terminal.focus();
    terminalRef.current = terminal;
    fitRef.current = fit;

    const pasteClipboardText = (text: string | null) => {
      if (text !== null && text !== "" && terminalRef.current === terminal) {
        terminal.paste(text);
      }
    };
    if (desktop?.platform === "win32") {
      terminal.attachCustomKeyEventHandler((event) => {
        const controlShortcut =
          event.ctrlKey && !event.shiftKey && !event.altKey && !event.metaKey;
        if (controlShortcut && event.code === "KeyC" && terminal.hasSelection()) {
          event.preventDefault();
          void desktop.terminal
            .writeClipboardText(terminal.getSelection())
            .then(() => {
              if (terminalRef.current === terminal) {
                terminal.clearSelection();
              }
            })
            .catch(showError);
          return false;
        }
        if (
          controlShortcut &&
          event.code === "KeyV"
        ) {
          event.preventDefault();
          void desktop.terminal.readClipboardText().then(pasteClipboardText).catch(showError);
          return false;
        }
        return true;
      });
    }
    const handleContextMenu = (event: MouseEvent) => {
      if (desktop === null) {
        return;
      }
      event.preventDefault();
      void desktop.terminal
        .openContextMenu(terminal.getSelection())
        .then((result) => {
          if (result?.action === "copy") {
            if (terminalRef.current === terminal) {
              terminal.clearSelection();
            }
          } else if (result?.action === "paste") {
            pasteClipboardText(result.text);
          }
        })
        .catch(showError)
        .finally(() => {
          if (terminalRef.current === terminal) {
            terminal.focus();
          }
        });
    };
    host.addEventListener("contextmenu", handleContextMenu);

    let ready = false;
    let lastStatus: string | null = null;
    const stream = api.openBidirectionalStream(
      StartedService.method.startTailscaleSSHSession,
      {
        onMessage: (message) => {
          switch (message.message.case) {
            case "authBanner": {
              const bannerMessage = message.message.value.message.trim();
              if (bannerMessage !== "") {
                setBanner((previous) => {
                  if (!previous) {
                    return bannerMessage;
                  }
                  if (previous.includes(bannerMessage)) {
                    return previous;
                  }
                  return `${previous}\n\n${bannerMessage}`;
                });
              }
              break;
            }
            case "ready":
              ready = true;
              lastStatus = null;
              setStatusLine(null);
              setConnecting(false);
              setBanner(null);
              break;
            case "output":
              terminal.write(message.message.value.data);
              break;
            case "exit": {
              const exit = message.message.value;
              let text = tRef.current("Session exited with code {code}", { code: exit.exitCode });
              if (exit.signal !== "") {
                text += ` ${tRef.current("(signal {signal})", { signal: exit.signal })}`;
              }
              if (exit.errorMessage !== "") {
                text += `: ${exit.errorMessage}`;
              }
              lastStatus = text;
              setStatusLine(text);
              setConnecting(false);
              onExitRef.current(exit.exitCode === 0 && exit.errorMessage === "");
              break;
            }
            case "error":
              lastStatus = message.message.value.message;
              setStatusLine(lastStatus);
              setConnecting(false);
              break;
          }
        },
        onEnd: (status, error) => {
          if (status && status.code !== 0) {
            setStatusLine(
              status.message ||
                tRef.current("Stream ended with status {code}", { code: status.code }),
            );
          } else if (error && !ready) {
            setStatusLine(error);
          } else {
            setStatusLine(lastStatus ?? tRef.current("Session closed"));
          }
          setConnecting(false);
          terminal.options.cursorBlink = false;
        },
      },
    );

    stream.send({
      message: {
        case: "start",
        value: {
          endpointTag: props.session.endpointTag,
          peerAddress: props.session.peerAddress,
          username: props.session.username,
          terminalType: props.session.terminalType,
          columns: terminal.cols,
          rows: terminal.rows,
          hostKeys: props.session.hostKeys,
        },
      },
    });

    const encoder = new TextEncoder();
    const sendRaw = (text: string) => {
      stream.send({
        message: {
          case: "input",
          value: { data: encoder.encode(text) },
        },
      });
    };
    sendRawRef.current = sendRaw;
    const dataSubscription = terminal.onData((data) => {
      const mods = modifiersRef.current;
      if (hasActiveModifier(mods)) {
        sendRaw(encodeText(data, mods));
        setModifiers((current) => consumeArmed(current));
      } else {
        sendRaw(String(data));
      }
    });
    const resizeSubscription = terminal.onResize((size) => {
      stream.send({
        message: {
          case: "resize",
          value: { columns: size.cols, rows: size.rows },
        },
      });
    });
    const titleSubscription = terminal.onTitleChange((title) => {
      onTitleChangeRef.current(title);
    });
    const resizeObserver = new ResizeObserver(() => {
      if (host.clientWidth > 0 && host.clientHeight > 0) {
        fit.fit();
      }
    });
    resizeObserver.observe(host);

    return () => {
      host.removeEventListener("contextmenu", handleContextMenu);
      resizeObserver.disconnect();
      dataSubscription.dispose();
      resizeSubscription.dispose();
      titleSubscription.dispose();
      stream.close();
      terminal.dispose();
      terminalRef.current = null;
      fitRef.current = null;
      sendRawRef.current = null;
    };
  }, [
    api,
    desktop,
    props.session,
    configRef,
    modifiersRef,
    onExitRef,
    onStatusLineRef,
    onTitleChangeRef,
    tRef,
  ]);

  useEffect(() => {
    if (!props.active) {
      return;
    }
    const host = hostRef.current;
    if (host && host.clientWidth > 0 && host.clientHeight > 0) {
      fitRef.current?.fit();
    }
    terminalRef.current?.focus();
  }, [props.active]);

  useEffect(() => {
    const observer = new MutationObserver(() => setScheme(currentScheme()));
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-theme"],
    });
    setScheme(currentScheme());
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const terminal = terminalRef.current;
    if (!terminal) {
      return;
    }
    terminal.options.fontFamily = terminalFontFamily(config);
    terminal.options.fontSize = terminalFontSize(config);
    let cancelled = false;
    void resolveTheme(config, scheme).then((theme) => {
      if (cancelled) {
        return;
      }
      const term = terminalRef.current;
      if (!term) {
        return;
      }
      term.options.theme = theme;
      setActiveTheme(() => theme);
      fitRef.current?.fit();
    });
    return () => {
      cancelled = true;
    };
  }, [config, scheme]);

  const handleModifier = (mod: ModKey) => {
    const now = Date.now();
    const doubleTap = now - armedAtRef.current[mod] < 300;
    armedAtRef.current[mod] = now;
    setModifiers((current) => armModifier(current, mod, doubleTap));
    terminalRef.current?.focus();
  };

  const handleKey = (key: TerminalKey) => {
    const mods = modifiersRef.current;
    let seq: string | null = null;
    if (key.kind === "special") {
      seq = encodeSpecial(key.id, mods);
    } else if (key.kind === "text") {
      seq = encodeText(key.char, mods);
    }
    if (seq !== null) {
      sendRawRef.current?.(seq);
    }
    setModifiers((current) => consumeArmed(current));
    terminalRef.current?.focus();
  };

  const handlePaste = () => {
    const readText = desktop?.terminal.readClipboardText() ?? navigator.clipboard?.readText();
    if (readText) {
      void readText
        .then((text) => {
          if (text) {
            terminalRef.current?.paste(text);
          }
        })
        .catch(showError);
    }
    setModifiers((current) => consumeArmed(current));
    terminalRef.current?.focus();
  };

  const keyboardVisible = isMobile && keyboardInset > 100;
  const barVisible = props.active && (keyboardVisible || (symbolBarAlwaysShow && !isMobile));
  const barFloating = barVisible && keyboardVisible;
  const wrapStyle: CSSProperties = {};
  if (activeTheme.background) {
    wrapStyle.background = activeTheme.background;
  }
  if (barFloating) {
    wrapStyle.paddingBottom = `calc(${keyboardInset + SYMBOL_BAR_HEIGHT + 8}px + env(safe-area-inset-bottom, 0px))`;
  }

  return (
    <TerminalSessionLayout
      active={props.active}
      hostRef={hostRef}
      wrapStyle={Object.keys(wrapStyle).length > 0 ? wrapStyle : undefined}
      connecting={connecting}
      banner={banner}
      connectingLabel={t("Connecting...")}
      barVisible={barVisible}
      barFloating={barFloating}
      keyboardInset={keyboardInset}
      modifiers={modifiers}
      onModifier={handleModifier}
      onKey={handleKey}
      onPaste={handlePaste}
    />
  );
}
