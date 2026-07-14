import { app, ipcMain } from "electron";
import { execFile } from "node:child_process";
import { join } from "node:path";

import { SETUP_CALL } from "../shared/ipc";
import type { ProfilesResult } from "../shared/ipc";

const EXIT_CODE_CANCELLED = 1223;
const EXIT_CODE_LAUNCH_FAILED = 1224;

const repairSupported = process.platform === "win32" || process.platform === "linux";

export function daemonBinaryPath(): string {
  const binaryName =
    process.platform === "win32" ? "sing-box-daemon.exe" : "sing-box-daemon";
  if (app.isPackaged) {
    return join(process.resourcesPath, "daemon", binaryName);
  }
  return join(app.getAppPath(), "bin", binaryName);
}

function runDaemonBinary(
  commandArguments: string[],
): Promise<{ exitCode: number; stdout: string }> {
  return new Promise((resolve, reject) => {
    execFile(
      daemonBinaryPath(),
      commandArguments,
      { timeout: 10000, windowsHide: true },
      (error, stdout) => {
        if (error && typeof error.code !== "number") {
          reject(error);
          return;
        }
        resolve({ exitCode: error === null ? 0 : (error.code as number), stdout });
      },
    );
  });
}

let cachedBundledVersion: Promise<string | null> | null = null;

export function bundledDaemonVersion(): Promise<string | null> {
  cachedBundledVersion ??= (async () => {
    try {
      const result = await runDaemonBinary(["version"]);
      if (result.exitCode !== 0) {
        return null;
      }
      const versionLine = result.stdout
        .split("\n")
        .find((line) => line.startsWith("sing-box-daemon version "));
      return versionLine?.slice("sing-box-daemon version ".length).trim() || null;
    } catch {
      return null;
    }
  })();
  return cachedBundledVersion;
}

export type ServiceProbeResult = "not-installed" | "not-running" | "running" | null;

export async function probeService(): Promise<ServiceProbeResult> {
  if (!repairSupported) {
    return null;
  }
  try {
    const result = await runDaemonBinary(["service", "status"]);
    switch (result.exitCode) {
      case 0:
        return "running";
      case 2:
        return "not-running";
      case 3:
        return "not-installed";
      default:
        return null;
    }
  } catch {
    return null;
  }
}

function powerShellQuote(value: string): string {
  return `'${value.replaceAll("'", "''")}'`;
}

const PKEXEC_EXIT_CODE_CANCELLED = 126;
const PKEXEC_EXIT_CODE_NOT_AUTHORIZED = 127;

function runElevatedLinux(commandArguments: string[]): Promise<number> {
  return new Promise((resolve, reject) => {
    execFile(
      "pkexec",
      [daemonBinaryPath(), ...commandArguments],
      { timeout: 120000 },
      (error) => {
        if (error && typeof error.code !== "number") {
          if (error.code === "ENOENT") {
            resolve(EXIT_CODE_LAUNCH_FAILED);
            return;
          }
          reject(error);
          return;
        }
        if (error === null) {
          resolve(0);
          return;
        }
        const exitCode = error.code as number;
        if (
          exitCode === PKEXEC_EXIT_CODE_CANCELLED ||
          exitCode === PKEXEC_EXIT_CODE_NOT_AUTHORIZED
        ) {
          resolve(EXIT_CODE_CANCELLED);
          return;
        }
        resolve(exitCode);
      },
    );
  });
}

function runElevatedWindows(commandArguments: string[]): Promise<number> {
  const argumentList = commandArguments.map(powerShellQuote).join(",");
  const script = [
    "try {",
    `$process = Start-Process -FilePath ${powerShellQuote(daemonBinaryPath())} -ArgumentList ${argumentList} -Verb RunAs -Wait -PassThru`,
    "exit $process.ExitCode",
    "} catch {",
    "if ($_.Exception.InnerException -is [System.ComponentModel.Win32Exception] -and $_.Exception.InnerException.NativeErrorCode -eq 1223) {",
    `exit ${EXIT_CODE_CANCELLED}`,
    "}",
    `exit ${EXIT_CODE_LAUNCH_FAILED}`,
    "}",
  ].join("\n");
  return new Promise((resolve, reject) => {
    execFile(
      "powershell.exe",
      ["-NoProfile", "-NonInteractive", "-Command", script],
      { timeout: 120000, windowsHide: true },
      (error) => {
        if (error && typeof error.code !== "number") {
          reject(error);
          return;
        }
        resolve(error === null ? 0 : (error.code as number));
      },
    );
  });
}

async function repair(action: "install" | "start", onRepaired: () => void): Promise<boolean> {
  if (!repairSupported) {
    throw new Error("service repair is not supported on this platform");
  }
  const serviceAction = process.platform === "linux" && action === "install" ? "restart" : action;
  const commandArguments = ["service", serviceAction];
  const exitCode = await (process.platform === "linux"
    ? runElevatedLinux(commandArguments)
    : runElevatedWindows(commandArguments));
  if (exitCode === 0) {
    onRepaired();
    return true;
  }
  if (exitCode === EXIT_CODE_CANCELLED) {
    return false;
  }
  if (exitCode === EXIT_CODE_LAUNCH_FAILED) {
    throw new Error("failed to launch the elevated service command");
  }
  throw new Error(`service ${serviceAction} failed with exit code ${exitCode}`);
}

export function registerSetup(onRepaired: () => void) {
  const handlers: Record<string, () => Promise<unknown>> = {
    repairInstall: () => repair("install", onRepaired),
    repairStart: () => repair("start", onRepaired),
  };
  ipcMain.handle(SETUP_CALL, async (_event, method: string): Promise<ProfilesResult> => {
    const handler = handlers[method];
    if (!handler) {
      return { ok: false, error: `unknown setup method: ${method}` };
    }
    try {
      const value = await handler();
      return { ok: true, value };
    } catch (error) {
      return { ok: false, error: error instanceof Error ? error.message : String(error) };
    }
  });
}
