# `git-cz ai` 子命令实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 为 git-cz-ai 新增 `ai` 子命令：取已暂存 diff → LLM 生成提交信息候选（JSON 数组）→ 命令行选择 → Enter 自动提交 / Ctrl-C 退出。

**架构：** 延续现有「lib 纯逻辑可测 + bin 编排」分层。新增 `src/ai.rs` 承载三个纯函数（`get_staged_diff` / `build_ai_prompt` / `parse_llm_response`），`src/main.rs` 用 clap 解析子命令并编排「ureq 请求 → promkit 选择 → perform_commit 提交」。token 支持回退环境变量 `GIT_CZ_AI_OPENAI_API_KEY`。

**技术栈：** Rust + git2（现有）+ promkit（现有）+ clap 4（derive + env）+ serde_json 1 + ureq 2（rustls）。

**设计规格：** `docs/superpowers/specs/2026-08-20-ai-subcommand-design.md`

---

## 文件结构

| 文件 | 动作 | 职责 |
|------|------|------|
| `Cargo.toml` | 修改 | 新增 `clap`、`serde_json`、`ureq` 三个依赖 |
| `src/lib.rs` | 修改 | 顶部加 `pub mod ai;` |
| `src/ai.rs` | 创建 | `AI_PROMPT_TEMPLATE`、`build_ai_prompt`、`parse_llm_response`、`get_staged_diff`（纯逻辑，可单测） |
| `src/main.rs` | 修改 | clap 子命令解析；原 `main()` 交互逻辑移至 `run_interactive()`；新增 `run_ai()` 编排 |
| `tests/main_test.rs` | 修改 | 追加 6 个 AI 相关测试 |
| `AGENTS.md` | 修改 | 按 writing-for-agents 技能更新项目知识（任务 5） |

---

### 任务 1：依赖 + 模块骨架 + `build_ai_prompt`

**文件：**
- 修改：`Cargo.toml`
- 修改：`src/lib.rs`
- 创建：`src/ai.rs`
- 测试：`tests/main_test.rs`

- [ ] **步骤 1：Cargo.toml 加依赖 + lib.rs 加模块声明**

`Cargo.toml` 的 `[dependencies]` 追加：

```toml
clap = { version = "4", features = ["derive", "env"] }
serde_json = "1"
ureq = "2"
```

`src/lib.rs` 顶部（现有 `use` 之前）加：

```rust
pub mod ai;
```

- [ ] **步骤 2：编写失败的测试**

`tests/main_test.rs` 顶部 `use` 追加：

```rust
use git_cz_ai::ai::{build_ai_prompt, get_staged_diff, parse_llm_response};
```

文件末尾追加：

```rust
#[test]
fn test_build_ai_prompt_placeholder() {
    let diff = "diff --git a/src/main.rs b/src/main.rs";
    let prompt = build_ai_prompt(diff);
    assert!(
        prompt.contains(diff),
        "diff 内容应替换 {{diff}} 占位符"
    );
    assert!(!prompt.contains("{{diff}}"), "占位符应被替换");
    assert!(prompt.contains("## 角色与任务"), "模板头部应保留");
    assert!(prompt.contains("Conventional Commits"), "模板正文应保留");
}
```

- [ ] **步骤 3：运行测试验证失败**

运行：`cargo test test_build_ai_prompt_placeholder`
预期：编译失败，`unresolved import git_cz_ai::ai` / `unresolved module ai`

- [ ] **步骤 4：创建 `src/ai.rs` 实现**

