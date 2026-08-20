use serde::Deserialize;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// 默认配置文件内容（用户规格原文，禁止改写）。
pub const DEFAULT_CONFIG_CONTENT: &str = "api_endpoint=\"https://api.deepseek.com/v1/chat/completions\"\n\
api_token=\"sk-your-token-string\"\n\
model_name=\"deepseek-v4-flash\"\n";

/// 配置文件结构；缺失字段为 None。
#[derive(Debug, Default, Deserialize)]
pub struct Config {
    pub api_endpoint: Option<String>,
    pub api_token: Option<String>,
    pub model_name: Option<String>,
}

/// 返回 `~/.config/git-cz/config.toml`；`$XDG_CONFIG_HOME` 未设时回退 `~/.config`；
/// 取不到 home 时报错。
pub fn config_path() -> Result<PathBuf, Box<dyn Error>> {
    let base = match env::var("XDG_CONFIG_HOME") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => {
            let home = env::var("HOME").map_err(|_| "Cannot determine home directory")?;
            PathBuf::from(home).join(".config")
        }
    };
    Ok(base.join("git-cz").join("config.toml"))
}

/// 加载配置文件；文件不存在返回全 None 的 Config（不报错）。
/// 解析失败返回 Err，消息含路径与解析错误。
pub fn load_config(path: &Path) -> Result<Config, Box<dyn Error>> {
    if !path.exists() {
        return Ok(Config::default());
    }
    let content = fs::read_to_string(path)?;
    let config: Config = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse config {}: {}", path.display(), e))?;
    Ok(config)
}

/// 初始化配置文件。已存在返回 Ok(false)（不动文件）；
/// 不存在则创建父目录并写入默认内容，返回 Ok(true)。
pub fn init_config(path: &Path) -> Result<bool, Box<dyn Error>> {
    if path.exists() {
        return Ok(false);
    }
    let dir = path.parent().ok_or("Config path has no parent directory")?;
    fs::create_dir_all(dir)?;
    fs::write(path, DEFAULT_CONFIG_CONTENT)?;
    Ok(true)
}

/// 合并后的 AI 请求参数（三字段均为确定值）。
#[derive(Debug, PartialEq)]
pub struct ResolvedAiArgs {
    pub api_endpoint: String,
    pub api_token: String,
    pub model_name: String,
}

/// 按「CLI（含 clap env 回退后的值）> config」合并三字段；
/// 任一字段三处均缺时返回 Err(缺失的 CLI 参数名列表，如 "--api-endpoint")。
pub fn resolve_ai_args(
    cli_api_endpoint: Option<String>,
    cli_api_token: Option<String>,
    cli_model_name: Option<String>,
    config: &Config,
) -> Result<ResolvedAiArgs, Vec<String>> {
    let api_endpoint = cli_api_endpoint.or_else(|| config.api_endpoint.clone());
    let api_token = cli_api_token.or_else(|| config.api_token.clone());
    let model_name = cli_model_name.or_else(|| config.model_name.clone());

    let mut missing = Vec::new();
    if api_endpoint.is_none() {
        missing.push("--api-endpoint".to_string());
    }
    if api_token.is_none() {
        missing.push("--api-token".to_string());
    }
    if model_name.is_none() {
        missing.push("--model-name".to_string());
    }
    if !missing.is_empty() {
        return Err(missing);
    }

    Ok(ResolvedAiArgs {
        api_endpoint: api_endpoint.unwrap(),
        api_token: api_token.unwrap(),
        model_name: model_name.unwrap(),
    })
}
