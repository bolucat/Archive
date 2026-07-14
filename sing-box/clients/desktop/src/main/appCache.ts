import { randomUUID } from "node:crypto";
import { mkdir, mkdtemp, readdir, rm, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";

import { app } from "electron";

export function applicationCacheDirectory(): string {
  return join(app.getPath("userData"), "application_cache");
}

async function directorySize(path: string): Promise<number> {
  let entries;
  try {
    entries = await readdir(path, { withFileTypes: true });
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") {
      return 0;
    }
    throw error;
  }
  const sizes = await Promise.all(entries.map(async (entry) => {
    const entryPath = join(path, entry.name);
    if (entry.isDirectory()) {
      return directorySize(entryPath);
    }
    if (entry.isFile()) {
      return (await stat(entryPath)).size;
    }
    return 0;
  }));
  return sizes.reduce((total, size) => total + size, 0);
}

export function applicationCacheSize(): Promise<number> {
  return directorySize(applicationCacheDirectory());
}

export function clearApplicationCache(): Promise<void> {
  return rm(applicationCacheDirectory(), { recursive: true, force: true });
}

export async function writeApplicationCacheFile(
  category: string,
  extension: string,
  data: Uint8Array | string,
): Promise<string> {
  const directory = join(applicationCacheDirectory(), category);
  await mkdir(directory, { recursive: true });
  const path = join(directory, `${randomUUID()}${extension}`);
  await writeFile(path, data);
  return path;
}

export async function createApplicationCacheTemporaryDirectory(
  category: string,
  prefix: string,
): Promise<string> {
  const directory = join(applicationCacheDirectory(), category);
  await mkdir(directory, { recursive: true });
  return mkdtemp(join(directory, prefix));
}
