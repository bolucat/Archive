import { BrowserWindow, ipcMain } from "electron";

import {
  PREFERENCES_CALL,
  PREFERENCES_CHANGED,
  PREFERENCES_SNAPSHOT,
} from "../shared/ipc";
import type { ProfilesResult } from "../shared/ipc";
import { DESKTOP_LANGUAGES } from "../shared/translations";
import {
  parseBooleanPreference,
  preferenceSnapshot,
  removePreference,
  setPreference,
} from "./database";

type PreferenceParser = (value: unknown) => unknown;

function stringChoice(...choices: string[]): PreferenceParser {
  return (value) => {
    if (typeof value !== "string" || !choices.includes(value)) {
      throw new Error("invalid string preference");
    }
    return value;
  };
}

function parseStringArray(value: unknown): string[] {
  if (!Array.isArray(value) || !value.every((entry) => typeof entry === "string")) {
    throw new Error("invalid string array preference");
  }
  return value;
}

function parseTailscaleSSH(value: unknown): unknown {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("invalid Tailscale SSH preference");
  }
  const result: Record<string, unknown> = Object.create(null) as Record<string, unknown>;
  for (const [identifier, entry] of Object.entries(value)) {
    if (typeof entry !== "object" || entry === null || Array.isArray(entry)) {
      throw new Error("invalid Tailscale SSH preference");
    }
    const candidate = entry as Record<string, unknown>;
    if (
      typeof candidate.username !== "string" ||
      typeof candidate.terminalType !== "string" ||
      typeof candidate.remember !== "boolean"
    ) {
      throw new Error("invalid Tailscale SSH preference");
    }
    result[identifier] = {
      username: candidate.username,
      terminalType: candidate.terminalType,
      remember: candidate.remember,
    };
  }
  return result;
}

function parseTerminalConfig(value: unknown): unknown {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new Error("invalid terminal preference");
  }
  const candidate = value as Record<string, unknown>;
  if (
    typeof candidate.symbolBarAlwaysShow !== "boolean" ||
    typeof candidate.lightThemeName !== "string" ||
    typeof candidate.darkThemeName !== "string" ||
    typeof candidate.lightThemeCustom !== "string" ||
    typeof candidate.darkThemeCustom !== "string" ||
    typeof candidate.fontFamily !== "string" ||
    typeof candidate.fontSize !== "number" ||
    !Number.isSafeInteger(candidate.fontSize) ||
    candidate.fontSize <= 0
  ) {
    throw new Error("invalid terminal preference");
  }
  return value;
}

const rendererPreferences: Record<string, PreferenceParser> = {
  theme: stringChoice("auto", "light", "dark"),
  accent: (value) => {
    if (
      typeof value !== "string" ||
      (!/^(blue|purple|pink|red|orange|yellow|green|graphite)$/.test(value) &&
        !/^#[0-9a-f]{6}$/.test(value))
    ) {
      throw new Error("invalid accent preference");
    }
    return value;
  },
  language: stringChoice(...DESKTOP_LANGUAGES),
  "dashboard-cards": (value) => {
    if (typeof value !== "object" || value === null || Array.isArray(value)) {
      throw new Error("invalid dashboard cards preference");
    }
    const candidate = value as Record<string, unknown>;
    return {
      enabled: parseStringArray(candidate.enabled),
      order: parseStringArray(candidate.order),
    };
  },
  "connection-state-filter": stringChoice("all", "active", "closed"),
  "connection-sort": stringChoice("date", "traffic", "trafficTotal"),
  "disable-deprecated-warnings": parseBooleanPreference,
  "tailscale-ssh": parseTailscaleSSH,
  "terminal-config": parseTerminalConfig,
  "desktop-active-server": (value) => {
    if (typeof value !== "string" || value === "") {
      throw new Error("invalid string preference");
    }
    return value;
  },
};

const rendererPreferenceNames = Object.keys(rendererPreferences);
const preferenceChangeListeners = new Set<(name: string) => void>();

export function onPreferenceChanged(listener: (name: string) => void): () => void {
  preferenceChangeListeners.add(listener);
  return () => preferenceChangeListeners.delete(listener);
}

function notifyPreferenceChanged(name: string, value?: unknown): void {
  for (const window of BrowserWindow.getAllWindows()) {
    if (!window.webContents.isDestroyed()) {
      window.webContents.send(PREFERENCES_CHANGED, name, value);
    }
  }
  for (const listener of preferenceChangeListeners) {
    listener(name);
  }
}

export function registerPreferences(): void {
  ipcMain.on(PREFERENCES_SNAPSHOT, (event) => {
    event.returnValue = preferenceSnapshot(rendererPreferenceNames);
  });
  ipcMain.handle(
    PREFERENCES_CALL,
    async (
      _event,
      method: string,
      ...callArguments: unknown[]
    ): Promise<ProfilesResult> => {
      try {
        const name = callArguments[0];
        if (typeof name !== "string") {
          throw new Error("invalid preference name");
        }
        switch (method) {
          case "set": {
            const parse = rendererPreferences[name];
            if (parse === undefined) {
              throw new Error("unknown renderer preference");
            }
            const value = parse(callArguments[1]);
            setPreference(name, value);
            notifyPreferenceChanged(name, value);
            return { ok: true, value: undefined };
          }
          case "remove":
            if (rendererPreferences[name] === undefined) {
              throw new Error("unknown renderer preference");
            }
            removePreference(name);
            notifyPreferenceChanged(name);
            return { ok: true, value: undefined };
          default:
            return { ok: false, error: `unknown preferences method: ${method}` };
        }
      } catch (error) {
        return {
          ok: false,
          error: error instanceof Error ? error.message : String(error),
        };
      }
    },
  );
}
