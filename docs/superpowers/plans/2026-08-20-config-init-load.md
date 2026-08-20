# 配置文件初始化与加载实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 新增 `git-cz --init-config`（初始化 `~/.config/git-cz/config.toml`）与 `git-cz ai` 的配置加载（优先级 CLI > env > config）。

**架构：** 新增 `src/config.rs` 纯逻辑模块（`config_path` / `init_config` / `load_config` / `resolve_ai_args`，可单测），挂载到 lib；`main.rs` 加 `--init-config` 顶层 flag、`AiArgs` 三字段改 `Option<String>`、`run_ai` 先加载配置再合并。

**技术栈：** Rust + toml 0.8 + serde 1（derive）+ clap 4（现有）。

**设计规格：** `docs/superpowers/specs/2026-08-20-config-init-load-design.md`

---

## 文件结构

| 文件 | 动作 | 职责 |
|------|------|------|
| `Cargo.toml` | 修改 | `[dependencies]` 追加 `toml = "0.8"`、`serde = { version = "1", features = ["derive"] }` |
| `src/lib.rs` | 修改 | 顶部加 `pub mod config;` |
| `src/config.rs` | 创建 | 配置路径、TOML 读写、参数合并（纯逻辑，可单测） |
| `src/main.rs` | 修改 | `--init-config` flag、`AiArgs` 改 `Option<String>`、`run_init_config`、`run_ai` 配置加载 |
| `tests/main_test.rs` | 修改 | 追加 9 个 config 相关测试 |

> 关键约束：`resolve_ai_args` 接收三个 `Option<String>` 参数（不引用 `AiArgs`）——`AiArgs` 在 bin 层 `main.rs`，lib 层 `config.rs` 不能依赖它。env 优先级由 clap `#[arg(long, env = "...")]` 在 `Option<String>` 字段上自动处理（CLI 未提供时读 env），`resolve_ai_args` 只需合并「CLI（含 env 回退后的值）> config」。

---

### 任务 1：依赖 + `src/config.rs` 骨架 + `load_config`

**文件：**
- 修改：`Cargo.toml`、`src/lib.rs`
- 创建：`src/config.rs`
- 测试：`tests/main_test.rs`

- [ ] **步骤 1：Cargo.toml 加依赖 + lib.rs 加模块声明**

`Cargo.toml` 的 `[dependencies]` 追加：

```toml
toml = "0.8"
serde = { version = "1", features = ["derive"] }
```

`src/lib.rs` 顶部（`pub mod ai;` 之后）加：

```rust
pub mod config;
```

- [ ] **步骤 2：编写失败的测试**

`tests/main_test.rs` 顶部 `use` 追加：

```rust
use git_cz_ai::config::{load_config, Config};
```

文件末尾追加：

```rust
#[test]
fn test_load_config_missing_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    let config = load_config(&path).unwrap();
    assert!(config.api_endpoint.is_none());
    assert!(config.api_token.is_none());
    assert!(config.model_name.is_none());
}

#[test]
fn test_load_config_parses_toml() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    std::fs::write(
        &path,
        "api_endpoint=\"https://api.deepseek.com/v1/chat/completions\"\n\
         api_token=\"sk-test\"\n\
         model_name=\"deepseek-v4-flash\"\n",
    )
    .unwrap();
    let config = load_config(&path).unwrap();
    assert_eq!(
        config.api_endpoint.as_deref(),
        Some("https://api.deepseek.com/v1/chat/completions")
    );
    assert_eq!(config.api_token.as_deref(), Some("sk-test"));
    assert_eq!(config.model_name.as_deref(), Some("deepseek-v4-flash"));
}

#[test]
fn test_load_config_invalid_toml() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    std::fs::write(&path, "not valid toml {{{").unwrap();
    let err = load_config(&path).unwrap_err();
    assert!(err.to_string().contains("Failed to parse config"));
}

#[test]
fn test_load_config_partial_fields() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    std::fs::write(&path, "model_name=\"gpt-test\"\n").unwrap();
    let config = load_config(&path).unwrap();
    assert!(config.api_endpoint.is_none());
    assert!(config.api_token.is_none());
    assert_eq!(config.model_name.as_deref(), Some("gpt-test"));
}
```

