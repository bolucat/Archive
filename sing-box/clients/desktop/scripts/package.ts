import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

import spawn, { sync as spawnSync } from "cross-spawn";
import {
  Arch,
  build as buildElectronApplication,
  Platform,
} from "electron-builder";

import { findBoxDirectory } from "./sing-box";
import { configureReproducibleBuild } from "./reproducibility";
import { readApplicationVersion, readGoVersion } from "./version";
import { buildWindowsShareModule } from "./windowsShare";

const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const singBoxDirectory = findBoxDirectory();
const dashboardDirectory = path.join(repositoryRoot, "dashboard");
const signingConfigurationPath = path.join(
  repositoryRoot,
  "signing.local.json",
);
const developmentPackage = process.argv[2] === "dev";
const packageModeArgumentIndex = developmentPackage ? 3 : 2;
const packageMode = process.argv[packageModeArgumentIndex] ?? "win";
const packageArguments = process.argv
  .slice(packageModeArgumentIndex + 1)
  .filter((argument) => argument !== "--");

const sourceDateEpoch = configureReproducibleBuild([
  repositoryRoot,
  singBoxDirectory,
  dashboardDirectory,
]);
const goVersion = readGoVersion();

interface WindowsSigningConfiguration {
  certificateFile: string;
  certificatePassword: string;
}

function goEnvironment(): NodeJS.ProcessEnv {
  return { ...process.env, GOTOOLCHAIN: goVersion };
}

function runChecked(
  command: string,
  commandArguments: string[],
  environment?: NodeJS.ProcessEnv,
  workingDirectory = repositoryRoot,
) {
  const result = spawnSync(command, commandArguments, {
    cwd: workingDirectory,
    stdio: "inherit",
    env: environment ?? process.env,
  });
  if (result.error) {
    throw new Error(`${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`${command} exited with code ${result.status ?? 1}`);
  }
}

function verifyGoVersion() {
  const result = spawnSync("go", ["env", "GOVERSION"], {
    cwd: singBoxDirectory,
    encoding: "utf-8",
    env: goEnvironment(),
  });
  if (result.error) {
    throw new Error(`go: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`go exited with code ${result.status ?? 1}`);
  }
  const actualVersion = result.stdout.trim();
  if (actualVersion !== goVersion) {
    throw new Error(
      `Go ${goVersion} is required, current version is ${actualVersion}`,
    );
  }
}

function runCheckedConcurrent(
  command: string,
  commandArguments: string[],
  environment?: NodeJS.ProcessEnv,
  workingDirectory = repositoryRoot,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, commandArguments, {
      cwd: workingDirectory,
      stdio: "inherit",
      env: environment ?? process.env,
    });
    child.once("error", (error) =>
      reject(new Error(`${command}: ${error.message}`)),
    );
    child.once("exit", (code, signal) => {
      if (code === 0) {
        resolve();
        return;
      }
      reject(
        new Error(
          `${command} exited with ${signal === null ? `code ${code ?? 1}` : `signal ${signal}`}`,
        ),
      );
    });
  });
}

function ensureGenerated() {
  if (!fs.existsSync(path.join(repositoryRoot, "dashboard", "package.json"))) {
    throw new Error(
      "dashboard submodule is not initialized, run: git submodule update --init --recursive",
    );
  }
  if (!fs.existsSync(path.join(repositoryRoot, "dashboard", "node_modules"))) {
    runChecked("pnpm", ["-C", "dashboard", "install", "--frozen-lockfile"]);
  }
  runChecked("pnpm", ["-C", "dashboard", "generate"]);
  runChecked("pnpm", ["generate"]);
}

function buildBoxdd(
  goOperatingSystem: string,
  goArchitecture: string,
  outputPath: string,
): Promise<void> {
  const suffix = goOperatingSystem === "windows" ? ".exe" : "";
  if (!outputPath.endsWith(suffix)) {
    throw new Error(
      `invalid ${goOperatingSystem} daemon output path: ${outputPath}`,
    );
  }
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  return runCheckedConcurrent(
    "go",
    [
      "run",
      "./cmd/internal/build_boxdd",
      `-target=${goOperatingSystem}/${goArchitecture}`,
      `-output=${outputPath}`,
    ],
    goEnvironment(),
    singBoxDirectory,
  );
}

function portableExecutableMachine(filePath: string): number {
  const executable = fs.readFileSync(filePath);
  if (
    executable.length < 64 ||
    executable[0] !== 0x4d ||
    executable[1] !== 0x5a
  ) {
    throw new Error(`${filePath} is not a Windows executable`);
  }
  const headerOffset = executable.readUInt32LE(0x3c);
  if (
    headerOffset + 6 > executable.length ||
    executable[headerOffset] !== 0x50 ||
    executable[headerOffset + 1] !== 0x45 ||
    executable[headerOffset + 2] !== 0 ||
    executable[headerOffset + 3] !== 0
  ) {
    throw new Error(`${filePath} has an invalid Windows executable header`);
  }
  return executable.readUInt16LE(headerOffset + 4);
}

