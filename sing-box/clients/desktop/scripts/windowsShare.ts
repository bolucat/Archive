import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

import { sync as spawnSync } from "cross-spawn";

const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const moduleDirectory = path.join(repositoryRoot, "native", "windows-share");
const manifestPath = path.join(moduleDirectory, "Cargo.toml");
const toolchainDirectory = path.join(
  repositoryRoot,
  "bin",
  "windows-share-toolchain",
);
const cargoTargetDirectory = path.join(toolchainDirectory, "cargo-target");
const xwinVersion = "0.9.0";
const windowsSdkVersion = "10.0.26100";
const windowsCrtVersion = "14.44.17.14";

const architectures = {
  x64: {
    rustTarget: "x86_64-pc-windows-msvc",
    xwin: "x86_64",
  },
  ia32: {
    rustTarget: "i686-pc-windows-msvc",
    xwin: "x86",
  },
  arm64: {
    rustTarget: "aarch64-pc-windows-msvc",
    xwin: "aarch64",
  },
} as const;

type WindowsArchitecture = keyof typeof architectures;

function runChecked(
  command: string,
  commandArguments: string[],
  environment: NodeJS.ProcessEnv = process.env,
) {
  const result = spawnSync(command, commandArguments, {
    cwd: repositoryRoot,
    env: environment,
    stdio: "inherit",
  });
  if (result.error) {
    throw new Error(`${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`${command} exited with code ${result.status ?? 1}`);
  }
}

function commandOutput(command: string, commandArguments: string[]): string {
  const result = spawnSync(command, commandArguments, {
    cwd: repositoryRoot,
    encoding: "utf-8",
  });
  if (result.error) {
    throw new Error(`${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`${command} exited with code ${result.status ?? 1}`);
  }
  return result.stdout.trim();
}

function rustSysroot(): string {
  return commandOutput("rustc", ["--print", "sysroot"]);
}

function ensureRustTarget(target: string) {
  const targetLibraryDirectory = path.join(
    rustSysroot(),
    "lib",
    "rustlib",
    target,
    "lib",
  );
  if (fs.existsSync(targetLibraryDirectory)) {
    return;
  }
  runChecked("rustup", ["target", "add", target]);
}

function cargoVariable(target: string, name: string): string {
  return `CARGO_TARGET_${target.replaceAll("-", "_").toUpperCase()}_${name}`;
}

function cargoEnvironment(
  target: string,
  rustFlags: string[],
  linker?: string,
): NodeJS.ProcessEnv {
  const environment: NodeJS.ProcessEnv = {
    ...process.env,
    CARGO_TARGET_DIR: cargoTargetDirectory,
    [cargoVariable(target, "RUSTFLAGS")]: rustFlags.join(" "),
  };
  if (linker !== undefined) {
    environment[cargoVariable(target, "LINKER")] = linker;
  }
  return environment;
}

function xwinExecutablePath(): string {
  return path.join(toolchainDirectory, "xwin", "bin", "xwin");
}

function ensureXwin() {
  const executablePath = xwinExecutablePath();
  if (fs.existsSync(executablePath)) {
    const installedVersion = commandOutput(executablePath, ["--version"]);
    if (installedVersion === `xwin ${xwinVersion}`) {
      return;
    }
    fs.rmSync(path.dirname(path.dirname(executablePath)), {
      recursive: true,
      force: true,
    });
  }
  runChecked("cargo", [
    "install",
    "xwin",
    "--locked",
    "--version",
    xwinVersion,
    "--root",
    path.join(toolchainDirectory, "xwin"),
  ]);
}

function ensureWindowsSdk(): string {
  ensureXwin();
  const outputPath = path.join(toolchainDirectory, "sdk");
  const versionPath = path.join(outputPath, ".versions.json");
  const expectedPaths = Object.values(architectures).map((architecture) =>
    path.join(
      outputPath,
      "sdk",
      "lib",
      "um",
      architecture.xwin,
      "windowsapp.lib",
    ),
  );
  const expectedVersions = JSON.stringify({
    crt: windowsCrtVersion,
    sdk: windowsSdkVersion,
  });
  if (
    expectedPaths.every((filePath) => fs.existsSync(filePath)) &&
    fs.existsSync(versionPath) &&
    fs.readFileSync(versionPath, "utf-8") === expectedVersions
  ) {
    return outputPath;
  }
  fs.rmSync(outputPath, { recursive: true, force: true });
  runChecked(xwinExecutablePath(), [
    "--accept-license",
    "--arch",
    "x86,x86_64,aarch64",
    "--sdk-version",
    windowsSdkVersion,
    "--crt-version",
    windowsCrtVersion,
    "--cache-dir",
    path.join(toolchainDirectory, "xwin-cache"),
    "splat",
    "--output",
    outputPath,
  ]);
  fs.writeFileSync(versionPath, expectedVersions);
  return outputPath;
}

function rustLinker(): string {
  const version = commandOutput("rustc", ["-vV"]);
  const host = /^host: (.+)$/mu.exec(version)?.[1];
  if (host === undefined) {
    throw new Error("rustc did not report its host target");
  }
  const rustLinkerPath = path.join(
    rustSysroot(),
    "lib",
    "rustlib",
    host,
    "bin",
    "rust-lld",
  );
  if (!fs.existsSync(rustLinkerPath)) {
    throw new Error(`Rust linker does not exist: ${rustLinkerPath}`);
  }
  const linkerPath = path.join(toolchainDirectory, "bin", "lld-link");
  fs.mkdirSync(path.dirname(linkerPath), { recursive: true });
  fs.rmSync(linkerPath, { force: true });
  fs.symlinkSync(rustLinkerPath, linkerPath);
  return linkerPath;
}

function cargoOutputPath(target: string): string {
  return path.join(
    cargoTargetDirectory,
    target,
    "release",
    "windows_share.dll",
  );
}

function buildWithCargo(
  target: string,
  outputPath: string,
  environment: NodeJS.ProcessEnv,
) {
  runChecked(
    "cargo",
    [
      "build",
      "--manifest-path",
      manifestPath,
      "--release",
      "--target",
      target,
    ],
    environment,
  );
  const builtPath = cargoOutputPath(target);
  if (!fs.existsSync(builtPath)) {
    throw new Error(`Windows sharing module does not exist: ${builtPath}`);
  }
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.copyFileSync(builtPath, outputPath);
}

function crossCompile(
  architecture: WindowsArchitecture,
  outputPath: string,
) {
  const target = architectures[architecture];
  ensureRustTarget(target.rustTarget);
  const sdkRoot = ensureWindowsSdk();
  const libraryDirectories = [
    path.join(sdkRoot, "crt", "lib", target.xwin),
    path.join(sdkRoot, "sdk", "lib", "ucrt", target.xwin),
    path.join(sdkRoot, "sdk", "lib", "um", target.xwin),
  ];
  const rustFlags = [
    "-Ctarget-feature=+crt-static",
    "-Clink-arg=/Brepro",
    "-Clink-arg=/ignore:4099",
    ...libraryDirectories.map((directory) => `-Lnative=${directory}`),
  ];
  buildWithCargo(
    target.rustTarget,
    outputPath,
    cargoEnvironment(target.rustTarget, rustFlags, rustLinker()),
  );
}

function buildOnWindows(
  architecture: WindowsArchitecture,
  outputPath: string,
) {
  const target = architectures[architecture].rustTarget;
  ensureRustTarget(target);
  buildWithCargo(
    target,
    outputPath,
    cargoEnvironment(target, [
      "-Ctarget-feature=+crt-static",
      "-Clink-arg=/Brepro",
    ]),
  );
}

export async function buildWindowsShareModule(
  architecture: string,
  outputPath: string,
) {
  if (!(architecture in architectures)) {
    throw new Error(`unsupported Windows sharing architecture: ${architecture}`);
  }
  const supportedArchitecture = architecture as WindowsArchitecture;
  if (process.platform === "win32") {
    buildOnWindows(supportedArchitecture, outputPath);
  } else {
    crossCompile(supportedArchitecture, outputPath);
  }
}
