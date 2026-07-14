import { app } from "electron";
import {
  copyFileSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { readFile, rm, writeFile } from "node:fs/promises";
import { join } from "node:path";

import type { CrashReportEntry, CrashReportExportOptions, CrashReportFile } from "../shared/ipc";
import { createApplicationCacheTemporaryDirectory } from "./appCache";
import { applicationService } from "./worker";

// Matches the daemon and mobile clients' timestamped directory layout.

const metadataFileName = "metadata.json";
const configurationFileName = "configuration.json";
const readMarkerFileName = ".read";
const runtimeLogFileName = "js.log";
const minidumpExtension = ".dmp";

// The daemon reads the "source" metadata field to tell a report's origin
// apart (as the mobile clients do); "Application" pairs with the daemon's
// "Daemon".
const reportSource = "Application";

function reportsDirectory(): string {
  return join(app.getPath("userData"), "crash_reports");
}

function uniqueReportPath(date: Date): string {
  // The daemon and libbox use Go's "2006-01-02T15-04-05" UTC layout.
  const pad = (value: number) => String(value).padStart(2, "0");
  const baseName =
    `${date.getUTCFullYear()}-${pad(date.getUTCMonth() + 1)}-${pad(date.getUTCDate())}` +
    `T${pad(date.getUTCHours())}-${pad(date.getUTCMinutes())}-${pad(date.getUTCSeconds())}`;
  let candidate = join(reportsDirectory(), baseName);
  for (let suffix = 1; existsSync(candidate); suffix++) {
    candidate = join(reportsDirectory(), `${baseName}-${suffix}`);
  }
  return candidate;
}

function existsSync(path: string): boolean {
  try {
    statSync(path);
    return true;
  } catch {
    return false;
  }
}

interface RuntimeCrashMetadata {
  exceptionName: string;
  exceptionReason: string;
}

export function captureRuntimeCrash(kind: string, error: unknown): void {
  const now = new Date();
  const errorObject = error instanceof Error ? error : new Error(String(error));
  const metadata: RuntimeCrashMetadata = {
    exceptionName: errorObject.name || kind,
    exceptionReason: errorObject.message,
  };
  const body =
    errorObject.stack && errorObject.stack.length > 0
      ? errorObject.stack
      : `${errorObject.name}: ${errorObject.message}`;
  try {
    const reportPath = uniqueReportPath(now);
    mkdirSync(reportPath, { recursive: true });
    writeMetadataSync(reportPath, now, metadata);
    writeFileSync(join(reportPath, runtimeLogFileName), `${kind}\n\n${body}\n`);
  } catch {
    return;
  }
}

function writeMetadataSync(reportPath: string, date: Date, extra: RuntimeCrashMetadata): void {
  const metadata: Record<string, string> = {
    source: reportSource,
    processName: app.getName(),
    processPath: process.execPath,
    appVersion: __APP_VERSION__,
    crashedAt: date.toISOString().replace(/\.\d{3}Z$/, "Z"),
    ...extra,
  };
  writeFileSync(join(reportPath, metadataFileName), JSON.stringify(metadata));
}

export function archiveNativeCrashDumps(): void {
  let dumps: string[];
  try {
    dumps = collectMinidumps(app.getPath("crashDumps"));
  } catch {
    return;
  }
  for (const dumpPath of dumps) {
    try {
      const info = statSync(dumpPath);
      const reportPath = uniqueReportPath(info.mtime);
      mkdirSync(reportPath, { recursive: true });
      writeMetadataSync(reportPath, info.mtime, {
        exceptionName: "NativeCrash",
        exceptionReason: "The application process crashed.",
      });
      copyFileSync(dumpPath, join(reportPath, `native${minidumpExtension}`));
      rmSync(dumpPath, { force: true });
    } catch {
      continue;
    }
  }
}

function collectMinidumps(root: string): string[] {
  const result: string[] = [];
  let entries;
  try {
    entries = readdirSync(root, { withFileTypes: true });
  } catch {
    return result;
  }
  for (const entry of entries) {
    const entryPath = join(root, entry.name);
    if (entry.isDirectory()) {
      result.push(...collectMinidumps(entryPath));
    } else if (entry.isFile() && entry.name.endsWith(minidumpExtension)) {
      result.push(entryPath);
    }
  }
  return result;
}

function reportPath(name: string): string {
  if (name === "" || name.includes("/") || name.includes("\\") || name === "." || name === "..") {
    throw new Error("invalid report name");
  }
  const path = join(reportsDirectory(), name);
  if (!existsSync(path) || !statSync(path).isDirectory()) {
    throw new Error("report not found");
  }
  return path;
}

function reportCrashedAt(path: string): number {
  try {
    const metadata = JSON.parse(readFileSync(join(path, metadataFileName), "utf-8")) as {
      crashedAt?: string;
    };
    if (metadata.crashedAt) {
      const parsed = Date.parse(metadata.crashedAt);
      if (!Number.isNaN(parsed)) {
        return parsed;
      }
    }
  } catch {}
  return statSync(path).mtimeMs;
}

export function list(): CrashReportEntry[] {
  let entries;
  try {
    entries = readdirSync(reportsDirectory(), { withFileTypes: true });
  } catch {
    return [];
  }
  const reports: CrashReportEntry[] = [];
  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const path = join(reportsDirectory(), entry.name);
    reports.push({
      name: entry.name,
      crashedAt: reportCrashedAt(path),
      isRead: existsSync(join(path, readMarkerFileName)),
    });
  }
  return reports;
}

