# 无 staged changes 时启动即退出——实现计划

> **面向 AI 代理的工作者：** 必需子技能：使用 superpowers:subagent-driven-development（推荐）或 superpowers:executing-plans 逐任务实现此计划。步骤使用复选框（`- [ ]`）语法来跟踪进度。

**目标：** 程序启动时（交互之前）按严格 Git 语义检查暂存区；无任何 staged changes 时输出 `Your git repository is clean` 并以退出码 1 退出；同时将 `perform_commit` 内的检查同步修正为严格语义。

**架构：** 在库层 `src/lib.rs` 新增共享检查函数 `ensure_staged_changes(repo: &Repository) -> Result<(), Box<dyn Error>>`（HEAD tree vs index 的 diff 非空即有 staged changes），`main.rs` 启动时调用（`eprintln!` + `exit(1)`），`perform_commit` 开头复用同一函数作为第二道防线。判定严格语义：只比较 HEAD vs index，untracked 与未 add 的工作区修改均被忽略；无 HEAD（空仓库）时以 `None` 基线（等价空树）比较。

**技术栈：** Rust 2021、git2 0.19.0、promkit 0.4.5、tempfile 3.x（测试）。

**规格文档：** `docs/superpowers/specs/2026-08-20-staged-changes-check-design.md`（已获批准）

**已验证的技术事实（探针实验 2026-08-20）：**
- 空仓库（无提交）时 `repo.head()` 返回 `Err(git2::ErrorCode::UnbornBranch)`，class = Reference，消息 `reference 'refs/heads/main' not found`。
- `repo.diff_tree_to_index(None, Some(&index), None)` 合法：`old_tree` 传 `None` 等价于空树；空仓库 deltas=0，add 一个文件后 deltas=1。

---

## 文件结构

| 文件 | 职责 | 变更类型 |
|------|------|----------|
| `src/lib.rs` | 新增 `ensure_staged_changes`；`perform_commit` 开头替换现有 `statuses(None)` 检查 | 修改 |
| `src/main.rs` | `main()` 开头、交互之前加入启动检查 | 修改 |
| `tests/main_test.rs` | 修复 7 处 E0061 调用；更新 1 处断言；新增 6 个测试 | 修改 |

任务顺序：任务 1（测试可编译）→ 任务 2（库层实现，TDD）→ 任务 3（bin 入口）。

---

### 任务 1：修复测试编译（7 处 E0061）

**文件：**
- 修改：`tests/main_test.rs`（7 处 `build_commit_message` 调用补第 5 个参数 `""`）

**背景：** `build_commit_message` 库层签名为 5 参数（含 `footer`），测试仍按 4 参数调用，导致 `cargo test` 无法编译。footer 为空时函数不追加任何内容，因此补 `""` 后**所有现有断言保持不变**（含 `test_build_commit_message_edge_cases` 对全空输入返回 `": "` 的断言）。

- [ ] **步骤 1：修改 7 处调用**

`tests/main_test.rs` 中，按以下对应关系给每个 `build_commit_message` 调用补上第 5 个参数 `""`（两处文本相同的 `let commit_message = build_commit_message(commit_type, scope, description, body);` 分别在 `test_build_commit_message`（约第 76 行）与 `test_full_workflow`（约第 250 行），用周围上下文区分）：

```rust
// test_build_commit_message（原第 76 行）
let commit_message = build_commit_message(commit_type, scope, description, body, "");
// test_build_commit_message（原第 82 行）
let commit_message_no_scope = build_commit_message(commit_type, "", description, body, "");
// test_build_commit_message（原第 88 行）
let commit_message_no_body = build_commit_message(commit_type, scope, description, "", "");
// test_build_commit_message_edge_cases（原第 95 行）
let empty_message = build_commit_message("", "", "", "", "");
// test_build_commit_message_edge_cases（原第 104 行）
let long_message = build_commit_message(&long_type, &long_scope, &long_description, &long_body, "");
// test_build_commit_message_edge_cases（原第 110 行）
let special_message = build_commit_message("type!", "scope@", "description#", "body$", "");
// test_full_workflow（原第 250 行）
let commit_message = build_commit_message(commit_type, scope, description, body, "");
```

- [ ] **步骤 2：运行测试确认编译通过且全绿**

运行：`cargo test`
预期：编译成功，11 个既有测试全部 PASS（含 4 个 `perform_commit`/`format_commit_types` 测试）。

- [ ] **步骤 3：Commit**

```bash
git add tests/main_test.rs
git commit -m "test: fix build_commit_message call sites to match 5-arg signature"
```

---

### 任务 2：库层 `ensure_staged_changes` + `perform_commit` 严格语义修正（TDD）

**文件：**
- 修改：`src/lib.rs`（新增 `ensure_staged_changes`；`perform_commit` 开头替换检查块）
- 测试：`tests/main_test.rs`（新增 6 个测试；更新 `test_perform_commit_no_changes` 断言）

- [ ] **步骤 1：编写失败的测试**

在 `tests/main_test.rs` 末尾追加以下内容：

