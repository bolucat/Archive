import { execFile } from "node:child_process";
import { createWriteStream } from "node:fs";
import { mkdir, readdir, rename, rm, stat } from "node:fs/promises";
import { basename, join } from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import type { ReadableStream as WebReadableStream } from "node:stream/web";

import { app, BrowserWindow, ipcMain } from "electron";
import { compare, gt, prerelease, valid } from "semver";

import { InstallUpdateResult } from "../shared/gen/experimental/boxdd/desktop_service_pb";
import { UPDATES_CALL, UPDATES_STATE_CHANGED } from "../shared/ipc";
import type {
  AppUpdateInfo,
  ProfilesResult,
  UpdateInstallResult,
  UpdatesState,
  UpdateTrack,
} from "../shared/ipc";
import { applicationCacheDirectory } from "./appCache";
import { parseBooleanPreference, Preference } from "./database";
import { desktopService } from "./daemon";
import { userAgent } from "./userAgent";

const RELEASES_URL = "https://api.github.com/repos/SagerNet/sing-box/releases";
const RELEASES_PER_PAGE = 100;
const RELEASES_REQUEST_TIMEOUT_MILLISECONDS = 30_000;
const EXIT_CODE_CANCELLED = 1223;
const EXIT_CODE_LAUNCH_FAILED = 1224;

const WINDOWS_UPDATE_ARCHITECTURES: Partial<Record<NodeJS.Architecture, string[]>> = {
  arm64: ["arm64", "x64", "x86"],
  ia32: ["x86"],
  x64: ["x64"],
};
const updateArchitectureTokens = WINDOWS_UPDATE_ARCHITECTURES[process.arch];
const UPDATES_SUPPORTED = process.platform === "win32" && updateArchitectureTokens !== undefined;
const APP_IS_PRERELEASE = prerelease(__APP_VERSION__) !== null;

function parseString(value: unknown): string {
  if (typeof value !== "string") {
    throw new Error("invalid string preference");
  }
  return value;
}

function parseUpdateTrack(value: unknown): UpdateTrack {
  if (value !== "stable" && value !== "beta") {
    throw new Error("invalid update track preference");
  }
  return value;
}

const updateTrackPreference = new Preference<UpdateTrack>(
  "update_track",
  APP_IS_PRERELEASE ? "beta" : "stable",
  parseUpdateTrack,
);
const checkUpdateEnabledPreference = new Preference(
  "check_update_enabled",
  false,
  parseBooleanPreference,
);
const updateCheckPromptedPreference = new Preference(
  "update_check_prompted",
  false,
  parseBooleanPreference,
);
const cachedUpdateInfoPreference = new Preference("cached_update_info", "", parseString);
const lastShownUpdateVersionPreference = new Preference(
  "last_shown_update_version",
  "",
  parseString,
);
const githubTokenPreference = new Preference("github_token", "", parseString);

function currentTrack(): UpdateTrack {
  return updateTrackPreference.get();
}

const runtime = {
  info: null as AppUpdateInfo | null,
  checking: false,
  downloading: false,
  installing: false,
  downloadProgress: 0,
};

function updatesState(): UpdatesState {
  return {
    supported: UPDATES_SUPPORTED,
    track: currentTrack(),
    checkUpdateEnabled: checkUpdateEnabledPreference.get(),
    prompted: updateCheckPromptedPreference.get(),
    info: runtime.info,
    checking: runtime.checking,
    downloading: runtime.downloading,
    installing: runtime.installing,
    downloadProgress: runtime.downloadProgress,
  };
}

function broadcastState(): void {
  const state = updatesState();
  for (const window of BrowserWindow.getAllWindows()) {
    if (!window.webContents.isDestroyed()) {
      window.webContents.send(UPDATES_STATE_CHANGED, state);
    }
  }
}

function shouldIncludeVersion(version: string, track: UpdateTrack): boolean {
  const normalizedVersion = valid(version);
  if (normalizedVersion === null) {
    return false;
  }
  return gt(normalizedVersion, __APP_VERSION__) || (track === "stable" && APP_IS_PRERELEASE);
}

