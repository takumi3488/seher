use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct UsageWindow {
    pub utilization: f64,
    pub resets_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UsageResponse {
    pub five_hour: Option<UsageWindow>,
    pub seven_day: Option<UsageWindow>,
    pub seven_day_sonnet: Option<UsageWindow>,
}

impl UsageResponse {
    pub fn next_reset_time(&self) -> Option<chrono::DateTime<Utc>> {
        [self.five_hour.as_ref(), self.seven_day.as_ref()]
            .into_iter()
            .flatten()
            .filter(|w| w.utilization >= 100.0)
            .map(|w| w.resets_at)
            .max()
    }
}
