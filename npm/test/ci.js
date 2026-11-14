// Run the same checks natively or in Alpine, according to the release target.
const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const path = require("node:path");

const npmDir = path.resolve(__dirname, "..");
const root = path.dirname(npmDir);
assert.ok(process.env.npm_execpath, "Run with npm run test:ci");

function npm(args) {
  execFileSync(process.execPath, [process.env.npm_execpath, ...args], {
    cwd: npmDir,
    stdio: "inherit",
  });
}

function runPhase(phase) {
  if (phase === "parser") {
    execFileSync(
      process.execPath,
      [path.join(__dirname, "parser.js"), "build"],
      {
        stdio: "inherit",
      },
    );
  } else {
    assert.equal(phase, "package", "Unknown test phase");
    npm(["install", "--ignore-scripts", "--omit=optional"]);
    npm(["test"]);
    npm(["run", "test:smoke"]);
  }
}

function runPhaseInAlpine(phase) {
  // Compile in one container, then test in a fresh image without build tools.
  const setup = phase === "parser" ? "apk add --no-cache build-base; " : "";
  execFileSync(
    "docker",
    [
      "run",
      "--rm",
      "-v",
      `${root}:/work`,
      "-w",
      "/work/npm",
      "node:22-alpine3.22",
      "sh",
      "-ec",
      `${setup}npm run test:ci -- ${phase}`,
    ],
    { stdio: "inherit" },
  );
}

function runAllTestPhases() {
  const runForTarget = process.env.TARGET?.endsWith("-musl")
    ? runPhaseInAlpine
    : runPhase;
  for (const phase of ["parser", "package"]) {
    runForTarget(phase);
  }
}

// argv[0] is Node, argv[1] is this script, and argv[2] is an optional phase:
// `npm run test:ci -- parser` or `npm run test:ci -- package`.
// Docker passes a phase to run it in the container without launching Docker again.
const requestedPhase = process.argv[2];
if (requestedPhase) {
  runPhase(requestedPhase);
} else {
  runAllTestPhases();
}
