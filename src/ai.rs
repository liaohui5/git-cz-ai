use clap::{Arg, ArgMatches, Command};
use inquire::{InquireError, Select};
use std::error;
use ureq;

use crate::{
    config::{self, APIConfig},
    git,
    loading::LoadingSpinner,
};

/// Create ai sub command
pub fn create_ai_cmd() -> Command {
    Command::new("ai")
        .about("Auto generate commit messages by llm api")
        .arg(optional_arg("api-endpoint", "Set the api url"))
        .arg(optional_arg("api-token", "Set the api access token"))
        .arg(optional_arg("model-name", "Set the model"))
}

/// An optional `--<name>` flag that takes a single value.
fn optional_arg(name: &'static str, help: &'static str) -> Arg {
    Arg::new(name)
        .long(name)
        .help(help)
        .required(false)
        .num_args(1)
}

/// ai sub command handler
pub fn handler(args: &ArgMatches) -> Result<(), Box<dyn error::Error>> {
    let file_config: APIConfig = config::load_config()?;
    let args_config = parse_args_to_config(args);
    let merged_config = config::merge_config(file_config, args_config); // CLI args override the file config

    let diff = git::get_staged_diff()?;
    let prompt = build_ai_prompt(&diff);
    let response_body = send_request(merged_config, prompt)?;
    let candidates = parse_llm_api_response(&response_body)?;
    match select_commit_message(candidates) {
        Some(message) => git::perform_commit(&message),
        None => {
            // Ctrl-C / Esc: user cancelled, not an error, skip the commit
            println!("Commit aborted.");
            Ok(())
        }
    }
}

pub fn send_request(config: APIConfig, prompt: String) -> Result<String, Box<dyn error::Error>> {
    // Return an error if any config field is missing, without making a network request
    let api_endpoint = config
        .api_endpoint
        .ok_or("Missing API config field (api_endpoint)")?;

    let api_token = config
        .api_token
        .ok_or("Missing API config field (api_token)")?;

    let model_name = config
        .model_name
        .ok_or("Missing API config field (model_name)")?;

    let mut loading_spinner = LoadingSpinner::default();
    loading_spinner.start("Request has been sent, please wait...");

    // Send the request
    let result = ureq::post(&api_endpoint)
        .header("Authorization", &format!("Bearer {api_token}"))
        .header("Content-Type", "application/json")
        .send_json(serde_json::json!({
            "model": model_name,
            "messages": [{ "role": "user", "content": prompt }],
        }));

    // Stop the spinner before returning the error on request failure
    loading_spinner.stop();
    if let Err(e) = result {
        return Err(format_request_error(e).into());
    }

    result
        .unwrap()
        .body_mut()
        .read_to_string()
        .map_err(|_| "Failed to read response body".into())
}

/// Map ureq request errors to brief english reasons (ureq::Error is #[non_exhaustive], needs a catch-all arm)
fn format_request_error(e: ureq::Error) -> String {
    let reason = match e {
        ureq::Error::StatusCode(code) => format!("HTTP {code}"),
        ureq::Error::HostNotFound => "could not resolve server hostname".to_string(),
        ureq::Error::Io(_) => "network connection error".to_string(),
        ureq::Error::Timeout(_) => "request timed out".to_string(),
        ureq::Error::Tls(_) => "TLS connection error".to_string(),
        ureq::Error::BadUri(_) => "invalid API endpoint URL".to_string(),
        ureq::Error::ConnectionFailed => "could not establish connection".to_string(),
        ureq::Error::TooManyRedirects => "too many redirects".to_string(),
        ureq::Error::RedirectFailed => "redirect failed".to_string(),
        ureq::Error::BodyExceedsLimit(_) => "request body too large".to_string(),
        _ => e.to_string(),
    };
    format!("AI request failed: {reason}")
}

pub fn select_commit_message(messages: Vec<String>) -> Option<String> {
    let options: Vec<&str> = messages.iter().map(|m| m.as_str()).collect();
    let page_size = options.len();

    let commit_message: Result<&str, InquireError> =
        Select::new("Select commit message by AI generated", options)
            .with_page_size(page_size)
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
    APIConfig {
        api_endpoint: args.get_one::<String>("api-endpoint").map(|s| s.to_owned()),
        api_token: args.get_one::<String>("api-token").map(|s| s.to_owned()),
        model_name: args.get_one::<String>("model-name").map(|s| s.to_owned()),
    }
}

