use clap::Command;
use serde::Deserialize;
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

/// Config
#[derive(Debug, Default, Deserialize)]
pub struct APIConfig {
    pub api_endpoint: Option<String>,
    pub api_token: Option<String>,
    pub model_name: Option<String>,
}

// create init config command
pub fn create_init_config_cmd() -> Command {
    Command::new("init-config").about("init default git-cz config")
}

// init config
pub fn handler() -> Result<(), Box<dyn Error>> {
    let config_path = get_config_path()?;
    init_config(&config_path)
}

// config path is ~/.config/git-cz/config.toml
pub fn get_config_path() -> Result<PathBuf, Box<dyn Error>> {
    // ensure HOME env var exists
    let home_path = env::var("HOME").map_err(|_| "Cannot determine home directory")?;

    // ~/.config
    let conf_path = PathBuf::from(home_path).join(".config");
    if !conf_path.exists() {
        fs::create_dir_all(&conf_path)?;
    }

    // ~/.config/git-cz
    let gitcz_path = conf_path.join("git-cz");
    if !gitcz_path.exists() {
        fs::create_dir_all(&gitcz_path)?;
    }

    // ~/.config/git-cz/config.toml
    Ok(gitcz_path.join("config.toml"))
}

/// default config
pub const DEFAULT_CONFIG_CONTENT: &str = r#"
api_endpoint = "https://api.deepseek.com/v1/chat/completions"
api_token = "sk-your-token-string"
model_name = "deepseek-v4-flash"
"#;

// init config to ~/.config/git-cz/config.toml
pub fn init_config(path: &Path) -> Result<(), Box<dyn Error>> {
    if path.exists() {
        println!("Config file already exists({})", path.display());
        return Ok(());
    }

    fs::write(path, DEFAULT_CONFIG_CONTENT)?;

    println!("Config file created({})", path.display());

    Ok(())
}

// load config from ~/.config/git-cz/config.toml
pub fn load_config() -> Result<APIConfig, Box<dyn Error>> {
    let conf_path = get_config_path()?;

    if !conf_path.exists() {
        return Err(format!(
            "Config not found at {}, please init-config first",
            conf_path.display()
        )
        .into());
    }

    let content = fs::read_to_string(&conf_path)?;
    let config: APIConfig = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse config {}:\n{}", conf_path.display(), e))?;
    Ok(config)
}

// merge config2 to config1
pub fn merge_config(mut config1: APIConfig, config2: APIConfig) -> APIConfig {
    if config2.api_endpoint.is_some() {
        config1.api_endpoint = config2.api_endpoint;
    }
    if config2.api_token.is_some() {
        config1.api_token = config2.api_token;
    }
    if config2.model_name.is_some() {
        config1.model_name = config2.model_name;
    }
    config1
}
