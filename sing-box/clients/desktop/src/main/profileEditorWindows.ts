import { BrowserWindow, ipcMain, shell } from "electron";
import type { WebContents } from "electron";
import { join } from "node:path";

import {
  PROFILE_EDITOR_WINDOW_CLOSE,
  PROFILE_EDITOR_WINDOW_CLOSE_REQUESTED,
  PROFILE_EDITOR_WINDOW_DIRTY,
  PROFILE_EDITOR_WINDOW_OPEN,
} from "../shared/ipc";
import { captureRuntimeCrash } from "./appReports";
import { developmentRendererURL } from "./development";
import { resourcePath } from "./resources";
import { titleBarOverlay } from "./titleBarOverlay";

interface ProfileEditorWindowState {
  allowClose: boolean;
  dirty: boolean;
}

export function registerProfileEditorWindows(onAllClosed: () => void): Set<BrowserWindow> {
  const windows = new Set<BrowserWindow>();
  const states = new Map<BrowserWindow, ProfileEditorWindowState>();
  const editorWindow = (sender: WebContents): BrowserWindow | null => {
    const window = BrowserWindow.fromWebContents(sender);
    if (window === null || !windows.has(window)) {
      return null;
    }
    return window;
  };

  ipcMain.handle(
    PROFILE_EDITOR_WINDOW_OPEN,
    async (_event, profileId: unknown, readOnly: unknown) => {
      if (typeof profileId !== "string" || profileId === "" || typeof readOnly !== "boolean") {
        throw new Error("invalid profile editor request");
      }
      const overlay = titleBarOverlay();
      const window = new BrowserWindow({
        width: 960,
        height: 640,
        minWidth: 640,
        minHeight: 400,
        show: true,
        backgroundColor: overlay?.color,
        title: readOnly ? "View Content" : "Edit Content",
        titleBarStyle: process.platform === "darwin" ? "hiddenInset" : "hidden",
        trafficLightPosition: process.platform === "darwin" ? { x: 18, y: 19 } : undefined,
        titleBarOverlay: overlay,
        icon: process.platform === "linux" ? resourcePath("icons", "512x512.png") : undefined,
        webPreferences: {
          preload: join(import.meta.dirname, "../preload/index.cjs"),
          contextIsolation: true,
          sandbox: true,
          nodeIntegration: false,
          backgroundThrottling: false,
        },
      });
      if (process.platform !== "darwin") {
        window.removeMenu();
      }
      windows.add(window);
      states.set(window, { allowClose: false, dirty: false });
      window.webContents.setWindowOpenHandler(({ url }) => {
        if (url.startsWith("https://") || url.startsWith("http://")) {
          void shell.openExternal(url);
        }
        return { action: "deny" };
      });
      window.webContents.on("will-navigate", (event, url) => {
        if (url !== window.webContents.getURL()) {
          event.preventDefault();
        }
      });
      window.webContents.on("render-process-gone", (_event, details) => {
        if (details.reason !== "clean-exit") {
          captureRuntimeCrash(
            "profile-editor-render-process-gone",
            new Error(`renderer ${details.reason} (exit code ${details.exitCode})`),
          );
        }
      });
      window.on("close", (event) => {
        const state = states.get(window);
        if (state?.dirty === true && !state.allowClose) {
          event.preventDefault();
          if (!window.webContents.isDestroyed()) {
            window.webContents.send(PROFILE_EDITOR_WINDOW_CLOSE_REQUESTED);
          }
        }
      });
      window.on("closed", () => {
        windows.delete(window);
        states.delete(window);
        if (windows.size === 0) {
          onAllClosed();
        }
      });
      try {
        const route = `profile-editor/${encodeURIComponent(profileId)}?readOnly=${readOnly}`;
        const rendererURL = developmentRendererURL();
        if (rendererURL !== "") {
          const url = new URL(rendererURL);
          url.hash = `#/${route}`;
          await window.loadURL(url.toString());
        } else {
          await window.loadFile(join(import.meta.dirname, "../renderer/index.html"), {
            hash: `/${route}`,
          });
        }
      } catch (error) {
        window.destroy();
        throw error;
      }
    },
  );

  ipcMain.on(PROFILE_EDITOR_WINDOW_DIRTY, (event, dirty: unknown) => {
    const window = editorWindow(event.sender);
    if (window === null || typeof dirty !== "boolean") {
      return;
    }
    const state = states.get(window);
    if (state !== undefined) {
      state.dirty = dirty;
    }
  });

  ipcMain.on(PROFILE_EDITOR_WINDOW_CLOSE, (event) => {
    const window = editorWindow(event.sender);
    if (window === null) {
      return;
    }
    const state = states.get(window);
    if (state !== undefined) {
      state.allowClose = true;
    }
    window.close();
  });

  return windows;
}
