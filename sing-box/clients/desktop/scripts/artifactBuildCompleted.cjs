const crypto = require("node:crypto");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const zlib = require("node:zlib");

function runChecked(command, commandArguments, workingDirectory) {
  const result = spawnSync(command, commandArguments, {
    cwd: workingDirectory,
    stdio: "inherit",
  });
  if (result.error) {
    throw new Error(`${command}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(`${command} exited with code ${result.status ?? 1}`);
  }
}

function archiveEntries(directory, prefix = "") {
  const entries = [];
  for (const entry of fs
    .readdirSync(path.join(directory, prefix), { withFileTypes: true })
    .sort((left, right) =>
      left.name < right.name ? -1 : left.name > right.name ? 1 : 0,
    )) {
    if (prefix === "" && entry.name === ".MTREE") {
      continue;
    }
    const relativePath = path.join(prefix, entry.name);
    entries.push(relativePath.split(path.sep).join("/"));
    if (entry.isDirectory()) {
      entries.push(...archiveEntries(directory, relativePath));
    }
  }
  return entries;
}

function normalizeModificationTimes(directory, timestamp) {
  const paths = archiveEntries(directory);
  for (const relativePath of paths.toReversed()) {
    const filePath = path.join(directory, relativePath);
    const stat = fs.lstatSync(filePath);
    if (stat.isSymbolicLink()) {
      fs.lutimesSync(filePath, timestamp, timestamp);
    } else {
      fs.utimesSync(filePath, timestamp, timestamp);
    }
  }
  fs.utimesSync(directory, timestamp, timestamp);
}

function normalizePackageInfo(filePath, sourceDateEpoch) {
  const packageInfo = fs.readFileSync(filePath, "utf-8");
  const normalized = packageInfo.replace(
    /^builddate = [0-9]+$/mu,
    `builddate = ${sourceDateEpoch}`,
  );
  if (normalized === packageInfo) {
    throw new Error(`${filePath} has no builddate`);
  }
  fs.writeFileSync(filePath, normalized);
}

function createMtree(directory, entries, temporaryDirectory) {
  const rawPath = path.join(temporaryDirectory, "mtree");
  runChecked(
    "bsdtar",
    [
      "-cf",
      rawPath,
      "--format=mtree",
      "--options=!all,use-set,type,uid,gid,mode,time,size,md5,sha256,link",
      "--no-recursion",
      ...entries,
    ],
    directory,
  );
  const mtree = fs
    .readFileSync(rawPath, "utf-8")
    .replace(/\buid=[0-9]+/gu, "uid=0")
    .replace(/\bgid=[0-9]+/gu, "gid=0");
  fs.writeFileSync(
    path.join(directory, ".MTREE"),
    zlib.gzipSync(mtree, { level: 9 }),
  );
}

function normalizePacmanPackage(packagePath, sourceDateEpoch) {
  const temporaryDirectory = fs.mkdtempSync(
    path.join(os.tmpdir(), "sfw-pacman-"),
  );
  const stagingDirectory = path.join(temporaryDirectory, "package");
  const outputPath = path.join(temporaryDirectory, "package.pkg.tar.zst");
  fs.mkdirSync(stagingDirectory);
  try {
    runChecked("bsdtar", ["-xf", packagePath, "-C", stagingDirectory]);
    fs.rmSync(path.join(stagingDirectory, ".MTREE"), { force: true });
    normalizePackageInfo(
      path.join(stagingDirectory, ".PKGINFO"),
      sourceDateEpoch,
    );
    const timestamp = new Date(Number(sourceDateEpoch) * 1000);
    normalizeModificationTimes(stagingDirectory, timestamp);
    const entries = archiveEntries(stagingDirectory);
    createMtree(stagingDirectory, entries, temporaryDirectory);
    fs.utimesSync(
      path.join(stagingDirectory, ".MTREE"),
      timestamp,
      timestamp,
    );
    const packageEntries = archiveEntries(stagingDirectory);
    packageEntries.push(".MTREE");
    packageEntries.sort((left, right) =>
      left < right ? -1 : left > right ? 1 : 0,
    );
    runChecked(
      "bsdtar",
      [
        "--zstd",
        "--options",
        "zstd:compression-level=19",
        "-cf",
        outputPath,
        "--uid",
        "0",
        "--gid",
        "0",
        "--uname",
        "root",
        "--gname",
        "root",
        "--no-recursion",
        ...packageEntries,
      ],
      stagingDirectory,
    );
    fs.copyFileSync(outputPath, packagePath);
  } finally {
    fs.rmSync(temporaryDirectory, { recursive: true, force: true });
  }
}

exports.artifactBuildCompleted = (context) => {
  if (context.target?.name !== "pacman") {
    return;
  }
  const sourceDateEpoch = process.env.SOURCE_DATE_EPOCH;
  if (sourceDateEpoch === undefined || !/^[0-9]+$/u.test(sourceDateEpoch)) {
    throw new Error("SOURCE_DATE_EPOCH is not set");
  }
  normalizePacmanPackage(context.file, sourceDateEpoch);
  if (context.updateInfo !== undefined) {
    const artifact = fs.readFileSync(context.file);
    context.updateInfo.sha512 = crypto
      .createHash("sha512")
      .update(artifact)
      .digest("base64");
    context.updateInfo.size = artifact.length;
  }
};