- [ ] **步骤 3：运行测试验证失败**

运行：`cargo test test_load_config`
预期：编译失败，`unresolved import git_cz_ai::config` / `unresolved module config`

- [ ] **步骤 4：创建 `src/config.rs` 实现**

```rust
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// 默认配置文件内容（用户规格原文，禁止改写）。
pub const DEFAULT_CONFIG_CONTENT: &str = "api_endpoint=\"https://api.deepseek.com/v1/chat/completions\"\n\
api_token=\"sk-your-token-string\"\n\
model_name=\"deepseek-v4-flash\"\n";

/// 配置文件结构；缺失字段为 None。
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub api_endpoint: Option<String>,
    pub api_token: Option<String>,
    pub model_name: Option<String>,
}

/// 返回 `~/.config/git-cz/config.toml`；`$XDG_CONFIG_HOME` 未设时回退 `~/.config`；
/// 取不到 home 时报错。
pub fn config_path() -> Result<PathBuf, Box<dyn Error>> {
    let base = match env::var("XDG_CONFIG_HOME") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            let home = env::var("HOME").map_err(|_| "Cannot determine home directory")?;
            PathBuf::from(home).join(".config")
        }
    };
    Ok(base.join("git-cz").join("config.toml"))
}

/// 加载配置文件；文件不存在返回全 None 的 Config（不报错）。
/// 解析失败返回 Err，消息含路径与解析错误。
pub fn load_config(path: &Path) -> Result<Config, Box<dyn Error>> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse config {}: {}", path.display(), e))?;
    Ok(config)
}
```

> `config_path` 与 `init_config`（任务 2）、`resolve_ai_args`（任务 3）暂未定义——本任务先实现 `load_config` 及其测试，其余函数在后续任务追加。若编译报 `unused` 警告属正常（后续任务会用上）。

- [ ] **步骤 5：运行测试验证通过**

运行：`cargo test test_load_config`
预期：PASS（4 passed）

- [ ] **步骤 6：Commit**

```bash
git add Cargo.toml Cargo.lock src/lib.rs src/config.rs tests/main_test.rs
git commit -m "feat: add config module with toml loading"
```

---

### 任务 2：`init_config`

**文件：**
- 修改：`src/config.rs`
- 测试：`tests/main_test.rs`

- [ ] **步骤 1：编写失败的测试**

`tests/main_test.rs` 顶部 `use` 追加：

```rust
use git_cz_ai::config::init_config;
```

文件末尾追加：

```rust
#[test]
fn test_init_config_creates_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("git-cz").join("config.toml");
    let created = init_config(&path).unwrap();
    assert!(created, "文件不存在时应创建并返回 true");
    assert!(path.exists(), "配置文件应被创建");
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("api_endpoint"), "默认内容应含 api_endpoint");
    assert!(content.contains("api_token"), "默认内容应含 api_token");
    assert!(content.contains("model_name"), "默认内容应含 model_name");
}

#[test]
fn test_init_config_exists() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    std::fs::create_dir_all(temp_dir.path()).unwrap();
    std::fs::write(&path, "original").unwrap();
    let created = init_config(&path).unwrap();
    assert!(!created, "文件已存在时应返回 false");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "original",
        "已存在的文件不应被覆盖"
    );
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test test_init_config`
预期：编译失败，`function not found in config: init_config`

- [ ] **步骤 3：实现 `init_config`**

`src/config.rs` 末尾追加（`fs`、`Path` 已引入）：

```rust
/// 初始化配置文件。已存在返回 Ok(false)（不动文件）；
/// 不存在则创建父目录并写入默认内容，返回 Ok(true)。
pub fn init_config(path: &Path) -> Result<bool, Box<dyn Error>> {
    if path.exists() {
        return Ok(false);
    }
    let dir = path.parent().ok_or("Config path has no parent directory")?;
    fs::create_dir_all(dir)?;
    fs::write(path, DEFAULT_CONFIG_CONTENT)?;
    Ok(true)
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test test_init_config`
预期：PASS（2 passed）

