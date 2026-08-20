use clap::{Args, Parser, Subcommand};
use git2::Repository;
use git_cz_ai::ai::{build_ai_prompt, get_staged_diff, parse_llm_response};
use git_cz_ai::{
    build_commit_message, build_commit_types, ensure_staged_changes, format_commit_types,
    perform_commit,
};
use promkit::preset::query_selector::QuerySelector;
use promkit::{preset::confirm::Confirm, preset::readline::Readline, suggest::Suggest};
use std::env;
use std::path::Path;
use std::process::Command;
use tempfile;

#[derive(Parser)]
struct Cli {
    /// 子命令；不传则保持现有交互式提交流程
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Subcommand)]
enum CliCommand {
    /// 用 AI 生成提交信息
    Ai(AiArgs),
}

#[derive(Args)]
struct AiArgs {
    /// LLM API 端点，如 https://api.openai.com/v1/chat/completions
    #[arg(long)]
    api_endpoint: String,
    /// API 令牌；未提供时回退到环境变量 GIT_CZ_AI_OPENAI_API_KEY
    #[arg(long, env = "GIT_CZ_AI_OPENAI_API_KEY")]
    api_token: String,
    /// 模型名称，如 gpt-5-mini
    #[arg(long)]
    model_name: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    match cli.command {
        Some(CliCommand::Ai(args)) => run_ai(&args),
        None => run_interactive(),
    }
}

fn run_ai(args: &AiArgs) -> Result<(), Box<dyn std::error::Error>> {
    // 1. 获取已暂存的 diff（git diff --cached）
    let diff = get_staged_diff(Path::new("."))?;

    // 2. 用 diff 替换提示词中的 {{diff}} 占位符
    let prompt = build_ai_prompt(&diff);

    // 3. 发送请求到 LLM API
    eprintln!("Request has been sent, waiting for response");
    let response = ureq::post(&args.api_endpoint)
        .set("Authorization", &format!("Bearer {}", args.api_token))
        .set("Content-Type", "application/json")
        .send_json(serde_json::json!({
            "model": args.model_name,
            "messages": [{ "role": "user", "content": prompt }],
        }));

    let body = match response {
        Ok(resp) => {
            eprintln!("Response received");
            resp.into_string()?
        }
        Err(ureq::Error::Status(code, resp)) => {
            let text = resp.into_string().unwrap_or_default();
            eprintln!("llm api error: HTTP {}: {}", code, text);
            std::process::exit(1);
        }
        Err(ureq::Error::Transport(e)) => {
            eprintln!("llm api request failed: {}", e);
            std::process::exit(1);
        }
    };

    // 4. 解析响应为 Vec<String>，失败立即退出
    let candidates = match parse_llm_response(&body) {
        Ok(candidates) => candidates,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };

    // 5. 命令行选择候选；Enter 选中，Ctrl-C 退出（不提交）
    let mut selector = QuerySelector::new(candidates, |text, items| -> Vec<String> {
        items
            .iter()
            .filter(|item| item.contains(text))
            .cloned()
            .collect()
    })
    .title("Select a commit message:")
    .listbox_lines(10)
    .prompt()?;

    let selection = match selector.run() {
        Ok(selection) => selection,
        Err(e) if e.to_string().contains("ctrl+c") => {
            println!("Commit aborted.");
            std::process::exit(0);
        }
        Err(e) => return Err(e.into()),
    };

    // 6. 自动提交
    perform_commit(Path::new("."), &selection)?;
    println!("Commit successful!");
    Ok(())
}

fn run_interactive() -> Result<(), Box<dyn std::error::Error>> {
    // 启动预检：无任何 staged changes 时直接退出
    let repo = Repository::open(".")?;
    if let Err(e) = ensure_staged_changes(&repo) {
        eprintln!("{}", e);
        std::process::exit(1);
    }

    let commit_types = build_commit_types();
    let commit_types_display = format_commit_types(commit_types);

    let mut p = QuerySelector::new(commit_types_display.clone(), |text, items| -> Vec<String> {
        items
            .iter()
            .filter(|item| item.contains(text))
            .cloned()
            .collect()
    })
    .title("Select the type of change that you're committing:")
    .listbox_lines(10)
    .prompt()?;

    let mut scope_input = Readline::default()
        .title("Denote the scope of this change (optional):")
        .enable_suggest(Suggest::from_iter([
            "app", "core", "ui", "db", "api", "frontend", "backend", "config", "build", "sec",
            "infra", "deps",
        ]))
        .prompt()?;

    let mut description_input = Readline::default()
        .title("Write a short, imperative tense description of the change:")
        .prompt()?;
    let mut body_input = Readline::default()
        .title("Provide a longer description of the change(press 'e' to open editor):")
        .prompt()?;

    let selection = p.run()?;
    let selected_type = selection.split_whitespace().next();

    if let Some(commit_type) = selected_type {
        let scope = scope_input.run()?;
        let description = description_input.run()?;
        let body = body_input.run()?;

        let body = if body.trim().to_lowercase() == "e" {
            // Create a temporary file
            let temp_file = tempfile::NamedTempFile::new()?;
            let temp_path = temp_file
                .path()
                .to_str()
                .expect("Failed to get temp file path");

            // Determine the editor command
            let editor_command = if cfg!(target_os = "windows") {
                env::var("EDITOR").unwrap_or_else(|_| "notepad".to_string())
            } else {
                env::var("EDITOR").unwrap_or_else(|_| "vim".to_string())
            };

            // Open the editor
            let status = Command::new(&editor_command).arg(temp_path).status()?;

            if !status.success() {
                eprintln!("Editor exited with non-zero status");
            }

            // Read the contents of the temp file
            std::fs::read_to_string(temp_path)?
        } else {
            body
        };

        // New footer confirmation prompt
        let mut footer_confirm = Confirm::new("Do you want to add a footer?").prompt()?;
        let footer = if footer_confirm.run()?.to_lowercase() == "y" {
            let mut footer_type_input = QuerySelector::new(
                vec!["fix".to_string(), "close".to_string()],
                |text, items| -> Vec<String> {
                    items
                        .iter()
                        .filter(|item| item.contains(text))
                        .cloned()
                        .collect()
                },
            )
            .title("Select the footer type:")
            .listbox_lines(2)
            .prompt()?;

            let mut issue_number_input = Readline::default()
                .title("Enter the issue number:")
                .validator(
                    |text| text.trim().parse::<i32>().is_ok(),
                    |text| format!("'{}' is not a valid integer", text),
                )
                .prompt()?;

            let footer_type = footer_type_input.run()?;
            let issue_number = issue_number_input.run()?;
            format!("{}: #{}", footer_type, issue_number)
        } else {
            String::new()
        };

        let full_commit_message =
            build_commit_message(&commit_type, &scope, &description, &body, &footer);

        let mut confirm_input =
            Confirm::new("Do you want to proceed with this commit?").prompt()?;
        let confirm = confirm_input.run()?;
        if confirm.to_lowercase() == "y" {
            perform_commit(Path::new("."), &full_commit_message)?;
            println!("Commit successful!");
        } else {
            println!("Commit aborted.");
        }
    }

    Ok(())
}
