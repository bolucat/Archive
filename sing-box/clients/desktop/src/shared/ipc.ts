export const DAEMON_UNARY = "daemon:unary";
export const DAEMON_STREAM_OPEN = "daemon:stream-open";
export const DAEMON_STREAM_SEND = "daemon:stream-send";
export const DAEMON_STREAM_END = "daemon:stream-end";
export const DAEMON_STREAM_CANCEL = "daemon:stream-cancel";
export const DAEMON_STREAM_EVENT = "daemon:stream-event";
export const DAEMON_STATE_GET = "daemon:state-get";
export const DAEMON_STATE_CHANGED = "daemon:state-changed";
export const DAEMON_RETRY = "daemon:retry";

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

export interface BridgeError {
  code: number;
  message: string;
}

export type UnaryResult = { ok: true; data: Uint8Array } | { ok: false; error: BridgeError };

export type StreamEvent =
  | { id: number; type: "message"; data: Uint8Array }
  | { id: number; type: "end"; error?: BridgeError };

export interface DaemonBridge {
  unary(
    service: string,
    method: string,
    header: [string, string][],
    request: Uint8Array,
  ): Promise<UnaryResult>;
  streamOpen(id: number, service: string, method: string, header: [string, string][]): void;
  streamSend(id: number, request: Uint8Array): void;
  streamEnd(id: number): void;
  streamCancel(id: number): void;
  onStreamEvent(listener: (event: StreamEvent) => void): () => void;
  getState(): Promise<DaemonConnectionState>;
  retryConnection(): void;
  onStateChanged(listener: (state: DaemonConnectionState) => void): () => void;
}

export const PROFILES_CALL = "profiles:call";
export const PROFILES_CHANGED = "profiles:changed";
export const SERVERS_CALL = "servers:call";
export const PREFERENCES_CALL = "preferences:call";
export const PREFERENCES_CHANGED = "preferences:changed";
export const PREFERENCES_SNAPSHOT = "preferences:snapshot";

export const TERMINAL_WINDOW_OPEN = "terminal:window-open";
export const TERMINAL_WINDOW_CLOSE = "terminal:window-close";
export const TERMINAL_CLIPBOARD_READ = "terminal:clipboard-read";
export const TERMINAL_CLIPBOARD_WRITE = "terminal:clipboard-write";
export const TERMINAL_CONTEXT_MENU = "terminal:context-menu";

export const PROFILE_EDITOR_WINDOW_OPEN = "profile-editor:window-open";
export const PROFILE_EDITOR_WINDOW_CLOSE = "profile-editor:window-close";
export const PROFILE_EDITOR_WINDOW_DIRTY = "profile-editor:window-dirty";
export const PROFILE_EDITOR_WINDOW_CLOSE_REQUESTED = "profile-editor:window-close-requested";

export type TerminalContextMenuResult =
  | { action: "copy" }
  | { action: "paste"; text: string }
  | null;

export interface TerminalBridge {
  openWindow(route: string): Promise<void>;
  closeWindow(): void;
  readClipboardText(): Promise<string>;
  writeClipboardText(text: string): Promise<void>;
  openContextMenu(selectionText: string): Promise<TerminalContextMenuResult>;
}

export interface ProfileEditorBridge {
  openWindow(profileId: string, readOnly: boolean): Promise<void>;
  closeWindow(): void;
  setDirty(dirty: boolean): void;
  onCloseRequested(listener: () => void): () => void;
}

export const OPENCONNECT_BROWSER_AUTHENTICATE = "openconnect-browser:authenticate";
export const OPENCONNECT_BROWSER_CANCEL = "openconnect-browser:cancel";

export interface OpenConnectBrowserRequest {
  url: string;
  finalURL: string;
  cookieNames: string[];
  earlyCookieNames: string[];
  headerNames: string[];
  callbackURLPrefixes: string[];
}

