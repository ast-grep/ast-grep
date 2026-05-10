const fs = require("fs");
const path = require("path");

const binaryName = process.platform === "win32" ? "ast-grep.exe" : "ast-grep";
const alternativeName = process.platform === "win32" ? "sg.exe" : "sg";

function detectPackageName() {
  const { platform, arch } = process;
  switch (platform) {
    case "darwin":
      if (arch === "arm64") return "@ast-grep/cli-darwin-arm64";
      if (arch === "x64") return "@ast-grep/cli-darwin-x64";
      break;
    case "linux": {
      const { MUSL, familySync } = require("detect-libc");
      if (familySync() === MUSL) return null;
      if (arch === "arm64") return "@ast-grep/cli-linux-arm64-gnu";
      if (arch === "x64") return "@ast-grep/cli-linux-x64-gnu";
      break;
    }
    case "win32":
      if (arch === "arm64") return "@ast-grep/cli-win32-arm64-msvc";
      if (arch === "ia32") return "@ast-grep/cli-win32-ia32-msvc";
      if (arch === "x64") return "@ast-grep/cli-win32-x64-msvc";
      break;
  }
  return null;
}

function resolveBinaryDir() {
  const pkgName = detectPackageName();
  if (pkgName) {
    try {
      const dir = path.dirname(
        require.resolve(`${pkgName}/package.json`, { paths: [__dirname] }),
      );
      if (fs.existsSync(path.join(dir, binaryName))) return dir;
    } catch (_) {
      // fall through to local dev paths
    }
  }
  for (const profile of ["release", "debug"]) {
    const dir = path.join(__dirname, "..", "target", profile);
    if (fs.existsSync(path.join(dir, binaryName))) return dir;
  }
  return null;
}

function resolveBinaryPath() {
  const dir = resolveBinaryDir();
  return dir ? path.join(dir, binaryName) : null;
}

function main() {
  const sourceDir = resolveBinaryDir();
  if (!sourceDir) {
    console.error("Failed to locate @ast-grep/cli native binary.");
    process.exit(1);
  }

  const src = path.join(sourceDir, binaryName);
  const destBin = path.join(__dirname, binaryName);
  const destAlt = path.join(__dirname, alternativeName);

  try {
    fs.linkSync(src, destBin);
    fs.linkSync(src, destAlt);
  } catch (_) {
    try {
      fs.copyFileSync(src, destBin);
      fs.copyFileSync(src, destAlt);
    } catch (err) {
      console.error("Failed to move @ast-grep/cli binary into place.");
      process.exit(1);
    }
  }

  // On Windows, the published shims `sg` and `ast-grep` (no `.exe`) are not
  // usable; remove them so only the `.exe` versions remain.
  if (process.platform === "win32") {
    for (const name of ["sg", "ast-grep"]) {
      try {
        fs.unlinkSync(path.join(__dirname, name));
      } catch (_) {}
    }
  }
}

module.exports = {
  binaryName,
  alternativeName,
  detectPackageName,
  resolveBinaryDir,
  resolveBinaryPath,
};

if (require.main === module) {
  main();
}