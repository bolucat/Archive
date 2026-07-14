import { BrowserWindow, app, screen } from "electron";
import type { Rectangle } from "electron";
import { join } from "node:path";

import { developmentRendererURL } from "./development";

const MENU_WIDTH = 832;
const MENU_HEIGHT = 480;
const PANEL_CENTER_NEAR = 170;
const PANEL_CENTER_FAR = MENU_WIDTH - PANEL_CENTER_NEAR;
const TASKBAR_GAP = 0;
const REOPEN_SUPPRESSION_MILLISECONDS = 250;

let menuWindow: BrowserWindow | null = null;
let menuWindowReady: Promise<void> | null = null;
let menuState: "closed" | "opening" | "open" = "closed";
let hiddenAt = 0;

function ensureTrayMenuWindow(initialBounds: Rectangle): BrowserWindow {
  if (menuWindow !== null && !menuWindow.isDestroyed()) {
    return menuWindow;
  }
  const window = new BrowserWindow({
    ...initialBounds,
    show: false,
    opacity: 0,
    frame: false,
    transparent: true,
    hasShadow: false,
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    focusable: false,
    webPreferences: {
      preload: join(import.meta.dirname, "../preload/index.cjs"),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
    },
  });
  window.on("blur", () => {
    hideTrayMenu();
  });
  window.on("close", (event) => {
    event.preventDefault();
    hideTrayMenu();
  });
  menuWindow = window;
  menuWindowReady = new Promise<void>((resolve) => {
    window.webContents.once("did-finish-load", () => resolve());
  });
  window.setIgnoreMouseEvents(true);
  window.showInactive();
  const rendererURL = developmentRendererURL();
  if (rendererURL !== "") {
    void window.loadURL(`${rendererURL}/tray.html`);
  } else {
    void window.loadFile(join(import.meta.dirname, "../renderer/tray.html"));
  }
  return window;
}

function hideTrayMenu() {
  if (menuWindow === null || menuState === "closed") {
    return;
  }
  menuState = "closed";
  hiddenAt = Date.now();
  menuWindow.setOpacity(0);
  menuWindow.setIgnoreMouseEvents(true);
  menuWindow.setFocusable(false);
  menuWindow.setAlwaysOnTop(false);
  if (!menuWindow.webContents.isDestroyed() && !menuWindow.webContents.isLoading()) {
    void menuWindow.webContents.executeJavaScript(
      `document.documentElement.dataset.trayOpen = "false"`,
    ).catch((error: unknown) => {
      console.error("failed to reset the tray menu animation", error);
    });
  }
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}

type CascadeSide = "left" | "right";
type VerticalAlignment = "top" | "center" | "bottom";

interface MenuPlacement {
  bounds: Rectangle;
  cascadeSide: CascadeSide;
  verticalAlignment: VerticalAlignment;
}

function menuPlacement(anchor: Rectangle): MenuPlacement {
  const display = screen.getDisplayMatching(anchor);
  const workArea = display.workArea;
  const bounds = display.bounds;
  const width = Math.min(MENU_WIDTH, workArea.width);
  const height = Math.min(MENU_HEIGHT, workArea.height);
  const anchorCenterX = anchor.x + anchor.width / 2;
  const anchorCenterY = anchor.y + anchor.height / 2;
  const roomLeft = anchorCenterX - workArea.x;
  const roomRight = workArea.x + workArea.width - anchorCenterX;
  const rightFits = roomLeft >= PANEL_CENTER_NEAR && roomRight >= PANEL_CENTER_FAR;
  const leftFits = roomLeft >= PANEL_CENTER_FAR && roomRight >= PANEL_CENTER_NEAR;
  const cascadeSide: CascadeSide = rightFits
    ? "right"
    : leftFits
      ? "left"
      : roomRight >= roomLeft
        ? "right"
        : "left";
  const panelCenter = cascadeSide === "right" ? PANEL_CENTER_NEAR : PANEL_CENTER_FAR;
  const insetTop = workArea.y - bounds.y;
  const insetLeft = workArea.x - bounds.x;
  const insetRight = bounds.x + bounds.width - (workArea.x + workArea.width);
  const clampX = (value: number) =>
    clamp(Math.round(value), workArea.x, workArea.x + workArea.width - width);
  const clampY = (value: number) =>
    clamp(Math.round(value), workArea.y, workArea.y + workArea.height - height);

  let x: number;
  let y: number;
  let verticalAlignment: VerticalAlignment;
  if (insetLeft > 0 && anchorCenterX < workArea.x) {
    x = workArea.x + TASKBAR_GAP;
    y = clampY(anchorCenterY - height / 2);
    verticalAlignment = "center";
  } else if (insetRight > 0 && anchorCenterX > workArea.x + workArea.width) {
    x = workArea.x + workArea.width - width - TASKBAR_GAP;
    y = clampY(anchorCenterY - height / 2);
    verticalAlignment = "center";
  } else if (insetTop > 0 && anchorCenterY < workArea.y) {
    x = clampX(anchorCenterX - panelCenter);
    y = workArea.y + TASKBAR_GAP;
    verticalAlignment = "top";
  } else {
    x = clampX(anchorCenterX - panelCenter);
    y = clampY(workArea.y + workArea.height - height - TASKBAR_GAP);
    verticalAlignment = "bottom";
  }
  return {
    bounds: { x, y, width, height },
    cascadeSide,
    verticalAlignment,
  };
}

export function prepareTrayMenuWindow(anchor: Rectangle): BrowserWindow {
  return ensureTrayMenuWindow(menuPlacement(anchor).bounds);
}

export async function showTrayMenu(anchor: Rectangle) {
  const placement = menuPlacement(anchor);
  const window = ensureTrayMenuWindow(placement.bounds);
  // Electron blurs the menu window before delivering the tray's right-click event.
  if (
    menuState !== "closed" ||
    Date.now() - hiddenAt < REOPEN_SUPPRESSION_MILLISECONDS
  ) {
    hideTrayMenu();
    return;
  }
  menuState = "opening";
  if (menuWindowReady !== null) {
    await menuWindowReady;
  }
  if (menuState !== "opening" || window.isDestroyed()) {
    return;
  }
  await window.webContents.executeJavaScript(`(async () => {
    await document.fonts.ready;
    document.documentElement.dataset.trayCascadeSide = ${JSON.stringify(placement.cascadeSide)};
    document.documentElement.dataset.trayVerticalAlignment = ${JSON.stringify(placement.verticalAlignment)};
    document.documentElement.dataset.trayOpen = "false";
    document.documentElement.getBoundingClientRect();
  })()`);
  if (menuState !== "opening" || window.isDestroyed()) {
    return;
  }
  window.setBounds(placement.bounds);
  window.setAlwaysOnTop(true);
  window.setFocusable(true);
  window.setIgnoreMouseEvents(false);
  window.setOpacity(1);
  menuState = "open";
  window.focus();
  await window.webContents.executeJavaScript(
    `document.documentElement.dataset.trayOpen = "true"`,
  );
}

export function destroyTrayMenuWindow() {
  menuWindow?.destroy();
  menuWindow = null;
  menuWindowReady = null;
  menuState = "closed";
}

app.on("before-quit", () => {
  destroyTrayMenuWindow();
});