export interface OpenConnectBrowserResult {
  finalURL: string;
  cookies: Array<{ name: string; value: string }>;
  headers: Array<{ name: string; values: string[] }>;
}

export interface OpenConnectBrowserBridge {
  authenticate(
    browserSessionID: string,
    storageID: string,
    request: OpenConnectBrowserRequest,
  ): Promise<OpenConnectBrowserResult | null>;
  cancel(browserSessionID: string): void;
}

export type ProfilesResult = { ok: true; value: unknown } | { ok: false; error: string };

export type ProfileType = "local" | "remote";

export interface ProfileMetadata {
  id: string;
  name: string;
  type: ProfileType;
  remoteUrl?: string;
  autoUpdate: boolean;
  autoUpdateIntervalMinutes: number;
  lastUpdated?: number;
}

export interface ProfilesState {
  selectedId: string | null;
  profiles: ProfileMetadata[];
}

export interface ProfileCreate {
  name: string;
  type: ProfileType;
  content?: string;
  remoteUrl?: string;
  autoUpdate?: boolean;
  autoUpdateIntervalMinutes?: number;
}

export interface ProfileMetadataPatch {
  name?: string;
  remoteUrl?: string;
  autoUpdate?: boolean;
  autoUpdateIntervalMinutes?: number;
}

export interface ProfilesBridge {
  list(): Promise<ProfilesState>;
  create(init: ProfileCreate): Promise<ProfileMetadata>;
  updateMetadata(id: string, patch: ProfileMetadataPatch): Promise<void>;
  remove(id: string): Promise<void>;
  reorder(ids: string[]): Promise<void>;
  select(id: string): Promise<void>;
  readContent(id: string): Promise<string>;
  writeContent(id: string, content: string): Promise<void>;
  updateRemote(id: string): Promise<void>;
  startService(): Promise<void>;
  takeOverService(): Promise<void>;
  pickImportFile(): Promise<{ fileName: string; data: Uint8Array } | null>;
  exportFile(id: string): Promise<boolean>;
  importData(fileName: string, data: Uint8Array): Promise<void>;
  decodeData(data: Uint8Array): Promise<{ name: string }>;
  exportData(id: string): Promise<boolean>;
  encodeData(id: string): Promise<Uint8Array>;
  onChanged(listener: () => void): () => void;
}

export interface ServersBridge {
  load(): Promise<{ servers: Array<{ id: string; name: string; url: string; secret: string }>; activeId: string | null }>;
  save(state: { servers: Array<{ id: string; name: string; url: string; secret: string }>; activeId: string | null }): Promise<void>;
}

export interface PreferencesBridge {
  initial: Record<string, unknown>;
  set(name: string, value: unknown): Promise<void>;
  remove(name: string): Promise<void>;
  onChanged(listener: (name: string, value?: unknown) => void): () => void;
}

export const SETUP_CALL = "setup:call";

export interface SetupBridge {
  repairInstall(): Promise<boolean>;
  repairStart(): Promise<boolean>;
}

export const CORE_CALL = "core:call";

export interface WorkingDirectoryInfo {
  path: string;
  size: number;
}

export interface CoreInfo {
  version: string;
}

export interface CoreSecuritySettings {
  available: boolean;
  insecureModeEnabled: boolean;
}

export interface CoreBridge {
  info(): Promise<CoreInfo>;
  securitySettings(): Promise<CoreSecuritySettings>;
  setInsecureModeEnabled(enabled: boolean): Promise<void>;
  workingDirectory(): Promise<WorkingDirectoryInfo>;
  destroyWorkingDirectory(): Promise<void>;
}

export const REPORTS_CALL = "reports:call";

export interface CrashReportEntry {
  name: string;
  crashedAt: number;
  isRead: boolean;
}

export interface CrashReportFile {
  name: string;
  content: string;
  isBinary: boolean;
}

export interface CrashReportExportOptions {
  withConfiguration: boolean;
  withLog: boolean;
  encrypt: boolean;
}

