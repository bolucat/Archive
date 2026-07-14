import { createContext, useContext, useEffect, useState } from "react";

import type { Code, Transport } from "@connectrpc/connect";

import { isTerminalCode, type StreamPhase, type StreamSnapshot } from "../api/stream";
import type { NetworkQualityTestProgress, STUNTestProgress } from "../gen/daemon/started_service_pb";
import type { PreferenceStorage } from "../lib/storage";
import { showError } from "./errorStore";
import { useLatestRef } from "./useLatest";

export type DaemonConnectionPhase =
  | "connecting"
  | "connected"
  | "owned-by-other-user"
  | "unavailable"
  | "not-installed"
  | "not-running"
  | "version-mismatch";

export interface DaemonConnectionState {
  phase: DaemonConnectionPhase;
  errorMessage?: string;
  daemonVersion?: string;
  bundledDaemonVersion?: string;
}

export type DesktopProfileType = "local" | "remote";

export interface DesktopProfile {
  id: string;
  name: string;
  type: DesktopProfileType;
  remoteUrl?: string;
  autoUpdate: boolean;
  autoUpdateIntervalMinutes: number;
  lastUpdated?: number;
}

export interface DesktopProfilesState {
  selectedId: string | null;
  profiles: DesktopProfile[];
}

export interface DesktopServer {
  id: string;
  name: string;
  url: string;
  secret: string;
}

export interface DesktopServersState {
  servers: DesktopServer[];
  activeId: string | null;
}

export interface DesktopProfileCreate {
  name: string;
  type: DesktopProfileType;
  content?: string;
  remoteUrl?: string;
  autoUpdate?: boolean;
  autoUpdateIntervalMinutes?: number;
}

export interface DesktopProfilePatch {
  name?: string;
  remoteUrl?: string;
  autoUpdate?: boolean;
  autoUpdateIntervalMinutes?: number;
}

export interface DesktopCrashReport {
  name: string;
  crashedAt: number;
  isRead: boolean;
}

export interface DesktopCrashReportFile {
  name: string;
  content: string;
  isBinary: boolean;
}

export interface DesktopOOMReport {
  name: string;
  recordedAt: number;
  isRead: boolean;
}

export interface DesktopOOMReportFile {
  name: string;
  content: string;
  isProfile: boolean;
}

export interface DesktopCrashReportExportOptions {
  withConfiguration: boolean;
  withLog: boolean;
  encrypt: boolean;
}

export type DesktopSpeedMode = "disabled" | "enabled" | "unified";

export interface DesktopSettingsState {
  speedMode: DesktopSpeedMode;
  openAtLogin: boolean;
  trayEnabled: boolean;
  trayInBackground: boolean;
  oomKillerEnabled: boolean;
  oomMemoryLimitMB: number;
  oomKillerKillConnections: boolean;
}

export interface DesktopHost {
  platform: string;
  appVersion(): Promise<string>;
  transport: Transport;
  preferences: PreferenceStorage;
  daemon: {
    getState(): Promise<DaemonConnectionState>;
    onStateChanged(listener: (state: DaemonConnectionState) => void): () => void;
    retryConnection(): void;
  };
  setup: {
    repairInstall(): Promise<boolean>;
    repairStart(): Promise<boolean>;
  };
  service: {
    start(): Promise<void>;
    stop(): Promise<void>;
    takeOver(): Promise<void>;
  };
  servers: {
    load(): Promise<DesktopServersState>;
    save(state: DesktopServersState): Promise<void>;
  };
  configuration: {
    check(content: string): Promise<void>;
    format(content: string): Promise<string>;
  };
  tools: {
    startStandaloneNetworkQualityTest(
      request: { configURL: string; serial: boolean; http3: boolean; maxRuntimeSeconds: number },
      options: { signal: AbortSignal },
    ): AsyncIterable<NetworkQualityTestProgress>;
    startStandaloneSTUNTest(
      request: { server: string },
      options: { signal: AbortSignal },
    ): AsyncIterable<STUNTestProgress>;
  };
  systemProxy: {
    status(): Promise<{ available: boolean; enabled: boolean }>;
    setEnabled(enabled: boolean): Promise<void>;
  };
  core: {
    info(): Promise<{ version: string }>;
    workingDirectory(): Promise<{ path: string; size: number }>;
    destroyWorkingDirectory(): Promise<void>;
  };
  reports: {
    crash: {
      list(): Promise<DesktopCrashReport[]>;
      read(name: string): Promise<DesktopCrashReportFile[]>;
      markRead(name: string): Promise<void>;
      exportFile(name: string, options: DesktopCrashReportExportOptions): Promise<boolean>;
      remove(name: string): Promise<void>;
      removeAll(): Promise<void>;
    };
    oom: {
      list(): Promise<DesktopOOMReport[]>;
      read(name: string): Promise<DesktopOOMReportFile[]>;
      markRead(name: string): Promise<void>;
      exportFile(name: string, options: DesktopCrashReportExportOptions): Promise<boolean>;
      remove(name: string): Promise<void>;
      removeAll(): Promise<void>;
    };
    triggerDebugCrash(type: "go" | "native"): Promise<void>;
    triggerAppCrash(type: "js" | "native"): Promise<void>;
    triggerOOMReport(): Promise<void>;
  };
  profiles: {
    list(): Promise<DesktopProfilesState>;
    onChanged(listener: () => void): () => void;
    create(init: DesktopProfileCreate): Promise<void>;
    updateMetadata(id: string, patch: DesktopProfilePatch): Promise<void>;
    remove(id: string): Promise<void>;
    reorder(ids: string[]): Promise<void>;
    select(id: string): Promise<void>;
    readContent(id: string): Promise<string>;
    writeContent(id: string, content: string): Promise<void>;
    updateRemote(id: string): Promise<void>;
    pickImportFile(): Promise<{ fileName: string; data: Uint8Array } | null>;
    exportFile(id: string): Promise<boolean>;
    importData(fileName: string, data: Uint8Array): Promise<void>;
    decodeData(data: Uint8Array): Promise<{ name: string }>;
    exportData(id: string): Promise<boolean>;
    encodeData(id: string): Promise<Uint8Array>;
  };
  settings: {
    get(): Promise<DesktopSettingsState>;
    setSpeedMode(mode: DesktopSpeedMode): Promise<void>;
    setOpenAtLogin(value: boolean): Promise<void>;
    setTrayEnabled(value: boolean): Promise<void>;
    setTrayInBackground(value: boolean): Promise<void>;
    setOOMKillerEnabled(value: boolean): Promise<void>;
    setOOMMemoryLimitMB(value: number): Promise<void>;
    setOOMKillerKillConnections(value: boolean): Promise<void>;
    cacheSize(): Promise<number>;
    clearCache(): Promise<void>;
  };
  // window.close() destroys sandboxed renderer webContents without emitting the host window's close event.
  application: {
    showMainWindow(): void;
    closeTrayMenu(): void;
    quit(): void;
  };
  onImportRemoteProfile(listener: (request: { name: string; url: string }) => void): () => void;
  onImportProfileFile(listener: (request: { fileName: string; data: Uint8Array }) => void): () => void;
}