```rust
use std::error::Error;
use std::path::Path;
use std::process::Command;

/// AI 提示词模板：用户提供的 markdown，{{diff}} 占位符由调用方替换。
/// 注意：raw string 用 r## 包裹（内容含双引号与反引号），禁止改动提示词原文。
const AI_PROMPT_TEMPLATE: &str = r##"## 角色与任务
你是一个专业的 Git 提交信息生成器。  
根据下方提供的 **git diff 输出**（即文件变化内容），生成一组符合 [Conventional Commits v1.0.0](https://www.conventionalcommits.org/en/v1.0.0/) 规范的提交信息。  
每个提交信息应**对应一个逻辑独立的变更单元**（例如：新增功能、修复错误、文档更新等），而不是简单地将整个 diff 拆分为机械的逐行记录。

## 输入：文件变化内容
以下是 `git diff` 命令的输出结果，它是你生成提交信息的唯一依据: {{diff}}

## 输出格式（必须严格遵守）
- 返回一个 **合法的 JSON 字符串**。
- JSON 顶层必须是一个 **字符串数组**，且至少包含 **3 个元素**。
- 除了 JSON 字符串外，**不得输出任何额外文本、注释或解释**，以便后续程序直接解析。

## 每个提交信息的要求
1. **格式规范**（严格遵循 `<type>[optional scope]: <description>`）：
   - `<type>` 为必填项，必须是以下名词之一（常用类型）：`feat`、`fix`、`docs`、`style`、`refactor`、`perf`、`test`、`chore`。
   - `[optional scope]` 为可选项，如果使用，必须用英文括号包裹，例如 `feat(parser)`。
   - 冒号 `:` 后**必须紧跟一个英文半角空格**。
2. **语言与大小写**：整个提交信息（包括 type、scope 和 description）**必须全部使用小写英文字符**。
3. **长度限制**：每个提交信息的总字符数（**不含数组元素两端的引号**）**必须小于 100**。
4. **内容质量**：描述部分应**简洁、准确**，清晰概括本次变更的内容，避免模糊或泛泛而谈。

## 如何根据 diff 推导提交信息（指导原则）
- 分析 diff 中的文件路径和代码变更，识别出**功能新增**（→ `feat`）、**错误修复**（→ `fix`）、**文档改动**（→ `docs`）、**代码风格调整**（→ `style`）、**重构**（→ `refactor`）等。
- 如果变更集中在某个模块或包内，可使用 `scope` 注明（例如 `feat(auth)`、`fix(api)`）。
- 请将较大的 diff 分解为多个**有意义的逻辑单元**，每个单元生成一条独立提交信息，确保最终输出至少 3 条, 最多 6 条。

## 返回示例
以下示例展示了一个符合所有要求的输出（注意：示例内容仅供参考，实际输出必须基于我提供的 diff）：

```json
[
  "feat: add user login endpoint",
  "fix(auth): correct token validation logic",
  "docs: update swagger api description"
]
```

## 注意事项

- 请确保 JSON 格式正确（使用双引号、逗号分隔、无尾随逗号）
- 如果你无法从 diff 中提取出至少 3 个逻辑单元，可以适当拆分，但必须保证每条信息都真实反映 diff 中的变更。
- 所有提交信息必须严格小写，且长度 < 100 字符。
"##;

/// 用 diff 内容替换提示词中的 {{diff}} 占位符。
pub fn build_ai_prompt(diff: &str) -> String {
    AI_PROMPT_TEMPLATE.replace("{{diff}}", diff)
}
```

> ⚠️ 提示词原文中的「返回示例」代码块以三个反引号开头结尾，会与上方 Rust 代码块语法冲突——把 `src/ai.rs` 中的模板常量按原文写入即可，无需转义（raw string 内反引号合法）。执行时请直接照抄用户在规格中给出的提示词全文。

- [ ] **步骤 5：运行测试验证通过**

运行：`cargo test test_build_ai_prompt_placeholder`
预期：PASS（1 passed）

- [ ] **步骤 6：Commit**

```bash
git add Cargo.toml src/lib.rs src/ai.rs tests/main_test.rs
git commit -m "feat: add ai module with prompt builder"
```

---

### 任务 2：`parse_llm_response`

**文件：**
- 修改：`src/ai.rs`
- 测试：`tests/main_test.rs`

- [ ] **步骤 1：编写失败的测试**

`tests/main_test.rs` 末尾追加：

```rust
#[test]
fn test_parse_llm_response_direct_array() {
    let body = r#"["feat: add login", "fix: fix bug"]"#;
    let result = parse_llm_response(body).unwrap();
    assert_eq!(result, vec!["feat: add login", "fix: fix bug"]);
}

#[test]
fn test_parse_llm_response_openai_envelope() {
    let body = r#"{"choices":[{"message":{"content":"[\"feat: add login\"]"}}]}"#;
    let result = parse_llm_response(body).unwrap();
    assert_eq!(result, vec!["feat: add login"]);
}

#[test]
fn test_parse_llm_response_invalid_json() {
    let err = parse_llm_response("not a json").unwrap_err();
    assert_eq!(err.to_string(), "llm api response is not a json string");
}

#[test]
fn test_parse_llm_response_content_not_array() {
    let body = r#"{"choices":[{"message":{"content":"{\"a\": 1}"}}]}"#;
    let err = parse_llm_response(body).unwrap_err();
    assert_eq!(err.to_string(), "llm api response is not a json string");
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test test_parse_llm_response`
预期：编译失败，`function not found in ai: parse_llm_response`

