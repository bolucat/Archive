import { useState, type DragEvent } from "react";

import { formatRelativeTime, isHttpUrl, type DelayTone } from "../api/format";
import { useStream } from "../api/stream";
import { useSupportsCapability } from "../app/capabilities";
import { navigate, useApi, useIsMobile, useNow } from "../app/context";
import { showError } from "../app/errorStore";
import { useStreamingAction } from "../app/hooks";
import { useI18n, type Translate } from "../app/i18n";
import { Icon } from "../components/Icon";
import { StreamStates } from "../components/StreamBanner";
import {
  Badge,
  Button,
  Card,
  CopyValue,
  DataLine,
  DetailSection,
  DetailShell,
  Dialog,
  Field,
  IconButton,
  MenuItem,
  NavLine,
  NavLines,
  OthersMenu,
  QRCode,
  Sparkline,
  StateDot,
} from "../components/ui";
import type {
  TailscaleEndpointStatus,
  TailscalePeer,
  TailscalePingResponse,
} from "../gen/daemon/started_service_pb";
import { cx } from "../lib/cx";
import { allPeers, loadSSHPrefs, peerDisplayName } from "../lib/tailscaleSSH";
import { useTaildropSendSessions } from "../lib/taildropSend";
import { taildropPath, useTaildropSend } from "./TaildropView";
import { useTailscaleSSH } from "./TailscaleSSHConnect";
import { ToolsPageHeader } from "./ToolsView";
import styles from "./TailscaleView.module.css";

export function TailscaleEndpointView(props: { tag: string }) {
  const api = useApi();
  const { t } = useI18n();
  const tailscale = useStream(api.tailscale);
  const isMobile = useIsMobile();
  const [peerDetail, setPeerDetail] = useState<string | null>(null);
  const [exitPickerOpen, setExitPickerOpen] = useState(false);
  const [authQROpen, setAuthQROpen] = useState(false);
  const ssh = useTailscaleSSH(props.tag);
  const taildropSupported = useSupportsCapability("taildrop");
  const taildrop = useTaildropSend(props.tag);
  const taildropSessions = useTaildropSendSessions(api, props.tag);
  const taildropSending = taildropSessions.length > 0;
  const taildropFailed = taildropSessions.some((session) => session.error !== "");

  const endpoint = tailscale.data.endpoints.find((entry) => entry.endpointTag === props.tag);
  const peers = allPeers(endpoint);
  const exitNodeCandidates = peers.filter((peer) => peer.exitNodeOption);
  const running = endpoint?.backendState === "Running";
  const detailPeer =
    peerDetail === "self"
      ? endpoint?.self
      : peers.find((peer) => peer.stableID === peerDetail);
  const canSendFiles = (peer: TailscalePeer) =>
    taildropSupported &&
    endpoint !== undefined &&
    endpoint.canShareFiles &&
    peer.online &&
    peer.canReceiveFiles;
  const detailCanSendFiles =
    detailPeer !== undefined && peerDetail !== "self" && canSendFiles(detailPeer);

  const dialogs = (
    <>
      {endpoint && exitPickerOpen && (
        <ExitNodePicker
          endpoint={endpoint}
          candidates={exitNodeCandidates}
          onClose={() => setExitPickerOpen(false)}
        />
      )}
      {endpoint && authQROpen && endpoint.authURL !== "" && (
        <Dialog onClose={() => setAuthQROpen(false)}>
          <h3>{t("Auth URL")}</h3>
          <QRCode value={endpoint.authURL} />
          <CopyValue value={endpoint.authURL} className={styles.qrCopy} />
        </Dialog>
      )}
      {ssh.element}
      {taildrop.element}
    </>
  );

  const detail = endpoint && detailPeer && (
    <DetailShell
      backLabel="Tailscale"
      title={peerDisplayName(detailPeer)}
      accessory={
        <Badge tone={detailPeer.online ? "good" : "neutral"}>
          {detailPeer.online ? t("Connected") : t("Not connected")}
        </Badge>
      }
      onClose={() => setPeerDetail(null)}
    >
      <PeerDetailBody
        endpoint={endpoint}
        peer={detailPeer}
        isSelf={peerDetail === "self"}
        onClose={() => setPeerDetail(null)}
        onConnectSSH={() => ssh.connect(detailPeer)}
        onEditSSH={() => ssh.prompt(detailPeer)}
        onPickFiles={detailCanSendFiles ? () => taildrop.pick(detailPeer) : undefined}
        onDropFiles={
          detailCanSendFiles ? (files) => taildrop.send(detailPeer, files) : undefined
        }
      />
    </DetailShell>
  );

  if (isMobile && detail) {
    return (
      <>
        {detail}
        {dialogs}
      </>
    );
  }

  return (
    <div className="page">
      <ToolsPageHeader
        title={
          tailscale.data.endpoints.length > 1 && props.tag !== ""
            ? t("Tailscale: {tag}", { tag: props.tag })
            : "Tailscale"
        }
      />
      <StreamStates
        snapshot={tailscale}
        loaded={tailscale.data.loaded}
        empty={!endpoint}
        emptyIcon="hub"
        emptyMessage={t("Endpoint not found")}
      />
      {endpoint && (
        <div className="settings-stack">
          <StatusCard
            endpoint={endpoint}
            hasExitNodes={exitNodeCandidates.length > 0}
            hasTaildrop={taildropSupported}
            hasSending={taildropSending}
            hasFailedSend={taildropFailed}
            onShowSelf={() => setPeerDetail("self")}
            onOpenExitPicker={() => setExitPickerOpen(true)}
            onOpenAuthQR={() => setAuthQROpen(true)}
            onOpenTaildrop={() => navigate(taildropPath(props.tag))}
          />
          {running && allPeers.length > 0 && (
            <PeerSections
              endpoint={endpoint}
              canSendFiles={canSendFiles}
              onShowPeer={setPeerDetail}
              onConnectSSH={ssh.connect}
              onSendFiles={taildrop.send}
            />
          )}
        </div>
      )}
      {detail}
      {dialogs}
    </div>
  );
}