export const DesktopHostContext = createContext<DesktopHost | null>(null);

export function useDesktopHost(): DesktopHost | null {
  return useContext(DesktopHostContext);
}

export const DesktopLocalContext = createContext(false);

export function useLocalDesktopHost(): DesktopHost | null {
  const host = useContext(DesktopHostContext);
  const local = useContext(DesktopLocalContext);
  return local ? host : null;
}

export function useDaemonConnection(host: DesktopHost): DaemonConnectionState {
  const [state, setState] = useState<DaemonConnectionState>({ phase: "connecting" });

  useEffect(() => {
    let stale = false;
    let pushed = false;
    const unsubscribe = host.daemon.onStateChanged((value) => {
      pushed = true;
      setState(() => value);
    });
    host.daemon
      .getState()
      .then((value) => {
        if (!stale && !pushed) {
          setState(() => value);
        }
      })
      .catch(() => {});
    return () => {
      stale = true;
      unsubscribe();
    };
  }, [host]);

  return state;
}

const REMOTE_RECONNECT_ATTEMPTS = 3;
const REMOTE_STABLE_CONNECTION_MS = 5000;

export interface RemoteSessionFailure {
  hadConnected: boolean;
  message: string;
}

export class RemoteSessionMonitor {
  private hadConnected = false;
  private cycleActive = false;
  private connectedAt = 0;
  private attempts = 0;

  constructor(private onEnd: (failure: RemoteSessionFailure) => void) {}

  update(phase: StreamPhase, errorMessage: string | undefined, errorCode: Code | undefined) {
    if (phase === "active") {
      if (!this.cycleActive) {
        this.cycleActive = true;
        this.hadConnected = true;
        this.connectedAt = Date.now();
      }
      return;
    }
    if (phase !== "error") {
      return;
    }
    const established = this.cycleActive;
    this.cycleActive = false;
    if (established && !isTerminalCode(errorCode)) {
      if (Date.now() - this.connectedAt >= REMOTE_STABLE_CONNECTION_MS) {
        this.attempts = 0;
      }
      if (this.attempts < REMOTE_RECONNECT_ATTEMPTS) {
        this.attempts += 1;
        return;
      }
    }
    this.onEnd({ hadConnected: this.hadConnected, message: errorMessage ?? "" });
  }
}

export function useRemoteSession(
  monitor: StreamSnapshot<unknown> | null,
  onEnd: (failure: RemoteSessionFailure) => void,
) {
  const onEndRef = useLatestRef(onEnd);
  const [session] = useState(() => new RemoteSessionMonitor((failure) => onEndRef.current(failure)));
  const phase = monitor?.phase ?? null;
  const errorMessage = monitor?.error;
  const errorCode = monitor?.errorCode;

  useEffect(() => {
    if (phase !== null) {
      session.update(phase, errorMessage, errorCode);
    }
  }, [session, phase, errorMessage, errorCode]);
}

export function useDesktopProfiles(host: DesktopHost): DesktopProfilesState {
  const [state, setState] = useState<DesktopProfilesState>({ selectedId: null, profiles: [] });

  useEffect(() => {
    let stale = false;
    let sequence = 0;
    const reload = () => {
      const token = ++sequence;
      host.profiles
        .list()
        .then((value) => {
          if (!stale && token === sequence) {
            setState(() => value);
          }
        })
        .catch(showError);
    };
    reload();
    const unsubscribe = host.profiles.onChanged(reload);
    return () => {
      stale = true;
      unsubscribe();
    };
  }, [host]);

  return state;
}
