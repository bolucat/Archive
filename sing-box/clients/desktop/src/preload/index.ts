import { contextBridge, ipcRenderer, webUtils } from "electron";
import type { IpcRendererEvent } from "electron";

import {
  APP_CALL,
  APP_NAVIGATE,
  APP_TITLE_BAR_OVERLAY,
  CORE_CALL,
  DAEMON_RETRY,
  DAEMON_STATE_CHANGED,
  DAEMON_STATE_GET,
  DAEMON_STREAM_CANCEL,
  DAEMON_STREAM_END,
  DAEMON_STREAM_EVENT,
  DAEMON_STREAM_OPEN,
  DAEMON_STREAM_SEND,
  DAEMON_UNARY,
  DEEP_LINK_IMPORT,
  OPENCONNECT_BROWSER_AUTHENTICATE,
  OPENCONNECT_BROWSER_CANCEL,
  PREFERENCES_CALL,
  PREFERENCES_CHANGED,
  PREFERENCES_SNAPSHOT,
  PROFILE_EDITOR_WINDOW_CLOSE,
  PROFILE_EDITOR_WINDOW_CLOSE_REQUESTED,
  PROFILE_EDITOR_WINDOW_DIRTY,
  PROFILE_EDITOR_WINDOW_OPEN,
  PROFILE_FILE_IMPORT,
  PROFILES_CALL,
  PROFILES_CHANGED,
  REPORTS_CALL,
  SERVERS_CALL,
  SETTINGS_CALL,
  SETUP_CALL,
  TAILDROP_DOWNLOAD,
  TAILDROP_DOWNLOAD_PROGRESS,
  TAILDROP_SEND_CANCEL,
  TAILDROP_SEND_EVENT,
  TAILDROP_SEND_REQUEST,
  TAILDROP_SEND_START,
  TERMINAL_CLIPBOARD_READ,
  TERMINAL_CLIPBOARD_WRITE,
  TERMINAL_CONTEXT_MENU,
  TERMINAL_WINDOW_CLOSE,
  TERMINAL_WINDOW_OPEN,
  UPDATES_CALL,
  UPDATES_PRESENT,
  UPDATES_STATE_CHANGED,
} from "../shared/ipc";
import type {
  DaemonConnectionState,
  DeepLinkImport,
  DesktopBridge,
  ProfileFileImport,
  ProfilesResult,
  StreamEvent,
  TaildropDownloadProgress,
  TaildropSendEvent,
  TaildropSendFile,
  UpdatesState,
} from "../shared/ipc";

async function callResult<T>(channel: string, method: string, ...callArguments: unknown[]): Promise<T> {
  const result = (await ipcRenderer.invoke(channel, method, ...callArguments)) as ProfilesResult;
  if (!result.ok) {
    throw new Error(result.error);
  }
  return result.value as T;
}

function callProfiles<T>(method: string, ...callArguments: unknown[]): Promise<T> {
  return callResult(PROFILES_CALL, method, ...callArguments);
}

function callSettings<T>(method: string, ...callArguments: unknown[]): Promise<T> {
  return callResult(SETTINGS_CALL, method, ...callArguments);
}

function callServers<T>(method: string, ...callArguments: unknown[]): Promise<T> {
  return callResult(SERVERS_CALL, method, ...callArguments);
}

function callSetup<T>(method: string): Promise<T> {
  return callResult(SETUP_CALL, method);
}

function callCore<T>(method: string, ...callArguments: unknown[]): Promise<T> {
  return callResult(CORE_CALL, method, ...callArguments);
}

function callReports<T>(method: string, ...callArguments: unknown[]): Promise<T> {
  return callResult(REPORTS_CALL, method, ...callArguments);
}

let updatePresentationPending = false;
const updatePresentationListeners = new Set<() => void>();

ipcRenderer.on(UPDATES_PRESENT, () => {
  if (updatePresentationListeners.size === 0) {
    updatePresentationPending = true;
    return;
  }
  for (const listener of updatePresentationListeners) {
    listener();
  }
});

