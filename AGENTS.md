# AGENTS.md — git-cz-ai 项目知识总结

> 本文档由 AI 通读项目源码后生成，所有描述均基于代码实际内容。
> 对无法从代码确定的部分，已明确标注「待确认」或「根据代码推断」。

---

## 1. 项目概述

- **项目名称**：`git-cz-ai`
- **版本**：`0.0.1`（见 `Cargo.toml` 的 `version` 字段）
- **一句话描述**：一个用 Rust 编写的交互式 Git 提交信息生成器（Commitizen 风格 CLI 工具）。
- **功能定位**：通过交互式终端引导用户逐步填写提交信息（类型、范围、描述、正文、页脚），最终生成符合 Conventional Commits 规范的提交消息并直接执行 `git commit`。
- **来源**：fork 自 [k3ii/git-cz](https://github.com/k3ii/git-cz)（见 `README.md`）。
- **目标用户/应用场景**：需要在终端中规范提交信息的开发者；可作为 `git commit` 的交互式替代工具。
- **关于名称中的 "ai"**：当前代码中**未发现任何 AI/LLM 相关逻辑**（无 API 调用、无模型集成）。「根据代码推断」名称中的 "ai" 是项目愿景或品牌命名，而非已实现的功能；若实际存在 AI 能力，属于「待确认」。

---

## 2. 技术栈与依赖

| 类别 | 名称 | 版本 | 用途 |
|------|------|------|------|
| 语言 | Rust | edition 2021（本机工具链 1.97.1） | — |
| Git 绑定 | `git2` | 0.19.0 | 仓库操作（状态检查、索引、提交） |
| 底层 Git | `libgit2-sys` | 0.17.0+1.8.1（内置 libgit2 1.8.1） | git2 的 C 绑定（传递依赖） |
| 交互式 UI | `promkit` | 0.4.5 | 终端交互组件（选择器、输入框、确认框） |
| 终端后端 | `crossterm` | 0.27.0 | promkit 的终端渲染后端（传递依赖） |
| OpenSSL | `openssl` | 0.10.66（`vendored` 特性） | git2 的 HTTPS/加密依赖，静态编译免系统依赖 |
| 临时文件 | `tempfile` | `Cargo.toml` 声明 3.2，lock 解析为 3.12.0 | 编辑器模式下的临时提交信息文件 |
| CLI 解析 | `clap` | 4.5.60（`derive` + `env` 特性） | `ai` 子命令、参数解析、token 环境变量回退 |
| JSON | `serde_json` | 1.0.127 | LLM 请求/响应编解码 |
| HTTP 客户端 | `ureq` | 2.12.1（`json` 特性，默认 rustls TLS） | 向 LLM API 发送 chat completions 请求 |

> 版本号来源：`Cargo.lock`（`tempfile` 的 `Cargo.toml` 写的是 `"3.2"`，属语义化版本区间 `>=3.2, <4.0`，实际锁定为 `3.12.0`）。

### 构建产物（bin）

`Cargo.toml` 中的 `[[bin]]` 段将可执行文件命名为 **`git-cz`**（与包名 `git-cz-ai` 不同），入口为 `src/main.rs`。

---

## 3. 架构设计

整体为**单二进制 CLI 应用**，采用 **库（lib）+ 二进制（bin）分离** 的 Rust 标准分层结构：

```
src/main.rs  (可执行入口：clap 子命令解析 + 交互/AI 流程编排)
    │ 调用
    ▼
src/lib.rs   (库：纯业务逻辑，可测试)
    ├── build_commit_types()      —— 提交类型定义
    ├── format_commit_types()     —— 类型列表格式化
    ├── build_commit_message()    —— 提交消息拼接
    └── perform_commit()          —— 执行 git 提交（git2）
    └── ai.rs（子模块，经 pub mod ai 挂载）
        ├── build_ai_prompt()     —— 提示词模板 {{diff}} 替换
        ├── parse_llm_response()  —— LLM 响应双层解析
        └── get_staged_diff()     —— git diff --cached 取暂存变更
```

- **`src/lib.rs`（库层）**：无任何终端交互依赖，全部为纯函数/Git 操作，是单元测试的直接对象。
- **`src/ai.rs`（AI 子命令纯逻辑层）**：提示词模板、LLM 响应解析、staged diff 获取，均为纯函数（`get_staged_diff` 仅调用外部 `git` 命令），可单测。
- **`src/main.rs`（应用层）**：clap 解析子命令（`ai` / 无子命令）；`run_ai` 编排「取 diff → 构建提示词 → ureq 请求 → 解析 → promkit 选择 → 提交」，`run_interactive` 负责交互式 UI（promkit 组件）、环境变量读取、外部编辑器调用，并串联库层函数完成端到端流程。

模块职责单一、交互清晰：**交互 → 收集参数 → 库层生成消息 → 库层提交**。

---

## 4. 目录结构

```
git-cz-ai/
├── Cargo.toml          # 包元数据与依赖声明（bin 名 git-cz）
├── Cargo.lock          # 依赖锁定文件（已提交）
├── README.md           # 极简说明：仅注明 fork 来源，无使用文档
├── LICENSE             # MIT 协议，Copyright (c) 2026 secret
├── .gitignore          # 仅忽略 /target
├── src/
│   ├── main.rs         # 可执行入口：clap 子命令解析 + 交互/AI 流程编排
│   ├── ai.rs           # AI 子命令纯逻辑层：提示词模板、LLM 响应解析、staged diff 获取
│   └── lib.rs          # 库：commit 类型、消息构建、git 提交执行（pub mod ai 挂载 ai.rs）
├── tests/
│   └── main_test.rs    # 集成测试（23 个测试函数）
└── target/             # 构建产物目录（git 忽略）
```

> 项目未配置 CI（无 `.github/workflows`）、无 `.env`、无任何配置文件——所有行为均在代码内硬编码。

---

## 5. 核心业务逻辑

### 5.1 库层 API（`src/lib.rs`）

| 函数 | 签名 | 说明 |
|------|------|------|
| `build_commit_types` | `() -> Vec<(&'static str, &'static str)>` | 返回 11 种提交类型 `(类型名, 英文描述)`：`feat`、`fix`、`docs`、`style`、`refactor`、`perf`、`test`、`chore`、`ci`、`build`、`revert` |
| `format_commit_types` | `(Vec<(&str, &str)>) -> Vec<String>` | 按**最长类型名 + 4** 的宽度左对齐格式化，如 `"feat     - A new feature"`；空列表返回空 Vec |
| `build_commit_message` | `(commit_type, scope, description, body, footer) -> String` | 拼接 Conventional Commits 消息 |
| `ensure_staged_changes` | `(repo: &Repository) -> Result<(), Box<dyn Error>>` | 严格语义预检：HEAD tree 与 index 的 diff 非空即有 staged changes；无 HEAD（空仓库）以空树为基线；无 staged 时报错 `Your git repository is clean` |
| `perform_commit` | `(repo_path: &Path, full_commit_message: &str) -> Result<(), Box<dyn Error>>` | 对指定仓库路径执行 Git 提交（先经 `ensure_staged_changes` 校验） |

### 5.2 提交消息格式（`build_commit_message`）

```
{type}({scope}): {description}      ← scope 为空时省略括号

{body}                              ← body 非空时追加（空行分隔）

{footer}                            ← footer 非空时追加（空行分隔）
```

- **注意**：该函数**不校验输入**。当所有参数均为空字符串时，返回 `": "`（冒号+空格，来自 `format!` 的固定模板），测试用例 `test_build_commit_message_edge_cases` 已断言此行为。

### 5.3 Git 提交流程（`perform_commit`）

1. `Repository::open(repo_path)` 打开仓库；
2. `ensure_staged_changes(&repo)?` 做**严格语义预检**：对比 HEAD tree 与 index 的 diff，**无任何 staged changes 则报错** `"Your git repository is clean"`（返回 `Err`，不 panic）。untracked 文件与未 `git add` 的工作区修改不计入；无 HEAD（空仓库）时以空树为基线；
3. `index.write_tree()` 将**索引（index/staging 区）** 内容写入 tree；
4. 读取 git 配置中的 `user.name` / `user.email` 构造签名（`Signature::now`）；
5. 通过 `repo.head()` 找到 HEAD 指向的父提交；
6. `repo.commit(Some("HEAD"), ...)` 创建新提交并更新 HEAD。

> **关键行为**：该函数只提交**已暂存（staged）的内容**——它不会自动执行 `git add`；无 staged 的提交会被步骤 2 的严格预检拦截（详见 §10 条目 1）。

### 5.4 交互流程（`src/main.rs`）

完整流程（Mermaid 图）：

```mermaid
flowchart TD
    A[QuerySelector 选择提交类型<br/>11 种类型，listbox 10 行，支持文本过滤] --> B[Readline 输入 scope<br/>带建议词: app/core/ui/db/api/frontend/backend/config/build/sec/infra/deps]
    B --> C[Readline 输入 description]
    C --> D[Readline 输入 body]
    D --> E{body 输入为 'e'?<br/>不区分大小写}
    E -- 是 --> F[创建临时文件<br/>调用 EDITOR 环境变量指定的编辑器<br/>Windows 默认 notepad / 其他默认 vim]
    F --> G[读取临时文件内容作为 body]
    E -- 否 --> H{Confirm 是否添加 footer?}
    G --> H
    H -- 是 --> I[QuerySelector 选择 footer 类型: fix / close]
    I --> J[Readline 输入 issue 号<br/>校验必须为 i32 整数]
    J --> K[生成 footer: '类型: #编号']
    H -- 否 --> L[footer 为空字符串]
    K --> M[build_commit_message 生成完整消息]
    L --> M
    M --> N{Confirm 是否提交?}
    N -- 是 --> O[perform_commit('.') 执行提交]
    O --> P[打印 'Commit successful!']
    N -- 否 --> Q[打印 'Commit aborted.']
```

流程要点：

1. **选择提交类型**：`QuerySelector` 带过滤闭包（大小写敏感的 `contains` 匹配），`listbox_lines(10)` 控制展示行数；选中项形如 `"feat     - A new feature"`，通过 `split_whitespace().next()` 取第一个 token 作为类型名。
2. **scope 输入**：`Readline` + `Suggest` 提供 12 个常用建议词（app/core/ui/db/api/frontend/backend/config/build/sec/infra/deps），可直接输入任意值或留空。
3. **body 输入**：输入 `e`（忽略大小写）时进入编辑器模式——创建 `NamedTempFile`，用 `EDITOR` 环境变量指定的编辑器打开（Windows 无 `EDITOR` 时回退 `notepad`，其余平台回退 `vim`）；**编辑器以非零状态退出仅打印警告，不中断流程**。
4. **footer（页脚）**：可选。类型限定为 `fix` 或 `close`，issue 号必须通过 `parse::<i32>()` 校验；格式为 `"{类型}: #{编号}"`（如 `fix: #123`）。
5. **最终确认**：`Confirm` 询问是否提交，回答 `y`（不区分大小写）则在当前目录（`Path::new(".")`）执行 `perform_commit`。

> 提示文案均为英文硬编码在源码中，无国际化。

### 5.5 AI 子命令（`git-cz ai`）

**入口**：`src/main.rs` 的 `run_ai`（bin 层编排，无自动化测试）；**纯逻辑**：`src/ai.rs`（可单测）。

| 函数/常量 | 签名 | 说明 |
|------|------|------|
| `AI_PROMPT_TEMPLATE` | `&'static str` | 用户提供的 markdown 提示词原样嵌入（raw string `r##` 包裹），含 `{{diff}}` 占位符与 `## 返回示例` 代码块；禁止改写原文 |
| `build_ai_prompt` | `(diff: &str) -> String` | `AI_PROMPT_TEMPLATE.replace("{{diff}}", diff)` 替换占位符 |
| `parse_llm_response` | `(body: &str) -> Result<Vec<String>, Box<dyn Error>>` | 双层解析：优先取 OpenAI 兼容 envelope `choices[0].message.content` 再解析为 `Vec<String>`；回退直接把响应体解析为数组；任一步失败返回统一错误 `llm api response is not a json string` |
| `get_staged_diff` | `(repo_path: &Path) -> Result<String, Box<dyn Error>>` | 执行 `git diff --cached`；stdout 为空时报错 `No staged changes. Please 'git add' your files first.` |

**数据流**（`run_ai`）：

```
git-cz ai --api-endpoint=<url> [--api-token=<token>] --api-model=<model>
  → get_staged_diff(".")        # git diff --cached，无 staged 即退出（非零）
  → build_ai_prompt(diff)        # {{diff}} 替换
  → ureq POST <endpoint>         # Authorization: Bearer <token>，body: {model, messages:[{role:user, content:prompt}]}
  → parse_llm_response(body)     # 失败 → 打印 "llm api response is not a json string"，exit(1)
  → QuerySelector 展示候选      # listbox 10 行，Enter 选中 / Ctrl-C 退出
  → perform_commit(".", 选中项) # Enter 后自动提交，无额外确认
```

**交互细节**：promkit 0.4.5 的 `QuerySelector::run()` 在 Ctrl-C 时返回 `Err`（消息含 `ctrl+c`），`run_ai` 匹配后打印 `Commit aborted.` 并退出码 0（不提交）；Enter 返回选中项直接 `perform_commit`。

> 注意：`src/ai.rs` 中的提示词模板由用户提供、原样嵌入代码，修改提示词即修改程序行为（含对 LLM 输出格式的约束）。

---

## 6. 数据模型与存储

- **无数据库、无缓存、无配置文件**。
- 项目唯一的「数据」载体是 **Git 仓库本身**：通过 `git2` 操作 `.git` 目录（索引、tree、commit 对象）。
- 作者签名数据来源为 Git 全局/仓库级配置 `user.name` 与 `user.email`（经 `repo.config().get_string()` 读取）。

---

## 7. 外部依赖与集成

| 集成点 | 方式 | 说明 |
|--------|------|------|
| Git 仓库 | `git2` 库（Rust 绑定 libgit2） | 状态检查、索引写入、commit 创建、HEAD 更新 |
| 系统编辑器 | `std::process::Command` 调用 `EDITOR` 环境变量 | 仅用于编辑 body；Windows 回退 notepad，其他平台回退 vim |
| 终端交互 | `promkit`（内部依赖 `crossterm`） | 选择器、输入框、确认框、建议词 |
| 系统 OpenSSL | `openssl` 0.10（`vendored` 特性） | 静态编译，构建时无需系统预装 OpenSSL |
| 临时文件 | `tempfile::NamedTempFile` | 编辑器模式下暂存提交正文 |
| LLM API | `ureq::post` → OpenAI 兼容 chat completions | `ai` 子命令：`Authorization: Bearer <token>` + JSON body；非 200/网络错误打印后退出 |

---

## 8. 配置与环境

### 8.1 环境变量

| 变量 | 用途 | 默认值 |
|------|------|--------|
| `EDITOR` | body 编辑器模式（输入 `e` 时）调用的编辑器命令 | Windows：`notepad`；其他平台：`vim` |
| `GIT_CZ_AI_OPENAI_API_KEY` | `ai` 子命令 `--api-token` 的回退来源（clap `env` 特性自动读取）；两者皆缺时 clap 报错退出 | 无 |

### 8.2 构建与运行命令

```bash
cargo build            # 调试构建（产物：target/debug/git-cz）
cargo build --release  # 发布构建（产物：target/release/git-cz）
cargo run              # 运行交互式 CLI
cargo test             # 运行测试（23 个测试函数全部通过）
cargo check            # 仅编译检查（当前可通过，2026-07 验证）
```

### 8.3 其他

- `openssl` 采用 `vendored` 特性，构建无需系统 OpenSSL 头文件，但需 `cc` 编译器（Cargo.lock 中存在 `cc`、`libc`、`shlex` 等编译链依赖）。
- 无发布/部署流程配置（无 CI、无 Dockerfile、无安装脚本）。

---

## 9. 测试策略

- **类型**：集成测试（`tests/main_test.rs`），共 **23 个测试函数**，使用 `tempfile::tempdir()` 创建临时 Git 仓库进行真实提交验证。
- **框架**：Rust 标准测试（`#[test]`），无第三方测试框架、无覆盖率工具配置。
- **AI 相关测试**（6 个）：均为纯函数测试，无需 mock 网络；`test_get_staged_diff` 依赖系统 `git` 命令。

| 测试函数 | 覆盖点 |
|----------|--------|
| `test_build_commit_types` | 类型列表非空且包含 `feat`/`fix` |
| `test_format_commit_types` | 对齐格式化输出（含精确字符串断言） |
| `test_format_commit_types_empty_list` | 空列表返回空 Vec |
| `test_format_commit_types_varying_lengths` | 不同长度类型名的对齐 |
| `test_build_commit_message` | 有/无 scope、无 body 的消息拼接 |
| `test_build_commit_message_edge_cases` | 全空输入返回 `": "`、超长字符串、特殊字符 |
| `test_perform_commit` | 临时仓库中真实提交并验证 HEAD 消息 |
| `test_perform_commit_multiple_files` | 多文件提交、tree 内容校验 |
| `test_perform_commit_no_changes` | 无变更时报错及错误消息断言 |
| `test_full_workflow` | 端到端：建仓 → 暂存 → 提交 → 校验 |
| `test_perform_commit_invalid_path` | 无效路径 `should_panic` |
| `test_ensure_staged_changes_with_staged` | 有 staged 文件（已 `git add`）时校验通过 |
| `test_ensure_staged_changes_clean` | 无任何变更时报错 `Your git repository is clean` |
| `test_ensure_staged_changes_untracked_only` | 仅 untracked 文件（未 add）不计入 staged，报错 |
| `test_ensure_staged_changes_workdir_modification_only` | 仅工作区修改（未 add）不计入 staged，报错 |
| `test_ensure_staged_changes_empty_repo_clean` | 空仓库无 staged 时报错 |
| `test_ensure_staged_changes_empty_repo_staged` | 空仓库已 add 文件时校验通过（以空树为基线） |
| `test_build_ai_prompt_placeholder` | 占位符 `{{diff}}` 被 diff 内容替换，模板头部/正文保留 |
| `test_parse_llm_response_direct_array` | 响应体直接是字符串数组 → `Ok(Vec)` |
| `test_parse_llm_response_openai_envelope` | OpenAI envelope 的 content 解析为数组 |
| `test_parse_llm_response_invalid_json` | 非法 JSON → 错误 `llm api response is not a json string` |
| `test_parse_llm_response_content_not_array` | envelope 存在但 content 非数组 → 同错误 |
| `test_get_staged_diff` | 临时仓库 staged 文件后返回含文件名的 diff |

- 辅助函数 `init_repo_with_initial_commit(path: &Path) -> Repository`：创建临时仓库并预置空 initial commit，供测试复用。

---

## 10. 已知问题与注意事项

1. **`perform_commit` 不执行暂存（staging）——已解决**：旧版直接对 index 写 tree 并提交，无 staged 时存在产生空提交/错误内容提交的隐患；现已通过 `ensure_staged_changes` 的严格语义检查在提交前拦截（无任何 staged changes 时报错 `Your git repository is clean`，不会产生空提交）。注意：该函数仍不会自动执行 `git add`，用户需先自行暂存。
2. **`build_commit_message` 无输入校验**：全空输入产生 `": "` 这种无意义消息（测试已断言该行为，属已知设计）。
3. **空仓库场景**：`perform_commit` 依赖 `repo.head()` 获取父提交；在无任何提交的仓库中会出错（测试通过预置 initial commit 规避）。
4. **README 信息缺失**：仅一行 fork 来源说明，无安装、使用、配置文档。
5. **「AI」名不副实——已实现基础 AI 子命令**：旧版代码无任何 AI 能力；现已新增 `git-cz ai` 子命令（取 staged diff → LLM 生成候选 → 选择 → 提交），依赖用户提供的 API endpoint/token/model，未内置任何模型。
6. **重复声明**：`Cargo.toml` 中 `tempfile` 同时在 `[dependencies]` 与 `[dev-dependencies]` 声明，冗余但不影响构建。
7. **编辑器模式边界**：body 输入 `e` 时若 `EDITOR` 指向不存在的命令，`Command::status()` 报错会中断流程（无友好提示）；编辑器非零退出仅告警。
8. **过滤闭包大小写敏感**：`QuerySelector` 过滤使用 `contains`，输入大写与小写不互通（如输入 `FEAT` 匹配不到 `feat`）。
9. **Git 仓库状态**：当前检出分支为 `dev`，另存在 `main` 分支（仍停留在初始提交）；`dev` 历史含 14 个提交（含 AI 子命令的 4 个 `feat` 提交与 1 个 `docs` 提交）；未配置远程（`git branch -a` 无 `remotes/` 条目）。
10. **构建前置条件**：`openssl vendored` 需要 C 编译器（`cc`）；跨平台构建在 Windows 上依赖 `winapi` 相关 crate（见 lock 中 `crossterm_winapi` 等）。

---

## 11. 附录：快速参考

- 库层 API 均在 `src/lib.rs`，无 `pub use` 重导出，测试/二进制通过 `git_cz_ai::` 路径引用（crate 名 `git-cz-ai` 的连字符映射为下划线）。
- 提交类型全集（11 种）：`feat` `fix` `docs` `style` `refactor` `perf` `test` `chore` `ci` `build` `revert`。
- scope 建议词（12 个）：`app` `core` `ui` `db` `api` `frontend` `backend` `config` `build` `sec` `infra` `deps`。
- footer 类型（2 种）：`fix`、`close`，格式 `类型: #编号`。
- `git-cz ai` 子命令参数：`--api-endpoint <URL>`（必填）、`--api-token <TOKEN>`（必填，可经环境变量 `GIT_CZ_AI_OPENAI_API_KEY` 回退）、`--api-model <MODEL>`（必填）。调用示例：`git-cz ai --api-endpoint=https://api.openai.com/v1/chat/completions --api-token=sk-xxx --api-model=gpt-5-mini`。
