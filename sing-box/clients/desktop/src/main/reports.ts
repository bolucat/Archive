import { copyFile } from "node:fs/promises";

import { ConnectError } from "@connectrpc/connect";
import { app, dialog, ipcMain } from "electron";

import { REPORTS_CALL } from "../shared/ipc";
import type {
  CrashReportEntry,
  CrashReportExportOptions,
  CrashReportFile,
  OOMReportEntry,
  OOMReportFile,
  ProfilesResult,
  ReportArchive,
} from "../shared/ipc";
import { writeApplicationCacheFile } from "./appCache";
import * as appReports from "./appReports";
import { desktopService } from "./daemon";

function requireDesktopService() {
  if (desktopService === null) {
    throw new Error("daemon is not available");
  }
  return desktopService;
}

// Mobile clients likewise aggregate daemon and application reports.
const DAEMON_PREFIX = "daemon:";
const APP_PREFIX = "app:";

function routeReport(name: string): { fromApp: boolean; bareName: string } {
  if (name.startsWith(APP_PREFIX)) {
    return { fromApp: true, bareName: name.slice(APP_PREFIX.length) };
  }
  if (name.startsWith(DAEMON_PREFIX)) {
    return { fromApp: false, bareName: name.slice(DAEMON_PREFIX.length) };
  }
  throw new Error(`report name without source prefix: ${name}`);
}

const handlers: Record<string, (...callArguments: never[]) => Promise<unknown>> = {
  async list(): Promise<CrashReportEntry[]> {
    const daemonReports =
      desktopService === null
        ? []
        : (await desktopService.listCrashReports({})).reports.map((report) => ({
            name: DAEMON_PREFIX + report.name,
            crashedAt: Number(report.crashedAt),
            isRead: report.isRead,
          }));
    const applicationReports = appReports
      .list()
      .map((report) => ({ ...report, name: APP_PREFIX + report.name }));
    return [...daemonReports, ...applicationReports].sort((a, b) => b.crashedAt - a.crashedAt);
  },

  async read(name: string): Promise<CrashReportFile[]> {
    const route = routeReport(name);
    if (route.fromApp) {
      return appReports.read(route.bareName);
    }
    const result = await requireDesktopService().readCrashReport({ name: route.bareName });
    return result.files.map((file) => ({ name: file.name, content: file.content, isBinary: false }));
  },

  async markRead(name: string): Promise<void> {
    const route = routeReport(name);
    if (route.fromApp) {
      appReports.markRead(route.bareName);
      return;
    }
    await requireDesktopService().markCrashReportRead({ name: route.bareName });
  },

  async exportFile(name: string, options?: CrashReportExportOptions): Promise<boolean> {
    const exportOptions = options ?? { withConfiguration: false, withLog: true, encrypt: false };
    const archive = await createCrashReportArchive(name, exportOptions);
    return saveArchive(archive.fileName, archive.data, exportOptions.encrypt);
  },

  async createArchive(name: string, options?: CrashReportExportOptions): Promise<ReportArchive> {
    return createCrashReportArchive(
      name,
      options ?? { withConfiguration: false, withLog: true, encrypt: false },
    );
  },

  async remove(name: string): Promise<void> {
    const route = routeReport(name);
    if (route.fromApp) {
      appReports.remove(route.bareName);
      return;
    }
    await requireDesktopService().deleteCrashReport({ name: route.bareName });
  },

  async removeAll(): Promise<void> {
    appReports.removeAll();
    if (desktopService !== null) {
      await desktopService.deleteAllCrashReports({});
    }
  },

  // Matches the Apple client's 200 ms Application crash trigger delay and the
  // daemon's --debug-less refusal outside development builds.
  async triggerAppCrash(type: "js" | "native"): Promise<void> {
    if (app.isPackaged) {
      throw new Error("debug crash trigger unavailable");
    }
    setTimeout(() => {
      if (type === "native") {
        process.crash();
      } else {
        throw new Error("debug js crash");
      }
    }, 200);
  },

  async oomList(): Promise<OOMReportEntry[]> {
    const result = await requireDesktopService().listOOMReports({});
    return result.reports.map((report) => ({
      name: report.name,
      recordedAt: Number(report.recordedAt),
      isRead: report.isRead,
    }));
  },

  async oomRead(name: string): Promise<OOMReportFile[]> {
    const result = await requireDesktopService().readOOMReport({ name });
    const decoder = new TextDecoder();
    return result.files.map((file) => ({
      name: file.name,
      content: file.isProfile ? "" : decoder.decode(file.content),
      isProfile: file.isProfile,
    }));
  },

  async oomMarkRead(name: string): Promise<void> {
    await requireDesktopService().markOOMReportRead({ name });
  },

  async oomExportFile(name: string, options?: CrashReportExportOptions): Promise<boolean> {
    const exportOptions = options ?? { withConfiguration: false, withLog: true, encrypt: false };
    const archive = await createOOMReportArchive(name, exportOptions);
    return saveArchive(archive.fileName, archive.data, exportOptions.encrypt);
  },

  async oomCreateArchive(name: string, options?: CrashReportExportOptions): Promise<ReportArchive> {
    return createOOMReportArchive(
      name,
      options ?? { withConfiguration: false, withLog: true, encrypt: false },
    );
  },

  async oomRemove(name: string): Promise<void> {
    await requireDesktopService().deleteOOMReport({ name });
  },

  async oomRemoveAll(): Promise<void> {
    await requireDesktopService().deleteAllOOMReports({});
  },

  async powerList(): Promise<OOMReportEntry[]> {
    const result = await requireDesktopService().listPowerReports({});
    return result.reports.map((report) => ({
      name: report.name,
      recordedAt: Number(report.recordedAt),
      isRead: report.isRead,
    }));
  },

  async powerRead(name: string): Promise<OOMReportFile[]> {
    const result = await requireDesktopService().readPowerReport({ name });
    const decoder = new TextDecoder();
    return result.files.map((file) => ({
      name: file.name,
      content: file.isProfile ? "" : decoder.decode(file.content),
      isProfile: file.isProfile,
    }));
  },

  async powerMarkRead(name: string): Promise<void> {
    await requireDesktopService().markPowerReportRead({ name });
  },

  async powerExportFile(name: string, options?: CrashReportExportOptions): Promise<boolean> {
    const exportOptions = options ?? { withConfiguration: false, withLog: true, encrypt: false };
    const archive = await createPowerReportArchive(name, exportOptions);
    return saveArchive(archive.fileName, archive.data, exportOptions.encrypt);
  },

  async powerCreateArchive(name: string, options?: CrashReportExportOptions): Promise<ReportArchive> {
    return createPowerReportArchive(
      name,
      options ?? { withConfiguration: false, withLog: true, encrypt: false },
    );
  },

  async powerRemove(name: string): Promise<void> {
    await requireDesktopService().deletePowerReport({ name });
  },

  async powerRemoveAll(): Promise<void> {
    await requireDesktopService().deleteAllPowerReports({});
  },
};