const bridge: DesktopBridge = {
  platform: process.platform,
  daemon: {
    unary: (service, method, header, request) =>
      ipcRenderer.invoke(DAEMON_UNARY, service, method, header, request),
    streamOpen: (id, service, method, header) => {
      ipcRenderer.send(DAEMON_STREAM_OPEN, id, service, method, header);
    },
    streamSend: (id, request) => {
      ipcRenderer.send(DAEMON_STREAM_SEND, id, request);
    },
    streamEnd: (id) => {
      ipcRenderer.send(DAEMON_STREAM_END, id);
    },
    streamCancel: (id) => {
      ipcRenderer.send(DAEMON_STREAM_CANCEL, id);
    },
    onStreamEvent: (listener) => {
      const handler = (_event: IpcRendererEvent, payload: StreamEvent) => listener(payload);
      ipcRenderer.on(DAEMON_STREAM_EVENT, handler);
      return () => {
        ipcRenderer.removeListener(DAEMON_STREAM_EVENT, handler);
      };
    },
    getState: () => ipcRenderer.invoke(DAEMON_STATE_GET),
    retryConnection: () => {
      ipcRenderer.send(DAEMON_RETRY);
    },
    onStateChanged: (listener) => {
      const handler = (_event: IpcRendererEvent, state: DaemonConnectionState) => listener(state);
      ipcRenderer.on(DAEMON_STATE_CHANGED, handler);
      return () => {
        ipcRenderer.removeListener(DAEMON_STATE_CHANGED, handler);
      };
    },
  },
  setup: {
    repairInstall: () => callSetup("repairInstall"),
    repairStart: () => callSetup("repairStart"),
  },
  core: {
    info: () => callCore("info"),
    securitySettings: () => callCore("securitySettings"),
    setInsecureModeEnabled: (enabled) => callCore("setInsecureModeEnabled", enabled),
    workingDirectory: () => callCore("workingDirectory"),
    destroyWorkingDirectory: () => callCore("destroyWorkingDirectory"),
  },
  reports: {
    list: () => callReports("list"),
    read: (name) => callReports("read", name),
    markRead: (name) => callReports("markRead", name),
    exportFile: (name, options) => callReports("exportFile", name, options),
    createArchive: (name, options) => callReports("createArchive", name, options),
    remove: (name) => callReports("remove", name),
    removeAll: () => callReports("removeAll"),
    oomList: () => callReports("oomList"),
    oomRead: (name) => callReports("oomRead", name),
    oomMarkRead: (name) => callReports("oomMarkRead", name),
    oomExportFile: (name, options) => callReports("oomExportFile", name, options),
    oomCreateArchive: (name, options) => callReports("oomCreateArchive", name, options),
    oomRemove: (name) => callReports("oomRemove", name),
    oomRemoveAll: () => callReports("oomRemoveAll"),
    powerList: () => callReports("powerList"),
    powerRead: (name) => callReports("powerRead", name),
    powerMarkRead: (name) => callReports("powerMarkRead", name),
    powerExportFile: (name, options) => callReports("powerExportFile", name, options),
    powerCreateArchive: (name, options) => callReports("powerCreateArchive", name, options),
    powerRemove: (name) => callReports("powerRemove", name),
    powerRemoveAll: () => callReports("powerRemoveAll"),
    triggerAppCrash: (type) => callReports("triggerAppCrash", type),
  },
  profiles: {
    list: () => callProfiles("list"),
    create: (init) => callProfiles("create", init),
    updateMetadata: (id, patch) => callProfiles("updateMetadata", id, patch),
    remove: (id) => callProfiles("remove", id),
    reorder: (ids) => callProfiles("reorder", ids),
    select: (id) => callProfiles("select", id),
    readContent: (id) => callProfiles("readContent", id),
    writeContent: (id, content) => callProfiles("writeContent", id, content),
    updateRemote: (id) => callProfiles("updateRemote", id),
    startService: () => callProfiles("startService"),
    takeOverService: () => callProfiles("takeOverService"),
    pickImportFile: () => callProfiles("pickImportFile"),
    exportFile: (id) => callProfiles("exportFile", id),
    importData: (fileName, data) => callProfiles("importData", fileName, data),
    decodeData: (data) => callProfiles("decodeData", data),
    exportData: (id) => callProfiles("exportData", id),
    encodeData: (id) => callProfiles("encodeData", id),
    onChanged: (listener) => {
      const handler = () => listener();
      ipcRenderer.on(PROFILES_CHANGED, handler);
      return () => {
        ipcRenderer.removeListener(PROFILES_CHANGED, handler);
      };
    },
  },
  servers: {
    load: () => callServers("load"),
    save: (state) => callServers("save", state),
  },
  preferences: {
    initial: ipcRenderer.sendSync(PREFERENCES_SNAPSHOT) as Record<string, unknown>,
    set: (name, value) => callResult(PREFERENCES_CALL, "set", name, value),
    remove: (name) => callResult(PREFERENCES_CALL, "remove", name),
    onChanged: (listener) => {
      const handler = (_event: IpcRendererEvent, name: string, value?: unknown) =>
        listener(name, value);
      ipcRenderer.on(PREFERENCES_CHANGED, handler);
      return () => {
        ipcRenderer.removeListener(PREFERENCES_CHANGED, handler);
      };
    },
  },
  terminal: {
    openWindow: (route) => ipcRenderer.invoke(TERMINAL_WINDOW_OPEN, route),
    closeWindow: () => {
      ipcRenderer.send(TERMINAL_WINDOW_CLOSE);
    },
    readClipboardText: () => ipcRenderer.invoke(TERMINAL_CLIPBOARD_READ),
    writeClipboardText: (text) => ipcRenderer.invoke(TERMINAL_CLIPBOARD_WRITE, text),
    openContextMenu: (selectionText) =>
      ipcRenderer.invoke(TERMINAL_CONTEXT_MENU, selectionText),
  },
  profileEditor: {
    openWindow: (profileId, readOnly) =>
      ipcRenderer.invoke(PROFILE_EDITOR_WINDOW_OPEN, profileId, readOnly),
    closeWindow: () => {
      ipcRenderer.send(PROFILE_EDITOR_WINDOW_CLOSE);
    },
    setDirty: (dirty) => {
      ipcRenderer.send(PROFILE_EDITOR_WINDOW_DIRTY, dirty);
    },
    onCloseRequested: (listener) => {
      const handler = () => listener();
      ipcRenderer.on(PROFILE_EDITOR_WINDOW_CLOSE_REQUESTED, handler);
      return () => {
        ipcRenderer.removeListener(PROFILE_EDITOR_WINDOW_CLOSE_REQUESTED, handler);
      };
    },
  },
  openConnectBrowser: {
    authenticate: (browserSessionID, storageID, request) =>
      ipcRenderer.invoke(OPENCONNECT_BROWSER_AUTHENTICATE, browserSessionID, storageID, request),
    cancel: (browserSessionID) => ipcRenderer.send(OPENCONNECT_BROWSER_CANCEL, browserSessionID),
  },
  settings: {
    get: () => callSettings("get"),
    setSpeedMode: (mode) => callSettings("setSpeedMode", mode),
    setOpenAtLogin: (value) => callSettings("setOpenAtLogin", value),
    setTrayEnabled: (value) => callSettings("setTrayEnabled", value),
    setTrayInBackground: (value) => callSettings("setTrayInBackground", value),
    setOOMKillerEnabled: (value) => callSettings("setOOMKillerEnabled", value),
    setOOMMemoryLimitMB: (value) => callSettings("setOOMMemoryLimitMB", value),
    setOOMKillerKillConnections: (value) => callSettings("setOOMKillerKillConnections", value),
    setPowerReportEnabled: (value) => callSettings("setPowerReportEnabled", value),
    cacheSize: () => callSettings("cacheSize"),
    clearCache: () => callSettings("clearCache"),
  },
  updates: {
    state: () => callResult(UPDATES_CALL, "state"),
    check: () => callResult(UPDATES_CALL, "check"),
    getGitHubToken: () => callResult(UPDATES_CALL, "getGitHubToken"),
    setGitHubToken: (value) => callResult(UPDATES_CALL, "setGitHubToken", value),
    downloadAndInstall: () => callResult(UPDATES_CALL, "downloadAndInstall"),
    installWithElevation: () => callResult(UPDATES_CALL, "installWithElevation"),
    setTrack: (track) => callResult(UPDATES_CALL, "setTrack", track),
    setCheckUpdateEnabled: (value) =>
      callResult(UPDATES_CALL, "setCheckUpdateEnabled", value),
    setPrompted: () => callResult(UPDATES_CALL, "setPrompted"),
    markShown: () => callResult(UPDATES_CALL, "markShown"),
    onStateChanged: (listener) => {
      const handler = (_event: IpcRendererEvent, state: UpdatesState) => listener(state);
      ipcRenderer.on(UPDATES_STATE_CHANGED, handler);
      return () => {
        ipcRenderer.removeListener(UPDATES_STATE_CHANGED, handler);
      };
    },
    onPresentRequested: (listener) => {
      updatePresentationListeners.add(listener);
      if (updatePresentationPending) {
        updatePresentationPending = false;
        listener();
      }
      return () => {
        updatePresentationListeners.delete(listener);
      };
    },
  },
  taildrop: {
    pathForFile: (file) => webUtils.getPathForFile(file),
    startSend: (sessionID, request) => {
      ipcRenderer.send(TAILDROP_SEND_START, sessionID, request);
    },
    cancelSend: (sessionID) => {
      ipcRenderer.send(TAILDROP_SEND_CANCEL, sessionID);
    },
    onSendEvent: (listener) => {
      const handler = (_event: IpcRendererEvent, payload: TaildropSendEvent) => listener(payload);
      ipcRenderer.on(TAILDROP_SEND_EVENT, handler);
      return () => {
        ipcRenderer.removeListener(TAILDROP_SEND_EVENT, handler);
      };
    },
    download: async (request) => {
      const result = (await ipcRenderer.invoke(TAILDROP_DOWNLOAD, request)) as ProfilesResult;
      if (!result.ok) {
        throw new Error(result.error);
      }
      return result.value as boolean;
    },
    onDownloadProgress: (listener) => {
      const handler = (_event: IpcRendererEvent, payload: TaildropDownloadProgress) =>
        listener(payload);
      ipcRenderer.on(TAILDROP_DOWNLOAD_PROGRESS, handler);
      return () => {
        ipcRenderer.removeListener(TAILDROP_DOWNLOAD_PROGRESS, handler);
      };
    },
  },
  app: {
    version: () => callResult(APP_CALL, "version"),
    shareFile: (fileName, data) => callResult(APP_CALL, "shareFile", fileName, data),
    showMainWindow: () => callResult(APP_CALL, "showMainWindow"),
    closeTrayMenu: () => callResult(APP_CALL, "closeTrayMenu"),
    quit: () => callResult(APP_CALL, "quit"),
    setTitleBarOverlay: (colors) => {
      ipcRenderer.send(APP_TITLE_BAR_OVERLAY, colors);
    },
    onDeepLinkImport: (listener) => {
      const handler = (_event: IpcRendererEvent, request: DeepLinkImport) => listener(request);
      ipcRenderer.on(DEEP_LINK_IMPORT, handler);
      return () => {
        ipcRenderer.removeListener(DEEP_LINK_IMPORT, handler);
      };
    },
    onProfileFileImport: (listener) => {
      const handler = (_event: IpcRendererEvent, request: ProfileFileImport) => listener(request);
      ipcRenderer.on(PROFILE_FILE_IMPORT, handler);
      return () => {
        ipcRenderer.removeListener(PROFILE_FILE_IMPORT, handler);
      };
    },
    onTaildropSendRequested: (listener) => {
      const handler = (_event: IpcRendererEvent, files: TaildropSendFile[]) => listener(files);
      ipcRenderer.on(TAILDROP_SEND_REQUEST, handler);
      return () => {
        ipcRenderer.removeListener(TAILDROP_SEND_REQUEST, handler);
      };
    },
    onNavigate: (listener) => {
      const handler = (_event: IpcRendererEvent, route: string) => listener(route);
      ipcRenderer.on(APP_NAVIGATE, handler);
      return () => {
        ipcRenderer.removeListener(APP_NAVIGATE, handler);
      };
    },
  },
};

contextBridge.exposeInMainWorld("desktop", bridge);
