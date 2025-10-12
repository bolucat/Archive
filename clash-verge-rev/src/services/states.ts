import { createContextState } from "foxact/create-context-state";
import { useLocalStorage } from "foxact/use-local-storage";
import { LogLevel } from "tauri-plugin-mihomo-api";

const [ThemeModeProvider, useThemeMode, useSetThemeMode] = createContextState<
  "light" | "dark"
>("light");

export type LogFilter = "all" | "debug" | "info" | "warn" | "err";

interface IClashLog {
  enable: boolean;
  logLevel: LogLevel;
  logFilter: LogFilter;
}
const defaultClashLog: IClashLog = {
  enable: true,
  logLevel: "info",
  logFilter: "all",
};
export const useClashLog = () =>
  useLocalStorage<IClashLog>("clash-log", defaultClashLog, {
    serializer: JSON.stringify,
    deserializer: JSON.parse,
  });

// export const useEnableLog = () => useLocalStorage("enable-log", false);

interface IConnectionSetting {
  layout: "table" | "list";
}

const defaultConnectionSetting: IConnectionSetting = { layout: "table" };

export const useConnectionSetting = () =>
  useLocalStorage<IConnectionSetting>(
    "connections-setting",
    defaultConnectionSetting,
    {
      serializer: JSON.stringify,
      deserializer: JSON.parse,
    },
  );

// save the state of each profile item loading
const [LoadingCacheProvider, useLoadingCache, useSetLoadingCache] =
  createContextState<Record<string, boolean>>({});

// save update state
const [UpdateStateProvider, useUpdateState, useSetUpdateState] =
  createContextState<boolean>(false);

export {
  ThemeModeProvider,
  useThemeMode,
  useSetThemeMode,
  LoadingCacheProvider,
  useLoadingCache,
  useSetLoadingCache,
  UpdateStateProvider,
  useUpdateState,
  useSetUpdateState,
};
