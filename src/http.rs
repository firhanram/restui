use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use reqwest::{Client, StatusCode, header::CONTENT_TYPE};

use crate::model::RequestConfig;

#[derive(Clone, Debug)]
pub struct HttpResponse {
    pub status: StatusCode,
    pub elapsed: Duration,
    pub bytes: usize,
    pub body: String,
    pub display_body: String,
    pub is_json: bool,
}

pub async fn execute(config: RequestConfig) -> Result<HttpResponse> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .user_agent(concat!("restui/", env!("CARGO_PKG_VERSION")))
        .build()
        .context("could not create HTTP client")?;

    let request = build_request(&client, &config)?;
    let started = Instant::now();
    let response = request.send().await.context("request failed")?;
    let status = response.status();
    let bytes = response
        .bytes()
        .await
        .context("could not read response body")?;
    let body = String::from_utf8_lossy(&bytes).into_owned();
    let (display_body, is_json) = format_response_body(&body);

    Ok(HttpResponse {
        status,
        elapsed: started.elapsed(),
        bytes: bytes.len(),
        body,
        display_body,
        is_json,
    })
}

fn build_request(client: &Client, config: &RequestConfig) -> Result<reqwest::RequestBuilder> {
    let method = reqwest::Method::from_bytes(config.method.as_str().as_bytes())?;
    let mut request = client.request(method, &config.url);
    let query: Vec<(&str, &str)> = config
        .params
        .iter()
        .filter(|param| param.enabled && !param.key.is_empty())
        .map(|param| (param.key.as_str(), param.value.as_str()))
        .collect();
    if !query.is_empty() {
        request = request.query(&query);
    }
    if config.method.supports_body() && !config.body.is_empty() {
        request = request
            .header(CONTENT_TYPE, "application/json")
            .body(config.body.clone());
    }
    Ok(request)
}

pub fn format_response_body(body: &str) -> (String, bool) {
    match serde_json::from_str::<serde_json::Value>(body) {
        Ok(value) => (
            serde_json::to_string_pretty(&value).unwrap_or_else(|_| body.to_string()),
            true,
        ),
        Err(_) => (body.to_string(), false),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::model::{Method, QueryParam};

    #[test]
    fn builds_get_query_params_and_ignores_disabled_params() {
        let client = Client::new();
        let config = RequestConfig {
            id: uuid::Uuid::new_v4(),
            name: "Search".into(),
            method: Method::Get,
            url: "https://example.test/search".into(),
            params: vec![
                QueryParam::new("q".into(), "rust lang".into()),
                QueryParam {
                    key: "debug".into(),
                    value: "true".into(),
                    enabled: false,
                },
            ],
            body: "ignored".into(),
        };

        let request = build_request(&client, &config).unwrap().build().unwrap();
        assert_eq!(
            request.url().as_str(),
            "https://example.test/search?q=rust+lang"
        );
        assert!(request.body().is_none());
    }

    #[test]
    fn attaches_json_body_for_write_methods() {
        let client = Client::new();
        let config = RequestConfig {
            id: uuid::Uuid::new_v4(),
            name: "Create".into(),
            method: Method::Post,
            url: "https://example.test/items".into(),
            params: Vec::new(),
            body: json!({"name": "example"}).to_string(),
        };

        let request = build_request(&client, &config).unwrap().build().unwrap();
        assert_eq!(request.headers()[CONTENT_TYPE], "application/json");
        assert_eq!(
            request.body().unwrap().as_bytes().unwrap(),
            config.body.as_bytes()
        );
    }

    #[test]
    fn pretty_prints_json_without_changing_plain_text() {
        let (formatted, is_json) = format_response_body("{\"ok\":true}");
        assert!(is_json);
        assert_eq!(formatted, "{\n  \"ok\": true\n}");

        let (plain, is_json) = format_response_body("not json");
        assert!(!is_json);
        assert_eq!(plain, "not json");
    }
}
