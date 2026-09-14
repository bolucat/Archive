import { app } from "electron";
import type { LoginItemSettings } from "electron";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, join } from "node:path";

export const LOGIN_ITEM_ARGUMENT = "--start-at-login";

const LINUX_LOGIN_ITEM_NAME = "sing-box.desktop";

function linuxLoginItemPath(): string {
  const environmentConfigHome = process.env.XDG_CONFIG_HOME;
  const configHome =
    environmentConfigHome && isAbsolute(environmentConfigHome)
      ? environmentConfigHome
      : join(app.getPath("home"), ".config");
  return join(configHome, "autostart", LINUX_LOGIN_ITEM_NAME);
}

function linuxLoginItemEnabled(): boolean {
  const loginItemPath = linuxLoginItemPath();
  if (!existsSync(loginItemPath)) {
    return false;
  }
  const entry = readFileSync(loginItemPath, "utf8");
  return !entry
    .split(/\r?\n/)
    .some((line) => line === "Hidden=true" || line === "X-GNOME-Autostart-enabled=false");
}

function escapeDesktopExecArgument(argument: string): string {
  const escaped = argument.replace(/[%\\"$`]/g, (character) =>
    character === "%" ? "%%" : `\\${character}`,
  );
  return `"${escaped}"`;
}

function setLinuxOpenAtLogin(value: boolean) {
  const loginItemPath = linuxLoginItemPath();
  if (!value) {
    rmSync(loginItemPath, { force: true });
    return;
  }
  mkdirSync(dirname(loginItemPath), { recursive: true, mode: 0o700 });
  writeFileSync(
    loginItemPath,
    [
      "[Desktop Entry]",
      "Type=Application",
      "Version=1.0",
      "Name=sing-box",
      `Exec=${escapeDesktopExecArgument(app.getPath("exe"))} ${LOGIN_ITEM_ARGUMENT}`,
      "Icon=sing-box",
      "Terminal=false",
      "StartupNotify=false",
      "X-GNOME-Autostart-enabled=true",
      "",
    ].join("\n"),
    { encoding: "utf8", mode: 0o600 },
  );
}

function getLoginItemSettings(): LoginItemSettings {
  if (process.platform === "win32") {
    return app.getLoginItemSettings({ args: [LOGIN_ITEM_ARGUMENT] });
  }
  return app.getLoginItemSettings();
}

export function openAtLogin(): boolean {
  if (process.platform === "linux") {
    return linuxLoginItemEnabled();
  }
  return getLoginItemSettings().openAtLogin;
}

export function setOpenAtLogin(value: boolean) {
  if (process.platform === "linux") {
    setLinuxOpenAtLogin(value);
    return;
  }
  app.setLoginItemSettings({
    openAtLogin: value,
    args: process.platform === "win32" ? [LOGIN_ITEM_ARGUMENT] : undefined,
  });
}

export function migrateLoginItem() {
  if (process.platform !== "win32") {
    return;
  }
  if (getLoginItemSettings().openAtLogin) {
    return;
  }
  const legacySettings = app.getLoginItemSettings();
  if (legacySettings.openAtLogin) {
    setOpenAtLogin(true);
  }
}

export function hasLoginItemArgument(argv: string[]): boolean {
  return argv.includes(LOGIN_ITEM_ARGUMENT);
}

export function wasOpenedAtLogin(): boolean {
  if (process.platform === "darwin") {
    return app.getLoginItemSettings().wasOpenedAtLogin;
  }
  return hasLoginItemArgument(process.argv);
}
