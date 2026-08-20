# 设计：`git-cz ai` 子命令（AI 生成提交信息）

- **日期**：2026-08-20
- **状态**：已获用户批准（2026-08-20）
- **涉及文件**：`src/ai.rs`（新增）、`src/main.rs`、`src/lib.rs`（`pub mod ai;`）、`Cargo.toml`、`tests/main_test.rs`

## 背景与目标

当前 `git-cz` 只有一套固定的交互式提交流程（手动选择类型/scope/描述）。项目名中的 "ai" 尚无任何 AI 能力。本次新增 `ai` 子命令：把已暂存变更的 `git diff` 交给 LLM API，由其生成一组 Conventional Commits 候选，用户命令行选择一条后自动提交。

**目标**：`git-cz ai --api-endpoint="https://api.openai.com/v1/chat/completions" --api-token="sk-xxx" --model-name="gpt-5-mini"` 即可完成「取 diff → 生成候选 → 选择 → 提交」全流程。

## 需求要点（已与用户逐项确认）

1. **diff 来源**：`git diff --cached`（仅已暂存内容），与 `perform_commit` 只提交 staged 内容的语义一致。
2. **token 回退**：`--api-token` 未传时回退到环境变量 `GIT_CZ_AI_OPENAI_API_KEY`；`--api-endpoint`、`--model-name` 必填。
3. **提示词**：用户提供的 markdown 提示词**原样嵌入代码**（不改写），仅将 `{{diff}}` 占位符替换为实际 diff 内容。
4. **JSON 解析失败**：立即退出程序并显示错误信息 `llm api response is not a json string`（退出码非零）。
5. **候选列表**：解析成功的 `Vec<String>` 用 promkit `QuerySelector` 展示供选择。
6. **Enter 选中** → 自动提交（`perform_commit`，无额外确认）。
7. **Ctrl-C 取消** → 退出程序，不提交。

## 设计

### 组件与接口（`src/ai.rs` 新增，纯逻辑、可单测）

| 函数 | 签名 | 说明 |
|------|------|------|
| `get_staged_diff` | `(repo_path: &Path) -> Result<String, Box<dyn Error>>` | `Command::new("git").args(["diff", "--cached"])` 输出；stdout 为空时报错 `No staged changes. Please 'git add' your files first.`（消息单点定义在此函数内） |
| `build_ai_prompt` | `(diff: &str) -> String` | `AI_PROMPT_TEMPLATE.replace("{{diff}}", diff)` |
| `parse_llm_response` | `(body: &str) -> Result<Vec<String>, Box<dyn Error>>` | 双层解析（见下） |

- 提示词模板：`const AI_PROMPT_TEMPLATE: &str = r#"..."#;`（raw string 原样嵌入用户提供的 markdown；内容无 `"#` 序列，`r#"..."#` 安全，必要时用 `r##` 保险）。
- `src/lib.rs` 新增 `pub mod ai;`，经 `git_cz_ai::ai::` 路径引用，与现有「lib 纯逻辑 + bin 编排」分层一致。

### `parse_llm_response` 双层解析

1. 解析 body 为 `serde_json::Value`。
2. 尝试 OpenAI 兼容 envelope：`body["choices"][0]["message"]["content"]` 若为字符串 → 将 content 解析为 `Vec<String>`。
3. 若 envelope 提取失败，回退：直接把 body 解析为 `Vec<String>`（兼容直接返回数组的 API）。
4. 任一步失败 → `Err`；错误消息统一为 `llm api response is not a json string`（单点定义在 `parse_llm_response` 内），由 main 打印后 `exit(1)`。

### CLI 结构（clap derive，`src/main.rs`）

```rust
#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// 用 AI 生成提交信息
    Ai(AiArgs),
}

#[derive(Args)]
struct AiArgs {
    /// LLM API 端点，如 https://api.openai.com/v1/chat/completions
    #[arg(long)]
    api_endpoint: String,
    /// API 令牌；未提供时回退到环境变量 GIT_CZ_AI_OPENAI_API_KEY
    #[arg(long, env = "GIT_CZ_AI_OPENAI_API_KEY")]
    api_token: String,
    /// 模型名称，如 gpt-5-mini
    #[arg(long)]
    model_name: String,
}
```

- `git-cz`（无子命令）→ 保持现有交互流程（`Command::None` 分支）。
- token 回退由 clap `env` 特性自动完成；两者皆缺时 clap 报错退出。

### 数据流

