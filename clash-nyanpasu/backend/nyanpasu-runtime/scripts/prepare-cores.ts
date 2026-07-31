#!/usr/bin/env -S deno run -A
/**
 * Download the four cores used by integration tests into `tests/bin/`:
 * mihomo, clash-rs, clash premium, meow.
 *
 * Usage:
 *   deno run -A scripts/prepare-cores.ts [--force] [--only mihomo,clash-rs]
 *     [--mihomo-version v1.19.29] [--clash-rs-version v0.10.8]
 *     [--clash-version 2023-09-05-gdcc8d87] [--meow-version v0.18.0]
 *
 * Versions and asset templates are kept in sync with clash-nyanpasu's
 * manifest/version.json and scripts/check.ts.
 */
import { parseArgs } from "jsr:@std/cli@1/parse-args";
import { ensureDir, exists } from "jsr:@std/fs";
import * as path from "jsr:@std/path";
// @ts-types="npm:@types/adm-zip"
import AdmZip from "npm:adm-zip";

const WORKSPACE_ROOT = path.join(import.meta.dirname!, "..");
const BIN_DIR = path.join(WORKSPACE_ROOT, "tests", "bin");

const IS_WIN = Deno.build.os === "windows";
const EXE_SUFFIX = IS_WIN ? ".exe" : "";

type ArchiveKind = "zip" | "gz" | "tar.gz" | "raw";

interface CoreDef {
  /** Target file name in tests/bin (without .exe suffix). */
  name: string;
  version: string;
  /** Download URL with the asset name as `{}`. */
  urlTemplate: string;
  /** Asset name template per `os-arch`, `{}` is the version. */
  archTemplate: Record<string, string>;
  archive: ArchiveKind;
}

// Same templates as clash-nyanpasu's manifest/version.json (arch_template.*)
const CORES: CoreDef[] = [
  {
    name: "mihomo",
    version: "v1.19.29",
    urlTemplate:
      "https://github.com/MetaCubeX/mihomo/releases/download/{ver}/{}",
    archTemplate: {
      "windows-x86_64": "mihomo-windows-amd64-v2-{}.zip",
      "windows-aarch64": "mihomo-windows-arm64-{}.zip",
      "linux-aarch64": "mihomo-linux-arm64-{}.gz",
      "linux-x86_64": "mihomo-linux-amd64-v2-{}.gz",
      "darwin-aarch64": "mihomo-darwin-arm64-{}.gz",
      "darwin-x86_64": "mihomo-darwin-amd64-v2-{}.gz",
    },
    archive: IS_WIN ? "zip" : "gz",
  },
  {
    name: "clash-rs",
    version: "v0.10.8",
    urlTemplate:
      "https://github.com/Watfaq/clash-rs/releases/download/{ver}/{}",
    archTemplate: {
      "windows-x86_64": "clash-rs-x86_64-pc-windows-msvc.exe",
      "windows-aarch64": "clash-rs-aarch64-pc-windows-msvc.exe",
      "linux-aarch64": "clash-rs-aarch64-unknown-linux-gnu",
      "linux-x86_64": "clash-rs-x86_64-unknown-linux-gnu-static-crt",
      "darwin-aarch64": "clash-rs-aarch64-apple-darwin",
      "darwin-x86_64": "clash-rs-x86_64-apple-darwin",
    },
    archive: "raw",
  },
  {
    // Dreamacro's premium release was taken down; use the community backup,
    // same as clash-nyanpasu's check.ts (getClashBackupInfo).
    name: "clash",
    version: "2023-09-05-gdcc8d87",
    urlTemplate:
      "https://github.com/zhongfly/Clash-premium-backup/releases/download/{ver}/{}",
    archTemplate: {
      "windows-x86_64": "clash-windows-amd64-n{}.zip",
      "windows-aarch64": "clash-windows-arm64-n{}.zip",
      "linux-aarch64": "clash-linux-arm64-n{}.gz",
      "linux-x86_64": "clash-linux-amd64-n{}.gz",
      "darwin-aarch64": "clash-darwin-arm64-n{}.gz",
      "darwin-x86_64": "clash-darwin-amd64-n{}.gz",
    },
    archive: IS_WIN ? "zip" : "gz",
  },
  {
    name: "meow",
    version: "v0.18.0",
    urlTemplate:
      "https://github.com/madeye/meow-rs/releases/download/{ver}/{}",
    archTemplate: {
      "windows-x86_64": "meow-{}-x86_64-pc-windows-msvc.zip",
      "windows-aarch64": "meow-{}-aarch64-pc-windows-msvc.zip",
      "linux-aarch64": "meow-{}-aarch64-unknown-linux-musl.tar.gz",
      "linux-x86_64": "meow-{}-x86_64-unknown-linux-musl.tar.gz",
      "darwin-aarch64": "meow-{}-aarch64-apple-darwin.tar.gz",
      "darwin-x86_64": "meow-{}-x86_64-apple-darwin.tar.gz",
    },
    archive: IS_WIN ? "zip" : "tar.gz",
  },
];

