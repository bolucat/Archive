const fs = require("node:fs");
const path = require("node:path");

function normalizeModificationTimes(directory, timestamp) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      normalizeModificationTimes(entryPath, timestamp);
      fs.utimesSync(entryPath, timestamp, timestamp);
    } else if (entry.isSymbolicLink()) {
      fs.lutimesSync(entryPath, timestamp, timestamp);
    } else {
      fs.utimesSync(entryPath, timestamp, timestamp);
    }
  }
  fs.utimesSync(directory, timestamp, timestamp);
}

exports.afterPack = async (context) => {
  if (context.electronPlatformName === "linux") {
    const sourceDateEpoch = process.env.SOURCE_DATE_EPOCH;
    if (sourceDateEpoch === undefined || !/^[0-9]+$/u.test(sourceDateEpoch)) {
      throw new Error("SOURCE_DATE_EPOCH is not set");
    }
    normalizeModificationTimes(
      context.appOutDir,
      new Date(Number(sourceDateEpoch) * 1000),
    );
    return;
  }
  if (context.electronPlatformName !== "win32") {
    return;
  }
  for (const relativePath of [
    ["daemon", "sing-box-daemon.exe"],
    ["native", "windows_share.node"],
  ]) {
    const executablePath = path.join(context.appOutDir, "resources", ...relativePath);
    const signed = await context.packager.signIf(executablePath);
    if (!signed) {
      throw new Error(`failed to sign ${relativePath.join("/")}`);
    }
  }
};
