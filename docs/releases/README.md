# Releases

One version for everything: the workspace's (`version` in `Cargo.toml`, which every crate takes)
and the plugin's (`crates/fm1-plugin/Cargo.toml`, a workspace of its own) are the same. A tag
`v<version>` releases it: `.github/workflows/release.yml` builds, tests and packages

- on macOS (Apple silicon): the plugin (AU, VST3, CLAP; Apple's AU validator and Steinberg's VST3
  validator) and fm1-live / fm1-emu;
- on Linux arm64 and x86-64: fm1-live / fm1-emu;

smoke-tests each fm1-live on FM-1 SID's package, and publishes one GitHub Release: the notes in
`docs/releases/v<version>.md`, then the pull requests merged since the last release (GitHub's
list). The page needs no release: it deploys from `master`.

## Steps

1. In a pull request: the version in `Cargo.toml` (`[workspace.package]`) and in
   `crates/fm1-plugin/Cargo.toml`; both `Cargo.lock`s follow (`cargo metadata` in the root and
   in `crates/fm1-plugin`); the notes, `docs/releases/v<version>.md` (what is new, for players:
   the merged pull requests are listed below them anyway).
2. Before merging: the pull request runs the release workflow (it changes the notes), which
   builds everything and keeps the files as the run's artifacts, no release. Download them
   from the run to try them.
3. Merge, then tag the merge on `master`:

   ```sh
   git checkout master && git pull
   git tag v0.2.0 && git push origin v0.2.0
   ```

The workflow refuses a tag that isn't the version, or a version without its notes.

Unsigned without the signing secrets (see crates/fm1-plugin/README.md); the notes and each
archive's README say how to let macOS open the downloads.
