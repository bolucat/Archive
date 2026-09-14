import { app } from "electron";
import { mkdirSync } from "node:fs";
import { join } from "node:path";

import { readWindowsInstallationLayout } from "./installationLayout";
import { secureApplicationUserData } from "./userDataSecurity";

export interface ApplicationPaths {
  userData: string;
  daemonData: string;
}

let configuredPaths: ApplicationPaths | null = null;

export function configureApplicationPaths(developmentUserDataPath: string): ApplicationPaths {
  if (configuredPaths !== null) {
    return configuredPaths;
  }
  const defaultUserDataPath = join(app.getPath("appData"), "sing-box");
  let paths: ApplicationPaths;
  if (developmentUserDataPath !== "") {
    paths = {
      userData: developmentUserDataPath,
      daemonData:
        process.platform === "win32"
          ? "C:\\ProgramData\\sing-box-daemon"
          : "/var/lib/sing-box-daemon",
    };
  } else if (process.platform === "win32" && app.isPackaged) {
    const layout = readWindowsInstallationLayout(defaultUserDataPath);
    paths = {
      userData: layout.applicationDataDirectory,
      daemonData: layout.daemonDataDirectory,
    };
  } else {
    paths = {
      userData: defaultUserDataPath,
      daemonData:
        process.platform === "win32"
          ? "C:\\ProgramData\\sing-box-daemon"
          : "/var/lib/sing-box-daemon",
    };
  }

  const sessionDataPath = join(paths.userData, "session");
  const crashDumpsPath = join(paths.userData, "crash_dumps");
  secureApplicationUserData(paths.userData);
  mkdirSync(sessionDataPath, { recursive: true, mode: 0o700 });
  mkdirSync(crashDumpsPath, { recursive: true, mode: 0o700 });
  app.setPath("userData", paths.userData);
  app.setPath("sessionData", sessionDataPath);
  app.setPath("crashDumps", crashDumpsPath);
  configuredPaths = paths;
  return paths;
}

export function applicationPaths(): ApplicationPaths {
  if (configuredPaths === null) {
    throw new Error("application paths are not configured");
  }
  return configuredPaths;
}
