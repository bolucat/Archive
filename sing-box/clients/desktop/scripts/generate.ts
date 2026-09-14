import * as path from "node:path";
import { fileURLToPath } from "node:url";

import { sync as spawnSync } from "cross-spawn";

import { findBoxDirectory } from "./sing-box";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const bufExecutable = path.join(
  repositoryRoot,
  "node_modules",
  ".bin",
  process.platform === "win32" ? "buf.CMD" : "buf",
);

const result = spawnSync(
  bufExecutable,
  [
    "generate",
    "--template",
    path.join(repositoryRoot, "buf.gen.yaml"),
    "--output",
    repositoryRoot,
    ".",
    "--path",
    "experimental/boxdd/desktop_service.proto",
    "--path",
    "daemon/started_service.proto",
    "--path",
    "daemon/managed_service.proto",
  ],
  { cwd: findBoxDirectory(), stdio: "inherit" },
);
if (result.error) {
  throw result.error;
}
if (result.status !== 0) {
  process.exit(result.status ?? 1);
}