function verifyPortableExecutableArchitecture(
  filePath: string,
  expectedMachine: number,
) {
  const actualMachine = portableExecutableMachine(filePath);
  if (actualMachine !== expectedMachine) {
    throw new Error(
      `${filePath} has Windows machine 0x${actualMachine.toString(16)}, expected 0x${expectedMachine.toString(16)}`,
    );
  }
}

function stageWindowsCronetLibrary(
  goArchitecture: string,
  builderArchitecture: string,
) {
  const modulePath = `github.com/sagernet/cronet-go/lib/windows_${goArchitecture}`;
  const result = spawnSync("go", ["list", "-m", "-f", "{{.Dir}}", modulePath], {
    cwd: singBoxDirectory,
    encoding: "utf-8",
    env: goEnvironment(),
  });
  if (result.error) {
    throw new Error(`go: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(
      `go exited with code ${result.status ?? 1}: ${(result.stderr || result.stdout).trim()}`,
    );
  }
  const moduleDirectory = result.stdout.trim();
  if (moduleDirectory === "") {
    throw new Error(`Go module has no source directory: ${modulePath}`);
  }
  const sourcePath = path.join(moduleDirectory, "libcronet.dll");
  if (!fs.existsSync(sourcePath)) {
    throw new Error(`Cronet library does not exist: ${sourcePath}`);
  }
  const destinationPath = path.join(
    repositoryRoot,
    "bin",
    "windows",
    builderArchitecture,
    "libcronet.dll",
  );
  fs.rmSync(destinationPath, { force: true });
  fs.copyFileSync(sourcePath, destinationPath);
  fs.chmodSync(destinationPath, 0o644);
}

function readWindowsSigningConfiguration(): WindowsSigningConfiguration {
  let value: unknown;
  try {
    value = JSON.parse(fs.readFileSync(signingConfigurationPath, "utf-8"));
  } catch (error) {
    throw new Error(`read ${path.basename(signingConfigurationPath)}`, {
      cause: error,
    });
  }
  const windows = (value as { windows?: unknown } | null)?.windows;
  if (
    typeof windows !== "object" ||
    windows === null ||
    typeof (windows as Record<string, unknown>).certificateFile !== "string" ||
    (windows as Record<string, unknown>).certificateFile === "" ||
    typeof (windows as Record<string, unknown>).certificatePassword !== "string"
  ) {
    throw new Error(
      `${path.basename(signingConfigurationPath)} has invalid Windows signing settings`,
    );
  }
  const configuration = windows as unknown as WindowsSigningConfiguration;
  const certificateFile = path.isAbsolute(configuration.certificateFile)
    ? configuration.certificateFile
    : path.resolve(repositoryRoot, configuration.certificateFile);
  if (!fs.existsSync(certificateFile)) {
    throw new Error(
      `Windows signing certificate does not exist: ${certificateFile}`,
    );
  }
  return { ...configuration, certificateFile };
}

async function runWindowsElectronBuilder(
  architecture: Arch,
  artifactArchitecture: string,
  signingConfiguration: WindowsSigningConfiguration,
): Promise<void> {
  const artifactName = `SFW-\${version}-${artifactArchitecture}${developmentPackage ? "-dev" : ""}.\${ext}`;
  const unpackedDirectory = {
    x64: "win-unpacked",
    x86: "win-ia32-unpacked",
    arm64: "win-arm64-unpacked",
  }[artifactArchitecture];
  if (unpackedDirectory === undefined) {
    throw new Error(
      `unsupported Windows artifact architecture: ${artifactArchitecture}`,
    );
  }
  fs.rmSync(path.join(repositoryRoot, "release", unpackedDirectory), {
    recursive: true,
    force: true,
  });
  const previousBuildCacheSetting =
    process.env.ELECTRON_BUILDER_DISABLE_BUILD_CACHE;
  const previousArchiveFilter = process.env.ELECTRON_BUILDER_7Z_FILTER;
  process.env.ELECTRON_BUILDER_DISABLE_BUILD_CACHE = "true";
  if (architecture === Arch.arm64) {
    process.env.ELECTRON_BUILDER_7Z_FILTER = "BCJ2";
  }
  try {
    await buildElectronApplication({
      projectDir: repositoryRoot,
      targets: Platform.WINDOWS.createTarget("nsis", architecture),
      publish: "never",
      config: {
        compression: developmentPackage ? "store" : undefined,
        extends: path.join(repositoryRoot, "electron-builder.yml"),
        extraMetadata: { version: readApplicationVersion() },
        npmRebuild: false,
        win: {
          artifactName,
          signtoolOptions: {
            certificateFile: signingConfiguration.certificateFile,
            certificatePassword: signingConfiguration.certificatePassword,
          },
        },
        nsis: { artifactName, warningsAsErrors: false },
      },
    });
  } finally {
    if (previousBuildCacheSetting === undefined) {
      delete process.env.ELECTRON_BUILDER_DISABLE_BUILD_CACHE;
    } else {
      process.env.ELECTRON_BUILDER_DISABLE_BUILD_CACHE =
        previousBuildCacheSetting;
    }
    if (previousArchiveFilter === undefined) {
      delete process.env.ELECTRON_BUILDER_7Z_FILTER;
    } else {
      process.env.ELECTRON_BUILDER_7Z_FILTER = previousArchiveFilter;
    }
  }
}

const windowsArchitectures = [
  {
    goArchitecture: "amd64",
    builderArchitecture: Arch.x64,
    builderArchitectureName: "x64",
    artifactArchitecture: "x64",
    portableExecutableMachine: 0x8664,
    includesCronet: true,
    winDivertDriver: "WinDivert64.sys",
    includesUSBIPDrivers: true,
  },
  {
    goArchitecture: "386",
    builderArchitecture: Arch.ia32,
    builderArchitectureName: "ia32",
    artifactArchitecture: "x86",
    portableExecutableMachine: 0x014c,
    includesCronet: false,
    winDivertDriver: "WinDivert32.sys",
    includesUSBIPDrivers: false,
  },
  {
    goArchitecture: "arm64",
    builderArchitecture: Arch.arm64,
    builderArchitectureName: "arm64",
    artifactArchitecture: "arm64",
    portableExecutableMachine: 0xaa64,
    includesCronet: true,
    winDivertDriver: null,
    includesUSBIPDrivers: true,
  },
] as const;

async function packageWindowsArchitecture(artifactArchitecture: string) {
  const architecture = windowsArchitectures.find(
    (candidate) => candidate.artifactArchitecture === artifactArchitecture,
  );
  if (architecture === undefined) {
    throw new Error(`unknown Windows architecture: ${artifactArchitecture}`);
  }
  const stagedPaths = ["sing-box-daemon.exe", "windows_share.node"];
  if (architecture.includesCronet) {
    stagedPaths.push("libcronet.dll");
  }
  if (architecture.winDivertDriver !== null) {
    stagedPaths.push(architecture.winDivertDriver);
  }
  if (architecture.includesUSBIPDrivers) {
    stagedPaths.push(
      "VBoxUSB.sys",
      "VBoxUSBMon.sys",
      "usbip2_ude.sys",
      "usbip2_filter.sys",
    );
  }
  for (const stagedPath of stagedPaths) {
    verifyPortableExecutableArchitecture(
      path.join(
        repositoryRoot,
        "bin",
        "windows",
        architecture.builderArchitectureName,
        stagedPath,
      ),
      architecture.portableExecutableMachine,
    );
  }
  const signingConfiguration = readWindowsSigningConfiguration();
  const startedAt = Date.now();
  console.info(`[package:${artifactArchitecture}] electron-builder started`);
  await runWindowsElectronBuilder(
    architecture.builderArchitecture,
    architecture.artifactArchitecture,
    signingConfiguration,
  );
  console.info(
    `[package:${artifactArchitecture}] electron-builder completed in ${Math.round((Date.now() - startedAt) / 1000)}s`,
  );
}

async function packageWindows() {
  const requestedArchitectures = new Set(packageArguments);
  const supportedArchitectures = new Set<string>(
    windowsArchitectures.map(
      (architecture) => architecture.artifactArchitecture,
    ),
  );
  for (const architecture of requestedArchitectures) {
    if (!supportedArchitectures.has(architecture)) {
      throw new Error(`unknown Windows architecture: ${architecture}`);
    }
  }
  const selectedArchitectures = windowsArchitectures.filter(
    (architecture) =>
      requestedArchitectures.size === 0 ||
      requestedArchitectures.has(architecture.artifactArchitecture),
  );
  runChecked("electron-vite", ["build"]);
  console.info(
    `[package] building Windows daemons concurrently: ${selectedArchitectures.map((architecture) => architecture.artifactArchitecture).join(", ")}`,
  );
  await Promise.all(
    selectedArchitectures.map(async (architecture) => {
      const outputPath = path.join(
        repositoryRoot,
        "bin",
        "windows",
        architecture.builderArchitectureName,
        "sing-box-daemon.exe",
      );
      await buildBoxdd("windows", architecture.goArchitecture, outputPath);
      verifyPortableExecutableArchitecture(
        outputPath,
        architecture.portableExecutableMachine,
      );
      const cronetLibraryPath = path.join(
        repositoryRoot,
        "bin",
        "windows",
        architecture.builderArchitectureName,
        "libcronet.dll",
      );
      if (architecture.includesCronet) {
        stageWindowsCronetLibrary(
          architecture.goArchitecture,
          architecture.builderArchitectureName,
        );
        verifyPortableExecutableArchitecture(
          cronetLibraryPath,
          architecture.portableExecutableMachine,
        );
      } else {
        fs.rmSync(cronetLibraryPath, { force: true });
      }
    }),
  );
  console.info(
    `[package] building Windows sharing modules: ${selectedArchitectures.map((architecture) => architecture.artifactArchitecture).join(", ")}`,
  );
  for (const architecture of selectedArchitectures) {
    await buildWindowsShareModule(
      architecture.builderArchitectureName,
      path.join(
        repositoryRoot,
        "bin",
        "windows",
        architecture.builderArchitectureName,
        "windows_share.node",
      ),
    );
  }
  for (const architecture of selectedArchitectures) {
    verifyPortableExecutableArchitecture(
      path.join(
        repositoryRoot,
        "bin",
        "windows",
        architecture.builderArchitectureName,
        "windows_share.node",
      ),
      architecture.portableExecutableMachine,
    );
  }
  const buildEnvironment = {
    ...process.env,
    ELECTRON_BUILDER_DISABLE_BUILD_CACHE: "true",
  };
  console.info(
    `[package] running electron-builder concurrently: ${selectedArchitectures.map((architecture) => architecture.artifactArchitecture).join(", ")}`,
  );
  const results = await Promise.allSettled(
    selectedArchitectures.map((architecture) =>
      runCheckedConcurrent(
        "tsx",
        [
          "scripts/package.ts",
          ...(developmentPackage ? ["dev"] : []),
          "win-architecture",
          architecture.artifactArchitecture,
        ],
        buildEnvironment,
      ),
    ),
  );
  const failure = results.find(
    (result): result is PromiseRejectedResult => result.status === "rejected",
  );
  if (failure !== undefined) {
    throw failure.reason;
  }
}

const linuxArchitectures = [
  {
    goArchitecture: "amd64",
    builderArchitectureArgument: "--x64",
    artifactArchitecture: "x64",
  },
  {
    goArchitecture: "arm64",
    builderArchitectureArgument: "--arm64",
    artifactArchitecture: "arm64",
  },
  {
    goArchitecture: "arm",
    builderArchitectureArgument: "--armv7l",
    artifactArchitecture: "armv7l",
  },
] as const;

const linuxTargets = new Set(["deb", "rpm", "pacman"]);

async function packageLinux() {
  const requestedArchitectures = new Set<string>();
  const requestedTargets: string[] = [];
  const supportedArchitectures = new Set<string>(
    linuxArchitectures.map((architecture) => architecture.artifactArchitecture),
  );
  for (const argument of packageArguments) {
    if (supportedArchitectures.has(argument)) {
      requestedArchitectures.add(argument);
    } else if (linuxTargets.has(argument)) {
      requestedTargets.push(argument);
    } else {
      throw new Error(`unknown Linux package argument: ${argument}`);
    }
  }
  const selectedArchitectures = linuxArchitectures.filter(
    (architecture) =>
      requestedArchitectures.size === 0 ||
      requestedArchitectures.has(architecture.artifactArchitecture),
  );
  runChecked("electron-vite", ["build"]);
  for (const architecture of selectedArchitectures) {
    await buildBoxdd(
      "linux",
      architecture.goArchitecture,
      path.join(repositoryRoot, "bin", "sing-box-daemon"),
    );
    const argumentsList = [
      "--linux",
      ...requestedTargets,
      architecture.builderArchitectureArgument,
      "--config",
      "electron-builder.yml",
      `--config.extraMetadata.version=${readApplicationVersion()}`,
      ...(developmentPackage
        ? [
            "--config.compression=store",
            "--config.linux.artifactName=SFL-${version}-${arch}-dev.${ext}",
            "--config.pacman.artifactName=SFL-${version}-${arch}-dev.pkg.tar.zst",
          ]
        : []),
      "--publish",
      "never",
    ];
    runChecked("electron-builder", argumentsList);
  }
}

async function main(): Promise<void> {
  console.info(`[package] SOURCE_DATE_EPOCH=${sourceDateEpoch}`);
  verifyGoVersion();
  if (packageMode !== "win-architecture") {
    ensureGenerated();
  }
  switch (packageMode) {
    case "win":
      await packageWindows();
      break;
    case "win-architecture":
      await packageWindowsArchitecture(packageArguments[0] ?? "");
      break;
    case "linux":
      await packageLinux();
      break;
    default:
      throw new Error(`unknown platform: ${packageMode}`);
  }
}

try {
  await main();
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
