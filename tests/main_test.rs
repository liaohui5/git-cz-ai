use git2::Repository;
use git2::Signature;
use git_cz_ai::ai::{build_ai_prompt, get_staged_diff, parse_llm_response};
use git_cz_ai::config::{load_config, Config};
use git_cz_ai::{
    build_commit_message, build_commit_types, ensure_staged_changes, format_commit_types,
    perform_commit,
};
use std::path::Path;
use tempfile;

#[test]
fn test_build_commit_types() {
    let commit_types = build_commit_types();
    assert!(!commit_types.is_empty(), "Commit types should not be empty");
    assert!(
        commit_types.iter().any(|&(t, _)| t == "feat"),
        "Should include 'feat' type"
    );
    assert!(
        commit_types.iter().any(|&(t, _)| t == "fix"),
        "Should include 'fix' type"
    );
}

#[test]
fn test_format_commit_types() {
    let commit_types = vec![
        ("feat", "A new feature"),
        ("fix", "A bug fix"),
        ("docs", "Documentation updates"),
    ];

    let formatted = format_commit_types(commit_types);

    let expected = vec![
        "feat     - A new feature".to_string(),
        "fix      - A bug fix".to_string(),
        "docs     - Documentation updates".to_string(),
    ];

    assert_eq!(formatted, expected);
}

#[test]
fn test_format_commit_types_empty_list() {
    let commit_types = vec![];
    let formatted = format_commit_types(commit_types);
    assert!(
        formatted.is_empty(),
        "Formatting an empty list should result in an empty vector"
    );
}

#[test]
fn test_format_commit_types_varying_lengths() {
    let commit_types = vec![
        ("a", "Short"),
        ("looong", "A longer type"),
        ("medium", "Medium length"),
    ];

    let formatted = format_commit_types(commit_types);

    let expected = vec![
        "a          - Short".to_string(),
        "looong     - A longer type".to_string(),
        "medium     - Medium length".to_string(),
    ];

    assert_eq!(formatted, expected);
}

#[test]
fn test_build_commit_message() {
    let commit_type = "feat";
    let scope = "ui";
    let description = "Add new button";
    let body = "This button allows users to submit the form.";

    let commit_message = build_commit_message(commit_type, scope, description, body, "");
    assert_eq!(
        commit_message,
        "feat(ui): Add new button\n\nThis button allows users to submit the form."
    );

    let commit_message_no_scope = build_commit_message(commit_type, "", description, body, "");
    assert_eq!(
        commit_message_no_scope,
        "feat: Add new button\n\nThis button allows users to submit the form."
    );

    let commit_message_no_body = build_commit_message(commit_type, scope, description, "", "");
    assert_eq!(commit_message_no_body, "feat(ui): Add new button");
}

#[test]
fn test_build_commit_message_edge_cases() {
    // All empty strings
    let empty_message = build_commit_message("", "", "", "", "");
    assert_eq!(empty_message, ": ", "Empty inputs should result in ': '");

    // Very long strings
    let long_type = "a".repeat(50);
    let long_scope = "b".repeat(50);
    let long_description = "c".repeat(100);
    let long_body = "d".repeat(1000);

    let long_message = build_commit_message(&long_type, &long_scope, &long_description, &long_body, "");
    assert!(long_message.starts_with(&format!("{}({}):", long_type, long_scope)));
    assert!(long_message.contains(&long_description));
    assert!(long_message.contains(&long_body));

    // Special characters
    let special_message = build_commit_message("type!", "scope@", "description#", "body$", "");
    assert_eq!(special_message, "type!(scope@): description#\n\nbody$");
}

