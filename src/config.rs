use clap::Command;
use serde::Deserialize;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::{env, fs};

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
    let home_path = env::var("HOME")
        .map_err(|_| "Cannot determine home directory (HOME environment variable not set)")?;

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
        return Err(format!("Config file not found at {:?}", conf_path).into());
    }

    let content = fs::read_to_string(&conf_path)?;
    let config: APIConfig = toml::from_str(&content)
        .map_err(|e| format!("Failed to parse config file {:?}:\n{}", conf_path, e))?;
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

#[cfg(test)]
mod config_test {
    use super::{init_config, merge_config, APIConfig, DEFAULT_CONFIG_CONTENT};
    use std::fs;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    /// Each test uses its own temp dir to avoid parallel test conflicts
    fn unique_temp_dir() -> std::path::PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let seq = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("git-cz-test-{ts}-{seq}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn merge_config_overrides_all() {
        let config1 = APIConfig {
            api_endpoint: Some("https://old.com/v1".into()),
            api_token: Some("old-token".into()),
            model_name: Some("old-model".into()),
        };
        let config2 = APIConfig {
            api_endpoint: Some("https://new.com/v1".into()),
            api_token: Some("new-token".into()),
            model_name: Some("new-model".into()),
        };
        let result = merge_config(config1, config2);
        assert_eq!(result.api_endpoint.unwrap(), "https://new.com/v1");
        assert_eq!(result.api_token.unwrap(), "new-token");
        assert_eq!(result.model_name.unwrap(), "new-model");
    }

    #[test]
    fn merge_config_partial_overrides() {
        let config1 = APIConfig {
            api_endpoint: Some("https://keep.com/v1".into()),
            api_token: Some("keep-token".into()),
            model_name: Some("keep-model".into()),
        };
        let config2 = APIConfig {
            api_endpoint: None,
            api_token: Some("override-token".into()),
            model_name: None,
        };
        let result = merge_config(config1, config2);
        assert_eq!(result.api_endpoint.unwrap(), "https://keep.com/v1");
        assert_eq!(result.api_token.unwrap(), "override-token");
        assert_eq!(result.model_name.unwrap(), "keep-model");
    }

    #[test]
    fn merge_config_no_overrides() {
        let config1 = APIConfig {
            api_endpoint: Some("https://stable.com/v1".into()),
            api_token: Some("stable-token".into()),
            model_name: Some("stable-model".into()),
        };
        let config2 = APIConfig::default();
        let result = merge_config(config1, config2);
        assert_eq!(result.api_endpoint.unwrap(), "https://stable.com/v1");
        assert_eq!(result.api_token.unwrap(), "stable-token");
        assert_eq!(result.model_name.unwrap(), "stable-model");
    }

    #[test]
    fn merge_config_overrides_with_none_in_config1() {
        // config1 has some None fields, config2 provides values
        let config1 = APIConfig::default();
        let config2 = APIConfig {
            api_endpoint: Some("https://set.com/v1".into()),
            api_token: None,
            model_name: Some("set-model".into()),
        };
        let result = merge_config(config1, config2);
        assert_eq!(result.api_endpoint.unwrap(), "https://set.com/v1");
        assert!(result.api_token.is_none());
        assert_eq!(result.model_name.unwrap(), "set-model");
    }

    #[test]
    fn init_config_creates_file() {
        let dir = unique_temp_dir();
        let config_path = dir.join("config.toml");

        assert!(!config_path.exists());
        init_config(&config_path).unwrap();
        assert!(config_path.exists());

        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(content, DEFAULT_CONFIG_CONTENT);

        // cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn init_config_already_exists() {
        let dir = unique_temp_dir();
        let config_path = dir.join("config.toml");

        // create the file first with custom content
        let custom_content = "custom = \"content\"\n";
        fs::write(&config_path, custom_content).unwrap();
        assert!(config_path.exists());

        // re-init should not overwrite the existing file
        init_config(&config_path).unwrap();
        let content = fs::read_to_string(&config_path).unwrap();
        assert_eq!(
            content, custom_content,
            "existing file should not be overwritten"
        );

        // cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn default_config_has_expected_fields() {
        // verify the default config content is valid TOML
        let config: APIConfig = toml::from_str(DEFAULT_CONFIG_CONTENT).unwrap();
        assert!(config.api_endpoint.is_some());
        assert!(config.api_token.is_some());
        assert!(config.model_name.is_some());
        assert_eq!(
            config.api_endpoint.unwrap(),
            "https://api.deepseek.com/v1/chat/completions"
        );
    }

    /// HOME environment guard: sets HOME, restores the original value and cleans up the temp dir on drop
    struct HomeGuard {
        old: Option<std::ffi::OsString>,
        dir: std::path::PathBuf,
    }

    impl HomeGuard {
        fn new(dir: &std::path::Path) -> Self {
            let old = std::env::var_os("HOME");
            std::env::set_var("HOME", dir);
            Self {
                old,
                dir: dir.to_path_buf(),
            }
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match &self.old {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn load_config_errors_return_english_messages() {
        let dir = unique_temp_dir();
        let _guard = HomeGuard::new(&dir);

        // Scenario 1: config file does not exist -> config file not found
        let err = super::load_config().unwrap_err();
        assert!(err.to_string().contains("Config file not found at"));

        // Scenario 2: config file exists but TOML is invalid -> failed to parse config file
        let config_file = dir.join(".config/git-cz/config.toml");
        std::fs::create_dir_all(config_file.parent().unwrap()).unwrap();
        std::fs::write(&config_file, "not = = valid toml {{{").unwrap();
        let err = super::load_config().unwrap_err();
        assert!(err.to_string().contains("Failed to parse config file"));
    }
}
