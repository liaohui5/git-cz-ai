# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.7](https://github.com/liaohui5/git-cz-ai/compare/v0.0.6...v0.0.7) - 2026-08-31

### Fixed

- *(git)* add perform commit test in empty repo

### Other

- modernize english readme with badges and usage guides

## [0.0.6](https://github.com/liaohui5/git-cz-ai/compare/v0.0.5...v0.0.6) - 2026-08-31

### Added

- map ai request errors to friendly chinese messages

### Fixed

- localize config parse error and tidy error handling
- localize config and staged-changes error messages
- abort gracefully when commit message selection is cancelled
- localize llm response parse error message

### Other

- use english error messages and comments in ai
- use english error messages and comments in config
- use english error messages in main and git
- print unified chinese error output in main
- replace unwraps in send_request with error handling

## [0.0.5](https://github.com/liaohui5/git-cz-ai/compare/v0.0.4...v0.0.5) - 2026-08-25

### Other

- update clap dependency description to remove env feature
- add unit tests
- rewrite main to use subcommand pattern

## [0.0.4](https://github.com/liaohui5/git-cz-ai/compare/v0.0.3...v0.0.4) - 2026-08-20

### Other

- Add GitHub Actions workflow for automated releases and PRs
