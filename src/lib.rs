pub mod ai;
pub mod config;
pub mod git;
pub mod manually;
pub mod loading;

/// Shared test helpers (unique temp dirs), used by the inline test modules.
#[cfg(test)]
pub mod test_util {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

    /// Unique temp dir per test so parallel tests do not collide; removes
    /// itself on drop so no leftovers accumulate in the system temp dir.
    pub struct TempDir(PathBuf);

    impl TempDir {
        pub fn new(name: &str) -> Self {
            let n = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir()
                .join(format!("git-cz-ai-test-{name}-{}-{n}", std::process::id()));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }

        pub fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
