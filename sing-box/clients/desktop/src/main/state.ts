import { ConnectError } from "@connectrpc/connect";
import { EventEmitter } from "node:events";

import { ServiceStatus_Type } from "../shared/gen/daemon/started_service_pb";
import type { Group } from "../shared/gen/daemon/started_service_pb";
import { DaemonOwnership } from "../shared/gen/experimental/boxdd/desktop_service_pb";
import type { DaemonConnectionState } from "../shared/ipc";
import { desktopService, startedService } from "./daemon";
import { bundledDaemonVersion, probeService } from "./repair";

const RECONNECT_DELAY = 3000;
const HANDSHAKE_TIMEOUT = 3000;
const MAXIMUM_BACKOFF = 5000;
const SESSION_RESTART_DELAY = 1000;

function sleep(durationMs: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, durationMs));
}

class DaemonState extends EventEmitter {
  connection: DaemonConnectionState = { phase: "connecting" };
  status: ServiceStatus_Type | null = null;
  groups: Group[] = [];

  private started = false;
  private wakeRetry: (() => void) | null = null;

  start() {
    if (this.started) {
      return;
    }
    this.started = true;
    if (desktopService === null || startedService === null) {
      this.setConnection({ phase: "unavailable", errorMessage: "daemon socket is not configured" });
      return;
    }
    void this.loopConnection();
  }

  retryConnection() {
    this.wakeRetry?.();
  }

  private setConnection(connection: DaemonConnectionState) {
    this.connection = connection;
    this.emit("connection");
    this.emit("change");
  }

  private interruptibleSleep(durationMs: number): Promise<void> {
    return new Promise((resolve) => {
      const timer = setTimeout(() => {
        this.wakeRetry = null;
        resolve();
      }, durationMs);
      this.wakeRetry = () => {
        clearTimeout(timer);
        this.wakeRetry = null;
        resolve();
      };
    });
  }

  private async loopConnection(): Promise<void> {
    let attempt = 0;
    for (;;) {
      try {
        const info = await desktopService!.getDaemonInfo({}, { timeoutMs: HANDSHAKE_TIMEOUT });
        const bundledVersion = await bundledDaemonVersion();
        if (bundledVersion !== null && info.version !== bundledVersion) {
          this.setConnection({
            phase: "version-mismatch",
            daemonVersion: info.version,
            bundledDaemonVersion: bundledVersion,
          });
          attempt = 0;
          await this.interruptibleSleep(RECONNECT_DELAY);
          continue;
        }
        if (info.ownership === DaemonOwnership.AVAILABLE) {
          await desktopService!.claimService({}, { timeoutMs: HANDSHAKE_TIMEOUT });
          continue;
        }
        if (info.ownership === DaemonOwnership.OTHER) {
          this.status = null;
          this.groups = [];
          this.setConnection({
            phase: "owned-by-other-user",
            daemonVersion: info.version,
          });
          attempt = 0;
          await this.interruptibleSleep(RECONNECT_DELAY);
          continue;
        }
        if (info.ownership !== DaemonOwnership.CALLER) {
          throw new Error("daemon returned an invalid ownership state");
        }
        attempt = 0;
        this.setConnection({ phase: "connected", daemonVersion: info.version });
      } catch (error) {
        attempt++;
        const probe = await probeService();
        if (probe === "not-installed" || probe === "not-running") {
          this.setConnection({ phase: probe });
        } else {
          this.setConnection({
            phase: "unavailable",
            errorMessage: ConnectError.from(error).rawMessage,
          });
        }
        await this.interruptibleSleep(Math.min(1000 * attempt, MAXIMUM_BACKOFF));
        continue;
      }
      const session = new AbortController();
      void this.loopGroups(session.signal);
      try {
        for await (const message of startedService!.subscribeServiceStatus(
          {},
          { signal: session.signal },
        )) {
          this.status = message.status ?? ServiceStatus_Type.IDLE;
          this.emit("change");
        }
      } catch {}
      session.abort();
      this.status = null;
      this.groups = [];
      this.emit("change");
      await sleep(SESSION_RESTART_DELAY);
    }
  }

  private async loopGroups(signal: AbortSignal): Promise<void> {
    while (!signal.aborted) {
      try {
        for await (const message of startedService!.subscribeGroups({}, { signal })) {
          this.groups = message.group;
          this.emit("change");
        }
      } catch {}
      if (this.groups.length > 0) {
        this.groups = [];
        this.emit("change");
      }
      if (!signal.aborted) {
        await sleep(RECONNECT_DELAY);
      }
    }
  }
}

export const daemonState = new DaemonState();
