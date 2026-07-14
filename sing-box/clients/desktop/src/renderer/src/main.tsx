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
// "color(srgb r g b)" once color-mix() is involved, and Electron's
// setTitleBarOverlay parses colors with content::ParseCssColorString
// (shell/browser/native_window_views.cc), which rejects the color()
// function — convert to the legacy rgb() form it accepts.
function legacyColor(computed: string): string {
  const value = computed.trim();
  const srgb = value.match(/^color\(srgb ([\d.]+) ([\d.]+) ([\d.]+)\)$/);
  if (!srgb) {
    return value;
  }
  const [red, green, blue] = srgb.slice(1).map((channel) => Math.round(Number(channel) * 255));
  return `rgb(${red}, ${green}, ${blue})`;
}

function watchTitleBarOverlayTheme() {
  if (window.desktop.platform === "darwin") {
    return;
  }
  const report = () => {
    const style = getComputedStyle(document.documentElement);
    const color = legacyColor(style.getPropertyValue("--window-controls-surface"));
    const symbolColor = legacyColor(style.getPropertyValue("--window-controls-text"));
    if (color && symbolColor) {
      window.desktop.app.setTitleBarOverlay({ color, symbolColor });
    }
  };
  new MutationObserver(report).observe(document.documentElement, {
    attributes: true,
    attributeFilter: ["data-theme", "data-scrim"],
  });
  report();
}

watchTitleBarOverlayTheme();

const desktop = createDesktopHost();
configurePreferenceStorage(desktop.preferences);

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App desktop={desktop} />
  </StrictMode>,
);
