use std::error;

use crate::git;
use inquire::{validator::Validation, Confirm, Editor, InquireError, Select, Text};

pub fn build_commit_message(
    commit_type: String,
    scope: String,
    break_change_mark: String,
    message: String,
    body: String,
    footer: String,
) -> String {
    let mut full_message = format!("{commit_type}{scope}{break_change_mark}: {message}");
    if !body.is_empty() {
        full_message.push_str(&format!("\n\n{body}"));
    }
    if !footer.is_empty() {
        full_message.push_str(&format!("\n\n{footer}"));
    }
    full_message
}

pub fn handler() -> Result<(), Box<dyn error::Error>> {
    git::has_staged_changes()?;

    let commit_type = select_commit_type();
    let commit_scope = input_commit_scope();
    let break_change_mark = confirm_is_breaking_change();
    let commit_message = input_commit_message();
    let commit_body = input_commit_body();
    let commit_footer = input_commit_footer();

    let full_commit_message = build_commit_message(
        commit_type,
        commit_scope,
        break_change_mark,
        commit_message,
        commit_body,
        commit_footer,
    );

    println!("{}", "-".repeat(50));
    println!("{}", full_commit_message);
    println!("{}", "-".repeat(50));

    if is_confirm_commit() {
        git::perform_commit(&full_commit_message)?;
    }

    Ok(())
}

pub fn select_commit_type() -> String {
    let commit_types = vec![
        "feat: a new feature",
        "fix: a bug fix",
        "docs: documentation only changes",
        "style: changes that do not affect the meaning of the code (white-space, formatting, etc.)",
        "refactor: a code change that neither fixes a bug nor adds a feature",
        "perf: a code change that improves performance",
        "test: adding missing tests or correcting existing tests",
        "chore: other changes that don't modify src or test files",
        "ci: changes to our ci configuration files and scripts",
        "build: changes that affect the build system or external dependencies",
        "revert: reverts a previous commit",
    ];
    let len = commit_types.len();

    let choice: Result<&str, InquireError> = Select::new(
        "Select the type of change that you're committing?",
        commit_types,
    )
    .with_page_size(len)
    .prompt();

    match choice {
        Ok(choice) => choice.split_once(":").unwrap().0.to_string(),
        Err(_) => String::new(),
    }
}

pub fn confirm_is_breaking_change() -> String {
    // y or n(default)
    let is_breaking_change = Confirm::new("Is this a breaking change(optional)?")
        .with_default(false)
        .prompt();

    if is_breaking_change.unwrap_or_default() {
        String::from("!")
    } else {
        String::new()
    }
}

pub fn input_commit_scope() -> String {
    // Input the scope of this change(optional)
    let commit_scope = Text::new("Input the scope of this change(optional)?")
        .prompt()
        .unwrap_or_default();
    if !commit_scope.is_empty() {
        format!("({commit_scope})")
    } else {
        String::new()
    }
}

pub fn input_commit_message() -> String {
    let commit_message = Text::new("Write a short, imperative tense description of the change?\n")
        .with_validator(|input: &str| {
            if input.trim().is_empty() {
                Ok(Validation::Invalid("commit message cannot be empty".into()))
            } else {
                Ok(Validation::Valid)
            }
        })
        .prompt();
    match commit_message {
        Ok(message) => message.to_string(),
        Err(_) => String::new(),
    }
}

pub fn input_commit_body() -> String {
    let commit_body = Editor::new("Provide a longer description of the change(optional):\n")
        .with_formatter(&|submission| {
            let char_count = submission.chars().count();
            if char_count == 0 {
                String::from("")
            } else if char_count <= 20 {
                submission.into()
            } else {
                let mut substr: String = submission.chars().take(17).collect();
                substr.push_str("...");
                substr
            }
        })
        .prompt();

    commit_body.unwrap_or_default()
}

