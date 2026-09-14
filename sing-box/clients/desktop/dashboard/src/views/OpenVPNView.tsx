import { useEffect, useId, useState, type FormEvent } from "react";

import { formatRelativeTime, isHttpUrl, type DelayTone } from "../api/format";
import { useStream } from "../api/stream";
import { useApi, useNow } from "../app/context";
import { showError } from "../app/errorStore";
import { useI18n } from "../app/i18n";
import { StreamStates } from "../components/StreamBanner";
import {
  Button,
  Card,
  CopyValue,
  DataLine,
  Dialog,
  QRCode,
  Spinner,
  StateDot,
} from "../components/ui";
import type {
  OpenVPNChallenge,
  OpenVPNTunnelInfo,
} from "../gen/daemon/started_service_pb";
import { cx } from "../lib/cx";
import { ToolsPageHeader } from "./ToolsView";
import styles from "./OpenVPNView.module.css";

export function OpenVPNView(props: { tag: string }) {
  const api = useApi();
  const { t } = useI18n();
  const status = useStream(api.openVPN);
  const endpoint = status.data.endpoints.find(
    (entry) => entry.endpointTag === props.tag,
  );
  const challenge =
    endpoint?.state === "auth-pending" ? endpoint.challenge : undefined;

  const endpointError = endpoint?.error ?? "";
  useEffect(() => {
    if (endpointError !== "") {
      showError(endpointError);
    }
  }, [endpointError]);

  return (
    <div className="page">
      <ToolsPageHeader
        title={
          status.data.endpoints.length > 1 && props.tag !== ""
            ? `OpenVPN: ${props.tag}`
            : "OpenVPN"
        }
      />
      <StreamStates
        snapshot={status}
        loaded={status.data.loaded}
        empty={!endpoint}
        emptyIcon="route"
        emptyMessage={t("Endpoint not found")}
      />
      {endpoint && (
        <div className="settings-stack">
          <div>
            <div className="list-section-title">{t("Status")}</div>
            <div className="detail-card">
              <DataLine
                label={t("State")}
                value={
                  <span className={styles.stateValue}>
                    <StateDot tone={STATE_TONES[endpoint.state] ?? "neutral"} />
                    {endpoint.stateText}
                  </span>
                }
              />
              {endpoint.state === "connected" && endpoint.tunnelInfo && (
                <TunnelDetails info={endpoint.tunnelInfo} />
              )}
            </div>
          </div>
          {challenge &&
            (challenge.kind === "open-url" ||
              challenge.kind === "credentials" ||
              challenge.kind === "secret") && (
              <div>
                <div className="list-section-title">{t("Authentication")}</div>
                <Card className={styles.authCard}>
                  {challenge.kind === "open-url" ? (
                    <AuthURLActions url={challenge.url} />
                  ) : (
                    <ChallengeForm
                      key={challenge.id}
                      endpointTag={endpoint.endpointTag}
                      challenge={challenge}
                    />
                  )}
                </Card>
              </div>
            )}
        </div>
      )}
    </div>
  );
}

const STATE_TONES: Record<string, DelayTone> = {
  connecting: "medium",
  "auth-pending": "bad",
  connected: "good",
  error: "bad",
};

function AuthURLActions(props: { url: string }) {
  const { t } = useI18n();
  const [qrOpen, setQROpen] = useState(false);
  return (
    <>
      <div className={styles.actions}>
        {isHttpUrl(props.url) && (
          <Button href={props.url} target="_blank" rel="noreferrer">
            {t("Open auth URL")}
          </Button>
        )}
        <Button onClick={() => setQROpen(true)}>
          {t("Show auth URL QR code")}
        </Button>
      </div>
      {qrOpen && (
        <Dialog onClose={() => setQROpen(false)}>
          <h3>{t("Auth URL")}</h3>
          <QRCode value={props.url} />
          <CopyValue value={props.url} className={styles.qrCopy} />
        </Dialog>
      )}
    </>
  );
}

