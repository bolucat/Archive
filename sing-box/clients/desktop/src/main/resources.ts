import { app } from "electron";
import { join } from "node:path";

export function resourcePath(...segments: string[]): string {
  if (app.isPackaged) {
    return join(process.resourcesPath, ...segments);
  }
  return join(app.getAppPath(), "resources", ...segments);
}
