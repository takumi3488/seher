use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub agents: Vec<AgentConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AgentConfig {
    pub command: String,
    pub args: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            agents: vec![AgentConfig {
                command: "claude".to_string(),
                args: vec![],
            }],
        }
    }
}

impl Settings {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let path = Self::settings_path()?;
        if !path.exists() {
            return Ok(Settings::default());
        }
        let content = std::fs::read_to_string(&path)?;
        let settings: Settings = serde_json::from_str(&content)?;
        Ok(settings)
    }

    fn settings_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
        let home = dirs::home_dir().ok_or("HOME directory not found")?;
        Ok(home.join(".seher").join("settings.json"))
    }
}