#[test]
fn test_perform_commit() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo = Repository::init(temp_dir.path()).unwrap();

    let mut index = repo.index().unwrap();
    let _oid = repo.refname_to_id("HEAD").unwrap_or_else(|_| {
        let tree = repo.treebuilder(None).unwrap().write().unwrap();
        repo.commit(
            Some("HEAD"),
            &repo.signature().unwrap(),
            &repo.signature().unwrap(),
            "Initial commit",
            &repo.find_tree(tree).unwrap(),
            &[],
        )
        .unwrap()
    });

    let full_commit_message = "test: Test commit";

    std::fs::write(temp_dir.path().join("test.txt"), "Test content").unwrap();
    index.add_path(Path::new("test.txt")).unwrap();
    index.write().unwrap();

    perform_commit(temp_dir.path(), &full_commit_message).unwrap();

    let head = repo.head().unwrap();
    let commit = repo.find_commit(head.target().unwrap()).unwrap();
    assert_eq!(commit.message().unwrap(), full_commit_message);
}

#[test]
fn test_perform_commit_multiple_files() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo = Repository::init(temp_dir.path()).unwrap();

    // Create initial commit
    let mut index = repo.index().unwrap();
    let _oid = repo.refname_to_id("HEAD").unwrap_or_else(|_| {
        let tree = repo.treebuilder(None).unwrap().write().unwrap();
        repo.commit(
            Some("HEAD"),
            &repo.signature().unwrap(),
            &repo.signature().unwrap(),
            "Initial commit",
            &repo.find_tree(tree).unwrap(),
            &[],
        )
        .unwrap()
    });

    // Create multiple files
    std::fs::write(temp_dir.path().join("file1.txt"), "Content 1").unwrap();
    std::fs::write(temp_dir.path().join("file2.txt"), "Content 2").unwrap();

    // Add files to index
    index.add_path(Path::new("file1.txt")).unwrap();
    index.add_path(Path::new("file2.txt")).unwrap();
    index.write().unwrap();

    let full_commit_message = "feat: Add multiple files";
    perform_commit(temp_dir.path(), &full_commit_message).unwrap();

    let head = repo.head().unwrap();
    let commit = repo.find_commit(head.target().unwrap()).unwrap();
    assert_eq!(commit.message().unwrap(), full_commit_message);

    // Verify both files are in the commit
    let tree = commit.tree().unwrap();
    assert!(tree.get_name("file1.txt").is_some());
    assert!(tree.get_name("file2.txt").is_some());
}

#[test]
fn test_perform_commit_no_changes() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo = Repository::init(temp_dir.path()).unwrap();

    // Create initial commit
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

    let full_commit_message = "feat: This commit should fail";
    let result = perform_commit(temp_dir.path(), &full_commit_message);

    assert!(result.is_err(), "Commit with no changes should fail");
    assert_eq!(
        result.unwrap_err().to_string(),
        "Your git repository is clean",
        "Error message should indicate nothing to commit"
    );
}

#[test]
fn test_full_workflow() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo = Repository::init(temp_dir.path()).unwrap();

    // Create initial commit
    let mut index = repo.index().unwrap();
    let _oid = repo.refname_to_id("HEAD").unwrap_or_else(|_| {
        let tree = repo.treebuilder(None).unwrap().write().unwrap();
        repo.commit(
            Some("HEAD"),
            &repo.signature().unwrap(),
            &repo.signature().unwrap(),
            "Initial commit",
            &repo.find_tree(tree).unwrap(),
            &[],
        )
        .unwrap()
    });

    // Create a file
    std::fs::write(temp_dir.path().join("feature.txt"), "New feature").unwrap();
    index.add_path(Path::new("feature.txt")).unwrap();
    index.write().unwrap();

    // Use build_commit_message to create a commit message
    let commit_type = "feat";
    let scope = "user-interface";
    let description = "Add new feature";
    let body = "This commit adds a new feature to improve user experience.";

    let commit_message = build_commit_message(commit_type, scope, description, body, "");

    // Perform the commit
    perform_commit(temp_dir.path(), &commit_message).unwrap();

    // Verify the commit
    let head = repo.head().unwrap();
    let commit = repo.find_commit(head.target().unwrap()).unwrap();
    assert_eq!(commit.message().unwrap(), commit_message);

    // Verify the file is in the commit
    let tree = commit.tree().unwrap();
    assert!(tree.get_name("feature.txt").is_some());
}

