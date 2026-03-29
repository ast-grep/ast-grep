# Release & Secrets Guide — ast-grep-dart v0.0.1

## Package Names

| Registry | Package | Scope |
|----------|---------|-------|
| npm | `@bramburn/ast-grep-dart` | CLI binary |
| npm | `@bramburn/ast-grep-dart-napi` | Node.js NAPI bindings |
| npm | `@bramburn/ast-grep-dart-wasm` | WebAssembly bindings |
| npm | `@bramburn/ast-grep-dart-cli-<platform>` | Per-platform CLI binaries |
| npm | `@bramburn/ast-grep-dart-napi-<platform>` | Per-platform NAPI binaries |
| PyPI | `ast-grep-dart-cli` | CLI binary (Python wheel) |
| crates.io | `ast-grep` (fork, v0.0.1) | Rust crate (optional) |

## Required GitHub Secrets

Configure these in **Settings → Secrets and variables → Actions**:

| Secret | Used by | How to get |
|--------|---------|------------|
| `NPM_TOKEN` | `release.yml`, `napi.yml`, `wasm.yml` | npm → Access Tokens → Generate (Automation type). The `@bramburn` scope must exist on npmjs.com. |
| `CARGO_REGISTRY_TOKEN` | `release.yml` | crates.io → API Tokens → New Token. Only needed if publishing Rust crates. |
| `CODECOV_TOKEN` | `coverage.yaml` | codecov.io → Settings → Repository Upload Token. Optional — CI won't fail without it. |

### Secrets you do NOT need to create

| Secret | Notes |
|--------|-------|
| `GITHUB_TOKEN` | Automatically provided by GitHub Actions |
| PyPI credentials | Uses OIDC trusted publishing (no secret needed — configure at pypi.org → Publishing → Add publisher for `bramburn/ast-grep`) |

## Pre-publish Checklist

1. **Create npm scope**: Log in to [npmjs.com](https://npmjs.com) and create the `@bramburn` org/scope if it doesn't exist.
2. **Add NPM_TOKEN secret**: Generate an Automation-type token on npm and add it to GitHub repo secrets.
3. **Configure PyPI trusted publishing**: On [pypi.org](https://pypi.org/manage/account/publishing/), add a pending publisher for `ast-grep-dart-cli` with:
   - Owner: `bramburn`
   - Repository: `ast-grep`
   - Workflow: `pypi.yml`
   - Environment: `release`
4. **(Optional) Add CARGO_REGISTRY_TOKEN**: Only if you want to publish Rust crates to crates.io.
5. **(Optional) Add CODECOV_TOKEN**: Only if you want code coverage reporting.

## How to Release v0.0.1

1. Ensure all secrets above are configured.
2. Tag and push:
   ```bash
   git tag 0.0.1
   git push origin 0.0.1
   ```
3. This triggers the following workflows automatically:
   - `release.yml` — builds binaries, creates GitHub release, publishes CLI to npm and crates.io
   - `napi.yml` — builds NAPI bindings for all platforms, publishes to npm
   - `wasm.yml` — builds WASM package, publishes to npm
   - `pypi.yml` — builds Python wheels, publishes to PyPI
   - `pyo3.yml` — builds Python bindings

4. Verify published packages:
   ```bash
   npm info @bramburn/ast-grep-dart
   pip install ast-grep-dart-cli==0.0.1
   ```

## Manual Publish Commands (if needed)

```bash
# npm CLI
cd npm && npm publish --access public

# npm NAPI
cd crates/napi && npm publish --access public

# npm WASM
cd crates/wasm/pkg && npm publish --access public

# npm platform packages (repeat for each platform dir)
cd npm/platforms/darwin-arm64 && npm publish --access public

# PyPI (requires uv)
uv publish wheels/*
```

## Version Bumping

All versions are centralized:
- **Rust crates**: `Cargo.toml` workspace version (`[workspace.package] version`)
- **npm packages**: Each `package.json` (update all consistently)
- **Python**: `pyproject.toml` `[project] version`

For future releases, update all version fields, commit, tag, and push.
