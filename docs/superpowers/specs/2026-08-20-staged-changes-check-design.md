# 设计：无 staged changes 时启动即退出

- **日期**：2026-08-20
- **状态**：已获用户批准（2026-08-20）
- **涉及文件**：`src/lib.rs`、`src/main.rs`、`tests/main_test.rs`

## 背景与目标

当前 `git-cz` 的提交流程存在两个问题：

1. **时机问题**：`perform_commit` 只在用户走完所有交互、最终提交时才检查仓库状态；如果仓库本来就没有可提交的内容，用户白白做了一轮无意义的输入。
2. **语义不严谨**：`perform_commit` 现有检查使用 `repo.statuses(None)`，只要工作区有任何改动（**包括未 `git add` 的修改**）就不会报错；而实际提交内容只来自 index（暂存区）。这存在"用户忘了 add 却提交了一个空/过期 tree"的隐患。

**目标**：程序启动时（任何交互之前）按严格 Git 语义检查暂存区；没有任何 staged changes 时直接退出，输出 `Your git repository is clean`，退出码 1。

## 需求要点（已与用户确认）

1. **检查时机**：程序启动时、交互开始之前。
2. **判定语义**：严格 Git 语义——只有 index 与 HEAD 的差异才算 staged changes；untracked 文件不算。
3. **错误输出**：`eprintln!` 到 stderr（不带 `Error:` 前缀），`std::process::exit(1)`。
4. **第二道防线**：同步修正 `perform_commit` 内的检查为严格语义，错误消息统一为 `Your git repository is clean`。

## 设计

### 组件与接口

- `src/lib.rs` 新增：

  ```rust
  pub fn ensure_staged_changes(repo: &Repository) -> Result<(), Box<dyn Error>>
  ```

  接受 `Repository` 引用（而非路径），避免重复 open，接口内聚、可直接被集成测试调用。

- `src/main.rs`：`main()` 开头、任何交互之前：

  ```rust
  let repo = Repository::open(".")?;
  if let Err(e) = ensure_staged_changes(&repo) {
      eprintln!("{}", e);
      std::process::exit(1);
  }
  ```

- `src/lib.rs` 的 `perform_commit`：开头改为

  ```rust
  let repo = Repository::open(repo_path)?;
  ensure_staged_changes(&repo)?;
  ```

  替换现有 `statuses(None)` 检查块（第 50–53 行附近）。

### 严格语义判定

1. 基线 tree：
   - 有 HEAD：`repo.head()` → 找到提交 → 取其 `tree()`。
   - 无 HEAD（空仓库）：用空 tree 作基线（`repo.treebuilder(None)?.write()?` 后 `find_tree`）。
2. 判定：`repo.diff_tree_to_index(Some(&base_tree), Some(&index), None)`；`diff.deltas().len() > 0` 即存在 staged changes。
3. 该实现天然满足严格语义：只比较 HEAD vs index；untracked（不在 index）与未 add 的工作区修改（不在 index）均被忽略。
4. 空仓库边界：index 相对空基线的任何条目都算 staged（`git init` 后 add 过的文件算数），与 `git diff --cached` 的直觉一致。

### 错误处理

- 无 staged changes → `Err("Your git repository is clean".into())`（消息单点定义在 `ensure_staged_changes` 内）。
- `main.rs`：捕获后 `eprintln!` + `exit(1)`。
- `perform_commit`：`?` 传播，调用方决定输出形式。

## 测试策略（tests/main_test.rs）

1. **修复现有 7 处 E0061**：为 `build_commit_message` 的 4 参数调用补第 5 个参数 `""`（第 76、82、88、95、104、110、250 行）。footer 为空不追加内容，**所有现有断言保持不变**（含 edge case 的 `": "` 断言）。
2. **更新 1 处断言**：`test_perform_commit_no_changes` 期望消息改为 `"Your git repository is clean"`。
3. **新增测试**（均基于 `tempfile::tempdir()` + `Repository::init`，必要时预置 initial commit）：
   - 有 staged（add 文件）→ `Ok`
   - 无 staged（index 与 HEAD 一致）→ `Err("Your git repository is clean")`
   - 仅 untracked 文件（未 add）→ `Err`
   - 仅未 add 的修改 → `Err`
   - 空仓库无内容 → `Err`
   - 空仓库 + add 文件 → `Ok`
4. 已知限制：`main.rs` 的启动分支是 bin 入口，无法被集成测试覆盖，靠手动运行验证。

## 明确不做（YAGNI）

- 不修改错误文案（严格按 `Your git repository is clean`）。
- 不自动 `git add`、不提示用户如何暂存。
- 不引入新依赖。
- 不重构 `perform_commit` 的其余部分（签名、提交逻辑保持不变）。

## 验收标准

- `cargo check` 通过（bin + tests）。
- `cargo test` 全部通过（修复后的 11 个既有测试 + 新增测试）。
- 手动验证：在无 staged changes 的仓库运行 `cargo run`，立即输出 `Your git repository is clean` 并以非零码退出，无任何交互提示；在有 staged changes 的仓库正常运行交互流程。
