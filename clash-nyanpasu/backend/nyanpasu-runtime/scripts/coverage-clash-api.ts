#!/usr/bin/env -S deno run -A

import * as path from "jsr:@std/path";

const WORKSPACE_ROOT = path.join(import.meta.dirname!, "..");
const COVERAGE_DIR = path.join(WORKSPACE_ROOT, "target", "llvm-cov");
const HTML_DIR = path.join(COVERAGE_DIR, "clash-api-html");
const SUMMARY_PATH = path.join(COVERAGE_DIR, "clash-api-summary.json");
const LCOV_PATH = path.join(COVERAGE_DIR, "clash-api.lcov");

async function run(args: string[]): Promise<void> {
  const status = await new Deno.Command("cargo", {
    args,
    cwd: WORKSPACE_ROOT,
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  }).spawn().status;

  if (!status.success) Deno.exit(status.code);
}

async function requireCoverageTool(): Promise<void> {
  const output = await new Deno.Command("cargo", {
    args: ["llvm-cov", "--version"],
    cwd: WORKSPACE_ROOT,
    stdout: "null",
    stderr: "null",
  }).output();
  if (output.success) return;

  console.error("cargo-llvm-cov is required:");
  console.error("  cargo install cargo-llvm-cov --locked");
  console.error("  rustup component add llvm-tools-preview");
  Deno.exit(1);
}

async function requireMihomo(): Promise<void> {
  const configured = Deno.env.get("MIHOMO_BIN");
  const binary = configured ?? path.join(
    WORKSPACE_ROOT,
    "tests",
    "bin",
    `mihomo${Deno.build.os === "windows" ? ".exe" : ""}`,
  );
  try {
    if ((await Deno.stat(binary)).isFile) return;
  } catch {
    // Report one actionable error below.
  }

  console.error(`mihomo was not found at ${binary}`);
  console.error("  deno run -A scripts/prepare-mihomo.ts");
  console.error("or set MIHOMO_BIN to a platform-compatible executable.");
  Deno.exit(1);
}

async function publicApiMethods(): Promise<string[]> {
  const apiDirectory = path.join(
    WORKSPACE_ROOT,
    "crates",
    "clash-api",
    "src",
    "api",
  );
  const methods = new Set<string>();
  for await (const entry of Deno.readDir(apiDirectory)) {
    if (!entry.isFile || !entry.name.endsWith(".rs")) continue;
    const source = await Deno.readTextFile(path.join(apiDirectory, entry.name));
    for (
      const match of source.matchAll(/pub\s+async\s+fn\s+([a-z_][a-z0-9_]*)/g)
    ) {
      methods.add(match[1]);
    }
  }
  return [...methods].sort();
}

async function assertedApiMethods(methods: string[]): Promise<Set<string>> {
  const integrationTest = path.join(
    WORKSPACE_ROOT,
    "crates",
    "clash-api",
    "tests",
    "mihomo.rs",
  );
  const source = await Deno.readTextFile(integrationTest);
  return new Set(
    methods.filter((method) => new RegExp(`\\.${method}\\s*\\(`).test(source)),
  );
}

type CoverageMetric = {
  count: number;
  covered: number;
  percent: number;
};

type CoverageSummary = {
  data: Array<{
    files: Array<{
      filename: string;
      summary: {
        functions: CoverageMetric;
        lines: CoverageMetric;
        regions: CoverageMetric;
      };
    }>;
    totals: {
      functions: CoverageMetric;
      lines: CoverageMetric;
      regions: CoverageMetric;
    };
  }>;
};

function combine(metrics: CoverageMetric[]): CoverageMetric {
  const count = metrics.reduce((total, metric) => total + metric.count, 0);
  const covered = metrics.reduce((total, metric) => total + metric.covered, 0);
  return {
    count,
    covered,
    percent: count === 0 ? 0 : covered / count * 100,
  };
}

await requireCoverageTool();
await requireMihomo();

const methods = await publicApiMethods();
const asserted = await assertedApiMethods(methods);
const missing = methods.filter((method) => !asserted.has(method));
if (missing.length > 0) {
  console.error(
    `public API methods without a real-mihomo test call: ${missing.join(", ")}`,
  );
  Deno.exit(1);
}
console.log(
  `real-mihomo API call inventory: ${asserted.size}/${methods.length}`,
);

await Deno.mkdir(COVERAGE_DIR, { recursive: true });
await run(["llvm-cov", "clean", "--workspace"]);
await run(["llvm-cov", "--no-report", "-p", "clash-api"]);
// `--no-clean` preserves the profiles from the normal suite while adding the
// ignored real-mihomo test. cargo-llvm-cov 0.6 does not allow combining it
// with `--no-report`, so this invocation also prints the first merged report.
await run([
  "llvm-cov",
  "--no-clean",
  "-p",
  "clash-api",
  "--test",
  "mihomo",
  "--",
  "--ignored",
]);
await run([
  "llvm-cov",
  "report",
  "-p",
  "clash-api",
  "--json",
  "--summary-only",
  "--output-path",
  SUMMARY_PATH,
  "--fail-under-lines",
  "80",
  "--fail-under-functions",
  "80",
]);
await run([
  "llvm-cov",
  "report",
  "-p",
  "clash-api",
  "--lcov",
  "--output-path",
  LCOV_PATH,
]);
await run([
  "llvm-cov",
  "report",
  "-p",
  "clash-api",
  "--html",
  "--output-dir",
  HTML_DIR,
]);

const summary = JSON.parse(
  await Deno.readTextFile(SUMMARY_PATH),
) as CoverageSummary;
const report = summary.data[0];
const apiFiles = report.files.filter((file) =>
  /[\\/]src[\\/]api[\\/].+\.rs$/.test(file.filename)
);
const apiFunctions = combine(apiFiles.map((file) => file.summary.functions));
const apiLines = combine(apiFiles.map((file) => file.summary.lines));
if (apiFunctions.percent < 85 || apiLines.percent < 85) {
  console.error(
    "API module coverage fell below the 85% function/line baseline.",
  );
  Deno.exit(1);
}

console.log(
  `API modules: ${apiFunctions.percent.toFixed(2)}% functions, ` +
    `${apiLines.percent.toFixed(2)}% lines`,
);
console.log(
  `Whole crate: ${report.totals.functions.percent.toFixed(2)}% functions, ` +
    `${report.totals.lines.percent.toFixed(2)}% lines`,
);
console.log(`HTML: ${path.join(HTML_DIR, "html", "index.html")}`);
console.log(`LCOV: ${LCOV_PATH}`);
