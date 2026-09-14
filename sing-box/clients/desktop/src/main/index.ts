import { app, BrowserWindow, crashReporter, dialog, ipcMain, screen, session, shell } from "electron";
import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { basename, join } from "node:path";

import {
  APP_NAVIGATE,
  APP_TITLE_BAR_OVERLAY,
  DEEP_LINK_IMPORT,
  PROFILE_FILE_IMPORT,
  TAILDROP_SEND_REQUEST,
  UPDATES_PRESENT,
} from "../shared/ipc";
import type {
  DeepLinkImport,
  ProfileFileImport,
  TaildropSendFile,
  TitleBarOverlayColors,
} from "../shared/ipc";
import { configureApplicationPaths } from "./applicationPaths";
import {
  archiveNativeCrashDumps,
  captureRuntimeCrash,
} from "./appReports";
import type { RuntimeCrashCaptureResult } from "./appReports";
import { registerApplication } from "./application";
import { registerDaemonBridge } from "./bridge";
import { registerCore } from "./core";
import { settingsDatabase } from "./database";
import { developmentRendererURL, developmentSwitchValue } from "./development";
import { applyDisplayScaleFactor } from "./displayScale";
import { hasLoginItemArgument, migrateLoginItem, wasOpenedAtLogin } from "./loginItem";
import { registerNotifications } from "./notifications";
import { registerPreferences } from "./preferences";
import { registerOpenConnectBrowser } from "./openConnectBrowser";
import { registerProfileEditorWindows } from "./profileEditorWindows";
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
import { createTaildropSendBatcher, registerTaildrop, taildropSendPaths } from "./taildrop";
import { initializeTray, updateTrayVisibility } from "./tray";
import { registerUpdates, runStartupUpdateCheck } from "./updates";
import { prepareTrayMenuWindow, showTrayMenu } from "./trayMenu";
import { registerTerminalWindows } from "./terminalWindows";
import { applyTitleBarOverlayColors, titleBarOverlay } from "./titleBarOverlay";
import {
  MAIN_WINDOW_MINIMUM_HEIGHT,
  MAIN_WINDOW_MINIMUM_WIDTH,
  restoredMainWindowBounds,
} from "./windowState";

let handlingFatalError = false;

function fatalErrorMessage(error: unknown, capture: RuntimeCrashCaptureResult): string {
  const errorObject = error instanceof Error ? error : new Error(String(error));
  const reason = `${errorObject.name}: ${errorObject.message}`;
  if (capture.reportPath !== null) {
    return `sing-box stopped unexpectedly.\n\n${reason}\n\nCrash report:\n${capture.reportPath}`;
  }
  return `sing-box stopped unexpectedly.\n\n${reason}\n\nThe crash report could not be saved:\n${capture.saveError ?? "unknown error"}`;
}

function handleFatal(kind: string, error: unknown): never {
  if (handlingFatalError) {
    process.exit(1);
  }
  handlingFatalError = true;
  const capture = captureRuntimeCrash(kind, error);
  const message = fatalErrorMessage(error, capture);
  try {
    dialog.showErrorBox("sing-box", message);
  } catch (dialogError) {
    process.stderr.write(`${message}\n\nFailed to show the error dialog: ${String(dialogError)}\n`);
  }
  process.exit(1);
}
process.on("uncaughtException", (error) => handleFatal("uncaughtException", error));
process.on("unhandledRejection", (reason) => handleFatal("unhandledRejection", reason));

// Electron selects native Wayland when WAYLAND_DISPLAY is set, where the
// custom tray menu window cannot be positioned; under XWayland it can. Ozone
// is initialized before the main script runs and the resolved platform is
// appended to the command line, so appendSwitch cannot change it anymore.
if (
  process.platform === "linux" &&
  process.env.DISPLAY !== undefined &&
  app.commandLine.getSwitchValue("ozone-platform") === "wayland" &&
  !process.argv.some((argument) => argument.startsWith("--ozone-platform")) &&
  process.env.ELECTRON_OZONE_PLATFORM_HINT === undefined
) {
  app.relaunch({ args: process.argv.slice(1).concat("--ozone-platform=x11") });
  app.exit(0);
}

if (process.platform === "linux" && app.commandLine.getSwitchValue("ozone-platform") === "x11") {
  applyDisplayScaleFactor();
}

configureApplicationPaths(developmentSwitchValue("user-data"));

crashReporter.start({ submitURL: "", uploadToServer: false, compress: false });

const testScriptPath = developmentSwitchValue("test-script");

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
    titleBarOverlay: titleBarOverlay(),
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
  window.webContents.on("preload-error", (_event, preloadPath, error) => {
    handleFatal(
      "main-window-preload-error",
      new Error(`preload script failed: ${preloadPath}`, { cause: error }),
    );
  });
  window.webContents.on("render-process-gone", (_event, details) => {
    if (!quitting && details.reason !== "clean-exit") {
      handleFatal(
        "render-process-gone",
        new Error(`renderer ${details.reason} (exit code ${details.exitCode})`),
      );
    }
  });
  const rendererURL = developmentRendererURL();
  let loadPromise: Promise<void>;
  if (rendererURL !== "") {
    loadPromise = window.loadURL(rendererURL);
  } else {
    loadPromise = window.loadFile(join(import.meta.dirname, "../renderer/index.html"));
  }
  void loadPromise.catch((error: unknown) => handleFatal("main-window-load", error));
  attachTestInstrumentation(window);
  mainWindow = window;
  window.on("closed", () => {
    if (mainWindow === window) {
      mainWindow = null;
    }
    maybeQuitAfterWindowClosed();
  });
  return window;
}