- [ ] **步骤 5：Commit**

```bash
git add src/config.rs tests/main_test.rs
git commit -m "feat: add config file initializer"
```

---

### 任务 3：`resolve_ai_args`

**文件：**
- 修改：`src/config.rs`
- 测试：`tests/main_test.rs`

- [ ] **步骤 1：编写失败的测试**

`tests/main_test.rs` 顶部 `use` 追加：

```rust
use git_cz_ai::config::resolve_ai_args;
```

文件末尾追加：

```rust
#[test]
fn test_resolve_ai_args_cli_wins() {
    let config = Config {
        api_endpoint: Some("https://config.example.com".to_string()),
        api_token: Some("config-token".to_string()),
        model_name: Some("config-model".to_string()),
    };
    let resolved = resolve_ai_args(
        Some("https://cli.example.com".to_string()),
        Some("cli-token".to_string()),
        Some("cli-model".to_string()),
        &config,
    )
    .unwrap();
    assert_eq!(resolved.api_endpoint, "https://cli.example.com");
    assert_eq!(resolved.api_token, "cli-token");
    assert_eq!(resolved.model_name, "cli-model");
}

#[test]
fn test_resolve_ai_args_config_fallback() {
    let config = Config {
        api_endpoint: Some("https://config.example.com".to_string()),
        api_token: Some("config-token".to_string()),
        model_name: Some("config-model".to_string()),
    };
    let resolved = resolve_ai_args(None, None, None, &config).unwrap();
    assert_eq!(resolved.api_endpoint, "https://config.example.com");
    assert_eq!(resolved.api_token, "config-token");
    assert_eq!(resolved.model_name, "config-model");
}

#[test]
fn test_resolve_ai_args_missing() {
    let config = Config::default();
    let missing = resolve_ai_args(None, None, None, &config).unwrap_err();
    assert_eq!(missing.len(), 3, "三个字段均缺应返回 3 个缺失项");
    assert!(missing.iter().any(|m| m.contains("api-endpoint")));
    assert!(missing.iter().any(|m| m.contains("api-token")));
    assert!(missing.iter().any(|m| m.contains("model-name")));
}
```

- [ ] **步骤 2：运行测试验证失败**

运行：`cargo test test_resolve_ai_args`
预期：编译失败，`function not found in config: resolve_ai_args`

- [ ] **步骤 3：实现 `resolve_ai_args`**

`src/config.rs` 末尾追加：

```rust
/// 合并后的 AI 请求参数（三字段均为确定值）。
#[derive(Debug, PartialEq)]
pub struct ResolvedAiArgs {
    pub api_endpoint: String,
    pub api_token: String,
    pub model_name: String,
}

/// 按「CLI（含 clap env 回退后的值）> config」合并三字段；
/// 任一字段三处均缺时返回 Err(缺失的 CLI 参数名列表，如 "--api-endpoint")。
pub fn resolve_ai_args(
    cli_api_endpoint: Option<String>,
    cli_api_token: Option<String>,
    cli_model_name: Option<String>,
    config: &Config,
) -> Result<ResolvedAiArgs, Vec<String>> {
    let api_endpoint = cli_api_endpoint.or_else(|| config.api_endpoint.clone());
    let api_token = cli_api_token.or_else(|| config.api_token.clone());
    let model_name = cli_model_name.or_else(|| config.model_name.clone());

    let mut missing = Vec::new();
    if api_endpoint.is_none() {
        missing.push("--api-endpoint".to_string());
    }
    if api_token.is_none() {
        missing.push("--api-token".to_string());
    }
    if model_name.is_none() {
        missing.push("--model-name".to_string());
    }
    if !missing.is_empty() {
        return Err(missing);
    }

    Ok(ResolvedAiArgs {
        api_endpoint: api_endpoint.unwrap(),
        api_token: api_token.unwrap(),
        model_name: model_name.unwrap(),
    })
}
```

