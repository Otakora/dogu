# Contributing to Dogu

Thanks for your interest in improving Dogu.

## Before opening something

- Use GitHub Discussions for questions, support, workflow ideas, and general conversation.
- Use GitHub Issues for actionable bugs, regressions, and feature requests.
- If you are unsure, start in Discussions and we can turn it into an Issue later if needed.

## Reporting bugs

Please include:

- What you expected to happen.
- What happened instead.
- Your platform and version of Dogu.
- Whether the path is local or remote.
- Steps to reproduce the problem.
- Screenshots or logs when they help.

## Suggesting features

Helpful feature requests usually explain:

- The real problem you are trying to solve.
- Why the current workflow is not enough.
- What a good result would look like.

## Pull requests

- Keep changes focused and explain the user-facing impact.
- Open pull requests against `dev` unless the maintainers say otherwise.
- External contributors do not need branches in the main repository; fork branches and PRs are the expected path.
- Update docs when behavior changes.
- Add or adjust tests when practical.
- Be careful with destructive file operations, queue behavior, and packaging changes.

## Branch and release channels

- `dev` is Dogu's Beta channel. It can contain recently merged work that is useful to test but not yet considered stable.
- `main` is Dogu's Stable channel. Stable releases should come from reviewed/promoted work.
- Beta release versions use semver prereleases such as `0.2.6-beta.1`.
- Stable release versions use clean semver such as `0.2.6`.
- User-facing changes should update `CHANGELOG.md` with a very short bilingual summary before a tagged release.

More release details are documented in `docs/release-channels.md`.

## Licensing

By submitting a contribution, you agree that your contribution will be licensed under
the same terms as the project: `GPL-2.0-or-later`.
