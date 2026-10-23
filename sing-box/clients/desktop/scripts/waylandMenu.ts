import * as path from "node:path";
import { fileURLToPath } from "node:url";

import {
  copyBuildOutput,
  ensureCargoTool,
  ensureRustTarget,
  runChecked,
} from "./rust";

const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
const moduleDirectory = path.join(repositoryRoot, "native", "wayland-menu");
const manifestPath = path.join(moduleDirectory, "Cargo.toml");
const toolchainDirectory = path.join(
  repositoryRoot,
  "bin",
  "wayland-menu-toolchain",
);
const cargoTargetDirectory = path.join(toolchainDirectory, "cargo-target");
const cargoZigbuildVersion = "0.23.4";
const glibcVersion = "2.28";

const rustTargets = {
  x64: "x86_64-unknown-linux-gnu",
  arm64: "aarch64-unknown-linux-gnu",
  armv7l: "armv7-unknown-linux-gnueabihf",
} as const;

export const waylandMenuSourcePaths = [
  path.join(moduleDirectory, "Cargo.lock"),
  path.join(moduleDirectory, "Cargo.toml"),
  path.join(moduleDirectory, "build.rs"),
  path.join(moduleDirectory, "src", "lib.rs"),
];

export function buildHostWaylandMenuModule(outputPath: string) {
  runChecked(
    "cargo",
    ["build", "--manifest-path", manifestPath, "--release"],
    { ...process.env, CARGO_TARGET_DIR: cargoTargetDirectory },
  );
  copyBuildOutput(
    path.join(cargoTargetDirectory, "release", "libwayland_menu.so"),
    outputPath,
  );
}

export function buildWaylandMenuModule(
  architecture: string,
  outputPath: string,
) {
  if (!(architecture in rustTargets)) {
    throw new Error(`unsupported Wayland menu architecture: ${architecture}`);
  }
  const target = rustTargets[architecture as keyof typeof rustTargets];
  ensureRustTarget(target);
  runChecked(
    ensureCargoTool(
      "cargo-zigbuild",
      cargoZigbuildVersion,
      path.join(toolchainDirectory, "cargo-zigbuild"),
    ),
    [
      "zigbuild",
      "--manifest-path",
      manifestPath,
      "--release",
      "--target",
      `${target}.${glibcVersion}`,
    ],
    { ...process.env, CARGO_TARGET_DIR: cargoTargetDirectory },
  );
  copyBuildOutput(
    path.join(cargoTargetDirectory, target, "release", "libwayland_menu.so"),
    outputPath,
  );
}
