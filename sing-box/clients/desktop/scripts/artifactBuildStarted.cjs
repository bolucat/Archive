const fs = require("node:fs");
const path = require("node:path");

function removeFinderMetadata(directory) {
  for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
    const entryPath = path.join(directory, entry.name);
    if (entry.name === ".DS_Store") {
      fs.rmSync(entryPath, { force: true, recursive: entry.isDirectory() });
    } else if (entry.isDirectory()) {
      removeFinderMetadata(entryPath);
    }
  }
}

exports.artifactBuildStarted = (context) => {
  if (context.targetPresentableName !== "nsis") {
    return;
  }
  removeFinderMetadata(path.dirname(context.file));
};
