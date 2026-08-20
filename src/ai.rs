use std::error::Error;
use std::path::Path;
use std::process::Command;

/// AI 提示词模板：用户提供的 markdown，{{diff}} 占位符由调用方替换。
/// 注意：raw string 用 r## 包裹（内容含双引号与反引号），禁止改动提示词原文。
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
  "feat: add user login endpoint",
  "fix(auth): correct token validation logic",
  "docs: update swagger api description"
]
```

## 注意事项

- 请确保 JSON 格式正确（使用双引号、逗号分隔、无尾随逗号）
- 如果你无法从 diff 中提取出至少 3 个逻辑单元，可以适当拆分，但必须保证每条信息都真实反映 diff 中的变更。
- 所有提交信息必须严格小写，且长度 < 100 字符。
"##;

/// 用 diff 内容替换提示词中的 {{diff}} 占位符。
pub fn build_ai_prompt(diff: &str) -> String {
    AI_PROMPT_TEMPLATE.replace("{{diff}}", diff)
}

/// 解析 LLM API 响应为提交信息候选列表。
/// 双层解析：
/// 1. 优先提取 OpenAI 兼容 envelope 的 choices[0].message.content，再把 content 解析为 Vec<String>；
/// 2. 回退：把整个响应体直接解析为 Vec<String>（兼容直接返回数组的 API）。
/// 任何一步失败返回 Err，消息统一为 "llm api response is not a json string"。
pub fn parse_llm_response(body: &str) -> Result<Vec<String>, Box<dyn Error>> {
    let parse_err = || -> Box<dyn Error> { "llm api response is not a json string".into() };

    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| parse_err())?;

    // 优先尝试 OpenAI 兼容 envelope：choices[0].message.content
    if let Some(content) = value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
    {
        return serde_json::from_str(content).map_err(|_| parse_err());
    }

    // 回退：响应体本身就是字符串数组
    serde_json::from_value(value).map_err(|_| parse_err())
}

/// 执行 `git diff --cached` 获取已暂存变更。
/// stdout 为空（无 staged changes）时报错，提示用户先 git add。
pub fn get_staged_diff(repo_path: &Path) -> Result<String, Box<dyn Error>> {
    let output = Command::new("git")
        .arg("diff")
        .arg("--cached")
        .current_dir(repo_path)
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "git diff --cached failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    if stdout.trim().is_empty() {
        return Err("No staged changes. Please 'git add' your files first.".into());
    }
    Ok(stdout)
}
