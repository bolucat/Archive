import { BrowserWindow, app, ipcMain } from "electron";

import { APP_CALL } from "../shared/ipc";
import type { ProfilesResult } from "../shared/ipc";

export function registerApplication(showMainWindow: () => void) {
  ipcMain.handle(APP_CALL, (event, method: string): ProfilesResult => {
    switch (method) {
      case "version":
        return { ok: true, value: __APP_VERSION__ };
      case "showMainWindow":
        showMainWindow();
        return { ok: true, value: undefined };
      // window.close() in a sandboxed renderer destroys the web contents
      // without firing the window's close event (verified against this
      // Electron build), so the tray menu window cannot dismiss itself; it
      // asks the main process to close it, which the tray menu window
      // intercepts into a hide.
      case "closeTrayMenu":
        BrowserWindow.fromWebContents(event.sender)?.close();
        return { ok: true, value: undefined };
      case "quit":
        app.quit();
        return { ok: true, value: undefined };
      default:
        return { ok: false, error: `unknown app method: ${method}` };
    }
  });
}