function taildropLineValue(
  endpoint: TailscaleEndpointStatus,
  sending: boolean,
  t: Translate,
): string {
  if (endpoint.waitingFileCount === 1) {
    return t("1 file");
  }
  if (endpoint.waitingFileCount > 1) {
    return t("{count} files", { count: endpoint.waitingFileCount });
  }
  if (endpoint.receivingFileCount > 0) {
    return t("Receive");
  }
  return sending ? t("Send") : "";
}

const BACKEND_STATE_TONES: Record<string, DelayTone> = {
  Running: "good",
  NeedsLogin: "bad",
  NeedsMachineAuth: "bad",
  Starting: "medium",
};

function StatusCard(props: {
  endpoint: TailscaleEndpointStatus;
  hasExitNodes: boolean;
  hasTaildrop: boolean;
  hasSending: boolean;
  hasFailedSend: boolean;
  onShowSelf: () => void;
  onOpenExitPicker: () => void;
  onOpenAuthQR: () => void;
  onOpenTaildrop: () => void;
}) {
  const { t } = useI18n();
  const endpoint = props.endpoint;
  const running = endpoint.backendState === "Running";

  return (
    <div>
      <div className="list-section-title">{t("Status")}</div>
      <Card>
        <NavLines>
          <NavLine
            icon="power_settings_new"
            label={t("State")}
            value={
              <>
                <StateDot tone={BACKEND_STATE_TONES[endpoint.backendState] ?? "neutral"} />
                {endpoint.stateText}
              </>
            }
          />
          {running && endpoint.self && (
            <NavLine
              icon="computer"
              label={t("This device")}
              value={peerDisplayName(endpoint.self)}
              chevron
              onClick={props.onShowSelf}
            />
          )}
          {running && props.hasExitNodes && (
            <NavLine
              icon="router"
              label={t("Exit node")}
              value={endpoint.exitNode ? peerDisplayName(endpoint.exitNode) : t("Disabled")}
              chevron
              onClick={props.onOpenExitPicker}
            />
          )}
          {running &&
            props.hasTaildrop &&
            (endpoint.waitingFileCount > 0 ||
              endpoint.receivingFileCount > 0 ||
              props.hasSending) && (
              <NavLine
                icon="download"
                label="Taildrop"
                value={
                  props.hasFailedSend ? (
                    <Badge tone="danger">!</Badge>
                  ) : (
                    taildropLineValue(endpoint, props.hasSending, t)
                  )
                }
                chevron
                onClick={props.onOpenTaildrop}
              />
            )}
          {endpoint.authURL !== "" && (
            <>
              {isHttpUrl(endpoint.authURL) && (
                <NavLine icon="open_in_new" label={t("Open auth URL")} href={endpoint.authURL} />
              )}
              <NavLine
                icon="qr_code"
                label={t("Show auth URL QR code")}
                onClick={props.onOpenAuthQR}
              />
            </>
          )}
        </NavLines>
      </Card>
    </div>
  );
}

