{ lib
, rustPlatform
, fetchFromGitHub
, stdenv
, installShellFiles
}:

let
  cargoToml = lib.importTOML ./Cargo.toml;
in

rustPlatform.buildRustPackage rec {
  pname = "ast-grep";
  version = cargoToml.workspace.package.version;

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = [ installShellFiles ];

  checkFlags = [
     # BUG: Broke by 0.12.1 update (https://github.com/NixOS/nixpkgs/pull/257385)
     # Please check if this is fixed in future updates of the package
     "--skip=verify::test_case::tests::test_unmatching_id"
  ];

  # error: linker `aarch64-linux-gnu-gcc` not found
  postPatch = ''
    rm .cargo/config.toml
  '';

  postInstall = ''
    installShellCompletion --cmd sg \
      --bash <($out/bin/sg completions bash) \
      --fish <($out/bin/sg completions fish) \
      --zsh <($out/bin/sg completions zsh)
  '';

  meta = with lib; {
    mainProgram = "sg";
    description = "A fast and polyglot tool for code searching, linting, rewriting at large scale";
    homepage = "https://ast-grep.github.io/";
    changelog = "https://github.com/ast-grep/ast-grep/blob/main/CHANGELOG.md";
    license = licenses.mit;
    maintainers = with maintainers; [ montchr lord-valen cafkafk ];
  };
}