```rust
fn init_repo_with_initial_commit(path: &Path) -> Repository {
    let repo = Repository::init(path).unwrap();
    let mut index = repo.index().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let signature = Signature::now("Test User", "test@example.com").unwrap();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Initial commit",
        &tree,
        &[],
    )
    .unwrap();
    repo
}

#[test]
fn test_ensure_staged_changes_with_staged() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo = init_repo_with_initial_commit(temp_dir.path());

    std::fs::write(temp_dir.path().join("b.txt"), "hi").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("b.txt")).unwrap();
    index.write().unwrap();

    ensure_staged_changes(&repo).unwrap();
}

#[test]
fn test_ensure_staged_changes_clean() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo = init_repo_with_initial_commit(temp_dir.path());

    let err = ensure_staged_changes(&repo).unwrap_err();
    assert_eq!(err.to_string(), "Your git repository is clean");
}

#[test]
fn test_ensure_staged_changes_untracked_only() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo = init_repo_with_initial_commit(temp_dir.path());

    // 新建文件但不 add：untracked，不算 staged
    std::fs::write(temp_dir.path().join("untracked.txt"), "new").unwrap();

    let err = ensure_staged_changes(&repo).unwrap_err();
    assert_eq!(err.to_string(), "Your git repository is clean");
}

#[test]
fn test_ensure_staged_changes_workdir_modification_only() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo = init_repo_with_initial_commit(temp_dir.path());

    // 初始提交包含 a.txt
    std::fs::write(temp_dir.path().join("a.txt"), "v1").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("a.txt")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let signature = Signature::now("Test User", "test@example.com").unwrap();
    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "commit a.txt",
        &tree,
        &[&repo.head().unwrap().peel_to_commit().unwrap()],
    )
    .unwrap();

    // 修改 a.txt 内容但不更新 index：仅工作区修改，不算 staged
    std::fs::write(temp_dir.path().join("a.txt"), "v2").unwrap();

    let err = ensure_staged_changes(&repo).unwrap_err();
    assert_eq!(err.to_string(), "Your git repository is clean");
}

#[test]
fn test_ensure_staged_changes_empty_repo_clean() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo = Repository::init(temp_dir.path()).unwrap();

    // 空仓库（无提交、无 add）：无 staged
    let err = ensure_staged_changes(&repo).unwrap_err();
    assert_eq!(err.to_string(), "Your git repository is clean");
}

#[test]
fn test_ensure_staged_changes_empty_repo_staged() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo = Repository::init(temp_dir.path()).unwrap();

    // 空仓库 + 已 add 文件：相对空基线有 staged
    std::fs::write(temp_dir.path().join("a.txt"), "hi").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("a.txt")).unwrap();
    index.write().unwrap();

    ensure_staged_changes(&repo).unwrap();
}
```

同时更新既有测试 `test_perform_commit_no_changes` 的断言（约第 223 行附近）：

```rust
    assert_eq!(
        result.unwrap_err().to_string(),
        "Your git repository is clean",
        "Error message should indicate nothing to commit"
    );
```

（把原断言中的 `"Nothing to commit, working directory clean"` 替换为 `"Your git repository is clean"`。）

- [ ] **步骤 2：运行测试确认失败**

运行：`cargo test`
预期：编译失败，报错 `cannot find function ensure_staged_changes in crate git_cz_ai`（或类似未定义错误）。同时 `tests/main_test.rs` 顶部需要把 `ensure_staged_changes` 加入 import（与步骤 1 的测试代码一起改，import 改为 `use git_cz_ai::{build_commit_message, build_commit_types, ensure_staged_changes, format_commit_types, perform_commit};`）。

- [ ] **步骤 3：实现最少代码**

在 `src/lib.rs` 中新增函数（放在 `perform_commit` 之前），并修改 `perform_commit`：

```rust
/// 严格语义检查：index（暂存区）与 HEAD 无任何差异时报错。
/// 仅统计 staged changes；untracked 文件与未 add 的工作区修改均不计入。
/// 无 HEAD（空仓库）时以空树为基线，此时任何已 add 的内容都算 staged。
pub fn ensure_staged_changes(repo: &Repository) -> Result<(), Box<dyn Error>> {
    let index = repo.index()?;

    // 基线 tree：有 HEAD 用 HEAD 的 tree；无 HEAD（unborn branch，即空仓库）用 None（等价空树）
    let base_tree = match repo.head() {
        Ok(head) => {
            let head_commit = repo.find_commit(head.target().ok_or("Failed to find HEAD target")?)?;
            Some(head_commit.tree()?)
        }
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e.into()),
    };

    let diff = repo.diff_tree_to_index(base_tree.as_ref(), Some(&index), None)?;
    if diff.deltas().len() == 0 {
        return Err("Your git repository is clean".into());
    }
    Ok(())
}

pub fn perform_commit(repo_path: &Path, full_commit_message: &str) -> Result<(), Box<dyn Error>> {
    let repo = Repository::open(repo_path)?;
    ensure_staged_changes(&repo)?;

    let mut index = repo.index()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let config = repo.config()?;
    let author_name = config.get_string("user.name")?;
    let author_email = config.get_string("user.email")?;
    let sig = Signature::now(&author_name, &author_email)?;

    let head = repo.head()?;
    let parent_commit = repo.find_commit(head.target().ok_or("Failed to find HEAD target")?)?;

    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &full_commit_message,
        &tree,
        &[&parent_commit],
    )?;

    Ok(())
}
```

