import { app } from "electron";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

const windowsAccessControlScript = String.raw`
$ErrorActionPreference = "Stop"
$path = $env:sing_box_user_data_path
$secureExistingData = $env:sing_box_secure_existing_data -eq "1"
$user = [System.Security.Principal.WindowsIdentity]::GetCurrent().User
$principals = @(
  $user,
  [System.Security.Principal.SecurityIdentifier]::new("S-1-5-18"),
  [System.Security.Principal.SecurityIdentifier]::new("S-1-5-32-544")
)

function Set-BoxAccessControl([string] $targetPath, [bool] $directory) {
  if ($directory) {
    $security = [System.Security.AccessControl.DirectorySecurity]::new()
    $inheritance = [System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor [System.Security.AccessControl.InheritanceFlags]::ObjectInherit
  } else {
    $security = [System.Security.AccessControl.FileSecurity]::new()
    $inheritance = [System.Security.AccessControl.InheritanceFlags]::None
  }
  $security.SetOwner($user)
  $security.SetAccessRuleProtection($true, $false)
  foreach ($principal in $principals) {
    $rule = [System.Security.AccessControl.FileSystemAccessRule]::new(
      $principal,
      [System.Security.AccessControl.FileSystemRights]::FullControl,
      $inheritance,
      [System.Security.AccessControl.PropagationFlags]::None,
      [System.Security.AccessControl.AccessControlType]::Allow
    )
    [void] $security.AddAccessRule($rule)
  }
  if ($directory) {
    [System.IO.Directory]::SetAccessControl($targetPath, $security)
  } else {
    [System.IO.File]::SetAccessControl($targetPath, $security)
  }
}

$root = Get-Item -LiteralPath $path -Force
if (($root.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
  throw "The sing-box user data directory is a reparse point"
}

if ($secureExistingData) {
  $items = @(Get-ChildItem -LiteralPath $path -Force -Recurse)
  foreach ($item in $items) {
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
      throw "The sing-box user data directory contains a reparse point"
    }
  }
  foreach ($item in ($items | Sort-Object { $_.FullName.Length } -Descending)) {
    Set-BoxAccessControl $item.FullName $item.PSIsContainer
  }
}

Set-BoxAccessControl $path $true
`;

function secureLinuxApplicationUserData(userDataPath: string): void {
  const userId = process.getuid?.();
  if (userId === undefined) {
    throw new Error("the current Linux user ID is unavailable");
  }
  process.umask(0o077);
  mkdirSync(userDataPath, { recursive: true, mode: 0o700 });
  const rootInfo = lstatSync(userDataPath);
  if (
    !rootInfo.isDirectory() ||
    rootInfo.isSymbolicLink() ||
    rootInfo.uid !== userId ||
    (rootInfo.mode & 0o077) !== 0
  ) {
    throw new Error(
      "the sing-box user data path is not a private directory owned by the current user",
    );
  }
}

export function secureApplicationUserData(userDataPath: string): void {
  if (process.platform === "linux") {
    secureLinuxApplicationUserData(userDataPath);
    return;
  }
  mkdirSync(userDataPath, { recursive: true });
  if (process.platform !== "win32" || !app.isPackaged) {
    return;
  }
  const markerPath = join(userDataPath, ".access-control");
  const systemRoot = process.env.SystemRoot ?? "C:\\Windows";
  const powerShellPath = join(
    systemRoot,
    "System32",
    "WindowsPowerShell",
    "v1.0",
    "powershell.exe",
  );
  const result = spawnSync(
    powerShellPath,
    [
      "-NoProfile",
      "-NonInteractive",
      "-EncodedCommand",
      Buffer.from(windowsAccessControlScript, "utf16le").toString("base64"),
    ],
    {
      windowsHide: true,
      encoding: "utf-8",
      env: {
        ...process.env,
        sing_box_user_data_path: userDataPath,
        sing_box_secure_existing_data: existsSync(markerPath) ? "0" : "1",
      },
    },
  );
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(
      result.stderr.trim() ||
        "failed to secure the sing-box user data directory",
    );
  }
  writeFileSync(markerPath, "");
}
