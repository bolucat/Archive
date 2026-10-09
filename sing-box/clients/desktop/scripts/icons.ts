import { spawnSync } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const applicationIconSource = path.resolve(repositoryRoot, "..", "sing-box", "docs", "assets", "icon.svg");
const statusBarIconSource = path.resolve(
  repositoryRoot,
  "..",
  "sing-box-for-apple",
  "MacLibrary",
  "Assets.xcassets",
  "MenuIcon.symbolset",
  "menu_icon.svg",
);
const windowsSizes = [16, 24, 32, 48, 64, 128, 256];
const linuxSizes = [512, 1024];
// Electron's Windows tray requests the icon at GetSystemMetrics(SM_CXSMICON)
// (electron_api_tray.cc), and only an icon created from an .ico path reaches
// LoadImage with that size (electron_api_native_image.cc GetHICON; other
// images are converted from the 1x bitmap, ignoring the size). The frames
// cover SM_CXSMICON at 100/125/150/200% DPI plus the 256 fallback
// representation the NativeImage constructor reads.
const windowsTraySizes = [16, 20, 24, 32, 48, 256];
for (const [source, project] of [
  [applicationIconSource, "https://github.com/SagerNet/sing-box"],
  [statusBarIconSource, "https://github.com/SagerNet/sing-box-for-apple"],
]) {
  if (!fs.existsSync(source)) {
    console.error(`missing ${source}, clone ${project} next to this repository`);
    process.exit(1);
  }
}

