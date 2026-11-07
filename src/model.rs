use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Method {
    #[default]
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Head,
    Options,
}

impl Method {
    pub const ALL: [Self; 7] = [
        Self::Get,
        Self::Post,
        Self::Put,
        Self::Patch,
        Self::Delete,
        Self::Head,
        Self::Options,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Put => "PUT",
            Self::Patch => "PATCH",
            Self::Delete => "DELETE",
            Self::Head => "HEAD",
            Self::Options => "OPTIONS",
        }
    }

    pub fn supports_body(self) -> bool {
        matches!(self, Self::Post | Self::Put | Self::Patch)
    }

    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|method| *method == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    pub fn previous(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|method| *method == self)
            .unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

impl std::str::FromStr for Method {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_uppercase().as_str() {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            "PUT" => Ok(Self::Put),
            "PATCH" => Ok(Self::Patch),
            "DELETE" => Ok(Self::Delete),
            "HEAD" => Ok(Self::Head),
            "OPTIONS" => Ok(Self::Options),
            _ => Err(anyhow::anyhow!("unsupported HTTP method: {value}")),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QueryParam {
    pub key: String,
    pub value: String,
    #[serde(default = "enabled")]
    pub enabled: bool,
}

fn enabled() -> bool {
    true
}

impl QueryParam {
    pub fn new(key: String, value: String) -> Self {
        Self {
            key,
            value,
            enabled: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RequestConfig {
    pub id: Uuid,
    pub name: String,
    pub method: Method,
    pub url: String,
    #[serde(default)]
    pub params: Vec<QueryParam>,
    #[serde(default)]
    pub body: String,
}

impl RequestConfig {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Untitled Request".to_string(),
            method: Method::Get,
            url: String::new(),
            params: Vec::new(),
            body: String::new(),
        }
    }
}

impl Default for RequestConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    #[serde(default)]
    pub requests: Vec<RequestConfig>,
}

impl Workspace {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            requests: vec![RequestConfig::new()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_write_methods_accept_bodies() {
        assert!(!Method::Get.supports_body());
        assert!(Method::Post.supports_body());
        assert!(Method::Put.supports_body());
        assert!(Method::Patch.supports_body());
    }

    #[test]
    fn methods_cycle_in_both_directions() {
        assert_eq!(Method::Get.previous(), Method::Options);
        assert_eq!(Method::Options.next(), Method::Get);
        assert_eq!(Method::Get.next(), Method::Post);
    }
}