- [ ] **步骤 4：运行测试验证通过**

运行：`cargo test test_resolve_ai_args`
预期：PASS（3 passed）

- [ ] **步骤 5：Commit**

```bash
git add src/config.rs tests/main_test.rs
git commit -m "feat: add ai args resolver with cli-over-config priority"
```

---

### 任务 4：CLI 集成（`--init-config` + `run_ai` 配置加载）

**文件：**
- 修改：`src/main.rs`
- 手动验证：mock LLM API（临时脚本，不入库）

- [ ] **步骤 1：修改 `src/main.rs`**

顶部 `use` 追加：

```rust
use git_cz_ai::config::{config_path, init_config, load_config, resolve_ai_args};
```

`Cli` struct 加 `--init-config` 顶层 flag：

```rust
#[derive(Parser)]
struct Cli {
    /// 初始化配置文件（~/.config/git-cz/config.toml）
    #[arg(long)]
    init_config: bool,
    /// 子命令；不传则保持现有交互式提交流程
    #[command(subcommand)]
    command: Option<CliCommand>,
}
```

`AiArgs` 三字段改为 `Option<String>`：

```rust
#[derive(Args)]
struct AiArgs {
    /// LLM API 端点，如 https://api.openai.com/v1/chat/completions
    #[arg(long)]
    api_endpoint: Option<String>,
    /// API 令牌；未提供时回退到环境变量 GIT_CZ_AI_OPENAI_API_KEY 与配置文件
    #[arg(long, env = "GIT_CZ_AI_OPENAI_API_KEY")]
    api_token: Option<String>,
    /// 模型名称，如 gpt-5-mini
    #[arg(long)]
    model_name: Option<String>,
}
```

`main()` 分派 `--init-config`（优先于子命令）：

```rust
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    if cli.init_config {
        return run_init_config();
    }
    match cli.command {
        Some(CliCommand::Ai(args)) => run_ai(&args),
        None => run_interactive(),
    }
}
```

新增 `run_init_config()`：

```rust
fn run_init_config() -> Result<(), Box<dyn std::error::Error>> {
    let path = config_path()?;
    if init_config(&path)? {
        println!("Config created: {}", path.display());
    } else {
        println!("Config is exists({})", path.display());
    }
    Ok(())
}
```

`run_ai` 开头改为先加载配置再合并（替换原第 49-54 行的开头部分，请求段改用 `resolved` 值）：

```rust
fn run_ai(args: &AiArgs) -> Result<(), Box<dyn std::error::Error>> {
    // 0. 加载配置文件并合并参数（优先级：CLI > env > config）
    let path = config_path()?;
    let config = load_config(&path)?;
    let resolved = match resolve_ai_args(
        args.api_endpoint.clone(),
        args.api_token.clone(),
        args.model_name.clone(),
        &config,
    ) {
        Ok(resolved) => resolved,
        Err(missing) => {
            for field in &missing {
                eprintln!(
                    "Missing {}. Set it via CLI, config file, or environment variable",
                    field
                );
            }
            std::process::exit(1);
        }
    };

    // 1. 获取已暂存的 diff（git diff --cached）
    let diff = get_staged_diff(Path::new("."))?;

    // 2. 用 diff 替换提示词中的 {{diff}} 占位符
    let prompt = build_ai_prompt(&diff);

    // 3. 发送请求到 LLM API（改用 resolved 值）
    eprintln!("Request has been sent, waiting for response");
    let response = ureq::post(&resolved.api_endpoint)
        .set("Authorization", &format!("Bearer {}", resolved.api_token))
        .set("Content-Type", "application/json")
        .send_json(serde_json::json!({
            "model": resolved.model_name,
            "messages": [{ "role": "user", "content": prompt }],
        }));
    // ... 后续（步骤 4-6：解析、选择、提交）保持不变，仅把 `args.api_endpoint` 等
    //     引用替换为 `resolved.api_endpoint` 等
```

