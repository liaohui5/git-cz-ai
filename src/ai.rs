use clap::{Arg, ArgMatches, Command};
use inquire::{InquireError, Select};
use std::error;
use ureq;

use crate::{
    config::{self, APIConfig},
    git,
    loading::LoadingSpinner,
};

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
    let commit_message = select_commit_message(result);
    git::perform_commit(&commit_message)
}

pub fn send_request(config: APIConfig, prompt: String) -> Result<String, Box<dyn error::Error>> {
    let mut loading_spinner = LoadingSpinner::default();
    loading_spinner.start("Request has been sent, waiting for response...");

    let mut response = ureq::post(&config.api_endpoint.unwrap())
        .header(
            "Authorization",
            &format!("Bearer {}", config.api_token.unwrap()),
        )
        .header("Content-Type", "application/json")
        .send_json(serde_json::json!({
            "model": &config.model_name.unwrap(),
            "messages": [{ "role": "user", "content": prompt }],
        }))
        .unwrap();

    loading_spinner.stop();
    let result = response.body_mut().read_to_string();
    if result.is_err() {
        return result.map_err(|_| "Failed to read response body".into());
    }
    Ok(result.unwrap())
}

pub fn select_commit_message(messages: Vec<String>) -> String {
    let messages: Vec<&str> = messages.iter().map(|m| m.as_str()).collect();
    let len = messages.len();

    let commit_message: Result<&str, InquireError> =
        Select::new("Select commit message by AI generated", messages)
            .with_page_size(len)
            .prompt();

    match commit_message {
        Ok(message) => String::from(message),
        Err(_) => String::new(),
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

/// AI 提示词模板：用户提供的 markdown, {{diff}} 占位符由调用时替换
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

/// 用 diff 内容替换提示词中的 {{diff}} 占位符。
pub fn build_ai_prompt(diff: &str) -> String {
    AI_PROMPT_TEMPLATE.replace("{{diff}}", diff)
}
