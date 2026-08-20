# `model_name` → `model_name` 字段重命名实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 将 `AiArgs` struct 的 `model_name` 字段改名为 `model_name`，CLI 参数同步改为 `--model-name`，并更新全部文档引用（README、AGENTS.md、历史 spec/plan）。

**架构：** 纯重命名，无行为逻辑变化。`src/main.rs` 改字段名与使用点（`#[arg(long)]` 保持，clap 自动生成 `--model-name`）；6 个文档文件把 `model_name`/`--model-name` 全部替换为 `model_name`/`--model-name`。

**技术栈：** Rust + clap 4（derive）。

**设计规格：** `docs/superpowers/specs/2026-08-20-rename-model-name-design.md`

---

## 文件结构

| 文件 | 动作 | 职责 |
|------|------|------|
| `src/main.rs` | 修改（38、62 行） | `model_name: String` → `model_name: String`；`args.model_name` → `args.model_name` |
| `README.md` | 修改（54、61、75、79 行） | `--model-name` → `--model-name` |
| `AGENTS.md` | 修改（177、296 行） | `--model-name` → `--model-name` |
| `docs/superpowers/specs/2026-08-20-ai-subcommand-design.md` | 修改（11、16、68、78、100、149 行） | 字段与参数引用 |
| `docs/superpowers/plans/2026-08-20-ai-subcommand.md` | 修改（370、393、623、626、630、634 行） | 字段、使用点、参数引用 |
| `docs/superpowers/specs/2026-08-20-waiting-for-response-design.md` | 修改（34 行） | `args.model_name` → `args.model_name` |
| `docs/superpowers/plans/2026-08-20-waiting-for-response.md` | 修改（41、135、152、163 行） | `args.model_name`、命令示例 |
| `docs/superpowers/specs/2026-08-20-rename-model-name-design.md` | 修改 | 全文档 `model_name` → `model_name`、`--model-name` → `--model-name`（含标题；用户裁定全扫，见任务 2 步骤 7） |
| `docs/superpowers/plans/2026-08-20-rename-model-name.md` | 修改 | 全文档同上（文件名本身含 `model-name` 属路径，不改） |

> 所有替换均为机械性文本替换：`model_name` → `model_name`（代码标识符）、`--model-name` → `--model-name`（CLI 参数）。**不要**动 `--api-endpoint`、`--api-token`、`GIT_CZ_AI_OPENAI_API_KEY`、JSON 请求体键 `"model"`。

---

### 任务 1：代码字段改名 + 编译测试

**文件：**
- 修改：`src/main.rs`（38、62 行）

- [ ] **步骤 1：修改字段声明**

`src/main.rs` 第 38 行：

```rust
    /// 模型名称，如 gpt-5-mini
    #[arg(long)]
    model_name: String,
```

（原 `model_name: String,`；`#[arg(long)]` 保持不动——clap 自动将 `model_name` 转为 `--model-name`。）

- [ ] **步骤 2：修改使用点**

`src/main.rs` 第 62 行：

```rust
            "model": args.model_name,
```

（原 `"model": args.model_name,`；JSON 键 `"model"` 保持不变。）

- [ ] **步骤 3：编译检查**

运行：`cargo build`
预期：编译通过，无警告

- [ ] **步骤 4：运行全部测试**

运行：`cargo test`
预期：23 passed；0 failed（测试不涉及 CLI 字段名，无回归）

- [ ] **步骤 5：验证 CLI 参数名**

运行：`target/debug/git-cz ai --help`
预期：帮助输出含 `--model-name <MODEL_NAME>`；不再出现旧参数名

- [ ] **步骤 6：Commit**

```bash
git add src/main.rs
git commit -m "refactor: rename model_name field to model_name"
```

---

### 任务 2：更新全部文档引用

**文件：**
- 修改：`README.md`、`AGENTS.md`、`docs/superpowers/specs/2026-08-20-ai-subcommand-design.md`、`docs/superpowers/plans/2026-08-20-ai-subcommand.md`、`docs/superpowers/specs/2026-08-20-waiting-for-response-design.md`、`docs/superpowers/plans/2026-08-20-waiting-for-response.md`

