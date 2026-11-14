// Run with npm run test:smoke after extracting the release archive into
// cli-artifacts/bin and building the custom parser in cli-artifacts/parser.
const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

assert.ok(process.env.npm_execpath, "Run with npm run test:smoke");
const npmDir = path.resolve(__dirname, "..");
const artifacts = path.resolve(npmDir, "../cli-artifacts");

function run(command, args, options = {}) {
  return execFileSync(command, args, { encoding: "utf8", ...options });
}

function npm(args) {
  // Invoke npm through node so Windows .cmd files and paths with spaces work.
  return run(process.execPath, [process.env.npm_execpath, ...args]);
}

function pack(directory, temp) {
  const result = npm(["pack", directory, "--pack-destination", temp, "--json"]);
  return path.join(temp, JSON.parse(result)[0].filename);
}

function verifyBins(prefix, global = false) {
  const binDir = global
    ? process.platform === "win32"
      ? prefix
      : path.join(prefix, "bin")
    : path.join(prefix, "node_modules/.bin");
  const env = {
    ...process.env,
    PATH: `${binDir}${path.delimiter}${process.env.PATH}`,
  };
  for (const bin of ["ast-grep", "sg"]) {
    let output;
    if (process.platform === "win32") {
      // Exercise the actual npm-generated global/local wrappers, including
      // their call through node to the extensionless JavaScript bin target.
      const wrapper = path.join(binDir, `${bin}.cmd`);
      output = run("cmd.exe", ["/d", "/s", "/c", `""${wrapper}" --version"`], {
        env,
        windowsVerbatimArguments: true,
      });
    } else {
      output = run(path.join(binDir, bin), ["--version"], { env });
    }
    assert.match(output, /ast-grep/);
  }
  return env;
}

function createPlatformPackage(temp, suffix) {
  const nativeDir = path.join(temp, "native");
  fs.cpSync(path.join(npmDir, "platforms", suffix), nativeDir, {
    recursive: true,
  });
  const native = JSON.parse(
    fs.readFileSync(path.join(nativeDir, "package.json")),
  );
  for (const file of native.files) {
    fs.copyFileSync(
      path.join(artifacts, "bin", file),
      path.join(nativeDir, file),
    );
  }
  const nativeTarball = pack(nativeDir, temp);
  return { name: native.name, tarball: nativeTarball };
}

function createCliPackage(temp, platformPackage) {
  // A local tarball lets npm exercise the real postinstall before publishing.
  const wrapperDir = path.join(temp, "wrapper");
  fs.mkdirSync(wrapperDir);
  for (const file of ["ast-grep", "sg", "postinstall.js"]) {
    fs.copyFileSync(path.join(npmDir, file), path.join(wrapperDir, file));
  }
  const manifest = JSON.parse(
    fs.readFileSync(path.join(npmDir, "package.json")),
  );
  manifest.optionalDependencies = {
    [platformPackage.name]: `file:${platformPackage.tarball}`,
  };
  fs.writeFileSync(
    path.join(wrapperDir, "package.json"),
    JSON.stringify(manifest),
  );
  return pack(wrapperDir, temp);
}

function installCliPackage(tarball, prefix, global = false) {
  npm([
    "install",
    ...(global ? ["--global"] : []),
    "--prefix",
    prefix,
    "--no-audit",
    "--no-fund",
    tarball,
  ]);
}

function verifyPlatformSelection(prefix, platformPackageName, suffix) {
  const cliDir = path.join(prefix, "node_modules/@ast-grep/cli");
  assert.equal(
    require(path.join(cliDir, "postinstall.js")).detectPackageName(),
    platformPackageName,
  );
  if (process.platform === "linux") {
    const libc = require(path.join(prefix, "node_modules/detect-libc"));
    assert.equal(
      libc.familySync(),
      suffix.endsWith("musl") ? libc.MUSL : libc.GLIBC,
    );
  }
}

function verifySearchAndRewrite(temp, cli, env) {
  const sample = path.join(temp, "sample.js");
  fs.writeFileSync(sample, 'console.log("hello");\n');
  assert.match(
    run(cli, ["run", "-p", "console.log($MSG)", "-l", "js", sample], { env }),
    /hello/,
  );
  run(
    cli,
    [
      "run",
      "-p",
      "console.log($MSG)",
      "-r",
      "console.info($MSG)",
      "-l",
      "js",
      "--update-all",
      sample,
    ],
    { env },
  );
  assert.equal(fs.readFileSync(sample, "utf8"), 'console.info("hello");\n');
}

function verifyCustomParserLoading(temp, cli, env) {
  // Exercise native custom language loading on every supported platform.
  const extension = { win32: "dll", darwin: "dylib", linux: "so" }[
    process.platform
  ];
  const config = path.join(temp, "sgconfig.yml");
  fs.writeFileSync(
    config,
    JSON.stringify({
      ruleDirs: [],
      customLanguages: {
        myjson: {
          libraryPath: path.join(artifacts, "parser", `json.${extension}`),
          languageSymbol: "tree_sitter_json",
          extensions: ["myjson"],
        },
      },
    }),
  );
  assert.match(
    run(
      cli,
      [
        "run",
        "-c",
        config,
        "--custom-languages",
        "allow",
        "-p",
        "123",
        "-l",
        "myjson",
        "--stdin",
      ],
      {
        env,
        input: '{"key":123}',
      },
    ),
    /123/,
  );
}

function runPackageSmokeTests() {
  const packageName = require("../postinstall.js").detectPackageName();
  assert.ok(packageName, "The current platform must have a CLI package");
  const suffix = packageName.replace("@ast-grep/cli-", "");
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), "ast-grep-package-"));
  try {
    const platformPackage = createPlatformPackage(temp, suffix);
    const wrapperTarball = createCliPackage(temp, platformPackage);
    const local = path.join(temp, "local");
    installCliPackage(wrapperTarball, local);
    const cliDir = path.join(local, "node_modules/@ast-grep/cli");
    verifyPlatformSelection(local, platformPackage.name, suffix);
    const env = verifyBins(local);
    const cli = path.join(
      cliDir,
      process.platform === "win32" ? "ast-grep.exe" : "ast-grep",
    );
    verifySearchAndRewrite(temp, cli, env);
    verifyCustomParserLoading(temp, cli, env);

    const global = path.join(temp, "global");
    installCliPackage(wrapperTarball, global, true);
    verifyBins(global, true);
    console.log(
      `npm local/global installation, both binaries, search, rewrite and custom parser passed (${suffix}).`,
    );
  } finally {
    fs.rmSync(temp, { recursive: true, force: true });
  }
}

runPackageSmokeTests();
