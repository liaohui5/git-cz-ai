use git2::{Commit, DiffFormat, ErrorCode, Repository, Signature};
use std::error::Error;
use std::path::Path;
use std::str::from_utf8;

const NO_STAGED_CHANGES: &str = "No staged changes, please 'git add' your files first";

/// execute `git diff --cached` get staged changes
pub fn get_staged_diff() -> Result<String, Box<dyn Error>> {
    get_staged_diff_in(Path::new("."))
}

fn get_staged_diff_in(repo_path: &Path) -> Result<String, Box<dyn Error>> {
    let repo = Repository::open(repo_path)?;
    let index = repo.index()?;
    // A freshly `git init` repo has no commits yet: diff the index against an
    // empty tree. A HEAD that exists but cannot be peeled to a tree also
    // degrades to that empty baseline (long-standing behavior; do not route
    // this through `head_commit`, which treats such a HEAD as an error).
    let head_tree = match repo.head() {
        Ok(head) => head.peel_to_tree().ok(),
        Err(e) if e.code() == ErrorCode::UnbornBranch => None,
        Err(e) => return Err(e.into()),
    };

    let diff = repo.diff_tree_to_index(head_tree.as_ref(), Some(&index), None)?;
    if diff.deltas().len() == 0 {
        return Err(NO_STAGED_CHANGES.into());
    }

    let mut output = String::new();
    diff.print(DiffFormat::Patch, |_delta, _hunk, line| {
        if let Ok(text) = from_utf8(line.content()) {
            output.push_str(text);
        }
        true
    })?;

    Ok(output)
}

/// Resolve the commit HEAD points at. `Ok(None)` when HEAD is unborn (a freshly
/// `git init` repo with no commits); errors when HEAD exists but cannot resolve
/// to a commit.
fn head_commit(repo: &Repository) -> Result<Option<Commit<'_>>, Box<dyn Error>> {
    match repo.head() {
        Ok(head) => {
            let id = head.target().ok_or("Failed to find HEAD target")?;
            Ok(Some(repo.find_commit(id)?))
        }
        Err(e) if e.code() == ErrorCode::UnbornBranch => Ok(None),
        Err(e) => Err(e.into()),
    }
}

pub fn has_staged_changes() -> Result<(), Box<dyn Error>> {
    let repo = Repository::open(".")?;
    let index = repo.index()?;

    // None when the repo has no commits yet: diff the index against an empty tree
    let base_tree = head_commit(&repo)?
        .map(|commit| commit.tree())
        .transpose()?;

    let diff = repo.diff_tree_to_index(base_tree.as_ref(), Some(&index), None)?;
    if diff.deltas().len() == 0 {
        return Err(NO_STAGED_CHANGES.into());
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

    // A first commit (unborn HEAD) has no parents
    let parents = head_commit(&repo)?;
    let parent_refs: Vec<&Commit> = parents.iter().collect();

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
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    static DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);
    // perform_commit runs against the process cwd ("."); serialize those tests
    static CWD_LOCK: Mutex<()> = Mutex::new(());

    /// Unique temp dir per test so parallel tests do not collide; removes
    /// itself on drop so no leftovers accumulate in the system temp dir.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let n = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir()
                .join(format!("git-cz-ai-test-{name}-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// Configure user.name/user.email locally so `perform_commit` can build a signature.
    fn set_test_identity(repo: &Repository) {
        let mut cfg = repo.config().unwrap();
        cfg.set_str("user.name", "test").unwrap();
        cfg.set_str("user.email", "test@example.com").unwrap();
    }

    /// Write `content` to `name` in the repo workdir and stage it in the index.
    fn stage_file(repo: &Repository, name: &str, content: &str) {
        fs::write(repo.workdir().unwrap().join(name), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new(name)).unwrap();
        index.write().unwrap();
    }

    /// A freshly `git init` repo with no commits (unborn branch)
    fn empty_repo(name: &str) -> TempDir {
        let dir = TempDir::new(name);
        let repo = Repository::init(dir.path()).unwrap();
        // precondition: HEAD is unborn, i.e. repo.head() errors with UnbornBranch
        match repo.head() {
            Err(e) => assert_eq!(e.code(), ErrorCode::UnbornBranch),
            Ok(_) => panic!("expected unborn branch in freshly initialized repo"),
        }
        dir
    }

    /// Repo with an initial commit on HEAD
    fn repo_with_initial_commit(name: &str) -> TempDir {
        let dir = TempDir::new(name);
        let repo = Repository::init(dir.path()).unwrap();
        stage_file(&repo, "base.txt", "base\n");
        let mut index = repo.index().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = Signature::now("test", "test@example.com").unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();
        dir
    }

    #[test]
    fn get_staged_diff_works_in_empty_repo_with_staged_file() {
        let dir = empty_repo("empty-staged");
        let repo = Repository::open(dir.path()).unwrap();
        stage_file(&repo, "hello.txt", "hello\n");

        let diff = get_staged_diff_in(dir.path()).unwrap();
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

        let err = get_staged_diff_in(dir.path()).unwrap_err();
        assert_eq!(
            err.to_string(),
            "No staged changes. Please 'git add' your files first."
        );
    }

    #[test]
    fn get_staged_diff_works_in_repo_with_initial_commit() {
        let dir = repo_with_initial_commit("with-commit");
        let repo = Repository::open(dir.path()).unwrap();
        stage_file(&repo, "feature.txt", "feature\n");

        let diff = get_staged_diff_in(dir.path()).unwrap();
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
        let repo = Repository::open(dir.path()).unwrap();
        stage_file(&repo, "file.txt", "content\n");
        set_test_identity(&repo);

        let previous = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let result = perform_commit("feat: first commit in empty repo");
        std::env::set_current_dir(previous).unwrap();
        result.unwrap();

        // the first commit should now be on HEAD (no parent, previously unborn branch)
        let repo = Repository::open(dir.path()).unwrap();
        let head = repo.head().unwrap();
        let commit = repo.find_commit(head.target().unwrap()).unwrap();
        assert_eq!(
            commit.message().unwrap().trim(),
            "feat: first commit in empty repo"
        );
        assert_eq!(commit.parent_count(), 0);
    }
}
