import type { DesktopBridge } from "@shared/ipc";

declare global {
  interface Window {
    desktop: DesktopBridge;
  }
}

export {};
