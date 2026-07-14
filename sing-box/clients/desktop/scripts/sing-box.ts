import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

export function findSingBoxDirectory(): string {
  const candidates = [
    path.resolve(repositoryRoot, "../.."),
    path.resolve(repositoryRoot, "../sing-box"),
  ];
  for (const candidate of candidates) {
    try {
      const moduleContent = fs.readFileSync(
        path.join(candidate, "go.mod"),
        "utf-8",
      );
      if (/^module github\.com\/sagernet\/sing-box$/m.test(moduleContent)) {
        return candidate;
      }
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "ENOENT") {
        throw error;
      }
    }
  }
  throw new Error(`sing-box source not found in: ${candidates.join(", ")}`);
}
