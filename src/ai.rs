use clap::{Arg, ArgMatches, Command};
use inquire::{InquireError, Select};
use std::error;
use ureq;

use crate::{
    config::{self, APIConfig},
    git,
    loading::LoadingSpinner,
};

/// Unified error message for missing API configuration
const MISSING_CONFIG_MSG: &str =
    "Missing API configuration (api_endpoint / api_token / model_name)";

/// Map ureq request errors to brief english reasons (ureq::Error is #[non_exhaustive], needs a catch-all arm)
fn format_request_error(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(code) => format!("AI request failed: HTTP {code}"),
        ureq::Error::HostNotFound => {
            "AI request failed: could not resolve server hostname".to_string()
        }
        ureq::Error::Io(_) => "AI request failed: network connection error".to_string(),
        ureq::Error::Timeout(_) => "AI request failed: request timed out".to_string(),
        ureq::Error::Tls(_) => "AI request failed: TLS connection error".to_string(),
        ureq::Error::BadUri(_) => "AI request failed: invalid API endpoint URL".to_string(),
        ureq::Error::ConnectionFailed => {
            "AI request failed: could not establish connection".to_string()
        }
        ureq::Error::TooManyRedirects => "AI request failed: too many redirects".to_string(),
        ureq::Error::RedirectFailed => "AI request failed: redirect failed".to_string(),
        ureq::Error::BodyExceedsLimit(_) => "AI request failed: request body too large".to_string(),
        _ => format!("AI request failed: {e}"),
    }
}

pub fn create_ai_cmd() -> Command {
    let api_endpoint_arg = Arg::new("api-endpoint")
        .long("api-endpoint")
        .help("Set the api url")
        .required(false)
        .num_args(1);

    let api_token_arg = Arg::new("api-token")
        .long("api-token")
        .help("Set the api access token")
        .required(false)
        .num_args(1);

    let model_name_arg = Arg::new("model-name")
        .long("model-name")
        .help("Set the model")
        .required(false)
        .num_args(1);

    Command::new("ai")
        .about("Auto generate commit messages by llm api")
        .arg(api_endpoint_arg)
        .arg(api_token_arg)
        .arg(model_name_arg)
}

pub fn handler(args: &ArgMatches) -> Result<(), Box<dyn error::Error>> {
    let file_config: APIConfig = config::load_config()?;
    let args_config = parse_args_to_config(args);
    let merged_config = config::merge_config(file_config, args_config); // args_config first

    let diff = git::get_staged_diff()?;
    let prompt = build_ai_prompt(&diff);
    let body = send_request(merged_config, prompt)?;
    let result = parse_llm_api_response(&body)?;
    let commit_message = match select_commit_message(result) {
        Some(message) => message,
        None => {
            // Ctrl-C / Esc: user cancelled, not an error, skip the commit
            println!("Commit aborted.");
            return Ok(());
        }
    };
    git::perform_commit(&commit_message)
}

pub fn send_request(config: APIConfig, prompt: String) -> Result<String, Box<dyn error::Error>> {
    // Return an error if any config field is missing, without making a network request
    let api_endpoint = config.api_endpoint.ok_or(MISSING_CONFIG_MSG)?;
    let api_token = config.api_token.ok_or(MISSING_CONFIG_MSG)?;
    let model_name = config.model_name.ok_or(MISSING_CONFIG_MSG)?;

    let mut loading_spinner = LoadingSpinner::default();
    loading_spinner.start("Request has been sent, waiting for response...");

    // Stop the spinner before returning the error on request failure
    let mut response = match ureq::post(&api_endpoint)
        .header("Authorization", &format!("Bearer {api_token}"))
        .header("Content-Type", "application/json")
        .send_json(serde_json::json!({
            "model": model_name,
            "messages": [{ "role": "user", "content": prompt }],
        })) {
        Ok(response) => response,
        Err(e) => {
            loading_spinner.stop();
            return Err(format_request_error(e).into());
        }
    };

    loading_spinner.stop();

    response
        .body_mut()
        .read_to_string()
        .map_err(|_| "Failed to read response body".into())
}

pub fn select_commit_message(messages: Vec<String>) -> Option<String> {
    let messages: Vec<&str> = messages.iter().map(|m| m.as_str()).collect();
    let len = messages.len();

    let commit_message: Result<&str, InquireError> =
        Select::new("Select commit message by AI generated", messages)
            .with_page_size(len)
            .prompt();

    match commit_message {
        Ok(message) => Some(String::from(message)),
        Err(_) => None,
    }
}

pub fn parse_llm_api_response(body: &str) -> Result<Vec<String>, Box<dyn error::Error>> {
    let parse_err = || -> Box<dyn error::Error> { "Failed to parse llm api response".into() };

    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| parse_err())?;

    // response.choices[0].message.content
    if let Some(content) = value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
    {
        return serde_json::from_str(content).map_err(|_| parse_err());
    }

    serde_json::from_value(value).map_err(|_| parse_err())
}