- [ ] **步骤 3：实现 `parse_llm_response`**

`src/ai.rs` 末尾追加（注意 `use std::error::Error;` 已在任务 1 引入）：

```rust
/// 解析 LLM API 响应为提交信息候选列表。
/// 双层解析：
/// 1. 优先提取 OpenAI 兼容 envelope 的 choices[0].message.content，再把 content 解析为 Vec<String>；
/// 2. 回退：把整个响应体直接解析为 Vec<String>（兼容直接返回数组的 API）。
/// 任何一步失败返回 Err，消息统一为 "llm api response is not a json string"。
pub fn parse_llm_response(body: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let parse_err = || -> Box<dyn Error> { "llm api response is not a json string".into() };

    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| parse_err())?;

    // 优先尝试 OpenAI 兼容 envelope：choices[0].message.content
    if let Some(content) = value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
    {
        return serde_json::from_str(content).map_err(|_| parse_err());
    }

    // 回退：响应体本身就是字符串数组
    serde_json::from_value(value).map_err(|_| parse_err())
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test test_parse_llm_response`
预期：PASS（4 passed）

- [ ] **步骤 5：Commit**

```bash
git add src/ai.rs tests/main_test.rs
git commit -m "feat: add llm response parser"
```

---

### 任务 3：`get_staged_diff`

**文件：**
- 修改：`src/ai.rs`
- 测试：`tests/main_test.rs`

- [ ] **步骤 1：编写失败的测试**

`tests/main_test.rs` 末尾追加（复用文件已有的 `init_repo_with_initial_commit` 辅助函数）：

```rust
#[test]
fn test_get_staged_diff() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo = init_repo_with_initial_commit(temp_dir.path());

    std::fs::write(temp_dir.path().join("a.txt"), "v1").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("a.txt")).unwrap();
    index.write().unwrap();

    let diff = get_staged_diff(temp_dir.path()).unwrap();
    assert!(diff.contains("a.txt"), "diff 应包含新增文件名");
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test test_get_staged_diff`
预期：编译失败，`function not found in ai: get_staged_diff`

- [ ] **步骤 3：实现 `get_staged_diff`**

`src/ai.rs` 末尾追加（`Path`、`Command`、`Error` 的 `use` 已在任务 1 引入）：

```rust
/// 执行 `git diff --cached` 获取已暂存变更。
/// stdout 为空（无 staged changes）时报错，提示用户先 git add。
pub fn get_staged_diff(repo_path: &Path) -> Result<String, Box<dyn Error>> {
    let output = Command::new("git")
        .arg("diff")
        .arg("--cached")
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "git diff --cached failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    if stdout.trim().is_empty() {
        return Err("No staged changes. Please 'git add' your files first.".into());
    }
    Ok(stdout)
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test test_get_staged_diff`
预期：PASS（1 passed）

- [ ] **步骤 5：Commit**

```bash
git add src/ai.rs tests/main_test.rs
git commit -m "feat: add staged diff fetcher"
```

---

### 任务 4：CLI 子命令 + main 编排

**文件：**
- 修改：`src/main.rs`（整体重写为下述最终代码）
- 手动验证：mock LLM API（临时脚本，不入库）

- [ ] **步骤 1：重写 `src/main.rs`**

最终代码（clap 子命令 + `run_ai` 编排 + 原交互流程移至 `run_interactive`）：

