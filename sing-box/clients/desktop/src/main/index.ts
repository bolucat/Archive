import { app, BrowserWindow, crashReporter, ipcMain, screen, session, shell } from "electron";
import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { basename, join } from "node:path";

import { APP_TITLE_BAR_OVERLAY, DEEP_LINK_IMPORT, PROFILE_FILE_IMPORT } from "../shared/ipc";
import type { DeepLinkImport, ProfileFileImport, TitleBarOverlayColors } from "../shared/ipc";
import { archiveNativeCrashDumps, captureRuntimeCrash } from "./appReports";
import { registerApplication } from "./application";
import { registerDaemonBridge } from "./bridge";
import { registerCore } from "./core";
import { settingsDatabase } from "./database";
import { developmentRendererURL, developmentSwitchValue, hardenPackagedRuntime } from "./development";
import { hasLoginItemArgument, migrateLoginItem, wasOpenedAtLogin } from "./loginItem";
import { registerPreferences } from "./preferences";
import { registerProfiles } from "./profiles";
import { registerSetup } from "./repair";
import { registerReports } from "./reports";
import { resourcePath } from "./resources";
import { registerServers } from "./servers";
import {
  registerSettings,
  saveMainWindowState,
  storedMainWindowState,
  trayEnabled,
  trayInBackground,
} from "./settings";
import { daemonState } from "./state";
import { initializeTray, updateTrayVisibility } from "./tray";
import { prepareTrayMenuWindow, showTrayMenu } from "./trayMenu";
import { secureApplicationUserData } from "./userDataSecurity";
import {
  MAIN_WINDOW_MINIMUM_HEIGHT,
  MAIN_WINDOW_MINIMUM_WIDTH,
  restoredMainWindowBounds,
} from "./windowState";

function handleFatal(kind: string, error: unknown): never {
  captureRuntimeCrash(kind, error);
  process.exit(1);
}
process.on("uncaughtException", (error) => handleFatal("uncaughtException", error));
process.on("unhandledRejection", (reason) => handleFatal("unhandledRejection", reason));
hardenPackagedRuntime();

const userDataPath = developmentSwitchValue("user-data");
if (userDataPath) {
  app.setPath("userData", userDataPath);
}
secureApplicationUserData();

crashReporter.start({ submitURL: "", uploadToServer: false, compress: false });

const testScriptPath = developmentSwitchValue("test-script");

const TITLE_BAR_OVERLAY_HEIGHT = 51;

// Electron's native Windows/Linux controls overlay does not follow the page theme.
let titleBarOverlayColors: TitleBarOverlayColors | undefined;

function createWindow(): BrowserWindow {
  const restoredState = process.platform === "win32" ? storedMainWindowState() : undefined;
  const restoredBounds =
    process.platform === "win32"
      ? restoredMainWindowBounds(
          restoredState,
          screen.getAllDisplays().map((display) => display.workArea),
          screen.getPrimaryDisplay().workArea,
        )
      : undefined;
  const window = new BrowserWindow({
    x: restoredBounds?.x,
    y: restoredBounds?.y,
    width: restoredBounds?.width ?? 1280,
    height: restoredBounds?.height ?? 800,
    minWidth: Math.min(
      MAIN_WINDOW_MINIMUM_WIDTH,
      restoredBounds?.width ?? MAIN_WINDOW_MINIMUM_WIDTH,
    ),
    minHeight: Math.min(
      MAIN_WINDOW_MINIMUM_HEIGHT,
      restoredBounds?.height ?? MAIN_WINDOW_MINIMUM_HEIGHT,
    ),
    show: false,
    titleBarStyle: process.platform === "darwin" ? "hiddenInset" : "hidden",
    trafficLightPosition: process.platform === "darwin" ? { x: 18, y: 19 } : undefined,
    titleBarOverlay:
      process.platform === "darwin"
        ? undefined
        : { height: TITLE_BAR_OVERLAY_HEIGHT, ...titleBarOverlayColors },
    icon: process.platform === "linux" ? resourcePath("icons", "512x512.png") : undefined,
    webPreferences: {
      preload: join(import.meta.dirname, "../preload/index.cjs"),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
      backgroundThrottling: !testScriptPath,
    },
  });
  if (process.platform === "win32") {
    registerMainWindowStatePersistence(window, restoredState?.maximized === true);
  }
  window.once("ready-to-show", () => {
    if (restoredState?.maximized === true) {
      window.maximize();
    }
    window.show();
  });
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
        "render-process-gone",
        new Error(`renderer ${details.reason} (exit code ${details.exitCode})`),
      );
    }
  });
  const rendererURL = developmentRendererURL();
  if (rendererURL !== "") {
    void window.loadURL(rendererURL);
  } else {
    void window.loadFile(join(import.meta.dirname, "../renderer/index.html"));
  }
  attachTestInstrumentation(window);
  mainWindow = window;
  window.on("closed", () => {
    if (mainWindow === window) {
      mainWindow = null;
    }
    if (!quitting && !trayInBackground()) {
      app.quit();
    }
  });
  return window;
}

let mainWindow: BrowserWindow | null = null;
let quitting = false;

app.on("before-quit", () => {
  quitting = true;
});

function registerMainWindowStatePersistence(window: BrowserWindow, initiallyMaximized: boolean) {
  let maximized = initiallyMaximized;

  const save = () => {
    const bounds = window.getNormalBounds();
    void saveMainWindowState({ ...bounds, maximized }).catch((error: unknown) => {
      console.error("failed to save the main window state", error);
    });
  };

  window.on("moved", save);
  window.on("resized", save);
  window.on("maximize", () => {
    maximized = true;
    save();
  });
  window.on("unmaximize", () => {
    maximized = false;
    save();
  });
  window.on("close", save);
}