function PeerSections(props: {
  endpoint: TailscaleEndpointStatus;
  canSendFiles: (peer: TailscalePeer) => boolean;
  onShowPeer: (id: string) => void;
  onConnectSSH: (peer: TailscalePeer) => void;
  onSendFiles: (peer: TailscalePeer, files: File[]) => void;
}) {
  const groups = props.endpoint.userGroups.flatMap((group) =>
    group.peers.length > 0 ? [{ group, peers: group.peers }] : [],
  );

  return (
    <>
      {groups.map(({ group, peers }) => (
        <div key={group.userID.toString()}>
          <div className="list-section-title">{group.displayName || group.loginName}</div>
          <div className={styles.peerList}>
            {peers.map((peer) => (
              <PeerRow
                key={peer.stableID}
                peer={peer}
                onOpen={() => props.onShowPeer(peer.stableID)}
                onConnectSSH={
                  peer.online && peer.sshHostKeys.length > 0 && peer.tailscaleIPs.length > 0
                    ? () => props.onConnectSSH(peer)
                    : undefined
                }
                onSendFiles={
                  props.canSendFiles(peer) ? (files) => props.onSendFiles(peer, files) : undefined
                }
              />
            ))}
          </div>
        </div>
      ))}
    </>
  );
}

function useFileDrop(onFiles: ((files: File[]) => void) | undefined) {
  const [active, setActive] = useState(false);
  if (onFiles === undefined) {
    return { active: false, props: {} };
  }
  return {
    active,
    props: {
      onDragOver: (event: DragEvent<HTMLElement>) => {
        if (!event.dataTransfer.types.includes("Files")) {
          return;
        }
        event.preventDefault();
        event.dataTransfer.dropEffect = "copy";
        setActive(true);
      },
      onDragLeave: (event: DragEvent<HTMLElement>) => {
        const target = event.relatedTarget;
        if (target instanceof Node && event.currentTarget.contains(target)) {
          return;
        }
        setActive(false);
      },
      onDrop: (event: DragEvent<HTMLElement>) => {
        event.preventDefault();
        setActive(false);
        onFiles(Array.from(event.dataTransfer.files));
      },
    },
  };
}

function PeerRow(props: {
  peer: TailscalePeer;
  onOpen: () => void;
  onConnectSSH?: () => void;
  onSendFiles?: (files: File[]) => void;
}) {
  const { t, language } = useI18n();
  const peer = props.peer;
  const now = useNow(30_000);
  const drop = useFileDrop(props.onSendFiles);
  return (
    <div className={cx(styles.peerItem, drop.active && styles.peerItemDropTarget)} {...drop.props}>
      <button type="button" className={styles.peerItemMain} onClick={props.onOpen}>
        <StateDot tone={peer.online ? "good" : undefined} />
        <span className="peer-name">{peerDisplayName(peer)}</span>
        <span className="peer-address">{peer.tailscaleIPs[0] ?? ""}</span>
        {peer.online && (
          <span className="badges">
            {peer.shareeNode && <Badge tone="danger">{t("Shared in")}</Badge>}
            {peer.exitNode && <Badge tone="info">{t("Exit node")}</Badge>}
            {peer.expired && <Badge tone="danger">{t("Expired")}</Badge>}
            {!peer.expired &&
              peer.keyExpiry > 0n &&
              Number(peer.keyExpiry) * 1000 - now < 30 * 86400_000 && (
                <Badge>
                  {t("Expires {time}", {
                    time: formatRelativeTime(Number(peer.keyExpiry) * 1000, now, language),
                  })}
                </Badge>
              )}
            {peer.sshHostKeys.length > 0 && <Badge tone="good">SSH</Badge>}
          </span>
        )}
      </button>
      {props.onConnectSSH && (
        <OthersMenu className={styles.peerMenu} icon="more_horiz">
          <MenuItem icon="terminal" onSelect={props.onConnectSSH}>
            {t("Connect via SSH")}
          </MenuItem>
        </OthersMenu>
      )}
    </div>
  );
}

function TaildropZone(props: { onPick: () => void; onDrop: (files: File[]) => void }) {
  const { t } = useI18n();
  const drop = useFileDrop(props.onDrop);
  return (
    <button
      type="button"
      className={cx(styles.dropZone, drop.active && styles.dropZoneActive)}
      onClick={props.onPick}
      {...drop.props}
    >
      <Icon name="upload_file" size={18} />
      {t("Drop files here to send, or click to select")}
    </button>
  );
}