变更点：`perform_commit` 开头由原来的

```rust
let repo = Repository::open(repo_path)?;

let statuses = repo.statuses(None)?;
if statuses.is_empty() {
    return Err("Nothing to commit, working directory clean".into());
}
```

替换为

```rust
let repo = Repository::open(repo_path)?;
ensure_staged_changes(&repo)?;
```

- [ ] **步骤 4：运行测试确认通过**

运行：`cargo test`
预期：编译成功；12 个既有/已更新测试 + 6 个新测试全部 PASS（其中 `test_perform_commit_no_changes` 与 `test_ensure_staged_changes_clean` 等均验证新错误消息）。`test_perform_commit_invalid_path` 仍 `should_panic(expected = "No such file or directory")`（`Repository::open` 失败先于检查发生）。

- [ ] **步骤 5：Commit**

```bash
git add src/lib.rs tests/main_test.rs
git commit -m "feat: add strict staged-changes check in lib layer"
```

---

### 任务 3：`main.rs` 启动检查 + 手动验证

**文件：**
- 修改：`src/main.rs`（import + `main()` 开头）

- [ ] **步骤 1：修改 `src/main.rs`**

在 `main()` 的最开头（`build_commit_types()` 之前）、任何交互之前加入启动检查：

```rust
use git2::Repository;
// 注意：现有 use git_cz_ai::{...} 需加入 ensure_staged_changes

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 启动预检：无任何 staged changes 时直接退出
    let repo = Repository::open(".")?;
    if let Err(e) = ensure_staged_changes(&repo) {
        eprintln!("{}", e);
        std::process::exit(1);
    }

    let commit_types = build_commit_types();
    // ... 其余交互流程保持不变
```

具体改动：
1. import 块改为：

```rust
use git_cz_ai::{
    build_commit_message, build_commit_types, ensure_staged_changes, format_commit_types,
    perform_commit,
};
use git2::Repository;
```

2. `fn main()` 第一行 `let commit_types = build_commit_types();` 之前插入上面的启动预检代码。

- [ ] **步骤 2：编译验证**

运行：`cargo check` 与 `cargo build`
预期：均成功，无警告新增。

- [ ] **步骤 3：手动验证（bin 入口无法被集成测试覆盖）**

```bash
# 场景 1：无任何 staged changes（含 untracked 文件）→ 立即退出
rm -rf /tmp/cztest && git init -q /tmp/cztest && cd /tmp/cztest
echo hi > notes.txt          # 仅 untracked
/Users/secret/codes/git-cz-ai/target/debug/git-cz
echo "exit=$?"               # 预期：输出 Your git repository is clean，exit=1

# 场景 2：仅工作区修改未 add → 立即退出
git commit -q --allow-empty -m "init"
echo change >> notes.txt && git add notes.txt && git commit -q -m "add notes"
echo more >> notes.txt       # 修改未 add
/Users/secret/codes/git-cz-ai/target/debug/git-cz
echo "exit=$?"               # 预期：Your git repository is clean，exit=1

# 场景 3：有 staged changes → 进入交互流程（Ctrl+C 退出即可）
git add notes.txt
/Users/secret/codes/git-cz-ai/target/debug/git-cz
# 预期：出现 "Select the type of change that you're committing:" 交互提示
cd / && rm -rf /tmp/cztest
```

- [ ] **步骤 4：Commit**

```bash
git add src/main.rs
git commit -m "feat: exit early when no staged changes in main entry"
```

---

## 自检

**1. 规格覆盖度：**
- 启动时检查（交互前）→ 任务 3 步骤 1 ✓
- 严格语义（HEAD vs index，忽略 untracked/未 add）→ 任务 2 `ensure_staged_changes` ✓
- 错误输出 `eprintln!`（无 `Error:` 前缀）+ `exit(1)` → 任务 3 步骤 1 ✓
- `perform_commit` 修正 + 消息统一 → 任务 2 步骤 3 ✓
- 修复 7 处 E0061 → 任务 1 ✓
- 更新 `test_perform_commit_no_changes` 断言 → 任务 2 步骤 1 ✓
- 新增 6 个测试（含空仓库、untracked、仅工作区修改边界）→ 任务 2 步骤 1 ✓

**2. 占位符扫描：** 无"待定"/"TODO"；所有步骤含具体代码与预期输出；无引用未定义的类型/函数（`ensure_staged_changes` 在任务 2 定义、任务 3 使用）。✓

**3. 类型一致性：** `ensure_staged_changes(repo: &Repository) -> Result<(), Box<dyn Error>>` 在任务 2 定义与任务 3 调用处签名一致；`diff_tree_to_index(base_tree.as_ref(), ...)` 中 `base_tree: Option<Tree>`，`as_ref()` 得 `Option<&Tree>`，与 API `old_tree: Option<&Tree>` 匹配。✓