pub fn parse_args_to_config(args: &ArgMatches) -> APIConfig {
    let mut config = APIConfig::default();

    let api_endpoint = args.get_one::<String>("api-endpoint");
    let api_token = args.get_one::<String>("api-token");
    let model_name = args.get_one::<String>("model-name");

    if let Some(url) = api_endpoint {
        config.api_endpoint = Some(url.to_owned());
    }
    if let Some(token) = api_token {
        config.api_token = Some(token.to_owned());
    }
    if let Some(model_name) = model_name {
        config.model_name = Some(model_name.to_owned());
    }

    config
}

/// AI prompt template: user-provided markdown, {{diff}} placeholder replaced at call time
const AI_PROMPT_TEMPLATE: &str = r##"## 角色与任务
你是一个专业的 Git 提交信息生成器。
根据下方提供的 **git diff 输出**（即文件变化内容），生成一组符合 [Conventional Commits v1.0.0](https://www.conventionalcommits.org/en/v1.0.0/) 规范的提交信息。
每个提交信息应**对应一个逻辑独立的变更单元**（例如：新增功能、修复错误、文档更新等），而不是简单地将整个 diff 拆分为机械的逐行记录。

## 输入：文件变化内容
以下是 `git diff` 命令的输出结果，它是你生成提交信息的唯一依据: {{diff}}

## 输出格式（必须严格遵守）
- 返回一个 **合法的 JSON 字符串**。
- JSON 顶层必须是一个 **字符串数组**，且至少包含 **3 个元素**。
- 除了 JSON 字符串外，**不得输出任何额外文本、注释或解释**，以便后续程序直接解析。

## 每个提交信息的要求
1. **格式规范**（严格遵循 `<type>[optional scope]: <description>`）：
   - `<type>` 为必填项，必须是以下名词之一（常用类型）：`feat`、`fix`、`docs`、`style`、`refactor`、`perf`、`test`、`chore`。
   - `[optional scope]` 为可选项，如果使用，必须用英文括号包裹，例如 `feat(parser)`。
   - 冒号 `:` 后**必须紧跟一个英文半角空格**。
2. **语言与大小写**：整个提交信息（包括 type、scope 和 description）**必须全部使用小写英文字符**。
3. **长度限制**：每个提交信息的总字符数（**不含数组元素两端的引号**）**必须小于 100**。
4. **内容质量**：描述部分应**简洁、准确**，清晰概括本次变更的内容，避免模糊或泛泛而谈。

## 如何根据 diff 推导提交信息（指导原则）
- 分析 diff 中的文件路径和代码变更，识别出**功能新增**（→ `feat`）、**错误修复**（→ `fix`）、**文档改动**（→ `docs`）、**代码风格调整**（→ `style`）、**重构**（→ `refactor`）等。
- 如果变更集中在某个模块或包内，可使用 `scope` 注明（例如 `feat(auth)`、`fix(api)`）。
- 请将较大的 diff 分解为多个**有意义的逻辑单元**，每个单元生成一条独立提交信息，确保最终输出至少 3 条, 最多 6 条。

## 返回示例
以下示例展示了一个符合所有要求的输出（注意：示例内容仅供参考，实际输出必须基于我提供的 diff）：

```json
[
  "feat(ui): add user login and logout page",
  "feat(login): add user login ui and unit test",
  "feat(auth): add login and logout api",
]
```

## 注意事项

- 请确保 JSON 格式正确（使用双引号、逗号分隔、无尾随逗号）
- 如果你无法从 diff 中提取出至少 3 个逻辑单元，可以适当拆分，但必须保证每条信息都真实反映所有 diff 的变更, 只是侧重点不同
- 所有提交信息必须严格小写，且长度 < 100 字符。
"##;

/// Replace the {{diff}} placeholder in the prompt with the diff content.
pub fn build_ai_prompt(diff: &str) -> String {
    AI_PROMPT_TEMPLATE.replace("{{diff}}", diff)
}

#[cfg(test)]
mod ai_test {
    use super::{build_ai_prompt, parse_llm_api_response};

    #[test]
    fn build_ai_prompt_replaces_placeholder() {
        let prompt = build_ai_prompt("@@ -1 +1 @@\n-feature\n+feature2\n");
        // placeholder has been replaced with the diff content
        assert!(prompt.contains("@@ -1 +1 @@\n-feature\n+feature2\n"));
        assert!(!prompt.contains("{{diff}}"));
        // template head and tail are preserved
        assert!(prompt.starts_with("## 角色与任务"));
        assert!(prompt.contains("根据下方提供的 **git diff 输出**"));
    }

    #[test]
    fn build_ai_prompt_with_empty_diff() {
        let prompt = build_ai_prompt("");
        assert!(!prompt.contains("{{diff}}"));
        assert!(prompt.contains("## 角色与任务"));
    }

    #[test]
    fn build_ai_prompt_multiple_occurrences() {
        // placeholder occurs once in the template, but even multiple occurrences should all be replaced
        let prompt = build_ai_prompt("a\nb\nc\n");
        assert_eq!(prompt.matches("{{diff}}").count(), 0);
        assert!(prompt.contains("a\nb\nc\n"));
    }

    #[test]
    fn parse_llm_api_response_direct_array() {
        let body = r#"["feat(ui): add login page","fix(api): fix auth bug"]"#;
        let result = parse_llm_api_response(body).unwrap();
        assert_eq!(
            result,
            vec![
                "feat(ui): add login page".to_string(),
                "fix(api): fix auth bug".to_string()
            ]
        );
    }

    #[test]
    fn parse_llm_api_response_openai_envelope() {
        let body = r#"{"choices":[{"message":{"content":"[\"feat(auth): add login api\",\"docs: update readme\"]"}}]}"#;
        let result = parse_llm_api_response(body).unwrap();
        assert_eq!(
            result,
            vec![
                "feat(auth): add login api".to_string(),
                "docs: update readme".to_string()
            ]
        );
    }

    #[test]
    fn parse_llm_api_response_invalid_json() {
        let body = "this is not json";
        let err = parse_llm_api_response(body).unwrap_err();
        assert_eq!(err.to_string(), "Failed to parse llm api response");
    }

    #[test]
    fn parse_llm_api_response_content_not_array() {
        // envelope exists but content is not an array
        let body = r#"{"choices":[{"message":{"content":"\"just a string\""}}]}"#;
        let err = parse_llm_api_response(body).unwrap_err();
        assert_eq!(err.to_string(), "Failed to parse llm api response");
    }

    #[test]
    fn parse_llm_api_response_array_of_non_strings() {
        // direct array but elements are not strings
        let body = r#"[1, 2, 3]"#;
        let err = parse_llm_api_response(body).unwrap_err();
        assert_eq!(err.to_string(), "Failed to parse llm api response");
    }

    #[test]
    fn parse_llm_api_response_envelope_content_invalid_json() {
        // content itself is not a valid JSON array
        let body = r#"{"choices":[{"message":{"content":"not-a-json"}}]}"#;
        let err = parse_llm_api_response(body).unwrap_err();
        assert_eq!(err.to_string(), "Failed to parse llm api response");
    }

    #[test]
    fn parse_llm_api_response_empty_array() {
        // an empty array should also parse successfully
        let body = r#"[]"#;
        let result = parse_llm_api_response(body).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn format_request_error_status_code() {
        assert_eq!(
            super::format_request_error(ureq::Error::StatusCode(401)),
            "AI request failed: HTTP 401"
        );
    }

    #[test]
    fn format_request_error_host_not_found() {
        assert_eq!(
            super::format_request_error(ureq::Error::HostNotFound),
            "AI request failed: could not resolve server hostname"
        );
    }

    #[test]
    fn format_request_error_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "refused");
        assert_eq!(
            super::format_request_error(ureq::Error::Io(io_err)),
            "AI request failed: network connection error"
        );
    }

    #[test]
    fn format_request_error_tls() {
        assert_eq!(
            super::format_request_error(ureq::Error::Tls("tls error")),
            "AI request failed: TLS connection error"
        );
    }

    #[test]
    fn format_request_error_bad_uri() {
        assert_eq!(
            super::format_request_error(ureq::Error::BadUri("bad".into())),
            "AI request failed: invalid API endpoint URL"
        );
    }

    #[test]
    fn format_request_error_connection_failed() {
        assert_eq!(
            super::format_request_error(ureq::Error::ConnectionFailed),
            "AI request failed: could not establish connection"
        );
    }

    #[test]
    fn format_request_error_fallback() {
        // unmapped variant (InvalidProxyUrl) falls back to Display
        let msg = super::format_request_error(ureq::Error::InvalidProxyUrl);
        assert!(msg.starts_with("AI request failed: "));
    }

    #[test]
    fn send_request_missing_config_returns_error() {
        // all-None config: must return an error before making a network request
        let err =
            super::send_request(crate::config::APIConfig::default(), "prompt".into()).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Missing API configuration (api_endpoint / api_token / model_name)"
        );
    }

    #[test]
    fn send_request_partial_config_returns_error() {
        // a missing token alone should also error
        let config = crate::config::APIConfig {
            api_endpoint: Some("https://example.com/v1".into()),
            api_token: None,
            model_name: Some("model-x".into()),
        };
        let err = super::send_request(config, "prompt".into()).unwrap_err();
        assert_eq!(
            err.to_string(),
            "Missing API configuration (api_endpoint / api_token / model_name)"
        );
    }
}
