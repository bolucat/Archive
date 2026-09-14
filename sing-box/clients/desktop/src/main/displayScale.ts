import { app } from "electron";
import { execFileSync } from "node:child_process";

const RESOURCE_MANAGER_COMMANDS = [
  { command: "xrdb", args: ["-query"] },
  { command: "xprop", args: ["-root", "RESOURCE_MANAGER"] },
];

// xrdb prints one resource per line, xprop prints the whole property as a
// single quoted string with the separators escaped.
const XFT_DPI = /Xft\.dpi:(?:[ \t]|\\t)*([0-9]+(?:\.[0-9]+)?)/;

function xftDPI(): number | undefined {
  for (const { command, args } of RESOURCE_MANAGER_COMMANDS) {
    let output: string;
    try {
      output = execFileSync(command, args, {
        encoding: "utf8",
        timeout: 2000,
        stdio: ["ignore", "pipe", "ignore"],
      });
    } catch {
      continue;
    }
    const match = XFT_DPI.exec(output);
    if (match === null) {
      continue;
    }
    return Number(match[1]);
  }
  return undefined;
}

// Chromium takes the desktop scale from an XSETTINGS manager, which Plasma
// does not run: it publishes the scale as Xft.dpi in the root window's
// RESOURCE_MANAGER property instead, both in X11 sessions and — when legacy
// applications are left to scale themselves — through XWayland. The forced
// factor also overrides the one Chromium derives from GDK_SCALE, which it
// applies on top of the DPI instead of in place of it.
export function applyDisplayScaleFactor(): void {
  if (process.argv.some((argument) => argument.startsWith("--force-device-scale-factor"))) {
    return;
  }
  const dpi = xftDPI();
  if (dpi === undefined) {
    return;
  }
  const scaleFactor = Math.round((dpi / 96) * 100) / 100;
  if (scaleFactor <= 1 || scaleFactor > 8) {
    return;
  }
  app.commandLine.appendSwitch("force-device-scale-factor", String(scaleFactor));
}
