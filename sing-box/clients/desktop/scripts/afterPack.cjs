const path = require("node:path");

exports.afterPack = async (context) => {
  if (context.electronPlatformName !== "win32") {
    return;
  }
  for (const relativePath of [["daemon", "sing-box-daemon.exe"]]) {
    const executablePath = path.join(context.appOutDir, "resources", ...relativePath);
    const signed = await context.packager.signIf(executablePath);
    if (!signed) {
      throw new Error(`failed to sign ${relativePath.join("/")}`);
    }
  }
};
