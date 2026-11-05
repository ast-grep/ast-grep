// Run in node:22-alpine3.22 against binaries built by release.yml.
const assert = require("node:assert/strict");
const { execFileSync } = require("node:child_process");
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");

const npmDir = path.resolve(__dirname, "..");
const targetDir = path.resolve(npmDir, "../target", process.env.TARGET);
const temp = fs.mkdtempSync(path.join(os.tmpdir(), "ast-grep-musl-"));

function run(command, args, options = {}) {
  return execFileSync(command, args, { encoding: "utf8", ...options });
}

function pack(directory) {
  const result = run("npm", [
    "pack",
    directory,
    "--pack-destination",
    temp,
    "--json",
  ]);
  return path.join(temp, JSON.parse(result)[0].filename);
}

try {
  // Point the wrapper at a local tarball so unpublished platform packages can
  // exercise npm's optional dependency selection and the real postinstall.
  const suffix = `linux-${process.arch}-musl`;
  const nativeDir = path.join(temp, "native");
  fs.cpSync(path.join(npmDir, "platforms", suffix), nativeDir, {
    recursive: true,
  });
  for (const bin of ["ast-grep", "sg"]) {
    fs.copyFileSync(
      path.join(targetDir, "release", bin),
      path.join(nativeDir, bin),
    );
  }
  const nativeTarball = pack(nativeDir);
  const wrapperDir = path.join(temp, "wrapper");
  fs.mkdirSync(wrapperDir);
  for (const file of ["ast-grep", "sg", "postinstall.js"]) {
    fs.copyFileSync(path.join(npmDir, file), path.join(wrapperDir, file));
  }
  const manifest = JSON.parse(
    fs.readFileSync(path.join(npmDir, "package.json")),
  );
  manifest.optionalDependencies = {
    [`@ast-grep/cli-${suffix}`]: `file:${nativeTarball}`,
  };
  fs.writeFileSync(
    path.join(wrapperDir, "package.json"),
    JSON.stringify(manifest),
  );
  const wrapperTarball = pack(wrapperDir);

  const local = path.join(temp, "local");
  run("npm", [
    "install",
    "--prefix",
    local,
    "--no-audit",
    "--no-fund",
    wrapperTarball,
  ]);
  const cliDir = path.join(local, "node_modules/@ast-grep/cli");
  const libc = require(path.join(local, "node_modules/detect-libc"));
  assert.equal(libc.familySync(), libc.MUSL);
  assert.equal(
    require(path.join(cliDir, "postinstall.js")).detectPackageName(),
    `@ast-grep/cli-${suffix}`,
  );
  const binDir = path.join(local, "node_modules/.bin");
  const env = { ...process.env, PATH: `${binDir}:${process.env.PATH}` };
  const cli = path.join(binDir, "ast-grep");
  assert.match(run(cli, ["--version"], { env }), /ast-grep/);
  assert.match(
    run(path.join(binDir, "sg"), ["--version"], { env }),
    /ast-grep/,
  );

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

  // A fully static musl binary can run built-in languages but cannot dlopen a
  // custom parser. Load a musl-built JSON parser to check this remains supported.
  const config = path.join(temp, "sgconfig.yml");
  fs.writeFileSync(
    config,
    JSON.stringify({
      ruleDirs: [],
      customLanguages: {
        myjson: {
          libraryPath: path.join(targetDir, "json.so"),
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

  const global = path.join(temp, "global");
  run("npm", [
    "install",
    "--global",
    "--prefix",
    global,
    "--no-audit",
    "--no-fund",
    wrapperTarball,
  ]);
  const globalBin = path.join(global, "bin");
  const globalEnv = {
    ...process.env,
    PATH: `${globalBin}:${process.env.PATH}`,
  };
  for (const bin of ["ast-grep", "sg"]) {
    assert.match(
      run(path.join(globalBin, bin), ["--version"], { env: globalEnv }),
      /ast-grep/,
    );
  }
  console.log(
    `Alpine npm install, global install, search, rewrite and custom parser passed (${suffix}).`,
  );
} finally {
  fs.rmSync(temp, { recursive: true, force: true });
}
