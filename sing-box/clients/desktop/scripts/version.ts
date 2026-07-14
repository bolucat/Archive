import { readFileSync } from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

export function readApplicationVersion(): string {
  const versionMetadata = JSON.parse(
    readFileSync(path.join(repositoryRoot, "version.json"), "utf-8"),
  ) as { version?: unknown };
  if (typeof versionMetadata.version !== "string" || versionMetadata.version === "") {
    throw new Error("version.json contains no application version");
  }
  return versionMetadata.version;
}
