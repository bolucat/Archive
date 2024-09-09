import { execSync } from "child_process";
import path from "node:path";
import fs from "fs-extra";
import { merge } from "lodash-es";
import { cwd, TAURI_APP_DIR } from "./utils/env";
import { consola } from "./utils/logger";

const TAURI_DEV_APP_CONF_PATH = path.join(
  TAURI_APP_DIR,
  "tauri.nightly.conf.json",
);
const TAURI_APP_CONF = path.join(TAURI_APP_DIR, "tauri.conf.json");
const TAURI_DEV_APP_OVERRIDES_PATH = path.join(
  TAURI_APP_DIR,
  "overrides/nightly.conf.json",
);
const ROOT_PACKAGE_JSON_PATH = path.join(cwd, "package.json");
const NYANPASU_PACKAGE_JSON_PATH = path.join(
  cwd,
  "frontend/nyanpasu/package.json",
);
// blocked by https://github.com/tauri-apps/tauri/issues/8447
// const WXS_PATH = path.join(TAURI_APP_DIR, "templates", "nightly.wxs");

const isNSIS = process.argv.includes("--nsis"); // only build nsis
const isMSI = process.argv.includes("--msi"); // only build msi

async function main() {
  consola.debug("Read config...");
  const tauriAppConf = await fs.readJSON(TAURI_APP_CONF);
  const tauriAppOverrides = await fs.readJSON(TAURI_DEV_APP_OVERRIDES_PATH);
  const tauriConf = merge(tauriAppConf, tauriAppOverrides);
  const packageJson = await fs.readJSON(NYANPASU_PACKAGE_JSON_PATH);
  const rootPackageJson = await fs.readJSON(ROOT_PACKAGE_JSON_PATH);
  // const wxsFile = await fs.readFile(WXS_PATH, "utf-8");

  if (isNSIS) {
    tauriConf.bundle.targets = ["nsis"];
  }

  consola.debug("Get current git short hash");
  const GIT_SHORT_HASH = execSync("git rev-parse --short HEAD")
    .toString()
    .trim();
  consola.debug(`Current git short hash: ${GIT_SHORT_HASH}`);

  const version = `${tauriConf.version}-alpha+${GIT_SHORT_HASH}`;
  // blocked by https://github.com/tauri-apps/tauri/issues/8447
  // 1. update wxs version
  // consola.debug("Write raw version to wxs");
  // const modifiedWxsFile = wxsFile.replace(
  //   "{{version}}",
  //   tauriConf.package.version,
  // );
  // await fs.writeFile(WXS_PATH, modifiedWxsFile);
  // consola.debug("wxs updated");
  // 2. update tauri version
  consola.debug("Write tauri version to tauri.nightly.conf.json");
  if (!isNSIS && !isMSI) tauriConf.version = version;
  await fs.writeJSON(TAURI_DEV_APP_CONF_PATH, tauriConf, { spaces: 2 });
  consola.debug("tauri.nightly.conf.json updated");
  // 3. update package version
  consola.debug("Write tauri version to package.json");
  packageJson.version = version;
  await fs.writeJSON(NYANPASU_PACKAGE_JSON_PATH, packageJson, { spaces: 2 });
  rootPackageJson.version = version;
  await fs.writeJSON(ROOT_PACKAGE_JSON_PATH, rootPackageJson, { spaces: 2 });
  consola.debug("package.json updated");
}

main();