```
git-cz ai --api-endpoint=... --api-token=... --model-name=...
  → clap 解析参数
  → get_staged_diff(".")              # git diff --cached
  → build_ai_prompt(diff)             # {{diff}} 替换
  → ureq POST api_endpoint
      headers: Authorization: Bearer <token>, Content-Type: application/json
      body: {"model": <model>, "messages": [{"role":"user","content":<prompt>}]}
  → parse_llm_response(body)          # 失败 → 打印 "llm api response is not a json string" 退出非零
  → promkit QuerySelector 展示 Vec<String> 候选
  → Enter 选中 → perform_commit(".", 选中项) 自动提交
  → Ctrl-C    → 静默退出（不提交）
```

### 交互细节（promkit 0.4.5 已验证）

- `QuerySelector::run()` 返回 `Err(anyhow!("ctrl+c"))` 表示 Ctrl-C（见 `query_selector/keymap.rs` 默认 keymap：`Char('c') + CONTROL` → `Err(anyhow::anyhow!("ctrl+c"))`）。main 中匹配 `err.to_string().contains("ctrl+c")` → 正常退出（退出码 0），不提交；其他错误按普通错误打印退出。
- `Enter` → `PromptSignal::Quit` → `run()` 返回选中项，直接 `perform_commit`。

### 错误处理汇总

| 场景 | 行为 |
|------|------|
| 缺 `--api-endpoint` / `--model-name` | clap 报错退出 |
| 缺 `--api-token` 且环境变量也没有 | clap 报错退出 |
| 无 staged changes | 打印 `No staged changes. Please 'git add' your files first.`，退出非零 |
| HTTP 非 200 / 网络错误 | 打印状态码与响应体，退出非零 |
| 响应 JSON 解析失败 | 打印 `llm api response is not a json string`，退出非零 |
| Ctrl-C | 退出，不提交（退出码 0） |
| 提交失败 | 打印错误，退出非零 |

### 依赖变更（`Cargo.toml`）

```toml
[dependencies]
clap = { version = "4", features = ["derive", "env"] }
serde_json = "1"
ureq = "2"
```

- `ureq` 2.x 默认 rustls TLS，无系统依赖、无 tokio。

## 测试策略

`tests/main_test.rs` 新增（均为纯函数测试，无需 mock 网络）：

1. `test_build_ai_prompt_placeholder`：占位符替换正确（diff 内容出现在结果中，模板其余部分保留）。
2. `test_parse_llm_response_direct_array`：`["a","b"]` → `Ok(vec!["a","b"])`。
3. `test_parse_llm_response_openai_envelope`：`{"choices":[{"message":{"content":"[\"a\"]"}}]}` → `Ok(vec!["a"])`。
4. `test_parse_llm_response_invalid_json`：非法字符串 → `Err`，消息含 `llm api response is not a json string`。
5. `test_parse_llm_response_content_not_array`：envelope 存在但 content 非数组 → `Err`。
6. `test_get_staged_diff`：临时仓库 + 写文件 + `index.add_path` + `index.write`，断言返回的 diff 字符串包含文件名（依赖系统 `git` 命令，测试环境已有）。

已知限制：`main.rs` 的 clap 解析、ureq 请求、promkit 交互属 bin 编排层，不做自动化测试，靠手动运行验证。

## 明确不做（YAGNI）

- 不改写用户提供的提示词（原样嵌入）。
- 不自动 `git add`（沿用现有「用户需自行暂存」约定）。
- 不加 temperature / max_tokens 等请求参数（保持 `model` + `messages` 最小请求体）。
- 不加 `--help` 之外的额外参数（无超时、无重试、无代理配置）。
- 不引入异步运行时。
- 不重构现有交互流程（仅新增子命令分支）。
- 提交后不追加额外提示（`Commit successful!` 沿用现有文案）。

## 验收标准

- `cargo check` 通过（bin + lib + tests）。
- `cargo test` 全部通过（既有 17 个 + 新增 6 个测试）。
- `cargo build` 成功。
- 手动验证：
  - `cargo run -- ai`（缺参）→ clap 报错退出。
  - `cargo run -- ai --api-endpoint=... --model-name=...`（无 token、无环境变量）→ clap 报错退出。
  - 无 staged 的仓库 → 打印 `No staged changes. Please 'git add' your files first.`，非零退出。
  - 有 staged 的仓库 + 本地 mock API → 展示候选列表；Enter 提交成功；Ctrl-C 退出不提交。
  - 指向返回非法 JSON 的 mock API → 打印 `llm api response is not a json string` 非零退出。