#[test]
#[should_panic(expected = "No such file or directory")]
fn test_perform_commit_invalid_path() {
    let invalid_path = Path::new("/this/path/does/not/exist");
    let commit_message = "This commit should fail";
    perform_commit(invalid_path, commit_message).unwrap();
}

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
    drop(tree); // 结束对 repo 的借用，才能把 repo 返回出去
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

#[test]
fn test_build_ai_prompt_placeholder() {
    let diff = "diff --git a/src/main.rs b/src/main.rs";
    let prompt = build_ai_prompt(diff);
    assert!(
        prompt.contains(diff),
        "diff 内容应替换 {{diff}} 占位符"
    );
    assert!(!prompt.contains("{{diff}}"), "占位符应被替换");
    assert!(prompt.contains("## 角色与任务"), "模板头部应保留");
    assert!(prompt.contains("Conventional Commits"), "模板正文应保留");
}

#[test]
fn test_parse_llm_response_direct_array() {
    let body = r#"["feat: add login", "fix: fix bug"]"#;
    let result = parse_llm_response(body).unwrap();
    assert_eq!(result, vec!["feat: add login", "fix: fix bug"]);
}

#[test]
fn test_parse_llm_response_openai_envelope() {
    let body = r#"{"choices":[{"message":{"content":"[\"feat: add login\"]"}}]}"#;
    let result = parse_llm_response(body).unwrap();
    assert_eq!(result, vec!["feat: add login"]);
}

#[test]
fn test_parse_llm_response_invalid_json() {
    let err = parse_llm_response("not a json").unwrap_err();
    assert_eq!(err.to_string(), "llm api response is not a json string");
}

#[test]
fn test_parse_llm_response_content_not_array() {
    let body = r#"{"choices":[{"message":{"content":"{\"a\": 1}"}}]}"#;
    let err = parse_llm_response(body).unwrap_err();
    assert_eq!(err.to_string(), "llm api response is not a json string");
}

#[test]
fn test_get_staged_diff() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo = init_repo_with_initial_commit(temp_dir.path());

    std::fs::write(temp_dir.path().join("a.txt"), "v1").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(Path::new("a.txt")).unwrap();
    index.write().unwrap();

    let diff = get_staged_diff(temp_dir.path()).unwrap();
    assert!(diff.contains("a.txt"), "diff 应包含新增文件名");
}

#[test]
fn test_load_config_missing_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    let config = load_config(&path).unwrap();
    assert!(config.api_endpoint.is_none());
    assert!(config.api_token.is_none());
    assert!(config.model_name.is_none());
}

#[test]
fn test_load_config_parses_toml() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    std::fs::write(
        &path,
        "api_endpoint=\"https://api.deepseek.com/v1/chat/completions\"\n\
         api_token=\"sk-test\"\n\
         model_name=\"deepseek-v4-flash\"\n",
    )
    .unwrap();
    let config = load_config(&path).unwrap();
    assert_eq!(
        config.api_endpoint.as_deref(),
        Some("https://api.deepseek.com/v1/chat/completions")
    );
    assert_eq!(config.api_token.as_deref(), Some("sk-test"));
    assert_eq!(config.model_name.as_deref(), Some("deepseek-v4-flash"));
}

#[test]
fn test_load_config_invalid_toml() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    std::fs::write(&path, "not valid toml {{{").unwrap();
    let err = load_config(&path).unwrap_err();
    assert!(err.to_string().contains("Failed to parse config"));
}

#[test]
fn test_load_config_partial_fields() {
    let temp_dir = tempfile::tempdir().unwrap();
    let path = temp_dir.path().join("config.toml");
    std::fs::write(&path, "model_name=\"gpt-test\"\n").unwrap();
    let config = load_config(&path).unwrap();
    assert!(config.api_endpoint.is_none());
    assert!(config.api_token.is_none());
    assert_eq!(config.model_name.as_deref(), Some("gpt-test"));
}
