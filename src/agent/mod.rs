use chrono::{DateTime, Utc};
use crate::config::AgentConfig;
use crate::Cookie;

pub struct Agent {
    pub config: AgentConfig,
    pub cookies: Vec<Cookie>,
    pub domain: String,
}

#[derive(Debug)]
pub enum AgentLimit {
    NotLimited,
    Limited { reset_time: Option<DateTime<Utc>> },
}

impl Agent {
    pub fn new(config: AgentConfig, cookies: Vec<Cookie>, domain: String) -> Self {
        Self { config, cookies, domain }
    }

    pub fn command(&self) -> &str {
        &self.config.command
    }

    pub fn args(&self) -> &[String] {
        &self.config.args
    }

    pub async fn check_limit(&self) -> Result<AgentLimit, Box<dyn std::error::Error>> {
        match self.domain.as_str() {
            "claude.ai" => self.check_claude_limit().await,
            "github.com" => self.check_copilot_limit().await,
            _ => Err(format!("Unknown domain: {}", self.domain).into()),
        }
    }

    async fn check_claude_limit(&self) -> Result<AgentLimit, Box<dyn std::error::Error>> {
        let usage = crate::claude::ClaudeClient::fetch_usage(&self.cookies).await?;
        
        if let Some(reset_time) = usage.next_reset_time() {
            Ok(AgentLimit::Limited { reset_time: Some(reset_time) })
        } else {
            let is_limited = usage.five_hour
                .as_ref()
                .map(|w| w.utilization >= 100.0)
                .unwrap_or(false);
            
            if is_limited {
                Ok(AgentLimit::Limited { reset_time: None })
            } else {
                Ok(AgentLimit::NotLimited)
            }
        }
    }

    async fn check_copilot_limit(&self) -> Result<AgentLimit, Box<dyn std::error::Error>> {
        let quota = crate::copilot::CopilotClient::fetch_quota(&self.cookies).await?;
        
        if quota.is_limited() {
            Ok(AgentLimit::Limited { reset_time: quota.reset_time })
        } else {
            Ok(AgentLimit::NotLimited)
        }
    }

    pub fn execute(&self, extra_args: Vec<String>) -> std::process::ExitStatus {
        let mut cmd = std::process::Command::new(self.command());
        cmd.args(self.args());
        cmd.args(extra_args);
        cmd.status().expect("command failed")
    }
}
