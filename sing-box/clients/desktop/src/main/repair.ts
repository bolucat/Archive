import { app, ipcMain } from "electron";
import { execFile } from "node:child_process";
import { join } from "node:path";

import { SETUP_CALL } from "../shared/ipc";
import type { ProfilesResult } from "../shared/ipc";
import { applicationPaths } from "./applicationPaths";

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

function windowsCommandLineQuote(value: string): string {
  if (value !== "" && !/[\s"]/u.test(value)) {
    return value;
  }
  let quoted = '"';
  let backslashes = 0;
  for (const character of value) {
    if (character === "\\") {
      backslashes += 1;
      continue;
    }
    if (character === '"') {
      quoted += "\\".repeat(backslashes * 2 + 1) + '"';
      backslashes = 0;
      continue;
    }
    quoted += "\\".repeat(backslashes) + character;
    backslashes = 0;
  }
  return quoted + "\\".repeat(backslashes * 2) + '"';
}

const PKEXEC_EXIT_CODE_CANCELLED = 126;

function runElevatedLinux(commandArguments: string[]): Promise<number> {
  return new Promise((resolve, reject) => {
    execFile(
      "pkexec",
      [daemonBinaryPath(), ...commandArguments],
      { timeout: 120000 },
      (error, _stdout, stderr) => {
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
        if (exitCode === PKEXEC_EXIT_CODE_CANCELLED) {
          resolve(EXIT_CODE_CANCELLED);
          return;
        }
        const message = stderr
          .split("\n")
          .map((line) => line.trim())
          .find((line) => line !== "");
        if (message !== undefined) {
          reject(new Error(message));
          return;
        }
        resolve(exitCode);
      },
    );
  });
}

function runElevatedWindows(commandArguments: string[]): Promise<number> {
  const argumentList = commandArguments.map(windowsCommandLineQuote).join(" ");
  const script = [
    "try {",
    "$startInfo = [System.Diagnostics.ProcessStartInfo]::new()",
    `$startInfo.FileName = ${powerShellQuote(daemonBinaryPath())}`,
    `$startInfo.Arguments = ${powerShellQuote(argumentList)}`,
    "$startInfo.UseShellExecute = $true",
    "$startInfo.Verb = 'runas'",
    "$process = [System.Diagnostics.Process]::Start($startInfo)",
    "$process.WaitForExit()",
    "exit $process.ExitCode",
    "} catch {",
    "$exception = $_.Exception",
    "while ($null -ne $exception) {",
    "if ($exception -is [System.ComponentModel.Win32Exception] -and $exception.NativeErrorCode -eq 1223) {",
    `exit ${EXIT_CODE_CANCELLED}`,
    "}",
    "$exception = $exception.InnerException",
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

export async function runElevatedServiceCommand(
  commandArguments: string[],
): Promise<boolean> {
  if (process.platform !== "win32" && process.platform !== "linux") {
    throw new Error("elevated service commands are not supported on this platform");
  }
  const exitCode = await (process.platform === "linux"
    ? runElevatedLinux(commandArguments)
    : runElevatedWindows(commandArguments));
  if (exitCode === 0) {
    return true;
  }
  if (exitCode === EXIT_CODE_CANCELLED) {
    return false;
  }
  if (exitCode === EXIT_CODE_LAUNCH_FAILED) {
    throw new Error("failed to launch the elevated service command");
  }
  throw new Error(`service command failed with exit code ${exitCode}`);
}

async function repair(action: "install" | "start", onRepaired: () => void): Promise<boolean> {
  if (!repairSupported) {
    throw new Error("service repair is not supported on this platform");
  }
  const serviceAction = process.platform === "linux" && action === "install" ? "restart" : action;
  const commandArguments = ["service", serviceAction];
  if (process.platform === "win32" && action === "install") {
    commandArguments.push("--working-directory", applicationPaths().daemonData);
  }
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
