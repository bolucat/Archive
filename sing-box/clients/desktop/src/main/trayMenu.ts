import { BrowserWindow, app, screen } from "electron";
import type { BrowserWindowConstructorOptions, Rectangle } from "electron";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import { join } from "node:path";

import { developmentRendererURL } from "./development";

const MENU_WIDTH = 832;
const MENU_HEIGHT = 480;
const PANEL_CENTER_NEAR = 170;
const PANEL_CENTER_FAR = MENU_WIDTH - PANEL_CENTER_NEAR;
const TASKBAR_GAP = 0;
const REOPEN_SUPPRESSION_MILLISECONDS = 250;

const LAYER_SHELL_ANCHOR_TOP = 1;
const LAYER_SHELL_ANCHOR_BOTTOM = 2;
const LAYER_SHELL_ANCHOR_LEFT = 4;
const LAYER_SHELL_ANCHOR_RIGHT = 8;
const LAYER_SHELL_FRAME_RATE = 60;
const LAYER_SHELL_AXIS_SCALE = 4;

const EVDEV_BUTTONS: Record<number, "left" | "right" | "middle"> = {
  0x110: "left",
  0x111: "right",
  0x112: "middle",
};

const EVDEV_SHIFT_KEYS = new Set([42, 54]);

const EVDEV_KEYS: Record<number, string> = {
  1: "Escape",
  15: "Tab",
  28: "Enter",
  57: "Space",
  96: "Enter",
  102: "Home",
  103: "Up",
  104: "PageUp",
  105: "Left",
  106: "Right",
  107: "End",
  108: "Down",
  109: "PageDown",
};

interface LayerShellEvent {
  kind: string;
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  usableWidth?: number;
  usableHeight?: number;
  scale?: number;
  button?: number;
  pressed?: boolean;
  key?: number;
  deltaX?: number;
  deltaY?: number;
  message?: string;
}

interface LayerShellMenu {
  locate(): void;
  show(
    anchor: number,
    marginTop: number,
    marginRight: number,
    marginBottom: number,
    marginLeft: number,
    width: number,
    height: number,
  ): void;
  present(data: Buffer, width: number, height: number): void;
  setCursor(name: string): void;
  hide(): void;
  destroy(): void;
}

interface WaylandMenuModule {
  LayerShellMenu: new (callback: (event: LayerShellEvent) => void) => LayerShellMenu;
}

const loadModule = createRequire(import.meta.url);

let menuWindow: BrowserWindow | null = null;
let menuWindowReady: Promise<void> | null = null;
let menuWindowScale = 0;
let menuState: "closed" | "opening" | "open" = "closed";
let hiddenAt = 0;
let layerShell: LayerShellMenu | null | undefined;
let pendingLocation: ((event: LayerShellEvent | null) => void) | null = null;
let shiftPressed = false;

function waylandMenuModulePath(): string {
  return app.isPackaged
    ? join(process.resourcesPath, "native", "wayland_menu.node")
    : join(app.getAppPath(), "native", "wayland-menu", "build", "Release", "wayland_menu.node");
}

function layerShellMenu(): LayerShellMenu | null {
  if (layerShell !== undefined) {
    return layerShell;
  }
  layerShell = null;
  if (process.platform !== "linux" || process.env.WAYLAND_DISPLAY === undefined) {
    return null;
  }
  const modulePath = waylandMenuModulePath();
  if (!existsSync(modulePath)) {
    console.error(`Wayland menu module does not exist: ${modulePath}`);
    return null;
  }
  try {
    const module = loadModule(modulePath) as WaylandMenuModule;
    layerShell = new module.LayerShellMenu(handleLayerShellEvent);
  } catch (error: unknown) {
    console.error("layer shell tray menu is unavailable:", error);
  }
  return layerShell;
}

function resolveLocation(event: LayerShellEvent | null) {
  const resolve = pendingLocation;
  pendingLocation = null;
  resolve?.(event);
}

