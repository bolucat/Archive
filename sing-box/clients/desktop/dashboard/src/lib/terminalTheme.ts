import type { ITheme } from "@xterm/xterm";

import {
  DEFAULT_DARK_THEME_NAME,
  DEFAULT_LIGHT_THEME_NAME,
  DEFAULT_TERMINAL_FONT_SIZE,
  type TerminalConfig,
} from "./tailscaleSSH";

export type Scheme = "light" | "dark";

export interface TerminalThemeEntry {
  name: string;
  isDark: boolean;
  theme: ITheme;
}

export const DEFAULT_TERMINAL_FONT = "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace";

// Default themes, matching sing-box-for-apple (light: Alabaster, dark: Afterglow).
const ALABASTER: TerminalThemeEntry = {
  name: DEFAULT_LIGHT_THEME_NAME,
  isDark: false,
  theme: {
    background: "#f7f7f7",
    foreground: "#000000",
    cursor: "#007acc",
    selectionBackground: "#bfdbfe",
    black: "#000000",
    red: "#aa3731",
    green: "#448c27",
    yellow: "#cb9000",
    blue: "#325cc0",
    magenta: "#7a3e9d",
    cyan: "#0083b2",
    white: "#b7b7b7",
    brightBlack: "#777777",
    brightRed: "#f05050",
    brightGreen: "#60cb00",
    brightYellow: "#f2af50",
    brightBlue: "#007acc",
    brightMagenta: "#e64ce6",
    brightCyan: "#00aacb",
    brightWhite: "#f7f7f7",
  },
};

const AFTERGLOW: TerminalThemeEntry = {
  name: DEFAULT_DARK_THEME_NAME,
  isDark: true,
  theme: {
    background: "#212121",
    foreground: "#d0d0d0",
    cursor: "#d0d0d0",
    selectionBackground: "#303030",
    black: "#151515",
    red: "#ac4142",
    green: "#7e8e50",
    yellow: "#e5b567",
    blue: "#6c99bb",
    magenta: "#9f4e85",
    cyan: "#7dd6cf",
    white: "#d0d0d0",
    brightBlack: "#505050",
    brightRed: "#ac4142",
    brightGreen: "#7e8e50",
    brightYellow: "#e5b567",
    brightBlue: "#6c99bb",
    brightMagenta: "#9f4e85",
    brightCyan: "#7dd6cf",
    brightWhite: "#f5f5f5",
  },
};

const SEED_THEMES: TerminalThemeEntry[] = [ALABASTER, AFTERGLOW];

export function currentScheme(): Scheme {
  return document.documentElement.dataset.theme === "light" ? "light" : "dark";
}

function fallbackTheme(scheme: Scheme): ITheme {
  return scheme === "dark" ? AFTERGLOW.theme : ALABASTER.theme;
}

export function seedTheme(name: string): TerminalThemeEntry | undefined {
  return SEED_THEMES.find((entry) => entry.name === name);
}

export function parseCustomTheme(json: string): ITheme | null {
  if (json.trim() === "") {
    return null;
  }
  try {
    const value: unknown = JSON.parse(json);
    if (value && typeof value === "object" && !Array.isArray(value)) {
      return value as ITheme;
    }
  } catch {
    return null;
  }
  return null;
}

function themeName(config: TerminalConfig, scheme: Scheme): string {
  return scheme === "dark" ? config.darkThemeName : config.lightThemeName;
}

export function resolveThemeSync(config: TerminalConfig, scheme: Scheme): ITheme {
  const name = themeName(config, scheme);
  if (name === "") {
    const custom = scheme === "dark" ? config.darkThemeCustom : config.lightThemeCustom;
    return parseCustomTheme(custom) ?? fallbackTheme(scheme);
  }
  return seedTheme(name)?.theme ?? fallbackTheme(scheme);
}

export async function resolveTheme(config: TerminalConfig, scheme: Scheme): Promise<ITheme> {
  const name = themeName(config, scheme);
  if (name === "" || seedTheme(name)) {
    return resolveThemeSync(config, scheme);
  }
  try {
    const { TERMINAL_THEMES } = await import("./terminalThemes");
    return TERMINAL_THEMES.find((entry) => entry.name === name)?.theme ?? fallbackTheme(scheme);
  } catch {
    return fallbackTheme(scheme);
  }
}

export function terminalFontFamily(config: TerminalConfig): string {
  const family = config.fontFamily.trim();
  if (!family) {
    return DEFAULT_TERMINAL_FONT;
  }
  const quoted = /\s/.test(family) ? `"${family}"` : family;
  return `${quoted}, ${DEFAULT_TERMINAL_FONT}`;
}

export function terminalFontSize(config: TerminalConfig): number {
  return config.fontSize > 0 ? config.fontSize : DEFAULT_TERMINAL_FONT_SIZE;
}