- [ ] **步骤 1：更新 `README.md`**

逐处替换（4 处，行号仅供参考）：

| 行 | 原文 | 改为 |
|----|------|------|
| 54 | `--model-name=<MODEL>` | `--model-name=<MODEL>` |
| 61 | `\| `--model-name` \| ✅ \| 模型名称，如 `gpt-5-mini` \|` | `\| `--model-name` \| ✅ \| 模型名称，如 `gpt-5-mini` \|` |
| 75 | `--model-name=gpt-5-mini` | `--model-name=gpt-5-mini` |
| 79 | `--model-name=gpt-5-mini` | `--model-name=gpt-5-mini` |

- [ ] **步骤 2：更新 `AGENTS.md`**

逐处替换（2 处）：

| 行 | 原文 | 改为 |
|----|------|------|
| 177 | `--model-name=<model>` | `--model-name=<model>` |
| 296 | `--model-name <MODEL>` 及 `--model-name=gpt-5-mini` | `--model-name <MODEL>` 及 `--model-name=gpt-5-mini` |

- [ ] **步骤 3：更新 `docs/superpowers/specs/2026-08-20-ai-subcommand-design.md`**

替换（6 处）：第 11、16、78、100、149 行的 `--model-name` → `--model-name`；第 68 行 `model_name: String,` → `model_name: String,`。

- [ ] **步骤 4：更新 `docs/superpowers/plans/2026-08-20-ai-subcommand.md`**

替换（6 处）：第 370 行 `model_name: String,` → `model_name: String,`；第 393 行 `args.model_name` → `args.model_name`；第 623 行错误预期文本、626、630、634 行命令中的 `--model-name` → `--model-name`。

- [ ] **步骤 5：更新 `docs/superpowers/specs/2026-08-20-waiting-for-response-design.md`**

第 34 行：`"model": args.model_name,` → `"model": args.model_name,`。

- [ ] **步骤 6：更新 `docs/superpowers/plans/2026-08-20-waiting-for-response.md`**

第 41 行：`"model": args.model_name,` → `"model": args.model_name,`；第 135、152、163 行命令中 `--model-name=gpt-test` → `--model-name=gpt-test`。

- [ ] **步骤 7：更新本计划的两个文档（用户裁定：全扫）**

用户裁定残留扫描按计划原样全扫（不排除本计划文档）。因此 `docs/superpowers/specs/2026-08-20-rename-model-name-design.md` 与 `docs/superpowers/plans/2026-08-20-rename-model-name.md` 中所有 `model_name` → `model_name`、`--model-name` → `--model-name` 一并替换，包括标题、说明文字、文件结构表对照列；两个文件名本身含 `model-name` 属路径，不改。

- [ ] **步骤 8：残留扫描**

运行：`grep -rn "model_name\|model-name" README.md AGENTS.md docs/ src/`
预期：**命中多处**（新名引用已全面就位；`target/` 不在扫描范围内）

- [ ] **步骤 9：Commit**

```bash
git add README.md AGENTS.md docs/
git commit -m "docs: rename --model-name to --model-name in all references"
```

---

## 自检记录

- **规格覆盖度**：代码字段（规格 §改动清单）→ 任务 1；CLI 参数验证 → 任务 1 步骤 5；文档 6 文件 → 任务 2 步骤 1-6；残留扫描 → 任务 2 步骤 7。✅
- **占位符扫描**：所有步骤含具体代码/命令/预期，无「TODO」「待定」。✅
- **类型一致性**：`args.model_name` 在所有文件中一致；`"model"` JSON 键未动。✅
- **注意**：`grep -rn "model_name\|model-name"` 会命中 `target/` 下的构建产物——任务 2 步骤 7 已限定扫描范围为 `README.md AGENTS.md docs/ src/`。✅
