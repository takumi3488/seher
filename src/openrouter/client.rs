use super::types::CreditsResponse;

pub struct OpenRouterClient;

impl OpenRouterClient {
    pub async fn fetch_credits(
        _management_key: &str,
    ) -> Result<CreditsResponse, Box<dyn std::error::Error>> {
        Err("OpenRouter Credits API client not yet implemented".into())
    }
}
