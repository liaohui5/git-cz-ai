# git-cz-ai

<p align="center">
  <b>English</b> | <a href="README.zh-CN.md">简体中文</a>
</p>

> An interactive Git commit message generator (Commitizen-style CLI) written in Rust.

[![Crates.io Version](https://img.shields.io/crates/v/git-cz-ai)](https://crates.io/crates/git-cz-ai)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Release workflow](https://github.com/liaohui5/git-cz-ai/actions/workflows/release-plz.yml/badge.svg)](https://github.com/liaohui5/git-cz-ai/actions/workflows/release-plz.yml)

## Introduction

`git-cz-ai` is a Command-line interface tool that helps you write
[Conventional Commits](https://www.conventionalcommits.org/) compliant commit messages.
It ships as a single binary named `git-cz` and supports two modes:

- **Interactive mode** — a terminal wizard that walks you through commit type, scope,
  breaking-change marker, description, body, and footer, then runs `git commit` for you.
- **AI mode** (`git-cz ai`) — reads the staged diff, asks an OpenAI-compatible LLM API to
  generate 3–6 commit message candidates, lets you pick one, and commits on Enter.

Inspired by [k3ii/git-cz](https://github.com/k3ii/git-cz) and [cz-git](https://github.com/Zhengqbbb/cz-git).

## Features

- ✅ 11 standard commit types (`feat` / `fix` / `docs` / `style` / `refactor` / `perf` / `test` / `chore` / `ci` / `build` / `revert`), each with an English description
- ✅ Optional scope, e.g. `feat(parser)`
- ✅ Optional breaking-change marker (`!`), e.g. `feat(core)!: ...`
- ✅ Description required — empty input is rejected with a validation error
- ✅ Body edited with your system editor via the `inquire` editor component
- ✅ Optional footer (`fix: #123` / `close: #456`) with issue number validated as a number
- ✅ **Commits only staged content** — exits with an error when nothing is staged, never creates empty commits
- ✅ Works in a freshly `git init` repository with no commits yet (unborn branch)
- ✅ `ai` subcommand: staged diff → LLM candidates (JSON array) → pick one → auto-commit
- ✅ Loading spinner while waiting for the LLM response

## Quick Start

### Requirements

- Rust toolchain (edition 2021). TLS is provided by `rustls` (bundled via `ureq`), so no system OpenSSL is required.

### Installation

```bash
cargo install git-cz-ai
```

Or build from source:

```bash
git clone https://github.com/liaohui5/git-cz-ai.git
cd git-cz-ai
cargo build --release
# binary at target/release/git-cz
```

### Minimal example

```bash
# stage your changes first
git add .

# run the interactive wizard
git-cz
```

```text
✔ Select the type of change that you're committing?
  feat: a new feature
  ...
```

## Usage

```sh
git-cz                      # interactive wizard (manual mode)
git-cz ai                   # AI-generated commit message candidates
git-cz init-config          # create ~/.config/git-cz/config.toml
```

### Interactive mode

```bash
git-cz
```

Follow the wizard: commit type → scope (optional) → breaking change (optional) →
description → body (optional) → footer (optional) → confirm commit.

<div align="center">
  <picture>
    <img alt="preview" src="https://raw.githubusercontent.com/liaohui5/git-cz-ai/refs/heads/main/preview.gif">
  </picture>
</div>

### AI mode

```bash
git-cz ai --api-endpoint=<URL> --api-token=<TOKEN> --model-name=<MODEL>
```

| Argument         | Required | Description                                                |
| ---------------- | -------- | ---------------------------------------------------------- |
| `--api-endpoint` | optional | LLM API endpoint; falls back to config file `api_endpoint` |
| `--api-token`    | optional | API token; falls back to config file `api_token`           |
| `--model-name`   | optional | Model name; falls back to config file `model_name`         |

How it works:

1. Reads the staged diff; exits if there is nothing staged
2. Embeds the diff into a built-in prompt template and asks the LLM for 3–6 Conventional
   Commits candidates (JSON string array); a spinner shows `Request has been sent, waiting for response...`
3. Shows the candidates for selection — **Enter commits immediately, Ctrl-C aborts without committing**

Example:

```bash
git-cz ai \
  --api-endpoint=https://api.openai.com/v1/chat/completions \
  --api-token=sk-xxx \
  --model-name=gpt-5-mini
```

### Configuration file

AI parameters can also be loaded from `~/.config/git-cz/config.toml`
(command-line arguments take precedence over the config file).

```bash
git-cz init-config
```

The first run creates the config file with the default content:

```toml
api_endpoint = "https://api.deepseek.com/v1/chat/completions"
api_token = "sk-your-token-string"
model_name = "deepseek-v4-flash"
```

After that, `git-cz ai` works without arguments (explicit CLI flags still override the file):

```bash
git-cz ai
```

## Commit message format

```
<type>(<scope>)<!>: <description>

<body>

<footer>
```

- `type`: one of the 11 standard types (see Features)
- `scope`: optional, e.g. `feat(parser)`
- `footer`: `fix: #123` or `close: #456`

## Documentation

- [CHANGELOG.md](CHANGELOG.md) — release history
- [AGENTS.md](AGENTS.md) — codebase knowledge summary (for AI agents)

## Testing

```bash
cargo test
```

39 unit tests covering message building, staged-change checks, real commits (including
first commit in an empty repo), AI prompt/response parsing, staged diff, and config
loading/init/merge.

## Tech stack

| Dependency                                        | Purpose                                                  |
| ------------------------------------------------- | -------------------------------------------------------- |
| [clap](https://crates.io/crates/clap)             | CLI parsing                                              |
| [git2](https://crates.io/crates/git2)             | Git repository operations (staged checks, index, commit) |
| [inquire](https://crates.io/crates/inquire)       | Terminal interaction (select, text, confirm, editor)     |
| [serde_json](https://crates.io/crates/serde_json) | LLM request/response encoding and decoding               |
| [toml](https://crates.io/crates/toml)             | Config file parsing                                      |
| [ureq](https://crates.io/crates/ureq)             | HTTP client (rustls TLS)                                 |

## Contributing

Contributions are welcome. Please follow the existing code style, keep error messages in
English, and add unit tests alongside any new behavior.

## License

[MIT](LICENSE) · Copyright (c) 2026 secret
