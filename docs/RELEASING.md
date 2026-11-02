# Releasing

Releases are automated with [release-please](https://github.com/googleapis/release-please) and the [Release workflow](../.github/workflows/release.yml). Versions follow [Semantic Versioning](https://semver.org); before 1.0, a breaking change bumps the minor version and features bump the patch version.

## How a version is chosen

Pull requests are squash-merged, and their [Conventional Commit](https://www.conventionalcommits.org) titles become the commits on `main` (checked by the [PR title workflow](../.github/workflows/pr-title.yml)). release-please reads the commits since the last release: `fix:` produces a patch release, `feat:` a minor release (patch before 1.0), and `!` or a `BREAKING CHANGE:` footer a major release (minor before 1.0). `docs:`, `test:`, `ci:`, and `chore:` commits alone do not trigger a release.

## Steps

1. A maintainer runs the Release workflow manually on `main` (**Actions → Release → Run workflow**, or `gh workflow run release.yml --ref main`). It opens or updates a release pull request that bumps the version in `Cargo.toml`, `crates/jotline/Cargo.toml`, `Cargo.lock`, and `version.txt`, and adds a `CHANGELOG.md` section.
2. Review the release pull request: check the version, edit the changelog wording if needed, and wait for CI.
3. Merge the release pull request. The workflow creates the `vX.Y.Z` tag and GitHub release, then builds `jotline` for Linux (x86_64, aarch64) and macOS (x86_64, aarch64) and uploads the archives with SHA-256 checksums to the release, together with `jotline-tui-vX.Y.Z.tgz` (the interactive UI, packed with `npm pack`). The one-line [installer](../install.sh) downloads exactly these files, so a release must finish all of these jobs before the installer can use it.

Normal pull requests and merges never publish anything.

## One-time repository setup

The workflow uses a GitHub App token so the release pull request triggers CI (pull requests created with the default `GITHUB_TOKEN` do not).

1. Create a GitHub App owned by your account or organization with repository permissions **Contents: read and write**, **Pull requests: read and write**, and **Issues: read and write**, and install it on this repository.
2. Add the App ID as the repository variable `RELEASE_APP_ID` and a private key as the repository secret `RELEASE_APP_PRIVATE_KEY`.
3. Protect `main`: require pull requests, require the `CI required` status check, and allow squash merging only.

## Publishing to crates.io

Publishing is manual and requires maintainer credentials. After a GitHub release, from a clean checkout of the tag:

```bash
cargo publish -p jotline-core
cargo publish -p jotline
```

`jotline-core` must be published first because `jotline` depends on it by version.