export interface ReportArchive {
  fileName: string;
  data: Uint8Array;
  mediaType: string;
}

export interface OOMReportEntry {
  name: string;
  recordedAt: number;
  isRead: boolean;
}

export interface OOMReportFile {
  name: string;
  content: string;
  isProfile: boolean;
}

export interface ReportsBridge {
  list(): Promise<CrashReportEntry[]>;
  read(name: string): Promise<CrashReportFile[]>;
  markRead(name: string): Promise<void>;
  exportFile(name: string, options?: CrashReportExportOptions): Promise<boolean>;
  createArchive(name: string, options?: CrashReportExportOptions): Promise<ReportArchive>;
  remove(name: string): Promise<void>;
  removeAll(): Promise<void>;
  oomList(): Promise<OOMReportEntry[]>;
  oomRead(name: string): Promise<OOMReportFile[]>;
  oomMarkRead(name: string): Promise<void>;
  oomExportFile(name: string, options?: CrashReportExportOptions): Promise<boolean>;
  oomCreateArchive(name: string, options?: CrashReportExportOptions): Promise<ReportArchive>;
  oomRemove(name: string): Promise<void>;
  oomRemoveAll(): Promise<void>;
  powerList(): Promise<OOMReportEntry[]>;
  powerRead(name: string): Promise<OOMReportFile[]>;
  powerMarkRead(name: string): Promise<void>;
  powerExportFile(name: string, options?: CrashReportExportOptions): Promise<boolean>;
  powerCreateArchive(name: string, options?: CrashReportExportOptions): Promise<ReportArchive>;
  powerRemove(name: string): Promise<void>;
  powerRemoveAll(): Promise<void>;
  triggerAppCrash(type: "js" | "native"): Promise<void>;
}

export const SETTINGS_CALL = "settings:call";
export const APP_CALL = "app:call";
export const APP_TITLE_BAR_OVERLAY = "app:title-bar-overlay";
export const DEEP_LINK_IMPORT = "app:deep-link-import";
export const PROFILE_FILE_IMPORT = "app:profile-file-import";
export const TAILDROP_SEND_REQUEST = "app:taildrop-send-request";
export const APP_NAVIGATE = "app:navigate";
export const TAILDROP_DOWNLOAD = "taildrop:download";
export const TAILDROP_SEND_START = "taildrop:send-start";
export const TAILDROP_SEND_CANCEL = "taildrop:send-cancel";
export const TAILDROP_SEND_EVENT = "taildrop:send-event";
export const TAILDROP_DOWNLOAD_PROGRESS = "taildrop:download-progress";

export interface TitleBarOverlayColors {
  color: string;
  symbolColor: string;
}

export type TraySpeedMode = "disabled" | "enabled" | "unified";

export interface DesktopSettings {
  speedMode: TraySpeedMode;
  openAtLogin: boolean;
  trayEnabled: boolean;
  trayInBackground: boolean;
  oomKillerEnabled: boolean;
  oomMemoryLimitMB: number;
  oomKillerKillConnections: boolean;
  powerReportEnabled: boolean;
}

export interface SettingsBridge {
  get(): Promise<DesktopSettings>;
  setSpeedMode(mode: TraySpeedMode): Promise<void>;
  setOpenAtLogin(value: boolean): Promise<void>;
  setTrayEnabled(value: boolean): Promise<void>;
  setTrayInBackground(value: boolean): Promise<void>;
  setOOMKillerEnabled(value: boolean): Promise<void>;
  setOOMMemoryLimitMB(value: number): Promise<void>;
  setOOMKillerKillConnections(value: boolean): Promise<void>;
  setPowerReportEnabled(value: boolean): Promise<void>;
  cacheSize(): Promise<number>;
  clearCache(): Promise<void>;
}

export const UPDATES_CALL = "updates:call";
export const UPDATES_STATE_CHANGED = "updates:state-changed";
export const UPDATES_PRESENT = "updates:present";

