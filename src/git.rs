use git2::{Repository, Signature};
use std::error::Error;

/// execute `git diff --cached` get staged changes
pub fn get_staged_diff() -> Result<String, Box<dyn Error>> {
    let repo = Repository::open(".")?;
    let index = repo.index()?;
    let head_tree = repo.head()?.peel_to_tree().ok();

    let diff = if let Some(tree) = head_tree {
        repo.diff_tree_to_index(Some(&tree), Some(&index), None)?
    } else {
        repo.diff_tree_to_index(None, Some(&index), None)?
    };

    if diff.deltas().len() == 0 {
        return Err("没有暂存变更，请先执行 git add".into());
    }

    let mut output = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        use std::str::from_utf8;
        if let Ok(text) = from_utf8(line.content()) {
            output.push_str(text);
        }
        true
    })?;

    Ok(output)
}

pub fn has_staged_changes() -> Result<(), Box<dyn Error>> {
    let repo = Repository::open(".")?;
    let index = repo.index()?;

    let base_tree = match repo.head() {
        Ok(head) => {
            let head_commit =
                repo.find_commit(head.target().ok_or("Failed to find HEAD target")?)?;
            Some(head_commit.tree()?)
        }
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e.into()),
    };

    let diff = repo.diff_tree_to_index(base_tree.as_ref(), Some(&index), None)?;
    if diff.deltas().len() == 0 {
        return Err("没有暂存变更，请先执行 git add".into());
    }

    Ok(())
}

pub fn perform_commit(full_commit_message: &str) -> Result<(), Box<dyn Error>> {
    let repo = Repository::open(".")?;

    let mut index = repo.index()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    let config = repo.config()?;
    let author_name = config.get_string("user.name")?;
    let author_email = config.get_string("user.email")?;
    let sig = Signature::now(&author_name, &author_email)?;

    let parents = match repo.head() {
        Ok(head) => {
            let parent_commit =
                repo.find_commit(head.target().ok_or("Failed to find HEAD target")?)?;
            vec![parent_commit]
        }
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => vec![],
        Err(e) => return Err(e.into()),
    };

    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    // execute git commit
    repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        full_commit_message,
        &tree,
        &parent_refs,
    )?;

    Ok(())
}