function setUpdateInfo(info: AppUpdateInfo | null): void {
  runtime.info = info;
  if (info === null) {
    cachedUpdateInfoPreference.set("");
    lastShownUpdateVersionPreference.set("");
  } else {
    cachedUpdateInfoPreference.set(JSON.stringify(info));
  }
  broadcastState();
}

function parseCachedUpdateInfo(cached: string): AppUpdateInfo | null {
  let value: unknown;
  try {
    value = JSON.parse(cached);
  } catch {
    return null;
  }
  if (typeof value !== "object" || value === null) {
    return null;
  }
  const candidate = value as Record<string, unknown>;
  if (
    typeof candidate.versionName !== "string" ||
    typeof candidate.releaseURL !== "string" ||
    typeof candidate.downloadURL !== "string" ||
    typeof candidate.releaseNotes !== "string" ||
    typeof candidate.isPrerelease !== "boolean" ||
    typeof candidate.fileSize !== "number"
  ) {
    return null;
  }
  return {
    versionName: candidate.versionName,
    releaseURL: candidate.releaseURL,
    downloadURL: candidate.downloadURL,
    releaseNotes: candidate.releaseNotes,
    isPrerelease: candidate.isPrerelease,
    fileSize: candidate.fileSize,
  };
}

function loadCachedUpdate(): boolean {
  const cached = cachedUpdateInfoPreference.get();
  if (cached === "") {
    return false;
  }
  const info = parseCachedUpdateInfo(cached);
  const track = currentTrack();
  if (
    info === null ||
    (track === "stable" && info.isPrerelease) ||
    !shouldIncludeVersion(info.versionName, track)
  ) {
    setUpdateInfo(null);
    return false;
  }
  runtime.info = info;
  return lastShownUpdateVersionPreference.get() !== info.versionName;
}

interface GitHubAsset {
  name: string;
  browser_download_url: string;
  size: number;
}

interface GitHubRelease {
  tag_name: string;
  html_url: string;
  body?: string | null;
  draft: boolean;
  prerelease: boolean;
  assets: GitHubAsset[];
}

async function fetchReleases(track: UpdateTrack, githubToken: string): Promise<GitHubRelease[]> {
  const releases: GitHubRelease[] = [];
  const headers = new Headers({
    "Accept": "application/vnd.github+json",
    "User-Agent": userAgent(),
  });
  const token = githubToken.trim();
  if (token !== "") {
    headers.set("Authorization", `token ${token}`);
  }
  let page = 1;
  for (;;) {
    const response = await fetch(
      `${RELEASES_URL}?per_page=${RELEASES_PER_PAGE}&page=${page}`,
      {
        headers,
        signal: AbortSignal.timeout(RELEASES_REQUEST_TIMEOUT_MILLISECONDS),
      },
    );
    if (!response.ok) {
      throw new Error(`fetch releases: HTTP ${response.status}`);
    }
    const pageReleases = (await response.json()) as GitHubRelease[];
    releases.push(...pageReleases);
    if (track !== "stable" || pageReleases.length < RELEASES_PER_PAGE) {
      return releases;
    }
    page += 1;
  }
}

function findWindowsAsset(assets: GitHubAsset[]): GitHubAsset | null {
  if (updateArchitectureTokens === undefined) {
    throw new Error(`unsupported Windows architecture: ${process.arch}`);
  }
  const executables = assets.filter(
    (asset) => asset.name.startsWith("SFW-") && asset.name.endsWith(".exe"),
  );
  for (const token of updateArchitectureTokens) {
    const match = executables.find((asset) => asset.name.endsWith(`-${token}.exe`));
    if (match !== undefined) {
      return match;
    }
  }
  return null;
}