> 步骤 4-6（`parse_llm_response`、`QuerySelector`、`perform_commit`）中所有 `args.api_*` / `args.model_name` 引用一律替换为 `resolved.api_*` / `resolved.model_name`。`run_interactive` 不改动。

- [ ] **步骤 2：编译检查**

运行：`cargo build`
预期：编译通过（bin `git-cz`）

- [ ] **步骤 3：运行全部测试**

运行：`cargo test`
预期：全部 PASS（既有 23 个 + 新增 9 个 config 测试 = 32 个）

- [ ] **步骤 4：手动验证（mock LLM API）**

创建 `/tmp/mock_llm_cfg.py`（不入库）：

```python
#!/usr/bin/env python3
import json
from http.server import BaseHTTPRequestHandler, HTTPServer

class Handler(BaseHTTPRequestHandler):
    def do_POST(self):
        length = int(self.headers.get("Content-Length", 0))
        body = json.loads(self.rfile.read(length))
        # 回显收到的 model，便于验证 CLI > config 优先级
        resp = json.dumps({
            "choices": [{"message": {"content": json.dumps([
                "feat: add login endpoint",
            ])}}],
            "echoed_model": body["model"],
        }).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(resp)))
        self.end_headers()
        self.wfile.write(resp)

HTTPServer(("127.0.0.1", 8125), Handler).serve_forever()
```

在隔离的 HOME 下验证（避免污染真实配置；用一个测试仓库，确保 user.name/user.email 已配置）：

```bash
# 场景 1：首次 init —— 创建配置
HOME=/tmp/cfgtest-home target/debug/git-cz --init-config
# 预期：Config created: /tmp/cfgtest-home/.config/git-cz/config.toml
# 并确认文件内容为默认三行

# 场景 2：再次 init —— 已存在
HOME=/tmp/cfgtest-home target/debug/git-cz --init-config
# 预期：Config is exists(/tmp/cfgtest-home/.config/git-cz/config.toml)

# 场景 3：仅配置、无 CLI 参数 —— 用配置值（mock 回显验证 model）
python3 /tmp/mock_llm_cfg.py &
cd /tmp/cfgtest-repo && echo x > a.txt && git add a.txt
HOME=/tmp/cfgtest-home target/debug/git-cz ai --api-endpoint=http://127.0.0.1:8125/v1/chat/completions --api-token=sk-test
# 预期：请求体 model 为 deepseek-v4-flash（来自配置）；Enter 提交成功

# 场景 4：CLI 覆盖 config —— 请求体 model 用 CLI 值
HOME=/tmp/cfgtest-home target/debug/git-cz ai --api-endpoint=http://127.0.0.1:8125/v1/chat/completions --api-token=sk-test --model-name=gpt-cli-test
# 预期：请求体 model 为 gpt-cli-test（CLI 优先）；Enter 提交成功

# 场景 5：三处均缺 model_name —— 报缺失项
# （临时改配置删掉 model_name 行，或用一个无配置的 HOME）
HOME=/tmp/cfgtest-empty target/debug/git-cz ai --api-endpoint=http://127.0.0.1:8125/v1/chat/completions --api-token=sk-test
# 预期：Missing --model-name. Set it via CLI, config file, or environment variable，退出码非零

# 场景 6：配置损坏 —— 报解析错误
echo "not valid toml {{" > /tmp/cfgtest-home/.config/git-cz/config.toml
HOME=/tmp/cfgtest-home target/debug/git-cz ai --api-endpoint=http://127.0.0.1:8125/v1/chat/completions --api-token=sk-test
# 预期：Failed to parse config /tmp/cfgtest-home/.config/git-cz/config.toml: ...，退出码非零
```

> `--init-config` 场景需 PTY 吗？不需要——init 与报错路径均无交互组件；只有场景 3/4 的候选选择需要 TTY（promkit），用之前验证过的 PTY 驱动方式（设置窗口尺寸 + 回应 `ESC[6n` 光标查询）或仅在能交互的终端中执行。

