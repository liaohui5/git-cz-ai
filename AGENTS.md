## 1. 项目概述

- **项目名称**：`git-cz-ai`
- **版本**：`0.0.6`（见 `Cargo.toml` 的 `version` 字段；CHANGELOG.md 记录 0.0.4 ~ 0.0.6）
- **一句话描述**：一个用 Rust 编写的交互式 Git 提交信息生成器（Commitizen 风格 CLI 工具），支持两种模式：
  - **交互式模式**（无子命令）：终端向导逐步填写提交信息（类型、scope、破坏性变更标记、描述、正文、页脚），生成 Conventional Commits 消息并执行 `git commit`。
  - **AI 模式**（`git-cz ai`）：读取 staged diff → 调用 OpenAI 兼容 LLM API 生成 3–6 条提交信息候选 → 命令行选择 → Enter 自动提交 / Ctrl-C 退出。
- **来源**：README 注明灵感来自 [k3ii/git-cz](https://github.com/k3ii/git-cz) 与 [cz-git](https://github.com/Zhengqbbb/cz-git)，git remote 指向 `git@github.com:liaohui5/git-cz-ai.git`（当前检出分支 `dev`，另存在 `main`）。

## 2. 技术栈与依赖

| 类别        | 名称          | 版本（Cargo.lock）                    | 用途                                                                          |
| ----------- | ------------- | ------------------------------------- | ----------------------------------------------------------------------------- |
| 语言        | Rust          | edition 2021（Cargo.toml）            | —                                                                             |
| CLI 解析    | `clap`        | 4.6.6                                 | 子命令（`ai` / `init-config` / 默认）与参数解析（**无 derive、无 env 特性**） |
| Git 绑定    | `git2`        | 0.21.0                                | 仓库打开、索引、diff、提交                                                    |
| 底层 Git    | `libgit2-sys` | 0.18.7+1.9.6（内置 libgit2 1.9.6）    | git2 的 C 绑定（传递依赖）                                                    |
| 交互式 UI   | `inquire`     | 0.9.4（`editor` 特性）                | Select / Text / Confirm / Editor 组件                                         |
| 终端后端    | `crossterm`   | 传递依赖                              | inquire 的终端渲染后端                                                        |
| HTTP 客户端 | `ureq`        | 3.4.0（`json` 特性，默认 rustls TLS） | 向 LLM API 发送 chat completions 请求                                         |
| 序列化      | `serde`       | 1.0.229（`derive` 特性）              | `APIConfig` 反序列化                                                          |
| JSON        | `serde_json`  | 1.0.151                               | LLM 请求/响应编解码                                                           |
| TOML 解析   | `toml`        | 1.1.4                                 | 配置文件（`config.toml`）解析                                                 |

> **与旧版（v0.0.1）的重要差异**：当前代码**不依赖 openssl**（Cargo.lock 中 openssl 出现次数为 0，ureq 3 默认使用 rustls）、**不依赖 tempfile**、**无外部编辑器/EDITOR 环境变量逻辑**（body 编辑改用 inquire 的 Editor 组件）、**无 `GIT_CZ_AI_OPENAI_API_KEY` 环境变量回退**（clap 未启用 env 特性）。

### 构建产物（bin）

`Cargo.toml` 的 `[[bin]]` 段将可执行文件命名为 **`git-cz`**（与包名 `git-cz-ai` 不同），入口为 `src/main.rs`。

## 3. 架构设计

单二进制 CLI 应用，模块化分层（`src/lib.rs` 仅挂载各 `pub mod`）：

```
src/main.rs     (入口：clap 子命令分发 + 统一错误输出)
    ├── ai::handler(args)          → AI 子命令流程
    ├── config::handler()          → init-config 子命令流程
    └── manually::handler()        → 默认交互式流程
```

各模块职责单一：

- **`src/git.rs`（Git 操作层）**：`get_staged_diff`（staged diff 文本）、`has_staged_changes`（staged 检查）、`perform_commit`（执行提交）。三者均正确处理**空仓库（unborn branch）**场景。
- **`src/ai.rs`（AI 子命令层）**：`create_ai_cmd`（clap 命令定义）、`handler`（流程编排）、`send_request`（ureq HTTP + LoadingSpinner）、`select_commit_message`（inquire Select）、`parse_llm_api_response`（响应解析）、`parse_args_to_config`（CLI 参数→配置）、`build_ai_prompt`（提示词模板）、`format_request_error`（ureq 错误映射）。
- **`src/config.rs`（配置层）**：`APIConfig` 结构、`get_config_path`（路径解析）、`init_config`（初始化默认配置）、`load_config`（读取解析）、`merge_config`（CLI > 文件 合并）、`create_init_config_cmd` / `handler`。
- **`src/manually.rs`（交互式流程层）**：`handler`（向导编排）、`build_commit_message`（消息拼接）、各类输入函数（类型/scope/破坏性标记/描述/正文/页脚/确认）。
- **`src/loading.rs`（UI 工具层）**：`LoadingSpinner`，后台线程渲染 `|/-\` 旋转动画，`start`/`stop` 控制。

## 4. 目录结构

```
git-cz-ai/
├── Cargo.toml          # 包元数据与依赖声明（bin 名 git-cz，v0.0.6）
├── Cargo.lock          # 依赖锁定文件（已提交）
├── README.md           # 使用文档（部分描述与代码不一致，见 §9）
├── CHANGELOG.md        # 版本变更记录（Keep a Changelog 格式）
├── LICENSE             # MIT 协议
├── .gitignore          # 忽略 /target、macOS 系统文件等
├── .github/workflows/
│   └── release-plz.yml # release-plz 自动发布工作流（push main 触发）
├── src/
│   ├── main.rs         # 入口：clap 子命令分发 + 统一错误输出
│   ├── lib.rs          # pub mod ai / config / git / manually / loading 挂载
│   ├── ai.rs           # AI 子命令逻辑（含 19 个内嵌单元测试）
│   ├── config.rs       # 配置系统（含 8 个内嵌单元测试）
│   ├── git.rs          # Git 操作层（含 4 个内嵌单元测试）
│   ├── manually.rs     # 交互式流程（含 8 个内嵌单元测试）
│   └── loading.rs      # LoadingSpinner 工具
├── preview.gif         # README 中的交互预览图
└── target/             # 构建产物（git 忽略）
```

> 无 `tests/` 目录——全部测试内嵌在各模块的 `#[cfg(test)] mod xxx_test` 中。

## 5. 核心业务逻辑

### 5.1 交互式流程（`src/manually.rs`）

```
has_staged_changes() 预检（无 staged 直接报错退出）
  → Select 提交类型（11 种 "type: desc"，取冒号前类型名）
  → Text 输入 scope（可选，非空则包成 (scope)）
  → Confirm 是否破坏性变更（默认否，是则加 "!"）
  → Text 输入描述（validator 拒绝空串）
  → Editor 组件输入正文（可选，inquire editor 特性）
  → Confirm 是否添加 footer（默认否）
      ├─ 是 → Select footer 类型（close / fix）→ Text issue 号（validator 要求 usize）→ "type: #num"
      └─ 否 → footer 为空
  → build_commit_message 拼接完整消息并打印预览（50 个 "-" 分隔线）
  → Confirm 确认提交（默认是）→ perform_commit
```

- **提交类型全集（11 种）**：`feat` `fix` `docs` `style` `refactor` `perf` `test` `chore` `ci` `build` `revert`。
- **`build_commit_message`**：`{type}{scope}{!}: {description}`，body/footer 非空时以空行分隔追加。**不校验输入**——全空时返回 `": "`（测试 `build_commit_message_all_empty` 已断言）。
- **footer 格式**：`类型: #编号`（如 `fix: #123`），issue 号必须能解析为 `usize`。
- **交互提示文案**：全部为英文，硬编码在源码中，无国际化。

### 5.2 Git 操作层（`src/git.rs`）

| 函数                 | 签名                                                        | 说明                                                                                                                                                                                                                                                                                                                         |
| -------------------- | ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `get_staged_diff`    | `() -> Result<String, Box<dyn Error>>`                      | 打开当前目录仓库，取 index 与 HEAD tree 的 diff，以 `DiffFormat::Patch` 回调拼接为文本；`repo.head()` 返回 `UnbornBranch`（空仓库）时以**空树为基线**（等价 `git diff --cached` 行为）；无 staged 时报错 `No staged changes. Please 'git add' your files first.`。内部实现为 `get_staged_diff_in(&Path)`（参数化以支持测试） |
| `has_staged_changes` | `() -> Result<(), Box<dyn Error>>`                          | staged 预检：HEAD 存在则 diff index vs HEAD tree，`UnbornBranch` 时以空树为基线；无任何 staged 报错同上                                                                                                                                                                                                                      |
| `perform_commit`     | `(full_commit_message: &str) -> Result<(), Box<dyn Error>>` | `index.write_tree()` → 读取 `user.name`/`user.email` 构造签名 → HEAD 存在则取父提交，`UnbornBranch`（空仓库首提）则无父提交 → `repo.commit(Some("HEAD"), ...)`                                                                                                                                                               |

> **关键行为**：只提交**已暂存**内容，不自动 `git add`；无 staged 被 `has_staged_changes`/`get_staged_diff` 拦截。

### 5.3 AI 子命令（`src/ai.rs`）

**数据流**（`handler`）：

```
load_config()（读 ~/.config/git-cz/config.toml）
  → parse_args_to_config(args)（CLI 参数）
  → merge_config(file_config, args_config)（args_config 覆盖 file_config，即 CLI > 配置文件）
  → get_staged_diff()（无 staged 即报错退出）
  → build_ai_prompt(&diff)（{{diff}} 占位符替换）
  → send_request(merged_config, prompt)
      ├─ LoadingSpinner 启动（"Request has been sent, waiting for response..."）
      ├─ ureq::post(endpoint) + Authorization: Bearer <token> + JSON body {model, messages:[{role:user, content:prompt}]}
      └─ 非 200/网络错误 → format_request_error 映射为英文消息后报错退出
  → parse_llm_api_response(body)
      ├─ 优先解析 OpenAI 兼容 envelope choices[0].message.content（其内容须是 JSON 字符串数组）
      └─ 回退直接把整个响应体解析为数组；任一步失败 → "Failed to parse llm api response"
  → select_commit_message(result)（inquire Select，支持过滤；Ctrl-C/Esc → 打印 "Commit aborted." 且不提交，退出码 0）
  → perform_commit(选中项)（Enter 选中即自动提交，无二次确认）
```

- **`AI_PROMPT_TEMPLATE`**：中文提示词模板（用户提供、原样嵌入 `r##` raw string），要求 LLM 返回 **JSON 字符串数组（3–6 条）**，每条为**全小写英文** Conventional Commits 消息、长度 < 100 字符、`<type>: <description>` 格式。修改提示词即修改程序行为。
- **错误映射**：ureq 各类错误（HTTP 状态码、DNS 解析失败、网络 IO、超时、TLS、无效 URL、连接失败、过多重定向等）映射为 `AI request failed: <原因>` 英文消息。

### 5.4 配置系统（`src/config.rs`）

| 函数/常量                | 说明                                                                                                                                 |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------ |
| `APIConfig`              | struct，三 `Option<String>` 字段：`api_endpoint` / `api_token` / `model_name`                                                        |
| `get_config_path`        | 返回 `$HOME/.config/git-cz/config.toml`（**只用 `HOME` 环境变量**，无 XDG 回退），目录不存在时自动创建                               |
| `DEFAULT_CONFIG_CONTENT` | 默认内容：endpoint=`https://api.deepseek.com/v1/chat/completions`、token=`sk-your-token-string`、model=`deepseek-v4-flash`           |
| `init_config`            | 文件已存在 → 提示不覆盖；不存在 → 写入默认内容                                                                                       |
| `load_config`            | 文件不存在 → 报错 `Config file not found at <path>`；解析失败 → `Failed to parse config file <path>: <错误>`；成功 → 缺失字段为 None |
| `merge_config`           | `config2` 的 Some 字段覆盖 `config1`（CLI > 文件）                                                                                   |

**优先级链**：CLI 参数 > 配置文件。**无环境变量参与**（旧版的 `GIT_CZ_AI_OPENAI_API_KEY` 已移除）。三个参数均缺时 `send_request` 报错 `Missing API configuration (api_endpoint / api_token / model_name)`（不发起网络请求）。

## 6. 数据存储与外部集成

- **无数据库、无缓存、无消息队列**。项目唯一的「数据」载体是 **Git 仓库本身**（经 git2 操作 `.git` 的索引/tree/commit）与**用户配置文件** `~/.config/git-cz/config.toml`。
- 作者签名数据来自 Git 仓库/全局配置的 `user.name` 与 `user.email`（经 `repo.config().get_string()` 读取；缺失时报错）。
- **第三方 API**：OpenAI 兼容 chat completions 接口（endpoint/token/model 均由用户配置，默认指向 DeepSeek；无内置密钥）。
- 配置文件中的 `api_token` 仅作为配置项读写，代码内不做任何密钥管理。

## 7. 测试策略

- **类型**：39 个单元测试（`cargo test` 全绿，2026-09-01 验证），全部内嵌在对应模块的 `#[cfg(test)] mod xxx_test` 中（ai 19 / config 8 / git 4 / manually 8），**无 tests/ 集成测试目录**。
- **框架**：Rust 标准 `#[test]`，无第三方测试框架、无覆盖率配置。
- **AI 相关测试**：提示词占位符替换、响应双层解析（envelope / 直接数组 / 非法 JSON / 空数组等）、ureq 错误映射、缺配置报错——均为纯函数测试，无网络 mock。
- **Git 相关测试**：用 `std::env::temp_dir()` + 原子计数器生成唯一临时目录创建真实仓库；`get_staged_diff_in` 接受路径参数以便测试；覆盖空仓库（unborn branch）staged/无 staged、有初始提交的仓库、`perform_commit` 空仓库首提（无父提交）。依赖进程 cwd（`.`）的 `perform_commit` 测试用 `CWD_LOCK` 互斥保护。
- **配置相关测试**：`merge_config` 各种覆盖组合、`init_config` 创建/不覆盖、`load_config` 错误消息（用 `HomeGuard` 临时改写 `HOME` 环境变量）。
- **交互式消息测试**：`build_commit_message` 各字段组合（含全空返回 `": "` 的边界断言）。

## 8. 构建与部署

- **构建**：`cargo build`（调试）/ `cargo build --release`；产物 `target/debug/git-cz`。`cargo test` 运行全部测试。`cargo clippy --all-targets` 当前零警告（2026-09-01 验证）。
- **安装**：README 提供 `cargo install git-cz-ai`。无 Dockerfile、无 Makefile、无发布脚本（除 CI）。
- **CI/CD**：`.github/workflows/release-plz.yml`——`main` 分支 push 时由 [release-plz](https://github.com/release-plz/release-plz) 自动：① `release` 命令发布 crate 到 crates.io（需要 `GH_TOKEN` 与 `CARGO_REGISTRY_TOKEN` secrets）；② `release-pr` 创建版本号/changelog 更新 PR。
- **版本管理**：CHANGELOG.md 遵循 Keep a Changelog；提交历史使用 Conventional Commits（`feat`/`fix`/`refactor`/`chore`/`docs` 等）。

## 9. 已知问题与注意事项

1. **README 与代码不一致（用户已确认以代码为准）**：
   - README 声称 body 支持"输入 `e` 打开外部编辑器（`$EDITOR`，默认 vim/notepad）"——**代码未实现**：`src/manually.rs` 的正文输入直接使用 inquire 的 `Editor` 组件（无"输入 e"步骤，也无 tempfile/EDITOR 逻辑）。
   - README 声称"共 32 个集成测试"——实际为 **39 个内嵌单元测试**。
   - README 声称依赖 `openssl vendored`——**当前不依赖 openssl**（ureq 3 使用 rustls）。
   - 建议后续同步更新 README。
2. **`build_commit_message` 无输入校验**：全空输入产生 `": "`（测试已断言，属已知设计）。
3. **配置路径无 XDG 支持**：`get_config_path` 仅使用 `HOME` 环境变量（`$HOME/.config/git-cz/config.toml`）；`HOME` 缺失时报错。
4. **无 API token 环境变量回退**：`--api-token` 只能来自 CLI 或配置文件（旧版 `GIT_CZ_AI_OPENAI_API_KEY` 已移除）。
5. **`send_request` 无超时/重试**：ureq 默认超时行为，网络异常直接报错退出（无重试）。
6. **交互式输入的取消行为**：`Select`/`Text`/`Confirm` 的 `prompt()` 返回 `Err` 时，各函数以 `unwrap_or_default()`/`String::new()` 静默降级（如取消类型选择会得到空类型名），无统一取消处理。
7. **`perform_commit` 不自动 `git add`**：用户必须先暂存；依赖 `user.name`/`user.email` 已配置。
8. **Git 仓库状态**：当前检出分支 `dev`；存在 `main` 分支与远程 `origin`（含多个 `release-plz-*` 临时分支）。历史提交含 `feat`/`fix`/`refactor`/`chore`/`docs` 等类型。
9. **AI 模式无额外确认**：Enter 选中候选即提交（README 有说明）；取消只能靠 Ctrl-C/Esc。

## 10. 待确认/待补充事项

- [ ] README.md 需同步修正：编辑器模式描述、测试数量（32 → 39）、openssl 依赖描述（用户已确认 AGENTS.md 以代码为准，README 修复另议）。
- [ ] `select_commit_message` 之外的其他交互输入（类型/scope/描述等）在用户 Ctrl-C 取消时的降级行为是否符合预期？
- [ ] 是否计划补充集成测试（`tests/` 目录）或将 `perform_commit`/`has_staged_changes` 也参数化为可测路径版本？
- [ ] 是否计划恢复环境变量 token 回退或 XDG 配置路径支持？
- [ ] 是否计划为 `send_request` 添加超时/重试机制？

---

## 11. 变更说明

- **2026-09-01（本次重写）**：旧版 AGENTS.md 基于 v0.0.1 代码，已完全过时（描述的是 `src/lib.rs` 大函数结构、`tempfile` + `EDITOR` 编辑器模式、`tests/main_test.rs` 32 个集成测试、openssl vendored、`GIT_CZ_AI_OPENAI_API_KEY` env 回退等，均与当前 v0.0.6 代码不符）。本次按 init-agent-md 技能全面重写为基于 v0.0.6 代码的版本，并附变更说明。生成后建议人工复核（尤其 §5 业务流程细节与 §9 注意事项）。
