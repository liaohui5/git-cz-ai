# git-cz-ai

<p align="center">
  <a href="README.md">English</a> | <b>简体中文</b>
</p>

> 一个用 Rust 编写的交互式 Git 提交信息生成器（Commitizen 风格 CLI 工具）

[![Crates.io Version](https://img.shields.io/crates/v/git-cz-ai)](https://crates.io/crates/git-cz-ai)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Release workflow](https://github.com/liaohui5/git-cz-ai/actions/workflows/release-plz.yml/badge.svg)](https://github.com/liaohui5/git-cz-ai/actions/workflows/release-plz.yml)

## 简介

`git-cz-ai` 是一个命令行工具，帮助你写出符合 [Conventional Commits](https://www.conventionalcommits.org/) 规范的提交信息。它打包为单个可执行文件 `git-cz`，支持两种工作模式：

- **交互式模式**：通过终端向导逐步填写提交信息（类型、scope、破坏性变更标记、描述、正文、页脚），然后替你执行 `git commit`。
- **AI 模式**（`git-cz ai`）：读取已暂存（staged）的 diff，调用 OpenAI 兼容的 LLM API 生成 3–6 条提交信息候选，由你选择，按 Enter 自动提交。

灵感来自 [k3ii/git-cz](https://github.com/k3ii/git-cz) 与 [cz-git](https://github.com/Zhengqbbb/cz-git)。

## 特性

- ✅ 11 种标准提交类型（`feat` / `fix` / `docs` / `style` / `refactor` / `perf` / `test` / `chore` / `ci` / `build` / `revert`），均带英文描述
- ✅ 可选 scope，如 `feat(parser)`
- ✅ 可选破坏性变更标记（`!`），如 `feat(core)!: ...`
- ✅ 描述必填——空输入会被校验器拒绝
- ✅ 正文通过 `inquire` 编辑器组件打开系统编辑器编辑
- ✅ 可选 footer（`fix: #123` / `close: #456`），issue 号自动校验为数字
- ✅ **只提交已暂存的内容**——无任何 staged changes 时直接报错退出，绝不产生空提交
- ✅ 支持刚 `git init`、还没有任何提交的空仓库（unborn branch）
- ✅ `ai` 子命令：staged diff → LLM 生成候选（JSON 数组）→ 选择 → 自动提交
- ✅ 等待 LLM 响应时显示加载动画

## 快速开始

### 环境要求

- Rust 工具链（edition 2021）。TLS 由 `rustls` 提供（经 `ureq` 内置），无需系统预装 OpenSSL。

### 安装

```bash
cargo install git-cz-ai
```

或从源码构建：

```bash
git clone https://github.com/liaohui5/git-cz-ai.git
cd git-cz-ai
cargo build --release
# 可执行文件位于 target/release/git-cz
```

### 最小示例

```bash
# 先暂存你的改动
git add .

# 运行交互式向导
git-cz
```

```text
✔ Select the type of change that you're committing?
  feat: a new feature
  ...
```

## 使用

```sh
git-cz                      # 交互式向导（手动模式）
git-cz ai                   # AI 生成提交信息候选
git-cz init-config          # 创建 ~/.config/git-cz/config.toml
```

### 交互式模式

```bash
git-cz
```

按向导依次选择/填写：提交类型 → scope（可选）→ 是否破坏性变更（可选）→ 描述 → 正文（可选）→ footer（可选）→ 确认提交。

<div align="center">
  <picture>
    <img alt="preview" src="https://raw.githubusercontent.com/liaohui5/git-cz-ai/refs/heads/main/preview.gif">
  </picture>
</div>

### AI 模式

```bash
git-cz ai --api-endpoint=<URL> --api-token=<TOKEN> --model-name=<MODEL>
```

| 参数 | 是否必填 | 说明 |
|------|----------|------|
| `--api-endpoint` | 可选 | LLM API 端点；未提供时回退到配置文件 `api_endpoint` |
| `--api-token` | 可选 | API 令牌；未提供时回退到配置文件 `api_token` |
| `--model-name` | 可选 | 模型名称；未提供时回退到配置文件 `model_name` |

工作流程：

1. 读取暂存区 diff；无 staged changes 时退出
2. 将 diff 嵌入内置提示词模板，请求 LLM 生成 3–6 条符合 Conventional Commits 的候选（JSON 字符串数组）；等待期间显示 `Request has been sent, waiting for response...`
3. 命令行展示候选列表——**Enter 选中即自动提交，Ctrl-C 退出不提交**

示例：

```bash
git-cz ai \
  --api-endpoint=https://api.openai.com/v1/chat/completions \
  --api-token=sk-xxx \
  --model-name=gpt-5-mini
```

### 配置文件

AI 参数也可以从 `~/.config/git-cz/config.toml` 加载（命令行参数优先于配置文件）。

```bash
git-cz init-config
```

首次运行会创建配置文件，内容如下：

```toml
api_endpoint = "https://api.deepseek.com/v1/chat/completions"
api_token = "sk-your-token-string"
model_name = "deepseek-v4-flash"
```

之后运行 `git-cz ai` 可省略命令行参数（显式传入的 CLI 参数仍会覆盖配置文件）：

```bash
git-cz ai
```

## 提交信息格式

```
<type>(<scope>)<!>: <description>

<body>

<footer>
```

- `type`：11 种标准类型之一（见「特性」）
- `scope`：可选，如 `feat(parser)`
- `footer`：`fix: #123` 或 `close: #456`

## 文档索引

- [CHANGELOG.md](CHANGELOG.md) — 版本变更记录
- [AGENTS.md](AGENTS.md) — 代码库知识总结（面向 AI 代理）

## 测试

```bash
cargo test
```

共 39 个单元测试，覆盖消息构建、暂存检查、真实提交（含空仓库首次提交）、AI 提示词/响应解析、staged diff、配置加载/初始化/合并等。

## 技术栈

| 依赖 | 用途 |
| ----- | ---- |
| [clap](https://crates.io/crates/clap) | CLI 参数解析 |
| [git2](https://crates.io/crates/git2) | Git 仓库操作（暂存检查、索引、提交） |
| [inquire](https://crates.io/crates/inquire) | 终端交互组件（选择器、输入框、确认框、编辑器） |
| [serde_json](https://crates.io/crates/serde_json) | LLM 请求/响应编解码 |
| [toml](https://crates.io/crates/toml) | 配置文件解析 |
| [ureq](https://crates.io/crates/ureq) | HTTP 客户端（rustls TLS） |

## 贡献

欢迎贡献。请遵循现有代码风格，保持错误消息为英文，并为新行为补充单元测试。

## 许可

[MIT](LICENSE) · Copyright (c) 2026 secret