export function read(name: string): CrashReportFile[] {
  const path = reportPath(name);
  const files: CrashReportFile[] = [];
  const metadataPath = join(path, metadataFileName);
  if (existsSync(metadataPath)) {
    files.push({ name: metadataFileName, content: readFileSync(metadataPath, "utf-8"), isBinary: false });
  }
  const runtimeLogPath = join(path, runtimeLogFileName);
  if (existsSync(runtimeLogPath)) {
    files.push({
      name: runtimeLogFileName,
      content: readFileSync(runtimeLogPath, "utf-8"),
      isBinary: false,
    });
  }
  for (const entry of readdirSync(path)) {
    if (entry.endsWith(minidumpExtension)) {
      files.push({ name: entry, content: "", isBinary: true });
    }
  }
  return files;
}

export function markRead(name: string): void {
  const path = reportPath(name);
  writeFileSync(join(path, readMarkerFileName), "");
}

export function remove(name: string): void {
  rmSync(reportPath(name), { recursive: true, force: true });
}

export function removeAll(): void {
  rmSync(reportsDirectory(), { recursive: true, force: true });
}

export async function exportArchive(
  name: string,
  options: CrashReportExportOptions,
): Promise<{ fileName: string; data: Uint8Array }> {
  const path = reportPath(name);
  const temporaryDirectory = await createApplicationCacheTemporaryDirectory(
    "reports",
    "application-",
  );
  try {
    const strippedPath = join(temporaryDirectory, name);
    await copyDirectory(path, strippedPath);
    await rm(join(strippedPath, readMarkerFileName), { force: true });
    // The daemon removes configuration and crash bodies under the corresponding export flags.
    if (!options.withConfiguration) {
      await rm(join(strippedPath, configurationFileName), { force: true });
    }
    if (!options.withLog) {
      await rm(join(strippedPath, runtimeLogFileName), { force: true });
      for (const entry of readdirSync(strippedPath)) {
        if (entry.endsWith(minidumpExtension)) {
          await rm(join(strippedPath, entry), { force: true });
        }
      }
    }
    const fileName = options.encrypt ? `${name}.zip.age` : `${name}.zip`;
    const archivePath = join(temporaryDirectory, fileName);
    await applicationService.archiveReport({
      sourcePath: strippedPath,
      destinationPath: archivePath,
      encrypt: options.encrypt,
    });
    const data = await readFile(archivePath);
    return { fileName, data };
  } finally {
    await rm(temporaryDirectory, { recursive: true, force: true });
  }
}

async function copyDirectory(source: string, destination: string): Promise<void> {
  mkdirSync(destination, { recursive: true });
  for (const entry of readdirSync(source, { withFileTypes: true })) {
    const sourcePath = join(source, entry.name);
    const destinationPath = join(destination, entry.name);
    if (entry.isDirectory()) {
      await copyDirectory(sourcePath, destinationPath);
    } else {
      await writeFile(destinationPath, await readFile(sourcePath));
    }
  }
}
