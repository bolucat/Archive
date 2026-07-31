#!/usr/bin/env -S deno run -A
/**
 * Download the mihomo core into `tests/bin/`, as the base for integration tests.
 *
 * Usage:
 *   deno run -A scripts/prepare-mihomo.ts [--force] [--version v1.19.28]
 *
 * Stripped down from clash-nyanpasu's scripts/check.ts (mihomo task only).
 */
import { parseArgs } from "jsr:@std/cli@1/parse-args";
import { ensureDir, exists } from "jsr:@std/fs";
import * as path from "jsr:@std/path";
// @ts-types="npm:@types/adm-zip"
import AdmZip from "npm:adm-zip";

// Keep in sync with clash-nyanpasu's manifest/version.json (latest.mihomo)
const DEFAULT_VERSION = "v1.19.28";

const WORKSPACE_ROOT = path.join(import.meta.dirname!, "..");
const BIN_DIR = path.join(WORKSPACE_ROOT, "tests", "bin");

const args = parseArgs(Deno.args, {
  boolean: ["force"],
  string: ["version"],
});

const VERSION = args.version ?? DEFAULT_VERSION;
const IS_WIN = Deno.build.os === "windows";
const TARGET_FILE = `mihomo${IS_WIN ? ".exe" : ""}`;
const TARGET_PATH = path.join(BIN_DIR, TARGET_FILE);

// Same templates as clash-nyanpasu's manifest/version.json (arch_template.mihomo)
const ARCH_MAPPING: Record<string, string> = {
  "windows-x86_64": "mihomo-windows-amd64-v2-{}.zip",
  "windows-aarch64": "mihomo-windows-arm64-{}.zip",
  "linux-aarch64": "mihomo-linux-arm64-{}.gz",
  "linux-x86_64": "mihomo-linux-amd64-v2-{}.gz",
  "darwin-aarch64": "mihomo-darwin-arm64-{}.gz",
  "darwin-x86_64": "mihomo-darwin-amd64-v2-{}.gz",
};

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
  const startedAt = performance.now();
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
  const entry = zip.getEntries().find((entry) =>
    entry.entryName.endsWith(".exe")
  );
  if (!entry) throw new Error("cannot find exe file in zip");
  Deno.writeFileSync(targetPath, entry.getData());
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

// === Main ===

const template = ARCH_MAPPING[`${Deno.build.os}-${Deno.build.arch}`];
if (!template) {
  console.error(`unsupported platform: ${Deno.build.os}-${Deno.build.arch}`);
  Deno.exit(1);
}

if (!args.force && (await exists(TARGET_PATH))) {
  console.log(`${TARGET_FILE} already exists at ${TARGET_PATH}, skip`);
  console.log("  use --force to re-download");
  Deno.exit(0);
}

const assetName = template.replace("{}", VERSION);
const downloadURL =
  `https://github.com/MetaCubeX/mihomo/releases/download/${VERSION}/${assetName}`;

await ensureDir(BIN_DIR);
const tmpFile = path.join(BIN_DIR, assetName);

try {
  console.log(`downloading ${downloadURL}`);
  const size = await downloadFile(downloadURL, tmpFile);
  console.log(`downloaded ${assetName} (${formatSize(size)})`);

  if (assetName.endsWith(".zip")) {
    extractZip(tmpFile, TARGET_PATH);
  } else {
    await gunzipFile(tmpFile, TARGET_PATH);
  }

  if (!IS_WIN) {
    await Deno.chmod(TARGET_PATH, 0o755);
  }
} catch (err) {
  await Deno.remove(TARGET_PATH).catch(() => {});
  throw err;
} finally {
  await Deno.remove(tmpFile).catch(() => {});
}

// sanity check: the binary should run and report its version
const { code, stdout } = await new Deno.Command(TARGET_PATH, {
  args: ["-v"],
  stdout: "piped",
}).output();
if (code !== 0) {
  console.error(`sanity check failed: \`${TARGET_FILE} -v\` exited with ${code}`);
  Deno.exit(1);
}

console.log(`mihomo ready: ${new TextDecoder().decode(stdout).trim()}`);
console.log(`  -> ${TARGET_PATH}`);