function PeerDetailBody(props: {
  endpoint: TailscaleEndpointStatus;
  peer: TailscalePeer;
  isSelf: boolean;
  onClose: () => void;
  onConnectSSH: () => void;
  onEditSSH: () => void;
  onPickFiles?: () => void;
  onDropFiles?: (files: File[]) => void;
}) {
  const api = useApi();
  const { t, language } = useI18n();
  const peer = props.peer;
  const now = useNow(30_000);
  const ipv4 = peer.tailscaleIPs.find((address) => !address.includes(":"));
  const ipv6 = peer.tailscaleIPs.find((address) => address.includes(":"));
  const sshAvailable = !props.isSelf && peer.online && peer.sshHostKeys.length > 0;
  const sshRemembered = loadSSHPrefs()[peer.stableID]?.remember ?? false;
  const canLogout = props.isSelf && !props.endpoint.keyAuth;
  const [confirmingLogout, setConfirmingLogout] = useState(false);

  return (
    <>
      {props.isSelf && (props.endpoint.networkName !== "" || canLogout) && (
        <>
          {props.endpoint.networkName !== "" && (
            <DetailSection title={t("Network")}>
              <DataLine label={t("Network")} value={props.endpoint.networkName} />
            </DetailSection>
          )}
          {canLogout && (
            <div className="row-actions" style={{ marginTop: 10 }}>
              <Button variant="danger" size="small" onClick={() => setConfirmingLogout(true)}>
                <Icon name="logout" size={13} />
                {t("Log out")}
              </Button>
            </div>
          )}
          {confirmingLogout && (
            <Dialog onClose={() => setConfirmingLogout(false)}>
              <h3>{t("Log out")}</h3>
              <div className="dialog-message">{t("Log out from this Tailscale network?")}</div>
              <div className="row-actions dialog-actions">
                <Button onClick={() => setConfirmingLogout(false)}>{t("Cancel")}</Button>
                <Button
                  variant="danger"
                  onClick={() => {
                    setConfirmingLogout(false);
                    void api.tailscaleLogout(props.endpoint.endpointTag).catch(showError);
                    props.onClose();
                  }}
                >
                  {t("Log out")}
                </Button>
              </div>
            </Dialog>
          )}
        </>
      )}

      <DetailSection title={t("Addresses")}>
        {peer.dnsName !== "" && (
          <DataLine label="MagicDNS" value={<CopyValue value={peer.dnsName.replace(/\.$/, "")} />} />
        )}
        <DataLine label={t("Hostname")} value={<CopyValue value={peer.hostName} />} />
        {ipv4 && <DataLine label="IPv4" value={<CopyValue value={ipv4} />} />}
        {ipv6 && <DataLine label="IPv6" value={<CopyValue value={ipv6} />} />}
      </DetailSection>

      {!props.isSelf && peer.online && (
        <PingSection endpoint={props.endpoint} peer={peer} />
      )}

      {props.onPickFiles && props.onDropFiles && (
        <>
          <div className="drawer-section">Taildrop</div>
          <TaildropZone onPick={props.onPickFiles} onDrop={props.onDropFiles} />
        </>
      )}

      <DetailSection title={t("Details")}>
        {peer.os !== "" && <DataLine label={t("OS")} value={peer.os} />}
        <DataLine
          label={t("Key expiry")}
          value={
            peer.expired
              ? t("Expired")
              : peer.keyExpiry > 0n
                ? formatRelativeTime(Number(peer.keyExpiry) * 1000, now, language)
                : t("Disabled")
          }
        />
        {!peer.online && peer.lastSeen > 0n && (
          <DataLine
            label={t("Last seen")}
            value={formatRelativeTime(Number(peer.lastSeen) * 1000, now, language)}
          />
        )}
        {peer.exitNodeOption && (
          <DataLine label={t("Exit node")} value={peer.exitNode ? t("Active") : t("Available")} />
        )}
        {peer.shareeNode && <DataLine label={t("Shared in")} value={t("Yes")} />}
      </DetailSection>
      {sshAvailable && (
        <div className="row-actions" style={{ marginTop: 14 }}>
          {sshRemembered && (
            <Button onClick={props.onEditSSH}>
              <Icon name="edit" size={13} />
              {t("Edit SSH Configuration")}
            </Button>
          )}
          <Button variant="primary" onClick={props.onConnectSSH}>
            <Icon name="terminal" size={13} />
            {t("Connect via SSH")}
          </Button>
        </div>
      )}
    </>
  );
}

