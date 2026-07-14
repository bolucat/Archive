import { app } from "electron";
import type { LoginItemSettings } from "electron";

export const LOGIN_ITEM_ARGUMENT = "--start-at-login";

function getLoginItemSettings(): LoginItemSettings {
  if (process.platform === "win32") {
    return app.getLoginItemSettings({ args: [LOGIN_ITEM_ARGUMENT] });
  }
  return app.getLoginItemSettings();
}

export function openAtLogin(): boolean {
  return getLoginItemSettings().openAtLogin;
}

export function setOpenAtLogin(value: boolean) {
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
