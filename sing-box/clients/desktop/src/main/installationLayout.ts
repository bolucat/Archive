import { app } from "electron";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

export interface WindowsInstallationLayout {
  applicationDataDirectory: string;
  daemonDataDirectory: string;
}

const installationRegistryPath = String.raw`SOFTWARE\SagerNet\sing-box`;

const installationLayoutScript = String.raw`
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$registryPath = $env:sing_box_installation_registry_path
$applicationDataDirectory = $env:sing_box_default_application_data_directory
$commonApplicationData = [Environment]::GetFolderPath(
  [Environment+SpecialFolder]::CommonApplicationData
)
$daemonDataDirectory = Join-Path $commonApplicationData "sing-box-daemon"

$registryView = if ([Environment]::Is64BitOperatingSystem) {
  [Microsoft.Win32.RegistryView]::Registry64
} else {
  [Microsoft.Win32.RegistryView]::Registry32
}
$baseKey = [Microsoft.Win32.RegistryKey]::OpenBaseKey(
  [Microsoft.Win32.RegistryHive]::LocalMachine,
  $registryView
)
try {
  $installationKey = $baseKey.OpenSubKey($registryPath)
  if ($null -ne $installationKey) {
    try {
      $layoutVersion = $installationKey.GetValue("LayoutVersion", $null)
      if ($null -ne $layoutVersion) {
        if ([int]$layoutVersion -ne 2) {
          throw "Unsupported sing-box installation layout version: $layoutVersion"
        }
        $configuredApplicationDataDirectory = $installationKey.GetValue(
          "ApplicationDataDirectory",
          "",
          [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
        )
        $configuredDaemonDataDirectory = $installationKey.GetValue(
          "DaemonDataDirectory",
          "",
          [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames
        )
        if ($configuredApplicationDataDirectory -isnot [string] -or
            $configuredDaemonDataDirectory -isnot [string] -or
            [string]::IsNullOrWhiteSpace($configuredDaemonDataDirectory)) {
          throw "The sing-box installation layout is invalid."
        }
        if (-not [string]::IsNullOrWhiteSpace($configuredApplicationDataDirectory)) {
          $applicationDataDirectory = $configuredApplicationDataDirectory
        }
        $daemonDataDirectory = $configuredDaemonDataDirectory
      }
    } finally {
      $installationKey.Dispose()
    }
  }
} finally {
  $baseKey.Dispose()
}

$result = @{
  applicationDataDirectory = $applicationDataDirectory
  daemonDataDirectory = $daemonDataDirectory
} | ConvertTo-Json -Compress
[System.IO.File]::WriteAllText(
  $env:sing_box_installation_layout_output,
  $result,
  [System.Text.UTF8Encoding]::new($false)
)
`;

export function readWindowsInstallationLayout(
  defaultApplicationDataDirectory: string,
): WindowsInstallationLayout {
  if (process.platform !== "win32" || !app.isPackaged) {
    throw new Error("Windows installation layout is unavailable");
  }
  const systemRoot = process.env.SystemRoot ?? "C:\\Windows";
  const powerShellPath = join(
    systemRoot,
    "System32",
    "WindowsPowerShell",
    "v1.0",
    "powershell.exe",
  );
  const temporaryDirectory = mkdtempSync(join(tmpdir(), "sing-box-installation-layout-"));
  const outputPath = join(temporaryDirectory, "layout.json");
  try {
    const result = spawnSync(
      powerShellPath,
      [
        "-NoProfile",
        "-NonInteractive",
        "-EncodedCommand",
        Buffer.from(installationLayoutScript, "utf16le").toString("base64"),
      ],
      {
        windowsHide: true,
        encoding: "utf-8",
        env: {
          ...process.env,
          sing_box_default_application_data_directory: defaultApplicationDataDirectory,
          sing_box_installation_registry_path: installationRegistryPath,
          sing_box_installation_layout_output: outputPath,
        },
      },
    );
    if (result.error) {
      throw result.error;
    }
    if (result.status !== 0) {
      throw new Error(result.stderr.trim() || "failed to read the installation layout");
    }
    let value: unknown;
    try {
      value = JSON.parse(readFileSync(outputPath, "utf-8"));
    } catch (error) {
      throw new Error("the installation layout returned invalid data", { cause: error });
    }
    if (
      typeof value !== "object" ||
      value === null ||
      typeof (value as Record<string, unknown>).applicationDataDirectory !== "string" ||
      typeof (value as Record<string, unknown>).daemonDataDirectory !== "string"
    ) {
      throw new Error("the installation layout is invalid");
    }
    return value as unknown as WindowsInstallationLayout;
  } finally {
    rmSync(temporaryDirectory, { recursive: true, force: true });
  }
}