const args = parseArgs(Deno.args, {
  boolean: ["force"],
  string: [
    "only",
    "mihomo-version",
    "clash-rs-version",
    "clash-version",
    "meow-version",
  ],
});

for (const core of CORES) {
  const override = args[`${core.name}-version`];
  if (typeof override === "string") core.version = override;
}

const selected = args.only
  ? CORES.filter((core) => args.only!.split(",").includes(core.name))
  : CORES;
if (selected.length === 0) {
  console.error(
    `no cores selected; valid names: ${CORES.map((c) => c.name).join(", ")}`,
  );
  Deno.exit(1);
}

function formatSize(size: number): string {
  const units = ["B", "KB", "MB", "GB"];
  let value = size;
  let unitIndex = 0;
  while (value >= 1024 && unitIndex < units.length - 1) {
    value /= 1024;
    unitIndex++;
  }
  return `${value.toFixed(unitIndex === 0 ? 0 : 1)} ${units[unitIndex]}`;
}

async function writeAll(file: Deno.FsFile, bytes: Uint8Array): Promise<void> {
  let written = 0;
  while (written < bytes.byteLength) {
    written += await file.write(bytes.subarray(written));
  }
}

async function downloadFile(url: string, filePath: string): Promise<number> {
  const response = await fetch(url, {
    method: "GET",
    headers: {
      "Content-Type": "application/octet-stream",
      "User-Agent":
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:131.0) Gecko/20100101 Firefox/131.0",
    },
  });

  if (!response.ok) {
    throw new Error(
      `download failed: ${response.statusText} (${response.status})`,
    );
  }
  if (!response.body) throw new Error("download failed: empty response body");

  const totalHeader = response.headers.get("content-length");
  const total = totalHeader ? Number.parseInt(totalHeader, 10) : undefined;
  const isTTY = Deno.stdout.isTerminal();
  let downloaded = 0;
  let lastLogAt = 0;

  const file = await Deno.open(filePath, {
    create: true,
    truncate: true,
    write: true,
  });

  try {
    const reader = response.body.getReader();
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      await writeAll(file, value);
      downloaded += value.byteLength;

      const now = performance.now();
      if (isTTY && now - lastLogAt >= 200) {
        lastLogAt = now;
        const progress = total
          ? `${formatSize(downloaded)}/${formatSize(total)}`
          : formatSize(downloaded);
        await Deno.stdout.write(new TextEncoder().encode(`\r  ${progress}`));
      }
    }
  } finally {
    file.close();
  }

  if (isTTY) await Deno.stdout.write(new TextEncoder().encode("\n"));
  return downloaded;
}

function extractZip(zipPath: string, targetPath: string): void {
  const zip = new AdmZip(zipPath);
  const entries = zip.getEntries();
  const entry = entries.find((entry) =>
    IS_WIN ? entry.entryName.endsWith(".exe") : !entry.isDirectory
  );
  if (!entry) throw new Error("cannot find binary in zip");
  Deno.writeFileSync(targetPath, entry.getData());
}