function attachTestInstrumentation(window: BrowserWindow) {
  if (!testScriptPath) {
    return;
  }
  window.webContents.on("console-message", (event) => {
    const text = event.message;
    if (text.startsWith("capture:")) {
      const name = text.slice("capture:".length).trim();
      void window.webContents.capturePage().then((image) => {
        writeFileSync(`/tmp/sbd-verify/${name}.png`, image.toPNG());
        console.log("captured", name);
      });
    }
    if (text.startsWith("note:")) {
      appendFileSync("/tmp/sbd-verify/notes.log", `${text.slice("note:".length)}\n`);
    }
  });
  window.webContents.on("did-finish-load", () => {
    const script = readFileSync(testScriptPath, "utf-8");
    void window.webContents.executeJavaScript(script);
  });
}

function showWindow(): BrowserWindow {
  if (mainWindow !== null && !mainWindow.isDestroyed()) {
    if (mainWindow.isMinimized()) {
      mainWindow.restore();
    }
    mainWindow.show();
    mainWindow.focus();
    return mainWindow;
  }
  return createWindow();
}

function parseImportLink(link: string): DeepLinkImport | null {
  let parsed: URL;
  try {
    parsed = new URL(link);
  } catch {
    return null;
  }
  if (parsed.protocol !== "sing-box:" || parsed.host !== "import-remote-profile") {
    return null;
  }
  const remoteUrl = parsed.searchParams.get("url");
  if (!remoteUrl) {
    return null;
  }
  let name: string;
  try {
    name = decodeURIComponent(parsed.hash.replace(/^#/, ""));
  } catch {
    return null;
  }
  if (name === "") {
    try {
      name = new URL(remoteUrl).host;
    } catch {
      return null;
    }
  }
  return { name, url: remoteUrl };
}

function sendWhenLoaded(channel: string, payload: unknown) {
  const window = showWindow();
  if (window.webContents.isLoading()) {
    window.webContents.once("did-finish-load", () => {
      window.webContents.send(channel, payload);
    });
  } else {
    window.webContents.send(channel, payload);
  }
}

function handleDeepLink(link: string) {
  const request = parseImportLink(link);
  if (request === null) {
    return;
  }
  sendWhenLoaded(DEEP_LINK_IMPORT, request);
}

function handleProfileFile(path: string) {
  void readFile(path).then(
    (data) => {
      sendWhenLoaded(PROFILE_FILE_IMPORT, {
        fileName: basename(path),
        data,
      } satisfies ProfileFileImport);
    },
    () => {},
  );
}

function deepLinkFromArguments(argv: string[]): string | undefined {
  return argv.find((argument) => argument.startsWith("sing-box://"));
}

function profileFileFromArguments(argv: string[]): string | undefined {
  return argv.find((argument) => argument.toLowerCase().endsWith(".bpf"));
}

const singleInstanceLock = app.requestSingleInstanceLock();
if (!singleInstanceLock) {
  app.quit();
} else {
  app.setAsDefaultProtocolClient("sing-box");

  app.on("second-instance", (_event, argv) => {
    const link = deepLinkFromArguments(argv);
    const profileFile = profileFileFromArguments(argv);
    if (!hasLoginItemArgument(argv) || link || profileFile) {
      showWindow();
    }
    if (link) {
      handleDeepLink(link);
    }
    if (profileFile) {
      handleProfileFile(profileFile);
    }
  });

  app.on("open-url", (event, url) => {
    event.preventDefault();
    handleDeepLink(url);
  });

  app.on("open-file", (event, path) => {
    event.preventDefault();
    handleProfileFile(path);
  });

  app.on("window-all-closed", () => {});

  app.on("activate", () => {
    showWindow();
  });

  void app.whenReady().then(async () => {
    session.defaultSession.setPermissionRequestHandler((_webContents, permission, callback) => {
      callback(permission === "clipboard-sanitized-write");
    });
    settingsDatabase();
    archiveNativeCrashDumps();
    registerApplication(showWindow);
    ipcMain.on(APP_TITLE_BAR_OVERLAY, (event, colors: TitleBarOverlayColors) => {
      if (process.platform === "darwin") {
        return;
      }
      titleBarOverlayColors = colors;
      const window = BrowserWindow.fromWebContents(event.sender);
      window?.setTitleBarOverlay({ ...colors, height: TITLE_BAR_OVERLAY_HEIGHT });
    });
    registerDaemonBridge();
    registerSetup(() => daemonState.retryConnection());
    registerCore();
    registerReports();
    registerPreferences();
    registerProfiles();
    registerServers();
    registerSettings(updateTrayVisibility);
    const link = deepLinkFromArguments(process.argv);
    const profileFile = profileFileFromArguments(process.argv);
    const startInTray =
      wasOpenedAtLogin() && trayEnabled() && trayInBackground() && !link && !profileFile;
    migrateLoginItem();
    if (!startInTray) {
      createWindow();
    }
    initializeTray(showWindow);
    updateTrayVisibility(trayEnabled());
    if (testScriptPath) {
      const workArea = screen.getPrimaryDisplay().workArea;
      const anchor = {
        x: workArea.x + Math.round(workArea.width / 2),
        y: workArea.y + workArea.height + 8,
        width: 0,
        height: 0,
      };
      attachTestInstrumentation(prepareTrayMenuWindow(anchor));
      void showTrayMenu(anchor);
    }
    if (link) {
      handleDeepLink(link);
    }
    if (profileFile) {
      handleProfileFile(profileFile);
    }
  });
}