```rust
use clap::{Args, Parser, Subcommand};
use git_cz_ai::ai::{build_ai_prompt, get_staged_diff, parse_llm_response};
use git_cz_ai::{
    build_commit_message, build_commit_types, ensure_staged_changes, format_commit_types,
    perform_commit,
};
use git2::Repository;
use promkit::preset::query_selector::QuerySelector;
use promkit::{preset::confirm::Confirm, preset::readline::Readline, suggest::Suggest};
use std::env;
use std::path::Path;
use std::process::Command;
use tempfile;

#[derive(Parser)]
struct Cli {
    /// 子命令；不传则保持现有交互式提交流程
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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Some(Command::Ai(args)) => run_ai(&args),
        None => run_interactive(),
    }
}

fn run_ai(args: &AiArgs) -> Result<(), Box<dyn std::error::Error>> {
    // 1. 获取已暂存的 diff（git diff --cached）
    let diff = get_staged_diff(Path::new("."))?;

    // 2. 用 diff 替换提示词中的 {{diff}} 占位符
    let prompt = build_ai_prompt(&diff);

    // 3. 发送请求到 LLM API
    let response = ureq::post(&args.api_endpoint)
        .set("Authorization", &format!("Bearer {}", args.api_token))
        .set("Content-Type", "application/json")
        .send_json(serde_json::json!({
            "model": args.model_name,
            "messages": [{ "role": "user", "content": prompt }],
        }));

    let body = match response {
        Ok(resp) => resp.into_string()?,
        Err(ureq::Error::Status(code, resp)) => {
            let text = resp.into_string().unwrap_or_default();
            eprintln!("llm api error: HTTP {}: {}", code, text);
            std::process::exit(1);
        }
        Err(ureq::Error::Transport(e)) => {
            eprintln!("llm api request failed: {}", e);
            std::process::exit(1);
        }
    };

    // 4. 解析响应为 Vec<String>，失败立即退出
    let candidates = match parse_llm_response(&body) {
        Ok(candidates) => candidates,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    // 5. 命令行选择候选；Enter 选中，Ctrl-C 退出（不提交）
    let mut selector = QuerySelector::new(candidates, |text, items| -> Vec<String> {
        items
            .iter()
            .filter(|item| item.contains(text))
            .cloned()
            .collect()
    })
    .title("Select a commit message:")
    .listbox_lines(10)
    .prompt()?;

    let selection = match selector.run() {
        Ok(selection) => selection,
        Err(e) if e.to_string().contains("ctrl+c") => {
            println!("Commit aborted.");
            std::process::exit(0);
        }
        Err(e) => return Err(e.into()),
    };

    // 6. 自动提交
    perform_commit(Path::new("."), &selection)?;
    println!("Commit successful!");
    Ok(())
}

fn run_interactive() -> Result<(), Box<dyn std::error::Error>> {
    // 启动预检：无任何 staged changes 时直接退出
    let repo = Repository::open(".")?;
    if let Err(e) = ensure_staged_changes(&repo) {
        eprintln!("{}", e);
        std::process::exit(1);
    }

    let commit_types = build_commit_types();
    let commit_types_display = format_commit_types(commit_types);

    let mut p = QuerySelector::new(commit_types_display.clone(), |text, items| -> Vec<String> {
        items
            .iter()
            .filter(|item| item.contains(text))
            .cloned()
            .collect()
    })
    .title("Select the type of change that you're committing:")
    .listbox_lines(10)
    .prompt()?;

    let mut scope_input = Readline::default()
        .title("Denote the scope of this change (optional):")
        .enable_suggest(Suggest::from_iter([
            "app", "core", "ui", "db", "api", "frontend", "backend", "config", "build", "sec",
            "infra", "deps",
        ]))
        .prompt()?;

    let mut description_input = Readline::default()
        .title("Write a short, imperative tense description of the change:")
        .prompt()?;
    let mut body_input = Readline::default()
        .title("Provide a longer description of the change(press 'e' to open editor):")
        .prompt()?;

    let selection = p.run()?;
    let selected_type = selection.split_whitespace().next();

    if let Some(commit_type) = selected_type {
        let scope = scope_input.run()?;
        let description = description_input.run()?;
        let body = body_input.run()?;

        let body = if body.trim().to_lowercase() == "e" {
            // Create a temporary file
            let temp_file = tempfile::NamedTempFile::new()?;
            let temp_path = temp_file
                .path()
                .to_str()
                .expect("Failed to get temp file path");

            // Determine the editor command
            let editor_command = if cfg!(target_os = "windows") {
                env::var("EDITOR").unwrap_or_else(|_| "notepad".to_string())
            } else {
                env::var("EDITOR").unwrap_or_else(|_| "vim".to_string())
            };

            // Open the editor
            let status = Command::new(&editor_command).arg(temp_path).status()?;

            if !status.success() {
                eprintln!("Editor exited with non-zero status");
            }

            // Read the contents of the temp file
            std::fs::read_to_string(temp_path)?
        } else {
            body
        };

        // New footer confirmation prompt
        let mut footer_confirm = Confirm::new("Do you want to add a footer?").prompt()?;
        let footer = if footer_confirm.run()?.to_lowercase() == "y" {
            let mut footer_type_input = QuerySelector::new(
                vec!["fix".to_string(), "close".to_string()],
                |text, items| -> Vec<String> {
                    items
                        .iter()
                        .filter(|item| item.contains(text))
                        .cloned()
                        .collect()
                },
            )
            .title("Select the footer type:")
            .listbox_lines(2)
            .prompt()?;

            let mut issue_number_input = Readline::default()
                .title("Enter the issue number:")
                .validator(
                    |text| text.trim().parse::<i32>().is_ok(),
                    |text| format!("'{}' is not a valid integer", text),
                )
                .prompt()?;

            let footer_type = footer_type_input.run()?;
            let issue_number = issue_number_input.run()?;
            format!("{}: #{}", footer_type, issue_number)
        } else {
            String::new()
        };

        let full_commit_message =
            build_commit_message(&commit_type, &scope, &description, &body, &footer);

        let mut confirm_input =
            Confirm::new("Do you want to proceed with this commit?").prompt()?;
        let confirm = confirm_input.run()?;
        if confirm.to_lowercase() == "y" {
            perform_commit(Path::new("."), &full_commit_message)?;
            println!("Commit successful!");
        } else {
            println!("Commit aborted.");
        }
    }

    Ok(())
}
```