pub fn input_commit_footer() -> String {
    // y or n(default)
    // y -> select(fix/close: issue num)
    let is_add_footer = Confirm::new("Do you want to add a footer(optional)?")
        .with_default(false)
        .prompt();

    if !is_add_footer.unwrap_or_default() {
        return String::new();
    }

    let footer_type_res: Result<&str, InquireError> =
        Select::new("Select the footer type:", vec!["close", "fix"]).prompt();

    let footer_text = Text::new("Enter the issues number:")
        .with_validator(|input: &str| {
            if input.trim().is_empty() {
                Ok(Validation::Invalid("issue number cannot be empty".into()))
            } else if input.trim().parse::<usize>().is_err() {
                Ok(Validation::Invalid("failed to prase issue number".into()))
            } else {
                Ok(Validation::Valid)
            }
        })
        .prompt();

    let footer_type = footer_type_res.unwrap_or_default();
    let footer_text = footer_text.unwrap_or_default();
    format!("{}: #{}", footer_type, footer_text)
}

pub fn is_confirm_commit() -> bool {
    // Are you sure to proceed with this commit?
    Confirm::new("Are you sure to proceed with this commit(Default: yes)?")
        .with_default(true)
        .prompt()
        .unwrap_or_default()
}

#[cfg(test)]
mod manual_test {
    use super::build_commit_message;

    #[test]
    fn build_commit_message_all_fields() {
        let commit_type = "feat".to_string();
        let scope = "(api)".to_string();
        let break_change_mark = "!".to_string();
        let message = "message".to_string();
        let body = "body".to_string();
        let footer = "footer".to_string();
        let full_commit_message =
            build_commit_message(commit_type, scope, break_change_mark, message, body, footer);
        assert_eq!(full_commit_message, "feat(api)!: message\n\nbody\n\nfooter");
    }

    #[test]
    fn build_commit_message_without_scope_and_breaking() {
        let full = build_commit_message(
            "feat".into(),
            String::new(),
            String::new(),
            "add feature".into(),
            String::new(),
            String::new(),
        );
        assert_eq!(full, "feat: add feature");
    }

    #[test]
    fn build_commit_message_only_body() {
        let full = build_commit_message(
            "fix".into(),
            String::new(),
            String::new(),
            "fix bug".into(),
            "some long body text".into(),
            String::new(),
        );
        assert_eq!(full, "fix: fix bug\n\nsome long body text");
    }

    #[test]
    fn build_commit_message_only_footer() {
        let full = build_commit_message(
            "fix".into(),
            String::new(),
            String::new(),
            "fix bug".into(),
            String::new(),
            "fix: #123".into(),
        );
        assert_eq!(full, "fix: fix bug\n\nfix: #123");
    }

    #[test]
    fn build_commit_message_with_scope_and_breaking_only() {
        let full = build_commit_message(
            "feat".into(),
            "(core)".into(),
            "!".into(),
            "breaking change".into(),
            String::new(),
            String::new(),
        );
        assert_eq!(full, "feat(core)!: breaking change");
    }

    #[test]
    fn build_commit_message_all_empty() {
        let full = build_commit_message(
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        );
        assert_eq!(full, ": ");
    }

    #[test]
    fn build_commit_message_body_and_footer_together() {
        let full = build_commit_message(
            "docs".into(),
            "(readme)".into(),
            String::new(),
            "update docs".into(),
            "detailed explanation".into(),
            "close: #456".into(),
        );
        assert_eq!(
            full,
            "docs(readme): update docs\n\ndetailed explanation\n\nclose: #456"
        );
    }

    #[test]
    fn build_commit_message_multiline_body() {
        let body = "line1\nline2\nline3".to_string();
        let full = build_commit_message(
            "refactor".into(),
            String::new(),
            String::new(),
            "refactor code".into(),
            body,
            String::new(),
        );
        assert_eq!(full, "refactor: refactor code\n\nline1\nline2\nline3");
    }
}
