import { useState } from "react";

import { navigate, useIsMobile } from "../app/context";
import { useDesktopHost } from "../app/desktop";
import { useI18n } from "../app/i18n";
import { Button, Dialog, Field, Toggle } from "../components/ui";
import type { TailscalePeer } from "../gen/daemon/started_service_pb";
import {
  buildSSHSession,
  loadSSHPrefs,
  peerDisplayName,
  saveSSHPrefs,
  SSH_DEFAULT_TERMINAL_TYPE,
  SSH_DEFAULT_USERNAME,
  sshSessionPath,
  type SSHSessionOptions,
} from "../lib/tailscaleSSH";
import { TerminalOverlay } from "./TerminalView";

export function useTailscaleSSH(tag: string) {
  const desktop = useDesktopHost();
  const isMobile = useIsMobile();
  const [promptPeer, setPromptPeer] = useState<TailscalePeer | null>(null);
  const [mobileSession, setMobileSession] = useState<SSHSessionOptions | null>(null);

  const openSession = (peer: TailscalePeer, username: string, terminalType: string) => {
    const path = sshSessionPath(tag, peer.stableID, username, terminalType);
    if (desktop) {
      desktop.terminal.openWindow(path);
      return;
    }
    if (isMobile) {
      setMobileSession(buildSSHSession(tag, peer, username, terminalType));
      return;
    }
    const url = new URL(location.href);
    url.hash = `#/${path}`;
    if (!window.open(url.toString(), "_blank", "width=960,height=640")) {
      navigate(path);
    }
  };

  const connect = (peer: TailscalePeer) => {
    const peerPrefs = loadSSHPrefs()[peer.stableID];
    if (peerPrefs?.remember) {
      openSession(peer, peerPrefs.username, peerPrefs.terminalType);
    } else {
      setPromptPeer(() => peer);
    }
  };

  const element = (
    <>
      {promptPeer && (
        <SSHPrompt
          key={promptPeer.stableID}
          peer={promptPeer}
          onCancel={() => setPromptPeer(null)}
          onConnect={(username, terminalType, remember) => {
            saveSSHPrefs(promptPeer.stableID, { username, terminalType, remember });
            setPromptPeer(null);
            openSession(promptPeer, username, terminalType);
          }}
        />
      )}
      {mobileSession && (
        <TerminalOverlay
          tag={tag}
          initialSession={mobileSession}
          onClose={() => setMobileSession(null)}
        />
      )}
    </>
  );

  return {
    connect,
    prompt: (peer: TailscalePeer) => setPromptPeer(() => peer),
    element,
  };
}

function SSHPrompt(props: {
  peer: TailscalePeer;
  onCancel: () => void;
  onConnect: (username: string, terminalType: string, remember: boolean) => void;
}) {
  const { t } = useI18n();
  const [initial] = useState(() => loadSSHPrefs()[props.peer.stableID]);
  const [username, setUsername] = useState(initial?.username ?? SSH_DEFAULT_USERNAME);
  const [terminalType, setTerminalType] = useState(
    initial?.terminalType ?? SSH_DEFAULT_TERMINAL_TYPE,
  );
  const [remember, setRemember] = useState(initial?.remember ?? false);

  const connect = () => {
    const trimmed = username.trim();
    if (trimmed === "") {
      return;
    }
    props.onConnect(trimmed, terminalType.trim() || SSH_DEFAULT_TERMINAL_TYPE, remember);
  };

  return (
    <Dialog onClose={props.onCancel}>
      <h3>{t("SSH Configuration")}</h3>
      <div className="hint" style={{ marginBottom: 12 }}>{peerDisplayName(props.peer)}</div>
      <Field label={t("Username")}>
        <input
          className="input"
          value={username}
          onChange={(event) => setUsername(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              connect();
            }
          }}
        />
      </Field>
      <Field label={t("Terminal type")}>
        <input
          className="input"
          value={terminalType}
          onChange={(event) => setTerminalType(event.target.value)}
        />
      </Field>
      <Toggle label={t("Remember SSH options")} value={remember} onChange={setRemember} />
      <div className="hint" style={{ display: "grid", gap: 6 }}>
        <div>
          {t(
            "If enabled, Connect will open the session directly without asking again. This also applies to the shortcut menu on this peer's entry in the peer list.",
          )}
        </div>
        <div>
          {t(
            "This peer will also appear in the New Session menu when connected to other peers via SSH.",
          )}
        </div>
      </div>
      <div className="row-actions dialog-actions">
        <Button onClick={props.onCancel}>
          {t("Cancel")}
        </Button>
        <Button variant="primary" disabled={username.trim() === ""} onClick={connect}>
          {t("Connect")}
        </Button>
      </div>
    </Dialog>
  );
}