async function checkForUpdate(): Promise<AppUpdateInfo | null> {
  if (!UPDATES_SUPPORTED) {
    throw new Error("updates are not supported on this platform");
  }
  if (runtime.checking) {
    throw new Error("update check already in progress");
  }
  runtime.checking = true;
  broadcastState();
  try {
    const track = currentTrack();
    const releases = await fetchReleases(track, githubTokenPreference.get());
    if (currentTrack() !== track) {
      return runtime.info;
    }
    let best: AppUpdateInfo | null = null;
    for (const release of releases) {
      if (release.draft) {
        continue;
      }
      const asset = findWindowsAsset(release.assets);
      if (asset === null) {
        continue;
      }
      if (track === "stable" && release.prerelease) {
        continue;
      }
      const version = release.tag_name.startsWith("v")
        ? release.tag_name.slice(1)
        : release.tag_name;
      if (!shouldIncludeVersion(version, track)) {
        continue;
      }
      if (best !== null && compare(version, best.versionName) <= 0) {
        continue;
      }
      best = {
        versionName: version,
        releaseURL: release.html_url,
        downloadURL: asset.browser_download_url,
        releaseNotes: release.body ?? "",
        isPrerelease: release.prerelease,
        fileSize: asset.size,
      };
    }
    setUpdateInfo(best);
    return best;
  } finally {
    runtime.checking = false;
    broadcastState();
  }
}

function reportDownloadProgress(progress: number): void {
  if (progress < 1 && progress - runtime.downloadProgress < 0.01) {
    return;
  }
  runtime.downloadProgress = progress;
  broadcastState();
}

async function downloadUpdate(info: AppUpdateInfo): Promise<string> {
  const directory = join(applicationCacheDirectory(), "updates");
  await mkdir(directory, { recursive: true });
  const fileName = basename(new URL(info.downloadURL).pathname);
  if (fileName === "" || fileName === "." || fileName === "..") {
    throw new Error("invalid update download URL");
  }
  const destination = join(directory, fileName);

  const existing = await stat(destination).catch(() => null);
  if (existing !== null && info.fileSize > 0 && existing.size === info.fileSize) {
    reportDownloadProgress(1);
    return destination;
  }

  const staleEntries = await readdir(directory).catch(() => [] as string[]);
  for (const entry of staleEntries) {
    if (entry !== fileName) {
      await rm(join(directory, entry), { recursive: true, force: true }).catch(() => {});
    }
  }
  await rm(destination, { force: true });

  const response = await fetch(info.downloadURL, {
    headers: { "User-Agent": userAgent() },
  });
  if (!response.ok || response.body === null) {
    throw new Error(`download update: HTTP ${response.status}`);
  }
  const totalBytes = Number(response.headers.get("content-length") ?? "") || info.fileSize;
  const temporary = `${destination}.download`;
  let written = 0;
  await pipeline(
    Readable.fromWeb(response.body as unknown as WebReadableStream),
    async function* (source: AsyncIterable<Buffer>) {
      for await (const chunk of source) {
        written += chunk.length;
        if (totalBytes > 0) {
          reportDownloadProgress(Math.min(written / totalBytes, 1));
        }
        yield chunk;
      }
    },
    createWriteStream(temporary),
  );
  if (info.fileSize > 0 && written !== info.fileSize) {
    await rm(temporary, { force: true });
    throw new Error(`downloaded update is truncated: ${written} of ${info.fileSize} bytes`);
  }
  await rename(temporary, destination);
  reportDownloadProgress(1);
  return destination;
}

function launchUpdateInstallerElevated(installerPath: string): Promise<boolean> {
  const quotedInstallerPath = `'${installerPath.replaceAll("'", "''")}'`;
  const script = [
    "$ErrorActionPreference = 'Stop'",
    "try {",
    `Start-Process -FilePath ${quotedInstallerPath} -ArgumentList '--updated','--force-run' -Verb RunAs`,
    "exit 0",
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
      { windowsHide: true },
      (error) => {
        if (error === null) {
          resolve(true);
          return;
        }
        if (typeof error.code !== "number") {
          reject(error);
          return;
        }
        if (error.code === EXIT_CODE_CANCELLED) {
          resolve(false);
          return;
        }
        reject(new Error(`failed to launch the elevated update installer (${error.code})`));
      },
    );
  });
}

function resetInstallationState(): void {
  runtime.downloading = false;
  runtime.installing = false;
  runtime.downloadProgress = 0;
  broadcastState();
}

