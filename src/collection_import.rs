use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde_json::Value;
use uuid::Uuid;

use crate::model::{Method, QueryParam, RequestConfig};

#[derive(Debug)]
pub struct ImportedCollection {
    pub name: String,
    pub requests: Vec<RequestConfig>,
}

pub fn import_collection(path: &Path) -> Result<ImportedCollection> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("could not read collection {}", path.display()))?;
    import_collection_str(&contents).with_context(|| format!("could not import {}", path.display()))
}

pub fn import_collection_str(contents: &str) -> Result<ImportedCollection> {
    let collection: Value =
        serde_json::from_str(contents).context("collection is not valid JSON")?;
    let name = collection
        .get("info")
        .and_then(|info| info.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("Imported Collection")
        .to_string();
    let items = collection
        .get("item")
        .and_then(Value::as_array)
        .context("collection has no item array")?;

    let mut requests = Vec::new();
    read_items(items, "", &mut requests)?;
    if requests.is_empty() {
        anyhow::bail!("collection contains no importable requests");
    }

    Ok(ImportedCollection { name, requests })
}

fn read_items(items: &[Value], folder: &str, output: &mut Vec<RequestConfig>) -> Result<()> {
    for item in items {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Request");
        let qualified_name = if folder.is_empty() {
            name.to_string()
        } else {
            format!("{folder} / {name}")
        };

        if let Some(children) = item.get("item").and_then(Value::as_array) {
            read_items(children, &qualified_name, output)?;
            continue;
        }

        if let Some(request) = item.get("request") {
            output.push(read_request(request, qualified_name)?);
        }
    }
    Ok(())
}

fn read_request(request: &Value, name: String) -> Result<RequestConfig> {
    if let Some(raw_url) = request.as_str() {
        let mut params = Vec::new();
        let mut url = raw_url.to_string();
        if let Some((base, query)) = raw_url.split_once('?') {
            params = url::form_urlencoded::parse(query.as_bytes())
                .map(|(key, value)| QueryParam::new(key.into_owned(), value.into_owned()))
                .collect();
            if !params.is_empty() {
                url = base.to_string();
            }
        }
        return Ok(RequestConfig {
            id: Uuid::new_v4(),
            name,
            method: Method::Get,
            url,
            params,
            body: String::new(),
        });
    }

    let method = request
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("GET")
        .parse()?;
    let url_value = request.get("url").context("request has no URL")?;
    let raw_url = match url_value {
        Value::String(url) => url.clone(),
        Value::Object(_) => url_value
            .get("raw")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        _ => anyhow::bail!("request URL is neither a string nor an object"),
    };

    let mut params = Vec::new();
    if let Some(query) = url_value.get("query").and_then(Value::as_array) {
        for param in query {
            params.push(QueryParam {
                key: param
                    .get("key")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                value: param
                    .get("value")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                enabled: !param
                    .get("disabled")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            });
        }
    } else if let Some((_, query)) = raw_url.split_once('?') {
        params = url::form_urlencoded::parse(query.as_bytes())
            .map(|(key, value)| QueryParam::new(key.into_owned(), value.into_owned()))
            .collect();
    }

    let url = if url_value.get("query").is_some() {
        raw_url
            .split_once('?')
            .map_or(raw_url.clone(), |(base, _)| base.to_string())
    } else if !params.is_empty() {
        raw_url
            .split_once('?')
            .map_or(raw_url.clone(), |(base, _)| base.to_string())
    } else {
        raw_url
    };

    let body = request
        .get("body")
        .and_then(|body| body.get("raw"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    Ok(RequestConfig {
        id: Uuid::new_v4(),
        name,
        method,
        url,
        params,
        body,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_nested_requests_params_and_raw_bodies() {
        let collection = r##"{
            "info": {"name": "Example API", "schema": "https://schema.example/collection.json"},
            "item": [{
                "name": "Users",
                "item": [{
                    "name": "Update user",
                    "request": {
                        "method": "PATCH",
                        "url": {
                            "raw": "{{baseUrl}}/users/1?ignored=yes",
                            "query": [
                                {"key": "notify", "value": "true"},
                                {"key": "debug", "value": "1", "disabled": true}
                            ]
                        },
                        "body": {"mode": "raw", "raw": "{\"name\":\"Ada\"}"}
                    }
                }]
            }]
        }"##;

        let imported = import_collection_str(collection).unwrap();
        assert_eq!(imported.name, "Example API");
        assert_eq!(imported.requests.len(), 1);
        let request = &imported.requests[0];
        assert_eq!(request.name, "Users / Update user");
        assert_eq!(request.method, Method::Patch);
        assert_eq!(request.url, "{{baseUrl}}/users/1");
        assert_eq!(request.params.len(), 2);
        assert!(request.params[0].enabled);
        assert!(!request.params[1].enabled);
        assert_eq!(request.body, "{\"name\":\"Ada\"}");
    }

    #[test]
    fn imports_string_urls_and_query_strings() {
        let collection = r#"{
            "info": {"name": "Simple"},
            "item": [{"name": "Search", "request": "https://example.test/search?q=rust&page=2"}]
        }"#;

        let imported = import_collection_str(collection).unwrap();
        let request = &imported.requests[0];
        assert_eq!(request.method, Method::Get);
        assert_eq!(request.url, "https://example.test/search");
        assert_eq!(
            request.params[0],
            QueryParam::new("q".into(), "rust".into())
        );
        assert_eq!(
            request.params[1],
            QueryParam::new("page".into(), "2".into())
        );
    }

    #[test]
    fn rejects_collections_without_requests() {
        let error = import_collection_str(r#"{"info":{"name":"Empty"},"item":[]}"#)
            .unwrap_err()
            .to_string();
        assert!(error.contains("no importable requests"));
    }
}
