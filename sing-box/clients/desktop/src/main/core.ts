import { ipcMain } from "electron";

import { ServiceStatus_Type } from "../shared/gen/daemon/started_service_pb";
import { CORE_CALL } from "../shared/ipc";
import type { CoreInfo, ProfilesResult, WorkingDirectoryInfo } from "../shared/ipc";
import { desktopService, managedService } from "./daemon";
import { daemonState } from "./state";

async function info(): Promise<CoreInfo> {
  if (desktopService === null) {
    throw new Error("daemon is not available");
  }
  const daemonInfo = await desktopService.getDaemonInfo({});
  return { version: daemonInfo.version };
}

async function workingDirectory(): Promise<WorkingDirectoryInfo> {
  if (desktopService === null) {
    throw new Error("daemon is not available");
  }
  const info = await desktopService.getWorkingDirectory({});
  return { path: info.path, size: Number(info.size) };
}

async function destroyWorkingDirectory(): Promise<void> {
  if (desktopService === null || managedService === null) {
    throw new Error("daemon is not available");
  }
  const status = daemonState.status;
  if (status === ServiceStatus_Type.STARTED || status === ServiceStatus_Type.STARTING) {
    await managedService.stopService({});
  }
  await desktopService.destroyWorkingDirectory({});
}

const handlers: Record<string, () => Promise<unknown>> = {
  info,
  workingDirectory,
  destroyWorkingDirectory,
};

export function registerCore() {
  ipcMain.handle(CORE_CALL, async (_event, method: string): Promise<ProfilesResult> => {
    const handler = handlers[method];
    if (!handler) {
      return { ok: false, error: `unknown core method: ${method}` };
    }
    try {
      const value = await handler();
      return { ok: true, value };
    } catch (error) {
      return { ok: false, error: error instanceof Error ? error.message : String(error) };
    }
  });
}