- [ ] **步骤 5：Commit**

```bash
git add src/main.rs
git commit -m "feat: add config init flag and ai arg resolution"
```

---

### 任务 5：更新 AGENTS.md 与 README

**文件：**
- 修改：`AGENTS.md`、`README.md`

- [ ] **步骤 1：更新 `README.md`**

在「使用」章节新增小节：

````markdown
### 配置文件

`git-cz ai` 的 API 参数可从配置文件加载（命令行参数优先于配置文件；token 还优先于环境变量 `GIT_CZ_AI_OPENAI_API_KEY`）。

初始化默认配置（`~/.config/git-cz/config.toml`）：

```bash
git-cz --init-config
```

首次运行会创建配置文件，内容如下：

```toml
api_endpoint="https://api.deepseek.com/v1/chat/completions"
api_token="sk-your-token-string"
model_name="deepseek-v4-flash"
```

之后运行 `git-cz ai` 可省略命令行参数（仍可显式传入以覆盖配置值）：

```bash
git-cz ai
# 等价于：git-cz ai --api-endpoint=https://api.deepseek.com/v1/chat/completions \
#                    --api-token=sk-your-token-string --model-name=deepseek-v4-flash
```
````

- [ ] **步骤 2：更新 `AGENTS.md`**

1. **§2 技术栈依赖表**：追加 `toml 0.8`（配置解析）、`serde 1`（derive，配置反序列化）。
2. **§3 架构 / §4 目录结构**：新增 `src/config.rs`（配置纯逻辑层：路径、TOML 读写、参数合并）。
3. **§5 核心业务逻辑**：新增小节「配置系统（`--init-config` 与配置加载）」——`config_path`（`$XDG_CONFIG_HOME` 回退 `~/.config`）、`init_config`（存在返回 false 不动文件 / 不存在创建目录+默认内容）、`load_config`（缺失文件返回全 None / 解析失败报错）、`resolve_ai_args`（CLI > env > config，缺失返回字段名列表）。
4. **§5.5 AI 子命令数据流**：开头补「加载配置 → 合并参数（CLI > env > config）」；`AiArgs` 三字段为 `Option<String>`。
5. **§8 配置与环境**：新增 `~/.config/git-cz/config.toml` 配置项说明（三字段、优先级链）。
6. **§9 测试策略**：测试表追加 9 个 config 测试，总数 23 → 32。
7. **§11 附录**：补充 `git-cz --init-config` 用法。

- [ ] **步骤 3：Commit**

```bash
git add README.md AGENTS.md
git commit -m "docs: document config init and load feature"
```

---

## 自检记录

- **规格覆盖度**：默认配置内容 → 任务 1 步骤 4 `DEFAULT_CONFIG_CONTENT`；`config_path` → 任务 1；`init_config` → 任务 2；`load_config` → 任务 1；`resolve_ai_args` → 任务 3；`--init-config` flag + `run_init_config` → 任务 4；`run_ai` 配置加载 → 任务 4；文档 → 任务 5。✅
- **占位符扫描**：所有步骤含具体代码或具体命令，无「TODO」「待定」。✅
- **类型一致性**：`Config`（三 `Option<String>` 字段）、`ResolvedAiArgs`（三 `String` 字段）、`resolve_ai_args(Option<String> × 3, &Config) -> Result<ResolvedAiArgs, Vec<String>>` 在任务 1/3/4 中签名一致；`run_ai` 内 `resolved.api_endpoint` / `resolved.api_token` / `resolved.model_name` 与 `ResolvedAiArgs` 字段一致。✅
- **注意**：`resolve_ai_args` 不引用 `AiArgs`（lib 层不依赖 bin 层类型），改为三个 `Option<String>` 参数——与规格中「`(cli: &AiArgs, config: &Config)`」的示意不同，但行为等价（env 由 clap 并入 CLI 层），已在文件结构节说明。✅
