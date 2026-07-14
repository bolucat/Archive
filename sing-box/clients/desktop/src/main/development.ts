import { app } from "electron";

export function developmentSwitchValue(name: string): string {
  if (app.isPackaged) {
    return "";
  }
  return app.commandLine.getSwitchValue(name);
}

export function developmentRendererURL(): string {
  return app.isPackaged ? "" : (process.env.ELECTRON_RENDERER_URL ?? "");
}

export function hardenPackagedRuntime(): void {
  if (!app.isPackaged) {
    return;
  }
  const unsafeEnvironment = [
    "ELECTRON_RENDERER_URL",
    "ELECTRON_ENABLE_LOGGING",
    "ELECTRON_LOG_FILE",
    "ELECTRON_RUN_AS_NODE",
    "NODE_OPTIONS",
    "NODE_EXTRA_CA_CERTS",
    "NODE_DEBUG",
    "NODE_PATH",
    "NODE_TLS_REJECT_UNAUTHORIZED",
    "NODE_USE_ENV_PROXY",
    "OPENSSL_CONF",
    "SSL_CERT_DIR",
    "SSL_CERT_FILE",
    "SSLKEYLOGFILE",
  ].find((name) => process.env[name] !== undefined);
  if (unsafeEnvironment !== undefined) {
    throw new Error(`unsafe packaged environment variable: ${unsafeEnvironment}`);
  }
  const forbiddenSwitches = [
    "allow-file-access-from-files",
    "allow-insecure-localhost",
    "allow-running-insecure-content",
    "allow-universal-access-from-files",
    "disable-features",
    "disable-gpu-sandbox",
    "disable-sandbox",
    "disable-site-isolation-trials",
    "disable-web-security",
    "disk-cache-dir",
    "enable-features",
    "enable-logging",
    "host-resolver-rules",
    "ignore-certificate-errors",
    "ignore-certificate-errors-spki-list",
    "in-process-gpu",
    "inspect",
    "inspect-brk",
    "js-flags",
    "load-extension",
    "no-sandbox",
    "proxy-server",
    "remote-debugging-address",
    "remote-debugging-pipe",
    "remote-debugging-port",
    "single-process",
    "ssl-key-log-file",
    "user-data-dir",
  ];
  const suppliedSwitches = new Set(
    process.argv
      .slice(1)
      .map((argument) => {
        let name: string;
        if (argument.startsWith("--")) {
          name = argument.slice(2);
        } else if (
          process.platform === "win32" &&
          (argument.startsWith("-") || argument.startsWith("/"))
        ) {
          name = argument.slice(1);
        } else {
          return null;
        }
        const valueSeparator = name.indexOf("=");
        return (valueSeparator === -1 ? name : name.slice(0, valueSeparator)).toLowerCase();
      })
      .filter((name) => name !== null),
  );
  const unsafeSwitch = forbiddenSwitches.find((name) => suppliedSwitches.has(name));
  if (unsafeSwitch !== undefined) {
    throw new Error(`unsafe packaged command-line switch: ${unsafeSwitch}`);
  }
}
