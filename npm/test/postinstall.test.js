const assert = require("node:assert/strict");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const source = fs.readFileSync(
  path.join(__dirname, "../postinstall.js"),
  "utf8",
);
const cli = require("../package.json");

// Mock OS and libc in an isolated module so every combination runs on any host.
function detectPackageOnPlatform(platform, arch, family) {
  const module = { exports: {} };
  vm.runInNewContext(source, {
    module,
    process: { platform, arch },
    require(name) {
      return name === "detect-libc"
        ? { MUSL: "musl", familySync: () => family }
        : require(name);
    },
  });
  return module.exports.detectPackageName();
}

function registerPackageSelectionTests() {
  for (const [platform, arch, family, suffix] of [
    ["linux", "x64", "musl", "linux-x64-musl"],
    ["linux", "arm64", "musl", "linux-arm64-musl"],
    ["linux", "x64", "glibc", "linux-x64-gnu"],
    ["linux", "arm64", "glibc", "linux-arm64-gnu"],
    ["linux", "x64", null, "linux-x64-gnu"],
    ["darwin", "x64", null, "darwin-x64"],
    ["darwin", "arm64", null, "darwin-arm64"],
    ["win32", "x64", null, "win32-x64-msvc"],
    ["win32", "arm64", null, "win32-arm64-msvc"],
    ["win32", "ia32", null, "win32-ia32-msvc"],
  ]) {
    test(`selects ${suffix} (${family})`, () => {
      const name = `@ast-grep/cli-${suffix}`;
      assert.equal(detectPackageOnPlatform(platform, arch, family), name);
      assert.equal(cli.optionalDependencies[name], cli.version);
      const native = require(`../platforms/${suffix}/package.json`);
      assert.equal(native.name, name);
      assert.equal(native.version, cli.version);
      assert.ok(native.os.includes(platform));
      assert.ok(native.cpu.includes(arch));
      if (family) assert.ok(native.libc.includes(family));
    });
  }

  test("unsupported platforms and architectures return null", () => {
    assert.equal(detectPackageOnPlatform("linux", "ppc64", "musl"), null);
    assert.equal(detectPackageOnPlatform("linux", "arm", "glibc"), null);
    assert.equal(detectPackageOnPlatform("freebsd", "x64", null), null);
  });
}

function registerInstallBinaryTests() {
  const { installBinary } = require("../postinstall.js");

  function setup() {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), "ast-grep-install-"));
    const src = path.join(dir, "native");
    const shim = path.join(dir, "shim");
    const dest = path.join(dir, "ast-grep");
    fs.writeFileSync(src, "native binary");
    fs.writeFileSync(shim, "js shim");
    return { dir, src, shim, dest };
  }

  test("replaces a hard-linked shim without writing through the link", () => {
    const { dir, src, shim, dest } = setup();
    fs.linkSync(shim, dest);
    installBinary(src, dest);
    assert.equal(fs.readFileSync(dest, "utf8"), "native binary");
    assert.equal(fs.readFileSync(shim, "utf8"), "js shim");
    assert.deepEqual(fs.readdirSync(dir).sort(), [
      "ast-grep",
      "native",
      "shim",
    ]);
  });

  test("installs when dest does not exist", () => {
    const { src, dest } = setup();
    installBinary(src, dest);
    assert.equal(fs.readFileSync(dest, "utf8"), "native binary");
  });
}

registerPackageSelectionTests();
registerInstallBinaryTests();
