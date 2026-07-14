import { createClient } from "@connectrpc/connect";

import type { DesktopHost } from "@dashboard/app/desktop";
import { showError } from "@dashboard/app/errorStore";

import { ApplicationService } from "@shared/gen/experimental/boxdd/desktop_service_pb";
import { DesktopApi } from "./api";
import { createIpcTransport } from "./transport";

function bufferedEvent<T>(subscribe: (listener: (value: T) => void) => void) {
  let pending: T | null = null;
  let active: ((value: T) => void) | null = null;
  subscribe((value) => {
    if (active !== null) {
      active(value);
    } else {
      pending = value;
    }
  });
  return (listener: (value: T) => void) => {
    active = listener;
    if (pending !== null) {
      listener(pending);
      pending = null;
    }
    return () => {
      if (active === listener) {
        active = null;
      }
    };
  };
}

export function createDesktopHost(): DesktopHost {
  const bridge = window.desktop;
  const transport = createIpcTransport();
  const desktopApi = new DesktopApi(transport);
  const applicationClient = createClient(ApplicationService, transport);
  const preferenceValues = { ...bridge.preferences.initial };
  const preferenceListeners = new Set<(name: string) => void>();

  bridge.preferences.onChanged((name, value) => {
    if (value === undefined) {
      delete preferenceValues[name];
    } else {
      preferenceValues[name] = value;
    }
    for (const listener of preferenceListeners) {
      listener(name);
    }
  });

  const importRemoteProfile = bufferedEvent<{ name: string; url: string }>((listener) => {
    bridge.app.onDeepLinkImport(listener);
  });
  const importProfileFile = bufferedEvent<{ fileName: string; data: Uint8Array }>((listener) => {
    bridge.app.onProfileFileImport(listener);
  });

  return {
    platform: bridge.platform,
    appVersion: () => bridge.app.version(),
    transport,
    preferences: {
      get: (name) => preferenceValues[name],
      set: (name, value) => {
        preferenceValues[name] = value;
        void bridge.preferences.set(name, value).catch(showError);
      },
      remove: (name) => {
        delete preferenceValues[name];
        void bridge.preferences.remove(name).catch(showError);
      },
      subscribe: (listener) => {
        preferenceListeners.add(listener);
        return () => preferenceListeners.delete(listener);
      },
    },
    daemon: {
      getState: () => bridge.daemon.getState(),
      onStateChanged: (listener) => bridge.daemon.onStateChanged(listener),
      retryConnection: () => bridge.daemon.retryConnection(),
    },
    setup: {
      repairInstall: () => bridge.setup.repairInstall(),
      repairStart: () => bridge.setup.repairStart(),
    },
    service: {
      start: () => bridge.profiles.startService(),
      stop: () => desktopApi.stopService(),
      takeOver: () => bridge.profiles.takeOverService(),
    },
    servers: {
      load: () => bridge.servers.load(),
      save: (state) => bridge.servers.save(state),
    },
    configuration: {
      check: async (content) => {
        await applicationClient.checkConfig({ content });
      },
      format: async (content) => (await applicationClient.formatConfig({ content })).content,
    },
    tools: {
      startStandaloneNetworkQualityTest: (request, options) =>
        applicationClient.startStandaloneNetworkQualityTest(
          {
            configUrl: request.configURL,
            serial: request.serial,
            http3: request.http3,
            maxRuntimeSeconds: request.maxRuntimeSeconds,
          },
          options,
        ),
      startStandaloneSTUNTest: (request, options) =>
        applicationClient.startStandaloneSTUNTest({ server: request.server }, options),
    },
    systemProxy: {
      status: async () => {
        const status = await desktopApi.systemProxyStatus();
        return { available: status.available, enabled: status.enabled };
      },
      setEnabled: (enabled) => desktopApi.setSystemProxyEnabled(enabled),
    },
    core: {
      info: () => bridge.core.info(),
      workingDirectory: () => bridge.core.workingDirectory(),
      destroyWorkingDirectory: () => bridge.core.destroyWorkingDirectory(),
    },
    reports: {
      crash: {
        list: () => bridge.reports.list(),
        read: (name) => bridge.reports.read(name),
        markRead: (name) => bridge.reports.markRead(name),
        exportFile: (name, options) => bridge.reports.exportFile(name, options),
        remove: (name) => bridge.reports.remove(name),
        removeAll: () => bridge.reports.removeAll(),
      },
      oom: {
        list: () => bridge.reports.oomList(),
        read: (name) => bridge.reports.oomRead(name),
        markRead: (name) => bridge.reports.oomMarkRead(name),
        exportFile: (name, options) => bridge.reports.oomExportFile(name, options),
        remove: (name) => bridge.reports.oomRemove(name),
        removeAll: () => bridge.reports.oomRemoveAll(),
      },
      triggerDebugCrash: (type) => desktopApi.triggerDebugCrash(type),
      triggerAppCrash: (type) => bridge.reports.triggerAppCrash(type),
      triggerOOMReport: () => desktopApi.triggerOOMReport(),
    },
    profiles: {
      list: () => bridge.profiles.list(),
      onChanged: (listener) => bridge.profiles.onChanged(listener),
      create: async (init) => {
        await bridge.profiles.create(init);
      },
      updateMetadata: (id, patch) => bridge.profiles.updateMetadata(id, patch),
      remove: (id) => bridge.profiles.remove(id),
      reorder: (ids) => bridge.profiles.reorder(ids),
      select: (id) => bridge.profiles.select(id),
      readContent: (id) => bridge.profiles.readContent(id),
      writeContent: (id, content) => bridge.profiles.writeContent(id, content),
      updateRemote: (id) => bridge.profiles.updateRemote(id),
      pickImportFile: () => bridge.profiles.pickImportFile(),
      exportFile: (id) => bridge.profiles.exportFile(id),
      importData: (fileName, data) => bridge.profiles.importData(fileName, data),
      decodeData: (data) => bridge.profiles.decodeData(data),
      exportData: (id) => bridge.profiles.exportData(id),
      encodeData: (id) => bridge.profiles.encodeData(id),
    },
    settings: {
      get: () => bridge.settings.get(),
      setSpeedMode: (mode) => bridge.settings.setSpeedMode(mode),
      setOpenAtLogin: (value) => bridge.settings.setOpenAtLogin(value),
      setTrayEnabled: (value) => bridge.settings.setTrayEnabled(value),
      setTrayInBackground: (value) => bridge.settings.setTrayInBackground(value),
      setOOMKillerEnabled: (value) => bridge.settings.setOOMKillerEnabled(value),
      setOOMMemoryLimitMB: (value) => bridge.settings.setOOMMemoryLimitMB(value),
      setOOMKillerKillConnections: (value) => bridge.settings.setOOMKillerKillConnections(value),
      cacheSize: () => bridge.settings.cacheSize(),
      clearCache: () => bridge.settings.clearCache(),
    },
    application: {
      showMainWindow: () => {
        void bridge.app.showMainWindow();
      },
      closeTrayMenu: () => {
        void bridge.app.closeTrayMenu();
      },
      quit: () => {
        void bridge.app.quit();
      },
    },
    onImportRemoteProfile: (listener) => importRemoteProfile(listener),
    onImportProfileFile: (listener) => importProfileFile(listener),
  };
}