function PingSection(props: { endpoint: TailscaleEndpointStatus; peer: TailscalePeer }) {
  const api = useApi();
  const { t } = useI18n();
  const [history, setHistory] = useState<number[]>([]);
  const [latest, setLatest] = useState<TailscalePingResponse | null>(null);
  const { running, error, reportError, start: startAction, stop } = useStreamingAction();

  const start = () =>
    startAction(async (signal) => {
      setHistory([]);
      setLatest(null);
      for await (const response of api.client.startTailscalePing(
        {
          endpointTag: props.endpoint.endpointTag,
          peerIP: props.peer.tailscaleIPs[0] ?? "",
        },
        { signal },
      )) {
        if (response.error !== "") {
          reportError(response.error);
          continue;
        }
        setLatest(response);
        setHistory((current) => {
          const next = current.concat(response.latencyMs);
          return next.length > 30 ? next.slice(next.length - 30) : next;
        });
      }
    });

  return (
    <DetailSection
      title={t("Ping")}
      accessory={
        <IconButton
          title={running ? t("Stop") : t("Start")}
          onClick={() => (running ? stop() : start())}
        >
          <Icon name={running ? "stop" : "play_arrow"} size={13} />
        </IconButton>
      }
    >
      {error !== "" && <div className="hint" style={{ color: "var(--danger)", padding: "9px 0" }}>{error}</div>}
      {latest && (
        <>
          <DataLine
            label={
              latest.isDirect
                ? t("Direct connection")
                : latest.peerRelay !== ""
                  ? t("Peer relay connection")
                  : t("DERP-relayed connection")
            }
            value={`${latest.latencyMs.toFixed(1)} ms`}
          />
          {!latest.isDirect && latest.peerRelay !== "" && (
            <DataLine label={t("Peer relay")} value={latest.peerRelay} />
          )}
          {!latest.isDirect && latest.peerRelay === "" && latest.derpRegionCode !== "" && (
            <DataLine label={t("DERP region")} value={latest.derpRegionCode} />
          )}
          {latest.isDirect && latest.endpoint !== "" && (
            <DataLine label={t("Endpoint")} value={latest.endpoint} />
          )}
          <div style={{ margin: "6px 0 8px" }}>
            <Sparkline
              data={history}
              color={latest.isDirect ? "var(--good)" : "var(--info)"}
              height={56}
            />
          </div>
        </>
      )}
      {running && !latest && error === "" && (
        <div className="hint" style={{ padding: "9px 0" }}>{t("Connecting...")}</div>
      )}
      {!latest && !running && <div className="hint" style={{ padding: "9px 0" }}>{t("No data")}</div>}
    </DetailSection>
  );
}

function ExitNodePicker(props: {
  endpoint: TailscaleEndpointStatus;
  candidates: TailscalePeer[];
  onClose: () => void;
}) {
  const api = useApi();
  const { t } = useI18n();
  const [search, setSearch] = useState("");
  const current = props.endpoint.exitNode?.stableID ?? "";

  const select = (stableID: string) => {
    void api.setTailscaleExitNode(props.endpoint.endpointTag, stableID).catch(showError);
    props.onClose();
  };

  const query = search.trim().toLowerCase();
  const filtered = props.candidates.filter(
    (peer) =>
      query === "" ||
      peerDisplayName(peer).toLowerCase().includes(query) ||
      peer.hostName.toLowerCase().includes(query) ||
      peer.dnsName.toLowerCase().includes(query) ||
      peer.tailscaleIPs.some((address) => address.includes(query)),
  );

  return (
    <Dialog onClose={props.onClose}>
      <h3>{t("Exit node")}</h3>
      <Field label={t("Search")}>
        <input
          className="input"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
      </Field>
      <button type="button" className="peer-row" onClick={() => select("")}>
        <span className="peer-name">{t("Disabled")}</span>
        {current === "" && (
          <span className="badges">
            <Icon name="check" size={14} />
          </span>
        )}
      </button>
      {filtered.map((peer) => (
        <button type="button" className="peer-row" key={peer.stableID} onClick={() => select(peer.stableID)}>
          <StateDot tone={peer.online ? "good" : undefined} />
          <span className="peer-name">{peerDisplayName(peer)}</span>
          <span className="peer-address">{peer.tailscaleIPs[0] ?? ""}</span>
          {current === peer.stableID && (
            <span className="badges">
              <Icon name="check" size={14} />
            </span>
          )}
        </button>
      ))}
    </Dialog>
  );
}
