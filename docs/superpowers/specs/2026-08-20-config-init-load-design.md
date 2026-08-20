# 配置文件初始化与加载设计规格

> 日期：2026-08-20 · 状态：已批准

## 需求

为 git-cz-ai 新增配置文件支持：

1. **初始化**：`git-cz --init-config` 检查 `~/.config/git-cz/config.toml`；存在则提示 `Config is exists(~/.config/git-cz/config.toml)`；不存在则创建目录与默认配置文件
2. **加载**：`git-cz ai` 运行时加载配置文件；取值优先级为**命令行参数 > 环境变量（仅 token）> 配置文件**

## 设计决策（经用户确认）

| 决策点 | 选择 | 理由 |
|--------|------|------|
| 配置路径 | `~/.config/git-cz/config.toml` | 用户确认（提示信息中的 `~/.config/.git-cz/` 为笔误） |
| 优先级链 | CLI > env（`GIT_CZ_AI_OPENAI_API_KEY`，仅 token）> config | 用户确认；CLI 最明确、env 次之、config 最持久 |
| 缺失值行为 | 报错并指明具体缺失项（如 `Missing --api-endpoint. Set it via CLI, config file, or environment variable`），退出码非零 | 用户确认；不静默猜测 |
| 创建成功输出 | 输出 `Config created: ~/.config/git-cz/config.toml` | 用户确认 |
| 配置容错 | TOML 解析失败 → 报错退出（指明路径与解析错误）；字段缺失 → 该字段回退 CLI/env，三处均无才报缺失项 | 用户确认；不静默用默认值 |
| 架构 | 新增 `src/config.rs` 纯逻辑模块（方案 A） | 延续项目「lib 纯逻辑可测 + bin 编排」分层；配置解析可单测 |

## 默认配置内容

```toml
api_endpoint="https://api.deepseek.com/v1/chat/completions"
api_token="sk-your-token-string"
model_name="deepseek-v4-flash"
```

## 改动清单

### 新增文件

**`src/config.rs`**（纯逻辑，可单测）：

| 项 | 签名 | 说明 |
|----|------|------|
| `DEFAULT_CONFIG_CONTENT` | `&'static str` | 默认配置内容（上节原文） |
| `Config` | struct | `api_endpoint` / `api_token` / `model_name`，均 `Option<String>`（TOML `#[derive(Deserialize)]`，缺失字段为 `None`） |
| `config_path` | `() -> PathBuf` | 返回 `~/.config/git-cz/config.toml`（`$XDG_CONFIG_HOME` 未设时回退 `~/.config`；取不到 home 时 `Err`） |
| `init_config` | `(path: &Path) -> Result<bool, Box<dyn Error>>` | 已存在返回 `Ok(false)`；不存在则创建目录 + 写默认内容，返回 `Ok(true)` |
| `load_config` | `(path: &Path) -> Result<Config, Box<dyn Error>>` | 读取并解析 TOML；文件不存在返回 `Ok(Config::default())`（全 `None`）；解析失败返回 `Err`（含路径与解析错误） |
| `resolve_ai_args` | `(cli: &AiArgs, config: &Config) -> Result<ResolvedAiArgs, Vec<String>>` | 按 CLI > env(token) > config 合并；返回缺失项列表或解析结果 |

### 修改文件

- **`Cargo.toml`**：`[dependencies]` 追加 `toml = "0.8"`、`serde = { version = "1", features = ["derive"] }`
- **`src/lib.rs`**：顶部加 `pub mod config;`
- **`src/main.rs`**：
  - `Cli` struct 加 `#[arg(long)] init_config: bool`（`--init-config` 顶层 flag）
  - `AiArgs` 三字段改为 `Option<String>`：`api_endpoint: Option<String>`、`api_token: Option<String>`、`model_name: Option<String>`（token 保留 `#[arg(long, env = "GIT_CZ_AI_OPENAI_API_KEY")]`）
  - 新增 `run_init_config() -> Result<(), Box<dyn Error>>`：调用 `config_path` + `init_config`，按结果输出提示
  - `main()` 分派：`cli.init_config` 为真 → `run_init_config()`；否则按子命令分派（`Ai` / 无子命令）
  - `run_ai` 改为：`load_config` → `resolve_ai_args` → 缺失则报错退出（逐项指明）→ 用解析结果发送请求（后续流程不变）

