import { BrowserWindow, app } from "electron";
import { existsSync } from "node:fs";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join } from "node:path";

interface WindowsShareModule {
  shareFile(windowHandle: bigint, path: string, title: string): Promise<void>;
}

const loadModule = createRequire(import.meta.url);
let loadedModule: WindowsShareModule | null = null;

function nativeWindowHandle(window: BrowserWindow): bigint {
  const handle = window.getNativeWindowHandle();
  if (handle.byteLength === 8) {
    return handle.readBigUInt64LE();
  }
  if (handle.byteLength === 4) {
    return BigInt(handle.readUInt32LE());
  }
  throw new Error(`unsupported native window handle size: ${handle.byteLength}`);
}

function windowsShareModule(): WindowsShareModule {
  if (loadedModule !== null) {
    return loadedModule;
  }
  const modulePath = windowsShareModulePath();
  if (!existsSync(modulePath)) {
    throw new Error(`Windows sharing module does not exist: ${modulePath}`);
  }
  loadedModule = loadModule(modulePath) as WindowsShareModule;
  return loadedModule;
}

function windowsShareModulePath(): string {
  return app.isPackaged
    ? join(process.resourcesPath, "native", "windows_share.node")
    : join(
        app.getAppPath(),
        "native",
        "windows-share",
        "build",
        "Release",
        "windows_share.node",
      );
}

export function safeFileName(fileName: string): string {
  let name = fileName
    .replace(/[\u0000-\u001f<>:"/\\|?*]/gu, "_")
    .replace(/[ .]+$/u, "");
  if (name.length === 0) {
    return "file";
  }
  const baseName = name.split(".", 1)[0];
  if (/^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/iu.test(baseName)) {
    name = `_${name}`;
  }
  const extensionPosition = name.lastIndexOf(".");
  const extension =
    extensionPosition > 0 && name.length - extensionPosition <= 20
      ? name.slice(extensionPosition)
      : "";
  let stem = name.slice(0, name.length - extension.length);
  const maximumStemLength = 180 - extension.length;
  while (stem.length > maximumStemLength) {
    const characters = Array.from(stem);
    characters.pop();
    stem = characters.join("").replace(/[ .]+$/u, "");
  }
  return stem.length === 0 ? "file" : stem + extension;
}

export function scheduleTemporaryRemoval(directory: string) {
  const timer = setTimeout(() => {
    void rm(directory, { recursive: true, force: true });
  }, 10 * 60 * 1000);
  timer.unref();
}

export async function shareFilePath(window: BrowserWindow, path: string, title: string) {
  if (process.platform !== "win32") {
    throw new Error("sharing files is only supported on Windows");
  }
  await windowsShareModule().shareFile(nativeWindowHandle(window), path, title);
}

export async function shareFile(window: BrowserWindow, fileName: string, data: Uint8Array) {
  const name = safeFileName(fileName);
  const directory = await mkdtemp(join(tmpdir(), "sing-box-share-"));
  const path = join(directory, name);
  try {
    await writeFile(path, Buffer.from(data), { mode: 0o600 });
    await shareFilePath(window, path, name);
  } catch (error) {
    await rm(directory, { recursive: true, force: true }).catch(() => undefined);
    throw error;
  }
  scheduleTemporaryRemoval(directory);
}
