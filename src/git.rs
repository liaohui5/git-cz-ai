use git2::{ErrorCode, Repository, Signature};
use std::error::Error;
use std::path::Path;

/// execute `git diff --cached` get staged changes
pub fn get_staged_diff() -> Result<String, Box<dyn Error>> {
    get_staged_diff_in(Path::new("."))
}

fn get_staged_diff_in(repo_path: &Path) -> Result<String, Box<dyn Error>> {
    let repo = Repository::open(repo_path)?;
    let index = repo.index()?;
    // a freshly `git init` repo has no commits yet: diff the index against an empty tree
    let head_tree = match repo.head() {
        Ok(head) => head.peel_to_tree().ok(),
        Err(e) if e.code() == ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e.into()),
    };

    let diff = if let Some(tree) = head_tree {
        repo.diff_tree_to_index(Some(&tree), Some(&index), None)?
    } else {
        repo.diff_tree_to_index(None, Some(&index), None)?
    };

    if diff.deltas().len() == 0 {
        return Err("No staged changes. Please 'git add' your files first.".into());
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
        return Err("No staged changes. Please 'git add' your files first.".into());
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

#[cfg(test)]
mod git_test {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    static DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);
    // perform_commit and has_staged_changes operate on the process cwd ("."); serialize those tests
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    /// Unique temp dir per test, so parallel tests do not collide
    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let n = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir =
            std::env::temp_dir().join(format!("git-cz-ai-test-{name}-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A freshly `git init` repo with no commits (unborn branch)
    fn empty_repo(name: &str) -> std::path::PathBuf {
        let dir = unique_temp_dir(name);
        let repo = Repository::init(&dir).unwrap();
        // precondition: HEAD is unborn, i.e. repo.head() errors with UnbornBranch
        match repo.head() {
            Err(e) => assert_eq!(e.code(), ErrorCode::UnbornBranch),
            Ok(_) => panic!("expected unborn branch in freshly initialized repo"),
        }
        dir
    }

    /// Repo with an initial commit on HEAD
    fn repo_with_initial_commit(name: &str) -> std::path::PathBuf {
        let dir = unique_temp_dir(name);
        let repo = Repository::init(&dir).unwrap();
        fs::write(dir.join("base.txt"), "base\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("base.txt")).unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "test").unwrap();
        cfg.set_str("user.email", "test@example.com").unwrap();
        let sig = Signature::now("test", "test@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();
        dir
    }

    #[test]
    fn get_staged_diff_works_in_empty_repo_with_staged_file() {
        let dir = empty_repo("empty-staged");
        fs::write(dir.join("hello.txt"), "hello\n").unwrap();
        let repo = Repository::open(&dir).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("hello.txt")).unwrap();
        index.write().unwrap();

        let diff = get_staged_diff_in(&dir).unwrap();
        assert!(
            diff.contains("hello.txt"),
            "diff should mention hello.txt, got: {diff}"
        );
        // with DiffFormat::Patch + callback, line content carries no '+' prefix; check the raw line
        assert!(
            diff.contains("hello"),
            "diff should contain added line content, got: {diff}"
        );
    }

    #[test]
    fn get_staged_diff_empty_repo_without_staged_errors() {
        let dir = empty_repo("empty-nostaged");

        let err = get_staged_diff_in(&dir).unwrap_err();
        assert_eq!(
            err.to_string(),
            "No staged changes. Please 'git add' your files first."
        );
    }

    #[test]
    fn get_staged_diff_works_in_repo_with_initial_commit() {
        let dir = repo_with_initial_commit("with-commit");
        fs::write(dir.join("feature.txt"), "feature\n").unwrap();
        let repo = Repository::open(&dir).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("feature.txt")).unwrap();
        index.write().unwrap();

        let diff = get_staged_diff_in(&dir).unwrap();
        assert!(
            diff.contains("feature.txt"),
            "diff should mention feature.txt, got: {diff}"
        );
        assert!(
            !diff.contains("base.txt"),
            "diff should not include committed file, got: {diff}"
        );
    }

    #[test]
    fn perform_commit_works_in_empty_repo() {
        // perform_commit runs against ".", so chdir into the temp repo under a lock
        let _guard = CWD_LOCK.lock().unwrap();
        let dir = empty_repo("commit-empty");
        fs::write(dir.join("file.txt"), "content\n").unwrap();
        let repo = Repository::open(&dir).unwrap();
        {
            let mut cfg = repo.config().unwrap();
            cfg.set_str("user.name", "test").unwrap();
            cfg.set_str("user.email", "test@example.com").unwrap();
        }
        {
            let mut index = repo.index().unwrap();
            index.add_path(Path::new("file.txt")).unwrap();
            index.write().unwrap();
        }

        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(&dir).unwrap();
        let result = perform_commit("feat: first commit in empty repo");
        std::env::set_current_dir(previous).unwrap();
        result.unwrap();

        // the first commit should now be on HEAD (no parent, previously unborn branch)
        let repo = Repository::open(&dir).unwrap();
        let head = repo.head().unwrap();
        let commit = repo.find_commit(head.target().unwrap()).unwrap();
        assert_eq!(
            commit.message().unwrap().trim(),
            "feat: first commit in empty repo"
        );
        assert_eq!(commit.parent_count(), 0);
    }
}