let mainWindow: BrowserWindow | null = null;
let terminalWindows: Set<BrowserWindow> | null = null;
let profileEditorWindows: Set<BrowserWindow> | null = null;
let quitting = false;

function maybeQuitAfterWindowClosed() {
  if (
    !quitting &&
    mainWindow === null &&
    !terminalWindows?.size &&
    !profileEditorWindows?.size &&
    !trayInBackground()
  ) {
    app.quit();
  }
}

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

function parseDeepLink(link: string): URL | null {
  let parsed: URL;
  try {
    parsed = new URL(link);
  } catch {
    return null;
  }
  if (parsed.protocol !== "sing-box:") {
    return null;
  }
  if (parsed.host !== "") {
    return parsed;
  }
  try {
    return new URL(`sing-box://${parsed.pathname}${parsed.search}${parsed.hash}`);
  } catch {
    return null;
  }
}

function parseImportLink(link: string): DeepLinkImport | null {
  const parsed = parseDeepLink(link);
  if (parsed === null || parsed.host !== "import-remote-profile") {
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

function notificationRoute(parsed: URL): string | null {
  if (parsed.host !== "taildrop") {
    return null;
  }
  const endpointTag = parsed.searchParams.get("endpoint");
  if (!endpointTag) {
    return null;
  }
  return `tools/tailscale/${encodeURIComponent(endpointTag)}/taildrop`;
}

function handleNotificationOpen(openURL: string) {
  const deepLink = parseDeepLink(openURL);
  if (deepLink !== null) {
    const route = notificationRoute(deepLink);
    if (route !== null) {
      sendWhenLoaded(APP_NAVIGATE, route);
    }
    return;
  }
  let external: URL;
  try {
    external = new URL(openURL);
  } catch {
    return;
  }
  if (external.protocol === "http:" || external.protocol === "https:") {
    void shell.openExternal(openURL);
  }
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

const TAILDROP_SEND_REQUEST_LIFETIME = 60_000;

function deliverTaildropSend(files: TaildropSendFile[]) {
  const window = showWindow();
  if (!window.webContents.isLoading()) {
    window.webContents.send(TAILDROP_SEND_REQUEST, files);
    return;
  }
  const deadline = Date.now() + TAILDROP_SEND_REQUEST_LIFETIME;
  window.webContents.once("did-finish-load", () => {
    if (Date.now() <= deadline) {
      window.webContents.send(TAILDROP_SEND_REQUEST, files);
    }
  });
}

const queueTaildropSend = createTaildropSendBatcher(deliverTaildropSend);

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
  if (process.platform === "win32") {
    app.setAppUserModelId("io.nekohasekai.sfw");
  }

  app.on("second-instance", (_event, argv, workingDirectory) => {
    const link = deepLinkFromArguments(argv);
    const profileFile = profileFileFromArguments(argv);
    const taildropPaths = taildropSendPaths(argv, workingDirectory);
    if (!hasLoginItemArgument(argv) || link || profileFile || taildropPaths.length > 0) {
      showWindow();
    }
    if (link) {
      handleDeepLink(link);
    }
    if (profileFile) {
      handleProfileFile(profileFile);
    }
    queueTaildropSend(taildropPaths);
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
    terminalWindows = registerTerminalWindows(maybeQuitAfterWindowClosed);
    profileEditorWindows = registerProfileEditorWindows(maybeQuitAfterWindowClosed);
    registerApplication(showWindow);
    ipcMain.on(APP_TITLE_BAR_OVERLAY, (event, colors: TitleBarOverlayColors) => {
      if (process.platform === "darwin") {
        return;
      }
      const window = BrowserWindow.fromWebContents(event.sender);
      if (
        window !== null &&
        (window === mainWindow ||
          terminalWindows?.has(window) === true ||
          profileEditorWindows?.has(window) === true)
      ) {
        applyTitleBarOverlayColors(window, colors);
      }
    });
    registerDaemonBridge();
    registerOpenConnectBrowser();
    registerSetup(() => daemonState.retryConnection());
    registerCore();
    registerReports();
    registerPreferences();
    registerProfiles();
    registerServers();
    registerSettings(updateTrayVisibility);
    registerTaildrop();
    registerNotifications(handleNotificationOpen);
    registerUpdates();
    const link = deepLinkFromArguments(process.argv);
    const profileFile = profileFileFromArguments(process.argv);
    const taildropPaths = taildropSendPaths(process.argv, process.cwd());
    const startInTray =
      wasOpenedAtLogin() &&
      trayEnabled() &&
      trayInBackground() &&
      !link &&
      !profileFile &&
      taildropPaths.length === 0;
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
    queueTaildropSend(taildropPaths);
    void runStartupUpdateCheck(() => sendWhenLoaded(UPDATES_PRESENT, null));
  });
}