/// AI prompt template: user-provided markdown, {{diff}} placeholder replaced at call time
const AI_PROMPT_TEMPLATE: &str = r##"## Role and Task
You are a professional Git commit message generator.  
Based on the **git diff output** (i.e., file changes) provided below, generate a set of commit messages that comply with the [Conventional Commits v1.0.0](https://www.conventionalcommits.org/en/v1.0.0/) specification.  
Each commit message should correspond to **one logically independent change unit** (e.g., new feature, bug fix, documentation update, etc.), rather than mechanically splitting the entire diff into line‑by‑line records.

## Input: File Changes
The following is the output of the `git diff` command, which is the sole basis for generating your commit messages: {{diff}}

## Output Format (must be strictly followed)
- Return a **valid JSON string**.
- The top‑level structure of the JSON must be a **string array** containing at least **3 elements**.
- **Do not output any extra text, comments, or explanations** besides the JSON string, so that subsequent programs can parse it directly.

## Requirements for Each Commit Message
1. **Format specification** (strictly follow `<type>[optional scope]: <description>`):
   - `<type>` is required and must be one of the following nouns (common types): `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `chore`.
   - `[optional scope]` is optional; if used, it must be wrapped in English parentheses, e.g., `feat(parser)`.
   - The colon `:` **must be immediately followed by a single English space**.
2. **Language and case**: The entire commit message (including type, scope, and description) **must be written entirely in lowercase English characters**.
3. **Length limit**: The total character count of each commit message (**excluding the quotation marks around the array element**) **must be less than 100**.
4. **Content quality**: The description should be **concise and accurate**, clearly summarising the change, avoiding vague or generic phrasing.

## How to Derive Commit Messages from the Diff (Guiding Principles)
- Analyse the file paths and code changes in the diff to identify **new features** (→ `feat`), **bug fixes** (→ `fix`), **documentation changes** (→ `docs`), **code style adjustments** (→ `style`), **refactoring** (→ `refactor`), etc.
- If changes are concentrated within a certain module or package, you may use a `scope` to indicate it (e.g., `feat(auth)`, `fix(api)`).
- Break down large diffs into multiple **meaningful logical units**, each generating one independent commit message, ensuring the final output has at least 3 and at most 6 messages.

## Example Return
The following example shows an output that meets all requirements (note: the example is for reference only; the actual output must be based on the diff provided to you):

```json
[
  "feat(ui): add user login and logout page",
  "feat(login): add user login ui and unit test",
  "feat(auth): add login and logout api"
]
```

## Important Notes

- Ensure the JSON format is correct (use double quotes, comma separation, no trailing comma).
- If you cannot extract at least 3 logical units from the diff, you may split appropriately, but you must guarantee that each message truthfully reflects the changes in the diff, only with different emphasis.
- All commit messages must be strictly lowercase and under 100 characters.

# Format requirements

- must be outupt `json`
- *The output must adhere to the aforementioned requirements and be in a well-formatted JSON format*
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
        assert!(prompt.starts_with("## Role and Task"));
        assert!(prompt.contains("The following is the output of the `git diff` command, which is the sole basis for generating your commit messages"));
    }

    #[test]
    fn build_ai_prompt_with_empty_diff() {
        let prompt = build_ai_prompt("");
        assert!(!prompt.contains("{{diff}}"));
        assert!(prompt.contains("## Role and Task"));
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
    fn format_request_error_maps_variants() {
        let cases: Vec<(ureq::Error, &str)> = vec![
            (ureq::Error::StatusCode(401), "AI request failed: HTTP 401"),
            (
                ureq::Error::HostNotFound,
                "AI request failed: could not resolve server hostname",
            ),
            (
                ureq::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::ConnectionRefused,
                    "refused",
                )),
                "AI request failed: network connection error",
            ),
            (
                ureq::Error::Tls("tls error"),
                "AI request failed: TLS connection error",
            ),
            (
                ureq::Error::BadUri("bad".into()),
                "AI request failed: invalid API endpoint URL",
            ),
            (
                ureq::Error::ConnectionFailed,
                "AI request failed: could not establish connection",
            ),
        ];
        for (err, expected) in cases {
            assert_eq!(super::format_request_error(err), expected);
        }
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
        assert_eq!(err.to_string(), "Missing API config field (api_endpoint)");
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
        assert_eq!(err.to_string(), "Missing API config field (api_token)");
    }
}