function runChecked(command: string, commandArguments: string[]) {
  const result = spawnSync(command, commandArguments, { stdio: "inherit" });
  if (result.error) {
    console.error(`${command}: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

function imageSize(imagePath: string): { width: number; height: number } {
  const result = spawnSync("magick", ["identify", "-format", "%w %h", imagePath], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "inherit"],
  });
  if (result.error) {
    console.error(`magick: ${result.error.message}`);
    process.exit(1);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
  const [width, height] = result.stdout.trim().split(" ").map(Number);
  return { width, height };
}

function centerOnCanvas(glyphPath: string, canvasSize: number, outputPath: string) {
  runChecked("magick", [
    glyphPath,
    "-background",
    "none",
    "-gravity",
    "center",
    "-extent",
    `${canvasSize}x${canvasSize}`,
    outputPath,
  ]);
}

function renderApplicationIcon(size: number, workingDirectory: string, outputPath: string) {
  const glyphPath = path.join(workingDirectory, `glyph-${size}.png`);
  runChecked("rsvg-convert", [
    "--width",
    String(size),
    "--height",
    String(size),
    "--keep-aspect-ratio",
    "--output",
    glyphPath,
    applicationIconSource,
  ]);
  centerOnCanvas(glyphPath, size, outputPath);
}

function renderColorTrayIcon(size: number, workingDirectory: string, outputPath: string) {
  const glyphPath = path.join(workingDirectory, `tray-color-glyph-${size}.png`);
  runChecked("rsvg-convert", [
    "--width",
    String(size),
    "--height",
    String(size),
    "--keep-aspect-ratio",
    "--output",
    glyphPath,
    applicationIconSource,
  ]);
  centerOnCanvas(glyphPath, size, outputPath);
}

// The source is an SF Symbols template sheet; Regular-M is the variant AppKit
// renders for the Apple client's status item. The stopped variant reproduces
// drawStatusItemIcon in the Apple client's StatusBarController: a slash across
// the glyph from top-left to bottom-right with the knockout gap on its
// upper-right side only, as SF Symbols slash variants are built.
function renderTrayIcons(
  contentSize: number,
  canvasSize: number,
  scale: number,
  workingDirectory: string,
  outputPath: string,
  stoppedOutputPath: string,
) {
  const glyphPath = path.join(workingDirectory, `tray-glyph-${canvasSize}.png`);
  runChecked("rsvg-convert", [
    "--export-id=Regular-M",
    "--width",
    String(contentSize),
    "--height",
    String(contentSize),
    "--keep-aspect-ratio",
    "--output",
    glyphPath,
    statusBarIconSource,
  ]);
  centerOnCanvas(glyphPath, canvasSize, outputPath);
  const glyph = imageSize(glyphPath);
  const lineWidth = Math.max(1.5, (0.09 * glyph.height) / scale) * scale;
  const left = (canvasSize - glyph.width) / 2 - 0.5;
  const top = (canvasSize - glyph.height) / 2 - 0.5;
  const right = left + glyph.width;
  const bottom = top + glyph.height;
  const knockoutOffset = lineWidth / (2 * Math.SQRT2);
  const strokeArguments = [
    "-size",
    `${canvasSize}x${canvasSize}`,
    "xc:none",
    "-fill",
    "none",
    "-stroke",
    "black",
  ];
  const knockoutLine = [
    `${left + knockoutOffset},${top - knockoutOffset}`,
    `${right + knockoutOffset},${bottom - knockoutOffset}`,
  ].join(" ");
  runChecked("magick", [
    outputPath,
    "(",
    ...strokeArguments,
    "-strokewidth",
    String(lineWidth * 2),
    "-draw",
    `stroke-linecap round line ${knockoutLine}`,
    ")",
    "-compose",
    "DstOut",
    "-composite",
    "(",
    ...strokeArguments,
    "-strokewidth",
    String(lineWidth),
    "-draw",
    `stroke-linecap round line ${left},${top} ${right},${bottom}`,
    ")",
    "-compose",
    "Over",
    "-composite",
    stoppedOutputPath,
  ]);
}

function desaturateIcon(inputPath: string, outputPath: string) {
  runChecked("magick", [inputPath, "-modulate", "100,0", outputPath]);
}

const temporaryDirectory = fs.mkdtempSync(path.join(os.tmpdir(), "sing-box-icons-"));
try {
  const windowsImages = windowsSizes.map((size) => {
    const imagePath = path.join(temporaryDirectory, `${size}.png`);
    renderApplicationIcon(size, temporaryDirectory, imagePath);
    return imagePath;
  });
  runChecked("magick", [
    ...windowsImages,
    "-type",
    "TrueColorAlpha",
    path.join(repositoryRoot, "resources", "icon.ico"),
  ]);
  const linuxDirectory = path.join(repositoryRoot, "resources", "icons");
  fs.mkdirSync(linuxDirectory, { recursive: true });
  for (const size of linuxSizes) {
    renderApplicationIcon(size, temporaryDirectory, path.join(linuxDirectory, `${size}x${size}.png`));
  }
  const resourcesDirectory = path.join(repositoryRoot, "resources");
  renderTrayIcons(
    14,
    16,
    1,
    temporaryDirectory,
    path.join(resourcesDirectory, "trayTemplate.png"),
    path.join(resourcesDirectory, "trayStoppedTemplate.png"),
  );
  renderTrayIcons(
    28,
    32,
    2,
    temporaryDirectory,
    path.join(resourcesDirectory, "trayTemplate@2x.png"),
    path.join(resourcesDirectory, "trayStoppedTemplate@2x.png"),
  );
  const windowsTrayImages = windowsTraySizes.map((size) => {
    const imagePath = path.join(temporaryDirectory, `tray-color-${size}.png`);
    renderColorTrayIcon(size, temporaryDirectory, imagePath);
    return imagePath;
  });
  runChecked("magick", [
    ...windowsTrayImages,
    "-type",
    "TrueColorAlpha",
    path.join(resourcesDirectory, "tray.ico"),
  ]);
  const windowsStoppedTrayImages = windowsTrayImages.map((imagePath) => {
    const stoppedPath = imagePath.replace(/\.png$/, "-stopped.png");
    desaturateIcon(imagePath, stoppedPath);
    return stoppedPath;
  });
  runChecked("magick", [
    ...windowsStoppedTrayImages,
    "-type",
    "TrueColorAlpha",
    path.join(resourcesDirectory, "trayStopped.ico"),
  ]);
  for (const [size, name] of [
    [24, "tray.png"],
    [48, "tray@2x.png"],
  ] as const) {
    const imagePath = path.join(resourcesDirectory, name);
    renderColorTrayIcon(size, temporaryDirectory, imagePath);
    desaturateIcon(imagePath, path.join(resourcesDirectory, name.replace("tray", "trayStopped")));
  }
  // electron-builder's assisted installer sidebar (MUI_WELCOMEFINISHPAGE_BITMAP,
  // 164x314 per app-builder-lib nsisOptions) is picked up from
  // build/installerSidebar.bmp by file name convention; a flat panel, so the
  // wizard shows a single brand mark (the window icon).
  runChecked("magick", [
    "-size",
    "164x314",
    "canvas:#ECEFF1",
    "-type",
    "TrueColor",
    `BMP3:${path.join(repositoryRoot, "build", "installerSidebar.bmp")}`,
  ]);
  // build/installerHeader.bmp makes app-builder-lib define MUI_HEADERIMAGE,
  // switching the interior header from the icon dialog variant (modern.exe,
  // whose control 1039 repeats the window icon) to the bitmap variant
  // (modern_headerbmpr.exe); MUI_BGCOLOR white leaves the header text-only.
  runChecked("magick", [
    "-size",
    "150x57",
    "canvas:#FFFFFF",
    "-type",
    "TrueColor",
    `BMP3:${path.join(repositoryRoot, "build", "installerHeader.bmp")}`,
  ]);
} finally {
  fs.rmSync(temporaryDirectory, { recursive: true, force: true });
}
