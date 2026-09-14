import "@fontsource-variable/schibsted-grotesk/index.css";
import "@fontsource-variable/source-serif-4/index.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "@dashboard/App";
import { configurePreferenceStorage } from "@dashboard/lib/storage";
import "@dashboard/styles/globals.css";
import "@dashboard/styles/shared.css";

import { createDesktopHost } from "./host";

// Chromium serializes the registered window-controls tokens as
// "color(srgb r g b)" once color-mix() is involved and as "oklab(...)"
// while they transition, and Electron's setTitleBarOverlay parses colors
// with content::ParseCssColorString (shell/browser/native_window_views.cc),
// which accepts only the legacy forms — normalize through a canvas.
const colorContext = document
  .createElement("canvas")
  .getContext("2d", { willReadFrequently: true })!;

function legacyColor(computed: string): string {
  const value = computed.trim();
  if (value === "") {
    return "";
  }
  colorContext.fillStyle = value;
  colorContext.fillRect(0, 0, 1, 1);
  const [red, green, blue] = colorContext.getImageData(0, 0, 1, 1).data;
  return `rgb(${red}, ${green}, ${blue})`;
}

interface WindowControlsOverlay {
  getTitlebarAreaRect(): DOMRect;
  addEventListener(type: "geometrychange", listener: () => void): void;
}

function watchTitleBarOverlay() {
  if (window.desktop.platform === "darwin") {
    return;
  }
  const controlsOverlay = (
    navigator as Navigator & { windowControlsOverlay?: WindowControlsOverlay }
  ).windowControlsOverlay;
  const reportGeometry = () => {
    if (controlsOverlay === undefined) {
      return;
    }
    const titlebarArea = controlsOverlay.getTitlebarAreaRect();
    const controlsWidth = Math.max(0, window.innerWidth - titlebarArea.width);
    document.documentElement.style.setProperty("--window-controls-width", `${controlsWidth}px`);
  };
  controlsOverlay?.addEventListener("geometrychange", reportGeometry);
  reportGeometry();

  let lastColor = "";
  let lastSymbolColor = "";
  const report = () => {
    const style = getComputedStyle(document.documentElement);
    const color = legacyColor(style.getPropertyValue("--window-controls-surface"));
    const symbolColor = legacyColor(style.getPropertyValue("--window-controls-text"));
    if (!color || !symbolColor || (color === lastColor && symbolColor === lastSymbolColor)) {
      return;
    }
    lastColor = color;
    lastSymbolColor = symbolColor;
    window.desktop.app.setTitleBarOverlay({ color, symbolColor });
  };
  let frame = 0;
  const followTransition = () => {
    cancelAnimationFrame(frame);
    const startedAt = performance.now();
    const step = () => {
      report();
      if (performance.now() - startedAt < 400) {
        frame = requestAnimationFrame(step);
      }
    };
    step();
  };
  new MutationObserver(followTransition).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme", "data-scrim", "dir"],
  });
  report();
}

watchTitleBarOverlay();

const desktop = createDesktopHost();
configurePreferenceStorage(desktop.preferences);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App desktop={desktop} />
  </StrictMode>,
);
