# git-cz-ai

一个用 Rust 编写的交互式 Git 提交信息生成器（Commitizen 风格 CLI 工具）。支持两种工作模式：

- **交互式模式**：通过终端向导逐步填写提交信息（类型、范围、描述、正文、页脚），生成符合 [Conventional Commits](https://www.conventionalcommits.org/) 规范的提交消息并执行 `git commit`。
- **AI 模式**（`git-cz ai`）：读取已暂存（staged）的 diff，调用 OpenAI 兼容的 LLM API 生成多条提交信息候选，由你在命令行中选择，Enter 自动提交 / Ctrl-C 退出。

> 灵感来自 [k3ii/git-cz](https://github.com/k3ii/git-cz)

---

## 特性

- ✅ 11 种标准提交类型（`feat` / `fix` / `docs` / `style` / `refactor` / `perf` / `test` / `chore` / `ci` / `build` / `revert`），带英文描述与对齐展示
- ✅ scope 建议词（`app` / `core` / `ui` / `db` / `api` / `frontend` / `backend` / `config` / `build` / `sec` / `infra` / `deps`）
- ✅ body 支持外部编辑器模式（输入 `e` 打开 `$EDITOR`，默认 `vim`，Windows 默认 `notepad`）
- ✅ 可选 footer（`fix: #123` / `close: #456`），issue 号自动校验为整数
- ✅ **只提交已暂存的内容**——无任何 staged changes 时直接报错退出，绝不产生空提交
- ✅ `ai` 子命令：staged diff → LLM 生成候选（JSON 数组）→ 命令行选择 → 自动提交
- ✅ token 支持环境变量 `GIT_CZ_AI_OPENAI_API_KEY` 回退

## 安装

需要 Rust 工具链（edition 2021）,`openssl` 采用 `vendored` 特性静态编译，构建时需 C 编译器（`cc`），无需系统预装 OpenSSL。

```bash
# 构建（产物：target/release/git-cz）
cargo build --release

# 或直接安装到 PATH
cargo install --path .
```

## 使用

### 交互式模式

```bash
git-cz
```

按向导依次选择/填写：提交类型 → scope（可选）→ description → body（输入 `e` 打开编辑器）→ footer（可选）→ 确认提交。

> ⚠️ 该模式**不会自动 `git add`**。请先暂存你的变更：
>
> ```bash
> git add .
> git-cz
> ```

### AI 模式

```bash
git-cz ai --api-endpoint=<URL> --api-token=<TOKEN> --model-name=<MODEL>
```

| 参数 | 必填 | 说明 |
|------|------|------|
| `--api-endpoint` | ✅ | LLM API 端点，如 `https://api.openai.com/v1/chat/completions` |
| `--api-token` | ✅ | API 令牌；未提供时回退到环境变量 `GIT_CZ_AI_OPENAI_API_KEY` |
| `--model-name` | ✅ | 模型名称，如 `gpt-5-mini` |

工作流程：

1. 读取暂存区 diff（`git diff --cached`）；无 staged changes 时报错退出
2. 将 diff 嵌入内置提示词模板，请求 LLM 生成 3–6 条符合 Conventional Commits 的候选（JSON 字符串数组）；请求发出后提示 `Request has been sent, waiting for response`，响应成功返回后提示 `Response received`（均输出到 stderr）
3. 命令行展示候选列表（支持文本过滤），**Enter 选中即自动提交，Ctrl-C 退出不提交**

示例：

```bash
git-cz ai \
  --api-endpoint=https://api.openai.com/v1/chat/completions \
  --api-token=sk-xxx \
  --model-name=gpt-5-mini

# 或用环境变量提供 token
export GIT_CZ_AI_OPENAI_API_KEY=sk-xxx
git-cz ai --api-endpoint=https://api.openai.com/v1/chat/completions --model-name=gpt-5-mini
```

## 环境变量

| 变量 | 用途 | 默认值 |
|------|------|--------|
| `EDITOR` | body 编辑器模式（输入 `e` 时）调用的编辑器命令 | Windows：`notepad`；其他平台：`vim` |
| `GIT_CZ_AI_OPENAI_API_KEY` | `git-cz ai` 的 `--api-token` 回退来源 | 无（两者皆缺时 clap 报错退出） |

## 提交信息格式

```
<type>(<scope>): <description>

<body>

<footer>
```

- `type`：11 种标准类型之一（见上）
- `scope`：可选，如 `feat(parser)`
- `footer`：`fix: #123` 或 `close: #456`

## 技术栈

| 依赖 | 用途 |
|------|------|
| [git2](https://crates.io/crates/git2) | Git 仓库操作（暂存检查、索引、提交） |
| [promkit](https://crates.io/crates/promkit) | 终端交互组件（选择器、输入框、确认框） |
| [clap](https://crates.io/crates/clap) 4（derive + env） | CLI 参数解析、token 环境变量回退 |
| [serde_json](https://crates.io/crates/serde_json) | LLM 请求/响应编解码 |
| [ureq](https://crates.io/crates/ureq) 2（rustls） | LLM API HTTP 客户端 |
| [tempfile](https://crates.io/crates/tempfile) | 编辑器模式的临时提交信息文件 |

## 测试

```bash
cargo test
```

共 23 个集成测试，覆盖消息构建、暂存检查、真实提交、AI 提示词/响应解析/staged diff 等。

## 许可证

[MIT](LICENSE) · Copyright (c) 2026 secret