function handleLayerShellEvent(event: LayerShellEvent) {
  switch (event.kind) {
    case "located":
      resolveLocation(event);
      return;
    case "dismiss":
      hideTrayMenu();
      return;
    case "error":
      console.error("layer shell tray menu failed:", event.message);
      hideTrayMenu();
      layerShell?.destroy();
      layerShell = undefined;
      destroyTrayMenuWindow();
      return;
  }
  const window = menuWindow;
  if (window === null || window.isDestroyed() || menuState !== "open") {
    return;
  }
  const contents = window.webContents;
  const x = Math.round(event.x ?? 0);
  const y = Math.round(event.y ?? 0);
  switch (event.kind) {
    case "enter":
      contents.sendInputEvent({ type: "mouseEnter", x, y });
      contents.sendInputEvent({ type: "mouseMove", x, y });
      break;
    case "motion":
      contents.sendInputEvent({ type: "mouseMove", x, y });
      break;
    case "leave":
      contents.sendInputEvent({ type: "mouseLeave", x, y });
      break;
    case "button": {
      const button = EVDEV_BUTTONS[event.button ?? 0];
      if (button !== undefined) {
        contents.sendInputEvent({
          type: event.pressed ? "mouseDown" : "mouseUp",
          x,
          y,
          button,
          clickCount: 1,
        });
      }
      break;
    }
    case "axis":
      contents.sendInputEvent({
        type: "mouseWheel",
        x,
        y,
        deltaX: -(event.deltaX ?? 0) * LAYER_SHELL_AXIS_SCALE,
        deltaY: -(event.deltaY ?? 0) * LAYER_SHELL_AXIS_SCALE,
        canScroll: true,
      });
      break;
    case "key": {
      if (EVDEV_SHIFT_KEYS.has(event.key ?? 0)) {
        shiftPressed = event.pressed === true;
        break;
      }
      const keyCode = EVDEV_KEYS[event.key ?? 0];
      if (keyCode === undefined) {
        break;
      }
      contents.sendInputEvent({
        type: event.pressed ? "keyDown" : "keyUp",
        keyCode,
        modifiers: shiftPressed ? ["shift"] : [],
      });
      if (event.pressed && (keyCode === "Enter" || keyCode === "Space")) {
        contents.sendInputEvent({ type: "char", keyCode: keyCode === "Enter" ? "\r" : " " });
      }
      break;
    }
  }
}

function createMenuWindow(options: BrowserWindowConstructorOptions): BrowserWindow {
  const window = new BrowserWindow({
    ...options,
    frame: false,
    transparent: true,
    hasShadow: false,
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    webPreferences: {
      ...options.webPreferences,
      preload: join(import.meta.dirname, "../preload/index.cjs"),
      contextIsolation: true,
      sandbox: true,
      nodeIntegration: false,
    },
  });
  window.on("close", (event) => {
    event.preventDefault();
    hideTrayMenu();
  });
  menuWindow = window;
  menuWindowReady = new Promise<void>((resolve) => {
    window.webContents.once("did-finish-load", () => resolve());
  });
  return window;
}

function loadMenuWindow(window: BrowserWindow) {
  const rendererURL = developmentRendererURL();
  if (rendererURL !== "") {
    void window.loadURL(`${rendererURL}/tray.html`);
  } else {
    void window.loadFile(join(import.meta.dirname, "../renderer/tray.html"));
  }
}

function ensureTrayMenuWindow(initialBounds: Rectangle): BrowserWindow {
  if (menuWindow !== null && !menuWindow.isDestroyed()) {
    return menuWindow;
  }
  const window = createMenuWindow({
    ...initialBounds,
    show: false,
    opacity: process.platform === "linux" ? undefined : 0,
    // Chromium creates a window that is not focusable at creation time as an
    // override-redirect window on X11, which the window manager never manages:
    // it stays unfocusable for the rest of its life, so blur is never
    // delivered, and its stacking order is out of the compositor's hands.
    // setSkipTaskbar does nothing on Linux, so the taskbar entry a focusable
    // window would get is avoided through the window type instead.
    focusable: process.platform === "linux",
    type: process.platform === "linux" ? "toolbar" : undefined,
  });
  window.on("blur", () => {
    hideTrayMenu();
  });
  window.setIgnoreMouseEvents(true);
  if (process.platform !== "linux") {
    window.showInactive();
  }
  loadMenuWindow(window);
  return window;
}

function ensureLayerShellMenuWindow(scale: number, width: number, height: number): BrowserWindow {
  if (menuWindow !== null && !menuWindow.isDestroyed()) {
    if (menuWindowScale === scale) {
      return menuWindow;
    }
    menuWindow.destroy();
  }
  const window = createMenuWindow({
    width,
    height,
    show: false,
    webPreferences: { offscreen: { deviceScaleFactor: scale } },
  });
  menuWindowScale = scale;
  window.webContents.setFrameRate(LAYER_SHELL_FRAME_RATE);
  window.webContents.stopPainting();
  window.webContents.on("paint", (_event, _dirty, image) => {
    if (menuState === "closed") {
      return;
    }
    const size = image.getSize(scale);
    layerShell?.present(image.toBitmap({ scaleFactor: scale }), size.width, size.height);
  });
  window.webContents.on("cursor-changed", (_event, type) => {
    layerShell?.setCursor(type);
  });
  loadMenuWindow(window);
  return window;
}

