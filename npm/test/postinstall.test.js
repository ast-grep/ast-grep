const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const test = require("node:test");
const vm = require("node:vm");

const source = fs.readFileSync(
  path.join(__dirname, "../postinstall.js"),
  "utf8",
);
const cli = require("../package.json");

function detect(platform, arch, family) {
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
    assert.equal(detect(platform, arch, family), name);
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
  assert.equal(detect("linux", "ppc64", "musl"), null);
  assert.equal(detect("linux", "arm", "glibc"), null);
  assert.equal(detect("freebsd", "x64", null), null);
});