async function extractTarGz(
  tarPath: string,
  destDir: string,
  targetPath: string,
): Promise<void> {
  const { code, stderr } = await new Deno.Command("tar", {
    args: ["-xzf", tarPath, "-C", destDir],
    stdout: "piped",
    stderr: "piped",
  }).output();
  if (code !== 0) {
    throw new Error(
      `tar extraction failed: ${new TextDecoder().decode(stderr)}`,
    );
  }
  // meow tarballs ship a single `meow` binary, possibly inside a directory.
  for (const entry of Deno.readDirSync(destDir)) {
    const full = path.join(destDir, entry.name);
    if (entry.isFile && entry.name.startsWith("meow")) {
      await Deno.rename(full, targetPath);
      return;
    }
    if (entry.isDirectory) {
      for (const nested of Deno.readDirSync(full)) {
        if (nested.isFile && nested.name.startsWith("meow")) {
          await Deno.rename(path.join(full, nested.name), targetPath);
          return;
        }
      }
    }
  }
  throw new Error("cannot find meow binary in tarball");
}

async function gunzipFile(
  inputPath: string,
  outputPath: string,
): Promise<void> {
  const input = await Deno.open(inputPath, { read: true });
  const output = await Deno.open(outputPath, { write: true, create: true });
  await input.readable
    .pipeThrough(new DecompressionStream("gzip"))
    .pipeTo(output.writable);
}

async function sanityCheck(targetPath: string, name: string): Promise<void> {
  // `-v` works for mihomo/clash; clash-rs/meow may only know `--version`.
  for (const flag of ["-v", "--version"]) {
    const { code, stdout, stderr } = await new Deno.Command(targetPath, {
      args: [flag],
      stdout: "piped",
      stderr: "piped",
    }).output();
    if (code === 0) {
      const out = new TextDecoder().decode(stdout).trim() ||
        new TextDecoder().decode(stderr).trim();
      console.log(`${name} ready: ${out.split("\n")[0]}`);
      console.log(`  -> ${targetPath}`);
      return;
    }
  }
  console.error(`sanity check failed: \`${name} -v/--version\` failed`);
  Deno.exit(1);
}

// === Main ===

const platformKey = `${Deno.build.os}-${Deno.build.arch}`;

await ensureDir(BIN_DIR);

for (const core of selected) {
  const targetFile = `${core.name}${EXE_SUFFIX}`;
  const targetPath = path.join(BIN_DIR, targetFile);

  if (!args.force && (await exists(targetPath))) {
    console.log(`${targetFile} already exists, skip (use --force)`);
    continue;
  }

  const template = core.archTemplate[platformKey];
  if (!template) {
    console.error(`unsupported platform for ${core.name}: ${platformKey}`);
    Deno.exit(1);
  }

  const assetName = template.replace("{}", core.version);
  const downloadURL = core.urlTemplate
    .replace("{ver}", core.version)
    .replace("{}", assetName);
  const tmpFile = path.join(BIN_DIR, assetName);
  const tmpDir = path.join(BIN_DIR, `.tmp-${core.name}`);

  try {
    console.log(`downloading ${downloadURL}`);
    const size = await downloadFile(downloadURL, tmpFile);
    console.log(`downloaded ${assetName} (${formatSize(size)})`);

    switch (core.archive) {
      case "zip":
        extractZip(tmpFile, targetPath);
        break;
      case "gz":
        await gunzipFile(tmpFile, targetPath);
        break;
      case "tar.gz":
        await ensureDir(tmpDir);
        await extractTarGz(tmpFile, tmpDir, targetPath);
        break;
      case "raw":
        // copyFile overwrites an existing target; rename cannot on Windows.
        await Deno.copyFile(tmpFile, targetPath);
        break;
    }

    if (!IS_WIN) {
      await Deno.chmod(targetPath, 0o755);
    }
  } catch (err) {
    await Deno.remove(targetPath).catch(() => {});
    throw err;
  } finally {
    await Deno.remove(tmpFile).catch(() => {});
    await Deno.remove(tmpDir, { recursive: true }).catch(() => {});
  }

  await sanityCheck(targetPath, core.name);
}

console.log("all cores ready");
