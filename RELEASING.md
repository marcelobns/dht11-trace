# Releasing

Kept out of the published package (`exclude` in `Cargo.toml`).

## Before the first release

- **Make the GitHub repository public.** The crate's page on crates.io links to it, and the
  README links point to files on `main`. Publishing makes the source public on crates.io in
  any case.
- **crates.io account.** Sign in at <https://crates.io> with GitHub, create an API token with
  the `publish-new` and `publish-update` scopes and run `cargo login`.
- **Publishing is permanent.** A published version cannot be deleted or replaced. `cargo yank`
  only stops new projects from depending on it; the code stays available for download.
- **Known limits of 0.1.0**, stated under *Status* in the README: the timing was checked on one
  ESP32 with one DHT11, and the restart of a stuck sensor has only run against the simulated
  line.

## Steps

The release follows the GitFlow in [`AGENTS.md`](AGENTS.md): it leaves from a `release/*`
branch.

1. Create the branch: `git switch -c release/0.1.0 develop`.
2. In `CHANGELOG.md`, replace `[Unreleased]` with `[0.1.0] - YYYY-MM-DD`, and check `version`
   in `Cargo.toml`.
3. Check the package: `cargo package --list` and `cargo publish --dry-run`.
4. Merge the branch into `main` and `develop`, tag `main` with `v0.1.0`, and push `main`,
   `develop` and the tag.
5. Wait for the CI on `main` to pass.
6. Publish from `main`: `cargo publish`.
7. Check <https://crates.io/crates/dht11-trace> and <https://docs.rs/dht11-trace>.
