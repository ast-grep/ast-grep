// Prepare a real tree-sitter parser and build it as a native shared library.
const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const path = require("node:path");

const root = path.resolve(__dirname, "../..");
const directory = path.join(root, "cli-artifacts/parser");

function prepareParserSource() {
  const metadata = JSON.parse(
    execFileSync("cargo", ["metadata", "--format-version=1", "--locked"], {
      cwd: root,
      encoding: "utf8",
      maxBuffer: 16 * 1024 * 1024,
    }),
  );
  const parser = metadata.packages.find(
    (pkg) => pkg.name === "tree-sitter-json",
  );
  assert.ok(parser, "tree-sitter-json must be in the dependency graph");
  fs.cpSync(path.join(path.dirname(parser.manifest_path), "src"), directory, {
    recursive: true,
  });
}

function buildParserOnWindows() {
  // Keep the exported name undecorated on 32-bit Windows as well as x64/ARM64.
  fs.writeFileSync(
    path.join(directory, "json.def"),
    "EXPORTS\ntree_sitter_json\n",
  );
  execFileSync(
    "cl.exe",
    [
      "/nologo",
      "/std:c11",
      "/DTREE_SITTER_HIDE_SYMBOLS",
      "/LD",
      "/I.",
      "parser.c",
      "/link",
      "/DEF:json.def",
      "/OUT:json.dll",
    ],
    {
      cwd: directory,
      stdio: "inherit",
    },
  );
}

function buildParserOnUnix() {
  const flags =
    process.platform === "darwin"
      ? ["-dynamiclib", "-o", "json.dylib"]
      : ["-shared", "-fPIC", "-o", "json.so"];
  execFileSync("cc", [...flags, "-I.", "parser.c"], {
    cwd: directory,
    stdio: "inherit",
  });
}

function buildCustomParser() {
  if (process.platform === "win32") {
    buildParserOnWindows();
  } else {
    buildParserOnUnix();
  }
}

// The first script argument selects the fixture operation, for example:
// `node npm/test/parser.js prepare` copies sources; `... build` compiles them.
const requestedOperation = process.argv[2];
if (requestedOperation === "prepare") {
  prepareParserSource();
} else {
  assert.equal(requestedOperation, "build", "Expected prepare or build");
  buildCustomParser();
}
