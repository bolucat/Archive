import type { BrowserWindow } from "electron";

import type { TitleBarOverlayColors } from "../shared/ipc";

const TITLE_BAR_OVERLAY_HEIGHT = 51;

// Electron's native Windows/Linux controls overlay does not follow the page theme.
let overlayColors: TitleBarOverlayColors | undefined;

export function titleBarOverlay(): Electron.TitleBarOverlay | undefined {
  if (process.platform === "darwin") {
    return undefined;
  }
  return { height: TITLE_BAR_OVERLAY_HEIGHT, ...overlayColors };
}

export function applyTitleBarOverlayColors(window: BrowserWindow, colors: TitleBarOverlayColors) {
  overlayColors = colors;
  window.setTitleBarOverlay({ ...colors, height: TITLE_BAR_OVERLAY_HEIGHT });
}
