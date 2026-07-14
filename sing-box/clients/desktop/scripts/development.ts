import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

import { findSingBoxDirectory } from "./sing-box";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const singBoxDirectory = findSingBoxDirectory();

const scriptArguments = process.argv.slice(2);
if (scriptArguments[0] === "--") {
  scriptArguments.shift();
}

const commandLine = parseArgs({
  args: scriptArguments,
  options: {
    "daemon-socket": { type: "string" },
    "user-data": { type: "string" },
    "test-script": { type: "string" },
  },
}).values;

let interrupted = false;
let daemonProcess: ChildProcess | null = null;
let applicationProcess: ChildProcess | null = null;

function killApplicationGroup(signal: NodeJS.Signals) {
  if (applicationProcess?.pid === undefined) {
    return;
  }
  try {
    process.kill(-applicationProcess.pid, signal);
  } catch {
    applicationProcess.kill(signal);
  }
}

function handleSignal() {
  interrupted = true;
  killApplicationGroup("SIGTERM");
  daemonProcess?.kill("SIGTERM");
}

process.on("SIGINT", handleSignal);
process.on("SIGTERM", handleSignal);

function runChecked(command: string, commandArguments: string[], workingDirectory = repositoryRoot) {
  const result = spawnSync(command, commandArguments, { cwd: workingDirectory, stdio: "inherit" });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function ensureGenerated() {
  if (!fs.existsSync(path.join(repositoryRoot, "dashboard", "package.json"))) {
    console.error("dashboard submodule is not initialized, run: git submodule update --init --recursive");
    process.exit(1);
  }
  if (!fs.existsSync(path.join(repositoryRoot, "dashboard", "node_modules"))) {
    runChecked("pnpm", ["-C", "dashboard", "install"]);
  }
  if (!fs.existsSync(path.join(repositoryRoot, "dashboard", "src", "gen"))) {
    runChecked("pnpm", ["-C", "dashboard", "generate"]);
  }
  if (!fs.existsSync(path.join(repositoryRoot, "src", "shared", "gen"))) {
    runChecked("pnpm", ["generate"]);
  }
}

function startApplication(socketPath: string): ChildProcess {
  const electronArguments = [`--daemon-socket=${socketPath}`];
  if (commandLine["user-data"]) {
    electronArguments.push(`--user-data=${commandLine["user-data"]}`);
  }
  if (commandLine["test-script"]) {
    electronArguments.push(`--test-script=${commandLine["test-script"]}`);
  }
  return spawn(
    path.join(repositoryRoot, "node_modules", ".bin", "electron-vite"),
    ["dev", "--", ...electronArguments],
    {
      cwd: repositoryRoot,
      stdio: "inherit",
      detached: true,
    },
  );
}

function processExited(child: ChildProcess): boolean {
  return child.exitCode !== null || child.signalCode !== null;
}

function waitForExit(child: ChildProcess, timeoutMilliseconds: number): Promise<boolean> {
  if (processExited(child)) {
    return Promise.resolve(true);
  }
  return new Promise((resolve) => {
    const timer = setTimeout(() => resolve(false), timeoutMilliseconds);
    child.once("exit", () => {
      clearTimeout(timer);
      resolve(true);
    });
  });
}

async function waitForSocket(socketPath: string, daemon: ChildProcess): Promise<void> {
  const deadline = Date.now() + 10000;
  while (Date.now() < deadline) {
    if (processExited(daemon)) {
      throw new Error("the daemon exited before listening");
    }
    if (fs.existsSync(socketPath)) {
      return;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error("timed out waiting for the daemon socket");
}

function applicationExitCode(application: ChildProcess): Promise<number> {
  return new Promise((resolve) => {
    application.on("exit", (code, signal) => {
      if (interrupted) {
        resolve(0);
      } else {
        resolve(code ?? (signal !== null ? 1 : 0));
      }
    });
  });
}

async function main(): Promise<number> {
  ensureGenerated();
  if (commandLine["daemon-socket"]) {
    applicationProcess = startApplication(commandLine["daemon-socket"]);
    return applicationExitCode(applicationProcess);
  }
  if (process.platform === "win32") {
    console.error("automatic daemon startup requires a unix domain socket, pass --daemon-socket <path> instead");
    return 1;
  }
  runChecked(
    "go",
    [
      "run",
      "./cmd/internal/build_boxdd",
      "-debug",
      `-output=${path.join(repositoryRoot, "bin", "sing-box-daemon")}`,
    ],
    singBoxDirectory,
  );
  const workingDirectory = fs.mkdtempSync("/tmp/sing-box-desktop-development-");
  const socketPath = path.join(workingDirectory, "daemon.sock");
  daemonProcess = spawn(
    path.join(repositoryRoot, "bin", "sing-box-daemon"),
    ["run", "--working-directory", workingDirectory, "--socket", socketPath],
    { stdio: "inherit" },
  );
  try {
    await waitForSocket(socketPath, daemonProcess);
    applicationProcess = startApplication(socketPath);
    daemonProcess.on("exit", () => {
      if (!interrupted && applicationProcess !== null && !processExited(applicationProcess)) {
        console.error("the daemon exited unexpectedly");
        killApplicationGroup("SIGTERM");
      }
    });
    return await applicationExitCode(applicationProcess);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    return 1;
  } finally {
    killApplicationGroup("SIGTERM");
    if (!processExited(daemonProcess)) {
      daemonProcess.kill("SIGTERM");
      const exited = await waitForExit(daemonProcess, 5000);
      if (!exited) {
        daemonProcess.kill("SIGKILL");
      }
    }
    fs.rmSync(workingDirectory, { recursive: true, force: true });
  }
}

process.exit(await main());