export function hideTrayMenu() {
  if (menuState === "closed") {
    return;
  }
  menuState = "closed";
  hiddenAt = Date.now();
  resolveLocation(null);
  if (layerShell) {
    layerShell.hide();
  }
  if (menuWindow === null || menuWindow.isDestroyed()) {
    return;
  }
  if (layerShell) {
    shiftPressed = false;
    menuWindow.webContents.sendInputEvent({ type: "mouseLeave", x: 0, y: 0 });
    menuWindow.webContents.stopPainting();
  } else {
    if (process.platform === "linux") {
      menuWindow.hide();
    } else {
      menuWindow.setOpacity(0);
    }
    menuWindow.setIgnoreMouseEvents(true);
    menuWindow.setFocusable(false);
    menuWindow.setAlwaysOnTop(false);
  }
  if (!menuWindow.webContents.isDestroyed() && !menuWindow.webContents.isLoading()) {
    void menuWindow.webContents.executeJavaScript(`
      document.documentElement.dataset.trayOpen = "false";
      document.activeElement?.blur();
    `).catch((error: unknown) => {
      console.error("failed to reset the tray menu", error);
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

interface LayerShellPlacement {
  anchor: number;
  marginTop: number;
  marginLeft: number;
  width: number;
  height: number;
  scale: number;
  cascadeSide: CascadeSide;
  verticalAlignment: VerticalAlignment;
}

function cascadeSideFor(roomLeft: number, roomRight: number): CascadeSide {
  if (roomLeft >= PANEL_CENTER_NEAR && roomRight >= PANEL_CENTER_FAR) {
    return "right";
  }
  if (roomLeft >= PANEL_CENTER_FAR && roomRight >= PANEL_CENTER_NEAR) {
    return "left";
  }
  return roomRight >= roomLeft ? "right" : "left";
}

function menuPlacement(anchor: Rectangle): MenuPlacement {
  const display = screen.getDisplayMatching(anchor);
  const workArea = display.workArea;
  const bounds = display.bounds;
  const width = Math.min(MENU_WIDTH, workArea.width);
  const height = Math.min(MENU_HEIGHT, workArea.height);
  const anchorCenterX = anchor.x + anchor.width / 2;
  const anchorCenterY = anchor.y + anchor.height / 2;
  const cascadeSide = cascadeSideFor(
    anchorCenterX - workArea.x,
    workArea.x + workArea.width - anchorCenterX,
  );
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

function layerShellPlacement(location: LayerShellEvent, fallbackAnchor: Rectangle): LayerShellPlacement {
  const outputWidth = location.width ?? MENU_WIDTH;
  const outputHeight = location.height ?? MENU_HEIGHT;
  const usableWidth = location.usableWidth ?? outputWidth;
  const usableHeight = location.usableHeight ?? outputHeight;
  let x = location.x;
  let y = location.y;
  if (x === undefined || y === undefined) {
    const bounds = screen.getDisplayMatching(fallbackAnchor).bounds;
    x = clamp(fallbackAnchor.x - bounds.x, 0, outputWidth);
    y = clamp(fallbackAnchor.y - bounds.y, 0, outputHeight);
  }
  const width = Math.min(MENU_WIDTH, usableWidth);
  const height = Math.min(MENU_HEIGHT, usableHeight);
  const scale = location.scale ?? 1;
  const onHorizontalBar = Math.min(y, outputHeight - y) <= outputHeight - usableHeight;
  const onVerticalBar = Math.min(x, outputWidth - x) <= outputWidth - usableWidth;
  if (onVerticalBar && !onHorizontalBar) {
    const left = x < outputWidth / 2;
    return {
      anchor: (left ? LAYER_SHELL_ANCHOR_LEFT : LAYER_SHELL_ANCHOR_RIGHT) | LAYER_SHELL_ANCHOR_TOP,
      marginTop: clamp(Math.round(y - height / 2), 0, usableHeight - height),
      marginLeft: 0,
      width,
      height,
      scale,
      cascadeSide: left ? "right" : "left",
      verticalAlignment: "center",
    };
  }
  const top = y < outputHeight / 2;
  const cascadeSide = cascadeSideFor(x, outputWidth - x);
  const panelCenter = cascadeSide === "right" ? PANEL_CENTER_NEAR : PANEL_CENTER_FAR;
  return {
    anchor: (top ? LAYER_SHELL_ANCHOR_TOP : LAYER_SHELL_ANCHOR_BOTTOM) | LAYER_SHELL_ANCHOR_LEFT,
    marginTop: 0,
    marginLeft: clamp(Math.round(x - panelCenter), 0, usableWidth - width),
    width,
    height,
    scale,
    cascadeSide,
    verticalAlignment: top ? "top" : "bottom",
  };
}

export function prepareTrayMenuWindow(anchor: Rectangle): BrowserWindow {
  if (layerShellMenu() !== null) {
    return ensureLayerShellMenuWindow(
      menuWindowScale || screen.getPrimaryDisplay().scaleFactor,
      MENU_WIDTH,
      MENU_HEIGHT,
    );
  }
  return ensureTrayMenuWindow(menuPlacement(anchor).bounds);
}

function prepareMenuDocument(
  window: BrowserWindow,
  cascadeSide: CascadeSide,
  verticalAlignment: VerticalAlignment,
): Promise<unknown> {
  return window.webContents.executeJavaScript(`(async () => {
    await document.fonts.ready;
    document.documentElement.dataset.trayCascadeSide = ${JSON.stringify(cascadeSide)};
    document.documentElement.dataset.trayVerticalAlignment = ${JSON.stringify(verticalAlignment)};
    document.documentElement.dataset.trayOpen = "false";
    document.documentElement.getBoundingClientRect();
  })()`);
}

async function showLayerShellTrayMenu(menu: LayerShellMenu, fallbackAnchor: Rectangle) {
  const location = await new Promise<LayerShellEvent | null>((resolve) => {
    pendingLocation = resolve;
    menu.locate();
  });
  if (location === null || menuState !== "opening") {
    return;
  }
  const placement = layerShellPlacement(location, fallbackAnchor);
  const window = ensureLayerShellMenuWindow(placement.scale, placement.width, placement.height);
  await menuWindowReady;
  if (menuState !== "opening" || window.isDestroyed()) {
    return;
  }
  window.setContentSize(placement.width, placement.height);
  await prepareMenuDocument(window, placement.cascadeSide, placement.verticalAlignment);
  if (menuState !== "opening" || window.isDestroyed()) {
    return;
  }
  menu.show(
    placement.anchor,
    placement.marginTop,
    0,
    0,
    placement.marginLeft,
    placement.width,
    placement.height,
  );
  menuState = "open";
  window.webContents.startPainting();
  window.webContents.invalidate();
  window.focus();
  window.webContents.focus();
  await window.webContents.executeJavaScript(
    `document.documentElement.dataset.trayOpen = "true"`,
  );
}

export async function showTrayMenu(anchor: Rectangle) {
  // Electron blurs the menu window before delivering the tray's right-click event.
  if (
    menuState !== "closed" ||
    Date.now() - hiddenAt < REOPEN_SUPPRESSION_MILLISECONDS
  ) {
    hideTrayMenu();
    return;
  }
  const menu = layerShellMenu();
  if (menu !== null) {
    menuState = "opening";
    await showLayerShellTrayMenu(menu, anchor).catch((error: unknown) => {
      console.error("failed to show the layer shell tray menu:", error);
      hideTrayMenu();
    });
    return;
  }
  const placement = menuPlacement(anchor);
  const window = ensureTrayMenuWindow(placement.bounds);
  menuState = "opening";
  if (menuWindowReady !== null) {
    await menuWindowReady;
  }
  if (menuState !== "opening" || window.isDestroyed()) {
    return;
  }
  await prepareMenuDocument(window, placement.cascadeSide, placement.verticalAlignment);
  if (menuState !== "opening" || window.isDestroyed()) {
    return;
  }
  window.setBounds(placement.bounds);
  window.setAlwaysOnTop(true);
  window.setFocusable(true);
  window.setSkipTaskbar(true);
  window.setIgnoreMouseEvents(false);
  if (process.platform === "linux") {
    window.show();
  } else {
    window.setOpacity(1);
  }
  menuState = "open";
  window.focus();
  await window.webContents.executeJavaScript(
    `document.documentElement.dataset.trayOpen = "true"`,
  );
}

export function destroyTrayMenuWindow() {
  resolveLocation(null);
  layerShell?.hide();
  menuWindow?.destroy();
  menuWindow = null;
  menuWindowReady = null;
  menuWindowScale = 0;
  menuState = "closed";
}

app.on("before-quit", () => {
  destroyTrayMenuWindow();
  layerShell?.destroy();
  layerShell = undefined;
});