function TunnelDetails(props: { info: OpenVPNTunnelInfo }) {
  const { t, language } = useI18n();
  const now = useNow(30_000);
  const info = props.info;
  const ipv4 = info.ipv4.filter(Boolean).join(", ");
  const ipv6 = info.ipv6.filter(Boolean).join(", ");
  const dns = info.dns.filter(Boolean).join(", ");
  return (
    <>
      {info.server && <DataLine label={t("Server")} value={info.server} mono />}
      {info.network && (
        <DataLine label={t("Network")} value={info.network} mono />
      )}
      {info.cipher && <DataLine label={t("Cipher")} value={info.cipher} mono />}
      {ipv4 && <DataLine label={t("IPv4")} value={ipv4} mono />}
      {ipv6 && <DataLine label={t("IPv6")} value={ipv6} mono />}
      {dns && <DataLine label={t("DNS")} value={dns} mono />}
      {info.mtu > 0 && (
        <DataLine label={t("MTU")} value={String(info.mtu)} mono />
      )}
      {info.connectedSince > 0n && (
        <DataLine
          label={t("Connected")}
          value={formatRelativeTime(
            Number(info.connectedSince) * 1000,
            now,
            language,
          )}
        />
      )}
    </>
  );
}

function ChallengeForm(props: {
  endpointTag: string;
  challenge: OpenVPNChallenge;
}) {
  const api = useApi();
  const { t } = useI18n();
  const idPrefix = useId();
  const [phase, setPhase] = useState<"idle" | "submitting" | "submitted">(
    "idle",
  );
  const [username, setUsername] = useState(props.challenge.username);
  const [password, setPassword] = useState("");
  const [secret, setSecret] = useState("");
  const now = useNow();
  const deadline = Number(props.challenge.deadline) * 1000;
  const expired = deadline > 0 && deadline <= now;
  const submitting = phase === "submitting";
  const kind = props.challenge.kind;

  const previousError = props.challenge.previousError;
  useEffect(() => {
    if (previousError !== "") {
      showError(previousError);
    }
  }, [previousError]);

  if (phase === "submitted") {
    return (
      <div className={styles.verifying} role="status">
        <Spinner />
        {t("Verifying")}
      </div>
    );
  }

  const submit = (event: FormEvent) => {
    event.preventDefault();
    setPhase("submitting");
    api.client
      .submitOpenVPNChallengeResponse({
        endpointTag: props.endpointTag,
        challengeID: props.challenge.id,
        username: kind === "credentials" ? username : "",
        password: kind === "credentials" ? password : "",
        secret,
      })
      .then(() => setPhase("submitted"))
      .catch((error: unknown) => {
        setPhase("idle");
        showError(error);
      });
  };

  return (
    <form className={styles.authForm} onSubmit={submit}>
      {props.challenge.message && (
        <p className={styles.message}>{props.challenge.message}</p>
      )}
      {deadline > 0 && <Deadline deadline={deadline} now={now} />}
      {kind === "credentials" && (
        <>
          <div className="field">
            <label htmlFor={`${idPrefix}-username`}>{t("Username")}</label>
            <input
              id={`${idPrefix}-username`}
              className="input"
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              autoComplete="off"
              autoFocus={!username}
              disabled={submitting || expired}
            />
          </div>
          <div className="field">
            <label htmlFor={`${idPrefix}-password`}>{t("Password")}</label>
            <input
              id={`${idPrefix}-password`}
              className="input"
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              autoComplete="off"
              autoFocus={Boolean(username)}
              disabled={submitting || expired}
            />
          </div>
          {props.challenge.secretMessage && (
            <div className="field">
              <label htmlFor={`${idPrefix}-secret`}>
                {props.challenge.secretMessage}
              </label>
              <input
                id={`${idPrefix}-secret`}
                className="input"
                type={props.challenge.echo ? "text" : "password"}
                value={secret}
                onChange={(event) => setSecret(event.target.value)}
                autoComplete="off"
                disabled={submitting || expired}
              />
            </div>
          )}
        </>
      )}
      {kind === "secret" && (
        <div className="field">
          <label htmlFor={`${idPrefix}-response`}>
            {props.challenge.secretMessage || t("Response")}
          </label>
          <input
            id={`${idPrefix}-response`}
            className="input"
            type={props.challenge.echo ? "text" : "password"}
            value={secret}
            onChange={(event) => setSecret(event.target.value)}
            autoComplete="off"
            autoFocus
            disabled={submitting || expired}
          />
        </div>
      )}
      <div className={styles.actions}>
        <Button
          variant="primary"
          type="submit"
          disabled={submitting || expired}
        >
          {submitting && <Spinner />}
          {t("Submit")}
        </Button>
      </div>
    </form>
  );
}

function Deadline(props: { deadline: number; now: number }) {
  const seconds = Math.max(0, Math.ceil((props.deadline - props.now) / 1000));
  const value = `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;
  return (
    <div className={cx(styles.deadline, seconds === 0 && styles.expired)}>
      {value}
    </div>
  );
}