function prepareInstallation(): AppUpdateInfo {
  if (!UPDATES_SUPPORTED) {
    throw new Error("updates are not supported on this platform");
  }
  const info = runtime.info;
  if (info === null) {
    throw new Error("no update available");
  }
  if (runtime.downloading || runtime.installing) {
    throw new Error("update installation already in progress");
  }
  runtime.downloading = true;
  runtime.installing = false;
  runtime.downloadProgress = 0;
  broadcastState();
  return info;
}

async function downloadAndInstall(): Promise<UpdateInstallResult> {
  const info = prepareInstallation();
  try {
    const installerPath = await downloadUpdate(info);
    if (desktopService === null) {
      throw new Error("daemon is not available");
    }
    runtime.downloading = false;
    runtime.installing = true;
    broadcastState();
    const response = await desktopService.installUpdate({ installerPath });
    switch (response.result) {
      case InstallUpdateResult.STARTED:
        app.quit();
        return "started";
      case InstallUpdateResult.SIGNER_MISMATCH:
        resetInstallationState();
        return "signer-mismatch";
      case InstallUpdateResult.NOT_NEWER:
        resetInstallationState();
        return "not-newer";
      default:
        throw new Error("daemon returned an invalid update installation result");
    }
  } catch (error) {
    resetInstallationState();
    throw error;
  }
}

async function installWithElevation(): Promise<boolean> {
  const info = prepareInstallation();
  try {
    const installerPath = await downloadUpdate(info);
    runtime.downloading = false;
    runtime.installing = true;
    broadcastState();
    const launched = await launchUpdateInstallerElevated(installerPath);
    if (!launched) {
      resetInstallationState();
      return false;
    }
    app.quit();
    return true;
  } catch (error) {
    resetInstallationState();
    throw error;
  }
}

const handlers: Record<string, (...callArguments: never[]) => Promise<unknown>> = {
  async state(): Promise<UpdatesState> {
    return updatesState();
  },

  async check(): Promise<AppUpdateInfo | null> {
    return checkForUpdate();
  },

  async getGitHubToken(): Promise<string> {
    return githubTokenPreference.get();
  },

  async setGitHubToken(value: string): Promise<void> {
    const token = parseString(value).trim();
    githubTokenPreference.set(token === "" ? null : token);
  },

  downloadAndInstall,

  installWithElevation,

  async setTrack(track: UpdateTrack): Promise<void> {
    const parsed = parseUpdateTrack(track);
    updateTrackPreference.set(parsed);
    if (runtime.info !== null && parsed === "stable" && runtime.info.isPrerelease) {
      setUpdateInfo(null);
    } else {
      broadcastState();
    }
  },

  async setCheckUpdateEnabled(value: boolean): Promise<void> {
    checkUpdateEnabledPreference.set(parseBooleanPreference(value));
    updateCheckPromptedPreference.set(true);
    broadcastState();
  },

  async setPrompted(): Promise<void> {
    updateCheckPromptedPreference.set(true);
    broadcastState();
  },

  async markShown(): Promise<void> {
    if (runtime.info !== null) {
      lastShownUpdateVersionPreference.set(runtime.info.versionName);
    }
  },
};

export function registerUpdates(): void {
  ipcMain.handle(
    UPDATES_CALL,
    async (
      _event,
      method: string,
      ...callArguments: unknown[]
    ): Promise<ProfilesResult> => {
      const handler = handlers[method];
      if (!handler) {
        return { ok: false, error: `unknown updates method: ${method}` };
      }
      try {
        const value = await handler(...(callArguments as never[]));
        return { ok: true, value };
      } catch (error) {
        return {
          ok: false,
          error: error instanceof Error ? error.message : String(error),
        };
      }
    },
  );
}

export async function runStartupUpdateCheck(present: () => void): Promise<void> {
  if (!UPDATES_SUPPORTED) {
    return;
  }
  const shouldPresentCached = loadCachedUpdate();
  if (!updateCheckPromptedPreference.get() || !checkUpdateEnabledPreference.get()) {
    return;
  }
  if (shouldPresentCached) {
    present();
  }
  let found: AppUpdateInfo | null;
  try {
    found = await checkForUpdate();
  } catch {
    return;
  }
  if (found !== null && lastShownUpdateVersionPreference.get() !== found.versionName) {
    present();
  }
}