### 测试（`tests/main_test.rs` 追加）

| 测试 | 覆盖点 |
|------|--------|
| `test_init_config_creates_file` | 不存在时创建目录+文件，内容为默认值，返回 `Ok(true)` |
| `test_init_config_exists` | 已存在时不动文件，返回 `Ok(false)` |
| `test_load_config_missing_file` | 文件不存在 → 全 `None` 的 `Config` |
| `test_load_config_parses_toml` | 有效 TOML → 三个字段正确解析 |
| `test_load_config_invalid_toml` | 非法 TOML → `Err` |
| `test_load_config_partial_fields` | 部分字段缺失 → 对应 `None` |
| `test_resolve_ai_args_cli_wins` | CLI 提供全部 → 用 CLI 值 |
| `test_resolve_ai_args_config_fallback` | CLI 全缺 + config 全有 → 用 config 值 |
| `test_resolve_ai_args_missing` | 三处均缺某字段 → 返回缺失项列表 |

> `config_path` 依赖 `HOME` 环境变量，测试中用 `tempdir` 模拟路径的用例通过直接传 `Path` 给 `init_config`/`load_config` 规避（不测 `config_path` 本身，或测其含 `git-cz/config.toml` 后缀）。

## 行为规格

| 场景 | 输出/行为 |
|------|-----------|
| `git-cz --init-config` + 配置已存在 | `Config is exists(~/.config/git-cz/config.toml)`，退出码 0，文件不动 |
| `git-cz --init-config` + 配置不存在 | 创建 `~/.config/git-cz/` 与 `config.toml`（默认内容），输出 `Config created: ~/.config/git-cz/config.toml`，退出码 0 |
| `git-cz ai` + CLI 全提供 | 用 CLI 值，不读配置 |
| `git-cz ai` + CLI 缺 `--api-token` + env 有 | 用 env 值（clap env 特性自动回退） |
| `git-cz ai` + CLI/env 缺 + config 有 | 用 config 值 |
| 三处均缺某字段 | `Missing --<field>. Set it via CLI, config file, or environment variable`（逐项列出全部缺失项），退出码非零 |
| 配置 TOML 解析失败 | `Failed to parse config <path>: <error>`，退出码非零 |
| 配置字段缺失（部分） | 该字段回退 CLI/env；三处均无才报缺失项 |
| `--init-config` 与子命令同用 | `--init-config` 优先执行（初始化后退出，不进入交互/AI 流程） |

## 非目标（YAGNI）

- 不做配置热重载、不做多配置文件（项目级 `.git-cz.toml`）、不做 TOML 写回（除 init 外不修改配置）
- 不改交互式模式（`run_interactive`）行为——配置仅作用于 `ai` 子命令
- 不新增 UI 交互（无 Confirm 询问是否覆盖已存在配置）

## 验证

1. `cargo test`：既有 23 个 + 新增 9 个 config 测试全部通过
2. `cargo build` 无警告
3. 手动验证：
   - `git-cz --init-config`（首次）→ 创建 + 提示；再次 → `Config is exists(...)`
   - `git-cz ai`（无 CLI 参数、有配置）→ 正常走请求流程（mock LLM）
   - `git-cz ai --api-endpoint=...`（CLI 覆盖 config）→ 请求体用 CLI 值
   - 缺 token 且无 env/config → `Missing --api-token...` 退出码非零
   - 配置文件写坏 → `Failed to parse config ...` 退出码非零
