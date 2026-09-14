import { app } from "electron";

export function developmentSwitchValue(name: string): string {
  if (app.isPackaged) {
    return "";
  }
  return app.commandLine.getSwitchValue(name);
}

export function developmentRendererURL(): string {
  return app.isPackaged ? "" : (process.env.ELECTRON_RENDERER_URL ?? "");
}