export type UpdateTrack = "stable" | "beta";
export type UpdateInstallResult = "started" | "signer-mismatch" | "not-newer";

export interface AppUpdateInfo {
  versionName: string;
  releaseURL: string;
  downloadURL: string;
  releaseNotes: string;
  isPrerelease: boolean;
  fileSize: number;
}

export interface UpdatesState {
  supported: boolean;
  track: UpdateTrack;
  checkUpdateEnabled: boolean;
  prompted: boolean;
  info: AppUpdateInfo | null;
  checking: boolean;
  downloading: boolean;
  installing: boolean;
  downloadProgress: number;
}

export interface UpdatesBridge {
  state(): Promise<UpdatesState>;
  check(): Promise<AppUpdateInfo | null>;
  getGitHubToken(): Promise<string>;
  setGitHubToken(value: string): Promise<void>;
  downloadAndInstall(): Promise<UpdateInstallResult>;
  installWithElevation(): Promise<boolean>;
  setTrack(track: UpdateTrack): Promise<void>;
  setCheckUpdateEnabled(value: boolean): Promise<void>;
  setPrompted(): Promise<void>;
  markShown(): Promise<void>;
  onStateChanged(listener: (state: UpdatesState) => void): () => void;
  onPresentRequested(listener: () => void): () => void;
}

export interface DeepLinkImport {
  name: string;
  url: string;
}

export interface ProfileFileImport {
  fileName: string;
  data: Uint8Array;
}

export interface TaildropSendFile {
  name: string;
  size: number;
  path: string;
}

export interface TaildropSendRequest {
  endpointTag: string;
  peerStableID: string;
  files: TaildropSendFile[];
}

export type TaildropSendEvent =
  | {
      sessionID: number;
      type: "progress";
      fileIndex: number;
      sentBytes: number;
      fileCompleted: boolean;
    }
  | { sessionID: number; type: "finished"; code: number; error: string };

export type TaildropDownloadAction = "save" | "open" | "share";

export interface TaildropDownloadRequest {
  downloadID: number;
  endpointTag: string;
  name: string;
  action: TaildropDownloadAction;
}

export interface TaildropDownloadProgress {
  downloadID: number;
  transferred: number;
  size: number;
}

export interface TaildropBridge {
  pathForFile(file: File): string;
  startSend(sessionID: number, request: TaildropSendRequest): void;
  cancelSend(sessionID: number): void;
  onSendEvent(listener: (event: TaildropSendEvent) => void): () => void;
  download(request: TaildropDownloadRequest): Promise<boolean>;
  onDownloadProgress(listener: (progress: TaildropDownloadProgress) => void): () => void;
}

export interface AppBridge {
  version(): Promise<string>;
  shareFile(fileName: string, data: Uint8Array): Promise<void>;
  showMainWindow(): Promise<void>;
  closeTrayMenu(): Promise<void>;
  quit(): Promise<void>;
  setTitleBarOverlay(colors: TitleBarOverlayColors): void;
  onDeepLinkImport(listener: (request: DeepLinkImport) => void): () => void;
  onProfileFileImport(listener: (request: ProfileFileImport) => void): () => void;
  onTaildropSendRequested(listener: (files: TaildropSendFile[]) => void): () => void;
  onNavigate(listener: (route: string) => void): () => void;
}

export interface DesktopBridge {
  platform: string;
  daemon: DaemonBridge;
  setup: SetupBridge;
  core: CoreBridge;
  reports: ReportsBridge;
  profiles: ProfilesBridge;
  servers: ServersBridge;
  preferences: PreferencesBridge;
  terminal: TerminalBridge;
  profileEditor: ProfileEditorBridge;
  openConnectBrowser: OpenConnectBrowserBridge;
  settings: SettingsBridge;
  updates: UpdatesBridge;
  taildrop: TaildropBridge;
  app: AppBridge;
}
