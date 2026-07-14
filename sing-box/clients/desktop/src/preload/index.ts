import { contextBridge, ipcRenderer } from "electron";
import type { IpcRendererEvent } from "electron";

import {
  APP_CALL,
  APP_TITLE_BAR_OVERLAY,
  CORE_CALL,
  DAEMON_RETRY,
  DAEMON_STATE_CHANGED,
  DAEMON_STATE_GET,
  DAEMON_STREAM_CANCEL,
  DAEMON_STREAM_EVENT,
  DAEMON_STREAM_OPEN,
  DAEMON_UNARY,
  DEEP_LINK_IMPORT,
  PREFERENCES_CALL,
  PREFERENCES_CHANGED,
  PREFERENCES_SNAPSHOT,
  PROFILE_FILE_IMPORT,
  PROFILES_CALL,
  PROFILES_CHANGED,
  REPORTS_CALL,
  SERVERS_CALL,
  SETTINGS_CALL,
  SETUP_CALL,
} from "../shared/ipc";
import type {
  DaemonConnectionState,
  DeepLinkImport,
  DesktopBridge,
  ProfileFileImport,
  ProfilesResult,
  StreamEvent,
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

function callCore<T>(method: string): Promise<T> {
  return callResult(CORE_CALL, method);
}

function callReports<T>(method: string, ...callArguments: unknown[]): Promise<T> {
  return callResult(REPORTS_CALL, method, ...callArguments);
}

const bridge: DesktopBridge = {
  platform: process.platform,
  daemon: {
    unary: (service, method, request) => ipcRenderer.invoke(DAEMON_UNARY, service, method, request),
    streamOpen: (id, service, method, request) => {
      ipcRenderer.send(DAEMON_STREAM_OPEN, id, service, method, request);
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
    workingDirectory: () => callCore("workingDirectory"),
    destroyWorkingDirectory: () => callCore("destroyWorkingDirectory"),
  },
  reports: {
    list: () => callReports("list"),
    read: (name) => callReports("read", name),
    markRead: (name) => callReports("markRead", name),
    exportFile: (name, options) => callReports("exportFile", name, options),
    remove: (name) => callReports("remove", name),
    removeAll: () => callReports("removeAll"),
    oomList: () => callReports("oomList"),
    oomRead: (name) => callReports("oomRead", name),
    oomMarkRead: (name) => callReports("oomMarkRead", name),
    oomExportFile: (name, options) => callReports("oomExportFile", name, options),
    oomRemove: (name) => callReports("oomRemove", name),
    oomRemoveAll: () => callReports("oomRemoveAll"),
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
  settings: {
    get: () => callSettings("get"),
    setSpeedMode: (mode) => callSettings("setSpeedMode", mode),
    setOpenAtLogin: (value) => callSettings("setOpenAtLogin", value),
    setTrayEnabled: (value) => callSettings("setTrayEnabled", value),
    setTrayInBackground: (value) => callSettings("setTrayInBackground", value),
    setOOMKillerEnabled: (value) => callSettings("setOOMKillerEnabled", value),
    setOOMMemoryLimitMB: (value) => callSettings("setOOMMemoryLimitMB", value),
    setOOMKillerKillConnections: (value) => callSettings("setOOMKillerKillConnections", value),
    cacheSize: () => callSettings("cacheSize"),
    clearCache: () => callSettings("clearCache"),
  },
  app: {
    version: () => callResult(APP_CALL, "version"),
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
  },
};

contextBridge.exposeInMainWorld("desktop", bridge);
