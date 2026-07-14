import { createContext, useContext, useEffect, useState } from "react";

import type { DaemonApi } from "../api/daemon";
import { loadStoredString, removeStoredValue, saveStoredString } from "../lib/storage";
import { useLatestRef } from "./useLatest";

export const ApiContext = createContext<DaemonApi | null>(null);

export function useApi(): DaemonApi {
  const api = useContext(ApiContext);
  if (!api) {
    throw new Error("missing api context");
  }
  return api;
}

export function useNow(intervalMs = 1000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), intervalMs);
    return () => clearInterval(timer);
  }, [intervalMs]);
  return now;
}

type NavigationGuard = (proceed: () => void) => void;

let navigationGuard: NavigationGuard | null = null;

export function navigate(path: string) {
  const go = () => {
    location.hash = `#/${path}`;
  };
  if (navigationGuard) {
    navigationGuard(go);
  } else {
    go();
  }
}

export function useNavigationGuard(active: boolean, onBlock: NavigationGuard) {
  const handler = useLatestRef(onBlock);
  useEffect(() => {
    if (!active) {
      return;
    }
    const guard = (proceed: () => void) => handler.current(proceed);
    navigationGuard = guard;
    const onBeforeUnload = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => {
      if (navigationGuard === guard) {
        navigationGuard = null;
      }
      window.removeEventListener("beforeunload", onBeforeUnload);
    };
  }, [active, handler]);
}

const MOBILE_QUERY = "(max-width: 720px)";

export function useIsMobile(): boolean {
  const [isMobile, setIsMobile] = useState(() => window.matchMedia(MOBILE_QUERY).matches);
  useEffect(() => {
    const media = window.matchMedia(MOBILE_QUERY);
    const onChange = () => setIsMobile(media.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  }, []);
  return isMobile;
}

export type ThemePreference = "auto" | "light" | "dark";

const THEME_KEY = "theme";

export function loadThemePreference(): ThemePreference {
  const value = loadStoredString(THEME_KEY);
  if (value === "light" || value === "dark") {
    return value;
  }
  return "auto";
}

export function saveThemePreference(preference: ThemePreference) {
  saveStoredString(THEME_KEY, preference);
}

export function applyTheme(preference: ThemePreference) {
  const dark =
    preference === "dark" ||
    (preference === "auto" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";

  const surface = getComputedStyle(document.documentElement).getPropertyValue("--surface").trim();
  if (surface) {
    document.querySelectorAll('meta[name="theme-color"]').forEach((meta) => {
      meta.setAttribute("content", surface);
    });
  }

  // iOS 26 Safari ignores theme-color and tints the status bar from the
  // .statusbar-tint probe (see globals.css). It only samples a fixed element
  // when a new node enters the render tree: color changes on a registered
  // element go unnoticed, and removals are dropped too (WebKit bug 300965).
  const tint = document.getElementById("statusbar-tint");
  if (tint) {
    const fresh = tint.cloneNode(true) as HTMLElement;
    tint.replaceWith(fresh);
    const flash = fresh.cloneNode(true) as HTMLElement;
    flash.removeAttribute("id");
    document.body.appendChild(flash);
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        flash.remove();
      });
    });
  }
}

export const ACCENT_PRESETS = [
  "default",
  "blue",
  "purple",
  "pink",
  "red",
  "orange",
  "yellow",
  "green",
  "graphite",
] as const;

export type AccentPreset = (typeof ACCENT_PRESETS)[number];

export type AccentPreference = AccentPreset | (string & {});

export function isAccentPreset(value: string): value is AccentPreset {
  return (ACCENT_PRESETS as readonly string[]).includes(value);
}

export function normalizeAccentColor(value: string): string | null {
  const hex = value.trim().replace(/^#/, "").toLowerCase();
  if (/^[0-9a-f]{3}$/.test(hex)) {
    return `#${hex[0]}${hex[0]}${hex[1]}${hex[1]}${hex[2]}${hex[2]}`;
  }
  if (/^[0-9a-f]{6}$/.test(hex)) {
    return `#${hex}`;
  }
  return null;
}

const ACCENT_KEY = "accent";

export function loadAccentPreference(): AccentPreference {
  const value = loadStoredString(ACCENT_KEY);
  if (!value) {
    return "default";
  }
  if (isAccentPreset(value)) {
    return value;
  }
  return normalizeAccentColor(value) ?? "default";
}

export function saveAccentPreference(preference: AccentPreference) {
  if (preference === "default") {
    removeStoredValue(ACCENT_KEY);
  } else {
    saveStoredString(ACCENT_KEY, preference);
  }
}

export function applyAccent(preference: AccentPreference) {
  const root = document.documentElement;
  if (isAccentPreset(preference)) {
    root.dataset.accent = preference;
    root.style.removeProperty("--custom-accent");
    root.style.removeProperty("--on-accent");
  } else {
    root.dataset.accent = "custom";
    root.style.setProperty("--custom-accent", preference);
    const channel = (index: number) => {
      const value = parseInt(preference.slice(1 + index * 2, 3 + index * 2), 16) / 255;
      return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
    };
    const luminance = 0.2126 * channel(0) + 0.7152 * channel(1) + 0.0722 * channel(2);
    root.style.setProperty("--on-accent", luminance > 0.45 ? "#1a1a1a" : "#ffffff");
  }
}

export function watchSystemTheme(getPreference: () => ThemePreference): () => void {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const onChange = () => applyTheme(getPreference());
  media.addEventListener("change", onChange);
  return () => media.removeEventListener("change", onChange);
}