> ⚠️ 若 `ureq` 2.x 的 `Error` 枚举变体名与代码不符（编译报错），用 `grep -n "pub enum Error" ~/.cargo/registry/src/*/ureq-*/src/error.rs` 核实实际变体名（2.x 为 `Status(u16, Response)` + `Transport(Transport)`）。

- [ ] **步骤 2：编译检查**

运行：`cargo build`
预期：编译通过（bin `git-cz`）

- [ ] **步骤 3：运行全部测试**

运行：`cargo test`
预期：全部 PASS（既有 17 个 + 任务 1-3 新增 6 个 = 23 个）

- [ ] **步骤 4：手动验证（mock LLM API）**

创建临时 mock 服务器（不入库，放在 `/tmp/mock_llm.py`）：

```python
#!/usr/bin/env python3
# 用法: python3 /tmp/mock_llm.py [ok|bad]   默认 ok 返回 OpenAI 兼容 envelope
import json, sys
from http.server import BaseHTTPRequestHandler, HTTPServer

mode = sys.argv[1] if len(sys.argv) > 1 else "ok"

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        self.rfile.read(length)
        if mode == "bad":
            body = b"not a json"
        else:
            body = json.dumps({
                "choices": [{"message": {"content": json.dumps([
                    "feat: add login endpoint",
                    "fix(auth): validate token",
                    "docs: update readme",
                ])}}]
            }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

HTTPServer(("127.0.0.1", 8123), Handler).serve_forever()
```

验证场景（在一个测试仓库中操作，确保 `user.name`/`user.email` 已配置）：

```bash
# 起 mock：python3 /tmp/mock_llm.py &    （bad 模式: python3 /tmp/mock_llm.py bad &）

# 场景 1：缺参 → clap 报错
cargo run -- ai
# 预期：error: the following required arguments were not provided: --api-endpoint, --api-token, --model-name

# 场景 2：缺 token 且无环境变量 → clap 报错
cargo run -- ai --api-endpoint=http://127.0.0.1:8123/v1/chat/completions --model-name=gpt-test
# 预期：error: the following required arguments were not provided: --api-token

# 场景 3：无 staged changes（干净仓库）
cargo run -- ai --api-endpoint=http://127.0.0.1:8123/v1/chat/completions --api-token=sk-test --model-name=gpt-test
# 预期：No staged changes. Please 'git add' your files first. 退出码非零

# 场景 4：正常流程（先 echo x > a.txt && git add a.txt）
cargo run -- ai --api-endpoint=http://127.0.0.1:8123/v1/chat/completions --api-token=sk-test --model-name=gpt-test
# 预期：显示 3 条候选 → Enter 选中第一条 → Commit successful! → git log -1 验证提交消息

# 场景 5：Ctrl-C（重复场景 4，在候选列表按 Ctrl-C）
# 预期：Commit aborted. 退出码 0，git log 无新提交

# 场景 6：非法 JSON（python3 /tmp/mock_llm.py bad &，重复场景 4）
# 预期：llm api response is not a json string 退出码非零

# 场景 7：环境变量回退（unset token 参数，export GIT_CZ_AI_OPENAI_API_KEY=sk-env）
# 预期：流程正常走到候选列表
```