async function createCrashReportArchive(
  name: string,
  options: CrashReportExportOptions,
): Promise<ReportArchive> {
  const route = routeReport(name);
  const archive = route.fromApp
    ? await appReports.exportArchive(route.bareName, options)
    : await requireDesktopService().exportCrashReport({
        name: route.bareName,
        withConfiguration: options.withConfiguration,
        withLog: options.withLog,
        encrypt: options.encrypt,
      });
  return {
    fileName: archive.fileName,
    data: archive.data,
    mediaType: options.encrypt ? "application/octet-stream" : "application/zip",
  };
}

async function createPowerReportArchive(
  name: string,
  options: CrashReportExportOptions,
): Promise<ReportArchive> {
  const archive = await requireDesktopService().exportPowerReport({
    name,
    withConfiguration: options.withConfiguration,
    withLog: options.withLog,
    encrypt: options.encrypt,
  });
  return {
    fileName: archive.fileName,
    data: archive.data,
    mediaType: options.encrypt ? "application/octet-stream" : "application/zip",
  };
}

async function createOOMReportArchive(
  name: string,
  options: CrashReportExportOptions,
): Promise<ReportArchive> {
  const archive = await requireDesktopService().exportOOMReport({
    name,
    withConfiguration: options.withConfiguration,
    withLog: options.withLog,
    encrypt: options.encrypt,
  });
  return {
    fileName: archive.fileName,
    data: archive.data,
    mediaType: options.encrypt ? "application/octet-stream" : "application/zip",
  };
}

async function saveArchive(fileName: string, data: Uint8Array, encrypted: boolean): Promise<boolean> {
  const result = await dialog.showSaveDialog({
    defaultPath: fileName,
    filters: encrypted
      ? [{ name: "age encrypted archive", extensions: ["age"] }]
      : [{ name: "ZIP archive", extensions: ["zip"] }],
  });
  if (result.canceled || !result.filePath) {
    return false;
  }
  const cachePath = await writeApplicationCacheFile(
    "reports",
    encrypted ? ".zip.age" : ".zip",
    data,
  );
  await copyFile(cachePath, result.filePath);
  return true;
}

export function registerReports() {
  ipcMain.handle(
    REPORTS_CALL,
    async (_event, method: string, ...callArguments: unknown[]): Promise<ProfilesResult> => {
      const handler = handlers[method];
      if (!handler) {
        return { ok: false, error: `unknown reports method: ${method}` };
      }
      try {
        const value = await handler(...(callArguments as never[]));
        return { ok: true, value };
      } catch (error) {
        if (error instanceof ConnectError) {
          return { ok: false, error: error.rawMessage };
        }
        return { ok: false, error: error instanceof Error ? error.message : String(error) };
      }
    },
  );
}
