use chrono::{DateTime, NaiveDate, Utc};
use serde::Deserialize;
use crate::Cookie;

#[derive(Debug, Deserialize)]
pub struct QuotaRemaining {
    #[serde(rename = "chatPercentage")]
    pub chat_percentage: f64,
    #[serde(rename = "premiumInteractionsPercentage")]
    pub premium_interactions_percentage: f64,
}

#[derive(Debug, Deserialize)]
pub struct Quotas {
    pub remaining: QuotaRemaining,
    #[serde(rename = "resetDate")]
    pub reset_date: String,
}

#[derive(Debug, Deserialize)]
pub struct CopilotQuotaResponse {
    pub quotas: Quotas,
}

#[derive(Debug)]
pub struct CopilotQuota {
    pub chat_utilization: f64,
    pub premium_utilization: f64,
    pub reset_time: Option<DateTime<Utc>>,
}

impl CopilotQuota {
    pub fn is_limited(&self) -> bool {
        self.chat_utilization >= 100.0 || self.premium_utilization >= 100.0
    }

    pub fn next_reset_time(&self) -> Option<DateTime<Utc>> {
        self.reset_time
    }
}

const USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) \
    AppleWebKit/537.36 (KHTML, like Gecko) Chrome/133.0.0.0 Safari/537.36";

pub struct CopilotClient;

impl CopilotClient {
    pub async fn fetch_quota(cookies: &[Cookie]) -> Result<CopilotQuota, Box<dyn std::error::Error>> {
        let cookie_header = Self::build_cookie_header(cookies);

        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .build()?;

        let response = client
            .get("https://github.com/github-copilot/chat")
            .header("Cookie", &cookie_header)
            .header("User-Agent", USER_AGENT)
            .header("github-verified-fetch", "true")
            .header("x-requested-with", "XMLHttpRequest")
            .header("accept", "application/json")
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await?;
            return Err(format!("GitHub Copilot API error: {} - {}", status, body).into());
        }

        let quota_response: CopilotQuotaResponse = response.json().await?;
        let quotas = quota_response.quotas;

        let _now = Utc::now();
        let reset_time = NaiveDate::parse_from_str(&quotas.reset_date, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| {
                let reset_utc = dt.and_utc();
                reset_utc
            });

        let chat_utilization = 100.0 - quotas.remaining.chat_percentage;
        let premium_utilization = 100.0 - quotas.remaining.premium_interactions_percentage;

        Ok(CopilotQuota {
            chat_utilization,
            premium_utilization,
            reset_time,
        })
    }

    fn build_cookie_header(cookies: &[Cookie]) -> String {
        cookies
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ")
    }
}
