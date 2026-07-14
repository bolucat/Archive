import { ipcMain } from "electron";

import { SETTINGS_CALL } from "../shared/ipc";
import type { DesktopSettings, ProfilesResult, TraySpeedMode } from "../shared/ipc";
import { applicationCacheSize, clearApplicationCache } from "./appCache";
import { Preference } from "./database";
import { openAtLogin, setOpenAtLogin } from "./loginItem";
import { parseMainWindowState } from "./windowState";
import type { MainWindowState } from "./windowState";

function parseBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw new Error("invalid boolean preference");
  }
  return value;
}

function parsePositiveNumber(value: unknown): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value <= 0) {
    throw new Error("invalid positive number preference");
  }
  return value;
}

function parseSpeedMode(value: unknown): TraySpeedMode {
  if (value !== "disabled" && value !== "enabled" && value !== "unified") {
    throw new Error("invalid speed mode preference");
  }
  return value;
}

const speedModePreference = new Preference<TraySpeedMode>(
  "speed_mode",
  "disabled",
  parseSpeedMode,
);
const oomKillerEnabledPreference = new Preference(
  "oom_killer_enabled",
  false,
  parseBoolean,
);
const oomMemoryLimitPreference = new Preference(
  "oom_memory_limit_mb",
  50,
  parsePositiveNumber,
);
const oomKillerKillConnectionsPreference = new Preference(
  "oom_killer_kill_connections",
  false,
  parseBoolean,
);
const trayEnabledPreference = new Preference("tray_enabled", true, parseBoolean);
const trayInBackgroundPreference = new Preference("tray_in_background", true, parseBoolean);
const mainWindowStatePreference = new Preference<MainWindowState | undefined>(
  "main_window_state",
  undefined,
  parseMainWindowState,
);

let setTrayVisibility: (enabled: boolean) => void = () => {};

export function storedMainWindowState(): MainWindowState | undefined {
  const state = mainWindowStatePreference.get();
  return state === undefined ? undefined : { ...state };
}

export function trayEnabled(): boolean {
  return trayEnabledPreference.get();
}

export function trayInBackground(): boolean {
  return trayEnabledPreference.get() && trayInBackgroundPreference.get();
}

export async function saveMainWindowState(state: MainWindowState): Promise<void> {
  mainWindowStatePreference.set({ ...state });
}

export async function oomStartOptions(): Promise<{
  oomKillerEnabled: boolean;
  oomKillerDisabled: boolean;
  oomMemoryLimit: bigint;
}> {
  return {
    oomKillerEnabled: oomKillerEnabledPreference.get(),
    oomKillerDisabled: !oomKillerKillConnectionsPreference.get(),
    oomMemoryLimit: BigInt(oomMemoryLimitPreference.get()) * 1024n * 1024n,
  };
}

const handlers: Record<string, (...callArguments: never[]) => Promise<unknown>> = {
  async get(): Promise<DesktopSettings> {
    return {
      speedMode: speedModePreference.get(),
      openAtLogin: openAtLogin(),
      trayEnabled: trayEnabledPreference.get(),
      trayInBackground: trayInBackgroundPreference.get(),
      oomKillerEnabled: oomKillerEnabledPreference.get(),
      oomMemoryLimitMB: oomMemoryLimitPreference.get(),
      oomKillerKillConnections: oomKillerKillConnectionsPreference.get(),
    };
  },

  async setSpeedMode(mode: TraySpeedMode): Promise<void> {
    speedModePreference.set(parseSpeedMode(mode));
  },

  async setOpenAtLogin(value: boolean): Promise<void> {
    setOpenAtLogin(parseBoolean(value));
  },

  async setTrayEnabled(value: boolean): Promise<void> {
    const enabled = parseBoolean(value);
    trayEnabledPreference.set(enabled);
    setTrayVisibility(enabled);
  },

  async setTrayInBackground(value: boolean): Promise<void> {
    trayInBackgroundPreference.set(parseBoolean(value));
  },

  async setOOMKillerEnabled(value: boolean): Promise<void> {
    oomKillerEnabledPreference.set(parseBoolean(value));
  },

  async setOOMMemoryLimitMB(value: number): Promise<void> {
    oomMemoryLimitPreference.set(parsePositiveNumber(value));
  },

  async setOOMKillerKillConnections(value: boolean): Promise<void> {
    oomKillerKillConnectionsPreference.set(parseBoolean(value));
  },

  async cacheSize(): Promise<number> {
    return applicationCacheSize();
  },

  async clearCache(): Promise<void> {
    await clearApplicationCache();
  },
};

export function registerSettings(updateTrayVisibility: (enabled: boolean) => void): void {
  setTrayVisibility = updateTrayVisibility;
  ipcMain.handle(
    SETTINGS_CALL,
    async (
      _event,
      method: string,
      ...callArguments: unknown[]
    ): Promise<ProfilesResult> => {
      const handler = handlers[method];
      if (!handler) {
        return { ok: false, error: `unknown settings method: ${method}` };
      }
      try {
        const value = await handler(...(callArguments as never[]));
        return { ok: true, value };
      } catch (error) {
        return {
          ok: false,
          error: error instanceof Error ? error.message : String(error),
        };
      }
    },
  );
}
