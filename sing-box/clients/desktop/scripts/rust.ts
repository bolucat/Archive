import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

import { sync as spawnSync } from "cross-spawn";

const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

export function runChecked(
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

export function commandOutput(command: string, commandArguments: string[]): string {
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

export function rustSysroot(): string {
  return commandOutput("rustc", ["--print", "sysroot"]);
}

export function ensureRustTarget(target: string) {
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

export function ensureCargoTool(
  crate: string,
  version: string,
  installDirectory: string,
): string {
  const executablePath = path.join(installDirectory, "bin", crate);
  if (fs.existsSync(executablePath)) {
    if (commandOutput(executablePath, ["--version"]) === `${crate} ${version}`) {
      return executablePath;
    }
    fs.rmSync(installDirectory, { recursive: true, force: true });
  }
  runChecked("cargo", [
    "install",
    crate,
    "--locked",
    "--version",
    version,
    "--root",
    installDirectory,
  ]);
  return executablePath;
}

export function copyBuildOutput(builtPath: string, outputPath: string) {
  if (!fs.existsSync(builtPath)) {
    throw new Error(`build output does not exist: ${builtPath}`);
  }
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
  fs.copyFileSync(builtPath, outputPath);
}