- [ ] **步骤 5：Commit**

```bash
git add src/main.rs
git commit -m "feat: add ai subcommand"
```

---

### 任务 5：更新 AGENTS.md 项目知识

**文件：**
- 修改：`AGENTS.md`

> 按 superpowers:writing-for-agents 技能更新（该技能规定 agent 文档的编写规范）。

- [ ] **步骤 1：读取 writing-for-agents 技能**

读取 `/Users/secret/.agents/skills/writing-for-agents/SKILL.md` 并遵循其规范。

- [ ] **步骤 2：更新 AGENTS.md 相关章节**

需要更新的位置（对照当前 AGENTS.md 行号）：

1. **§2 技术栈依赖表**：追加 `clap 4`（CLI 解析）、`serde_json 1`（JSON）、`ureq 2`（HTTP 客户端，rustls）。
2. **§3 架构**：新增 `src/ai.rs`（AI 子命令纯逻辑层）。
3. **§4 目录结构**：`src/` 下追加 `ai.rs`。
4. **§5 核心业务逻辑**：新增小节「AI 子命令（`git-cz ai`）」——`AI_PROMPT_TEMPLATE`（用户提供的提示词，`{{diff}}` 占位符）、`build_ai_prompt`、`parse_llm_response`（双层解析：OpenAI envelope → content → `Vec<String>`；回退直接数组；失败统一报 `llm api response is not a json string`）、`get_staged_diff`（`git diff --cached`，空则报 `No staged changes. Please 'git add' your files first.`）。
5. **§5.4 交互流程**：新增 `run_ai` 流程（diff → prompt → ureq POST → parse → QuerySelector 选择 → Enter 提交 / Ctrl-C 退出，promkit 0.4.5 的 Ctrl-C 表现为 `Err("ctrl+c")`）。
6. **§7 外部依赖与集成**：新增「LLM API」（OpenAI 兼容 chat completions，`Authorization: Bearer <token>`）。
7. **§8 环境变量**：追加 `GIT_CZ_AI_OPENAI_API_KEY`（`--api-token` 的回退来源，clap `env` 特性实现）。
8. **§9 测试策略**：测试表追加 6 个新测试函数（`test_build_ai_prompt_placeholder`、`test_parse_llm_response_direct_array`、`test_parse_llm_response_openai_envelope`、`test_parse_llm_response_invalid_json`、`test_parse_llm_response_content_not_array`、`test_get_staged_diff`），总数 17 → 23。
9. **§11 附录**：补充 `git-cz ai` 调用示例与参数说明。
10. **§1「AI 名不副实」已知问题（§10 条目 5）**：更新为「已实现基础 AI 子命令」。

- [ ] **步骤 3：Commit**

```bash
git add AGENTS.md
git commit -m "docs: update AGENTS.md for ai subcommand"
```

---

## 自检记录

- **规格覆盖度**：设计文档的每个需求（diff=--cached、token 环境变量回退、提示词原样嵌入、解析失败错误消息、候选列表选择、Enter 提交、Ctrl-C 退出）→ 分别对应任务 3 / 任务 4 / 任务 1 / 任务 2 / 任务 4 / 任务 4 / 任务 4。✅
- **占位符扫描**：所有步骤含具体代码或具体命令，无「TODO」「待定」「适当处理」。✅
- **类型一致性**：`parse_llm_response` 返回 `Result<Vec<String>, Box<dyn Error>>`、`get_staged_diff` 返回 `Result<String, Box<dyn Error>>`、`build_ai_prompt` 返回 `String`，任务 4 的 `run_ai` 调用签名与任务 1-3 定义一致。✅
