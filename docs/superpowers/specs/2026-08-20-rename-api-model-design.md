# `api_model` → `model_name` 字段重命名设计规格

> 日期：2026-08-20 · 状态：已批准

## 需求

将 `AiArgs` struct 的 `api_model` 字段改名为 `model_name`，并同步更新 CLI 参数名 `--api-model` → `--model-name` 及全部文档引用。

## 设计决策（经用户确认）

| 决策点 | 选择 | 理由 |
|--------|------|------|
| CLI 参数名 | 字段与 CLI 参数都改：`--api-model` → `--model-name` | 用户明确选择；属破坏性变更，需同步文档 |
| 文档范围 | 全部文档更新（README、AGENTS.md、历史 spec/plan） | 用户明确选择；历史归档也保持一致性 |
| `#[arg(long)]` 属性 | 保持默认（不写显式 `long`） | clap 默认将 snake_case 字段 `model_name` 转 kebab-case `--model-name` |
| JSON 请求体字段 | `"model"` 不变 | 这是 LLM API 协议字段，不属于 Rust 侧命名 |

## 改动清单

### 代码（1 个文件）

**`src/main.rs`：**

- 第 38 行：`api_model: String,` → `model_name: String,`（`#[arg(long)]` 属性保持）
- 第 62 行：`"model": args.api_model,` → `"model": args.model_name,`

### 文档（6 个文件，`--api-model` → `--model-name`，`api_model` → `model_name`）

| 文件 | 改动处 |
|------|--------|
| `README.md` | 用法示例、参数表、env 示例（4 处 CLI 参数） |
| `AGENTS.md` | §5.5 数据流图、§11 附录（2 处 CLI 参数） |
| `docs/superpowers/specs/2026-08-20-ai-subcommand-design.md` | `api_model` 字段、`--api-model` 引用 |
| `docs/superpowers/plans/2026-08-20-ai-subcommand.md` | 字段、使用点、参数引用 |
| `docs/superpowers/specs/2026-08-20-waiting-for-response-design.md` | `args.api_model` 代码块 |
| `docs/superpowers/plans/2026-08-20-waiting-for-response.md` | `args.api_model`、命令示例 |

## 行为规格

| 项目 | 改动前 | 改动后 |
|------|--------|--------|
| CLI 参数 | `--api-model <MODEL>` | `--model-name <MODEL>` |
| Rust 字段 | `args.api_model` | `args.model_name` |
| 请求体 JSON | `"model": <value>` | `"model": <value>`（不变） |
| 环境变量回退 | `GIT_CZ_AI_OPENAI_API_KEY`（仅 token） | 不变 |

## 验证

1. `cargo build` 编译通过、无警告
2. `cargo test` 23/23 通过（无回归——测试不涉及 CLI 字段名）
3. `target/debug/git-cz ai --help` 输出含 `--model-name`、不含 `--api-model`
4. 手动 mock 验证：`--model-name=gpt-test` 正常走完流程，请求体含 `"model": "gpt-test"`
5. `grep -rn "api_model\|api-model" .`（排除 `target/`）确认无残留

## 非目标（YAGNI）

- 不改 `api_endpoint` / `api_token` 字段
- 不改请求体 `"model"` JSON 键
- 不加新依赖、不加测试（CLI 字段名无自动化测试覆盖点，手动 `--help` 验证足够）
