use serde::{Deserialize, Serialize};
use reqwest::Client;
use log::{debug, error};
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchProvider {
    Google,
    Bing,
}

#[derive(Debug)]
pub struct WebSearchResult {
    pub title: String,
    pub snippet: String,
    pub url: String,
}

pub struct WebSearchClient {
    google_api_key: Option<String>,
    google_cx: Option<String>,  // Google Custom Search Engine ID
    bing_api_key: Option<String>,
    client: Client,
}

impl WebSearchClient {
    pub fn new(
        google_api_key: Option<String>,
        google_cx: Option<String>,
        bing_api_key: Option<String>,
    ) -> Self {
        Self {
            google_api_key,
            google_cx,
            bing_api_key,
            client: Client::new(),
        }
    }

    pub async fn search(
        &self,
        query: &str,
        provider: SearchProvider,
        num_results: usize,
    ) -> Result<Vec<WebSearchResult>, Box<dyn Error + Send + Sync>> {
        match provider {
            SearchProvider::Google => self.google_search(query, num_results).await,
            SearchProvider::Bing => self.bing_search(query, num_results).await,
        }
    }

    async fn google_search(
        &self,
        query: &str,
        num_results: usize,
    ) -> Result<Vec<WebSearchResult>, Box<dyn Error + Send + Sync>> {
        let api_key = self.google_api_key.as_ref()
            .ok_or("Google API key not configured")?;
        let cx = self.google_cx.as_ref()
            .ok_or("Google Custom Search Engine ID not configured")?;

        debug!("Performing Google search for query: {}", query);

        let response = self.client
            .get("https://www.googleapis.com/customsearch/v1")
            .query(&[
                ("key", api_key.as_str()),
                ("cx", cx.as_str()),
                ("q", query),
                ("num", &num_results.to_string()),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            error!("Google search API error: {}", error_text);
            return Err(format!("Google API error: {}", error_text).into());
        }

        let json: serde_json::Value = response.json().await?;

        let results = json["items"]
            .as_array()
            .ok_or("No items field in response")?
            .iter()
            .filter_map(|item| {
                Some(WebSearchResult {
                    title: item["title"].as_str()?.to_string(),
                    snippet: item["snippet"].as_str()?.to_string(),
                    url: item["link"].as_str()?.to_string(),
                })
            })
            .collect();

        Ok(results)
    }

    async fn bing_search(
        &self,
        query: &str,
        num_results: usize,
    ) -> Result<Vec<WebSearchResult>, Box<dyn Error + Send + Sync>> {
        let api_key = self.bing_api_key.as_ref()
            .ok_or("Bing API key not configured")?;

        debug!("Performing Bing search for query: {}", query);

        let response = self.client
            .get("https://api.bing.microsoft.com/v7.0/search")
            .header("Ocp-Apim-Subscription-Key", api_key)
            .query(&[
                ("q", query.to_string()),
                ("count", num_results.to_string()),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            error!("Bing search API error: {}", error_text);
            return Err(format!("Bing API error: {}", error_text).into());
        }

        let json: serde_json::Value = response.json().await?;

        let results = json["webPages"]["value"]
            .as_array()
            .ok_or("No webPages.value field in response")?
            .iter()
            .filter_map(|item| {
                Some(WebSearchResult {
                    title: item["name"].as_str()?.to_string(),
                    snippet: item["snippet"].as_str()?.to_string(),
                    url: item["url"].as_str()?.to_string(),
                })
            })
            .collect();

        Ok(results)
    }

    pub fn format_results(results: &[WebSearchResult]) -> String {
        results.iter()
            .enumerate()
            .map(|(i, result)| {
                format!(
                    "{}. {}\n   {}\n   Source: {}\n",
                    i + 1,
                    result.title,
                    result.snippet,
                    result.url
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}