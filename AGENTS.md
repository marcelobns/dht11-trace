# AGENTS

## Git

- Prefer GitFlow as the branching model: `main` holds released code, `develop` is the integration branch, and all work happens on short-lived branches created from `develop` (`feature/*`, `release/*`, `hotfix/*` — the latter branching from `main`).
- Never commit directly to `main`; merge into it only through `release/*` or `hotfix/*`, and merge hotfixes back into `develop`.
- Keep the working tree clean and maintain a clear commit history.
- Commit changes after completing a coherent development cycle or logical unit of work.
- Keep commits focused: avoid mixing refactoring, formatting, dependency changes, and feature work when they are unrelated.
- After committing, verify that the working tree is in the expected state.
- Never blindly overwrite local changes.
- After synchronization, verify the resulting history and working tree.
