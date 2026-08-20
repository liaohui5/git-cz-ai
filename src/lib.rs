pub mod ai;
pub mod config;

use git2::{Repository, Signature};
use std::error::Error;
use std::path::Path;

pub fn build_commit_types() -> Vec<(&'static str, &'static str)> {
    vec![
        ("feat", "A new feature"),
        ("fix", "A bug fix"),
        ("docs", "Documentation only changes"),
        (
            "style",
            "Changes that do not affect the meaning of the code (white-space, formatting, etc.)",
        ),
        (
            "refactor",
            "A code change that neither fixes a bug nor adds a feature",
        ),
        ("perf", "A code change that improves performance"),
        ("test", "Adding missing tests or correcting existing tests"),
        ("chore", "Other changes that don't modify src or test files"),
        ("ci", "Changes to our CI configuration files and scripts"),
        (
            "build",
            "Changes that affect the build system or external dependencies",
        ),
        ("revert", "Reverts a previous commit"),
    ]
}

pub fn format_commit_types(commit_types: Vec<(&str, &str)>) -> Vec<String> {
    // Determine the maximum length of commit type strings for proper alignment
    let max_type_length = commit_types
        .iter()
        .map(|(typ, _)| typ.len())
        .max()
        .unwrap_or(0);

    commit_types
        .iter()
        .map(|(typ, desc)| {
            // Adjust the width to account for proper spacing
            format!("{:<width$} - {}", typ, desc, width = max_type_length + 4)
        })
        .collect()
}

pub fn build_commit_message(
    commit_type: &str,
    scope: &str,
    description: &str,
    body: &str,
    footer: &str,
) -> String {
    let message = format!(
        "{}{}: {}",
        commit_type,
        if scope.is_empty() {
            String::new()
        } else {
            format!("({})", scope)
        },
        description
    );

    let mut full_message = message;

    if !body.is_empty() {
        full_message.push_str(&format!("\n\n{}", body));
    }

    if !footer.is_empty() {
        full_message.push_str(&format!("\n\n{}", footer));
    }

    full_message
}

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

    let parents = match repo.head() {
        Ok(head) => {
            let parent_commit = repo.find_commit(head.target().ok_or("Failed to find HEAD target")?)?;
            vec![parent_commit]
        }
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => vec![],
        Err(e) => return Err(e.into()),
    };

    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        &full_commit_message,
        &tree,
        &parent_refs,
    )?;

    Ok(())
}
