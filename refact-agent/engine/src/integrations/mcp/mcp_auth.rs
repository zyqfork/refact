use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex as AMutex;

fn default_auth_type() -> String {
    "none".to_string()
}

#[derive(Deserialize, Serialize, Clone, Default, Debug, PartialEq)]
pub struct MCPAuthSettings {
    #[serde(default = "default_auth_type")]
    pub auth_type: String,
    #[serde(default)]
    pub bearer_token: String,
    #[serde(default)]
    pub oauth2_client_id: String,
    #[serde(default)]
    pub oauth2_client_secret: String,
    #[serde(default)]
    pub oauth2_token_url: String,
    #[serde(default)]
    pub oauth2_scopes: Vec<String>,
}

struct TokenState {
    access_token: String,
    expires_at: Option<Instant>,
}

pub struct MCPTokenManager {
    settings: MCPAuthSettings,
    token_cache: Arc<AMutex<Option<TokenState>>>,
}

impl MCPTokenManager {
    pub fn new(settings: MCPAuthSettings) -> Self {
        Self {
            settings,
            token_cache: Arc::new(AMutex::new(None)),
        }
    }

    pub async fn get_token(&self) -> Result<String, String> {
        match self.settings.auth_type.as_str() {
            "none" => Err("No auth configured".to_string()),
            "bearer" => {
                if self.settings.bearer_token.is_empty() {
                    return Err("Bearer token is empty".to_string());
                }
                Ok(self.settings.bearer_token.clone())
            }
            "oauth2" => self.get_oauth2_token().await,
            other => Err(format!("Unknown auth_type: {}", other)),
        }
    }

    async fn get_oauth2_token(&self) -> Result<String, String> {
        {
            let cache = self.token_cache.lock().await;
            if let Some(state) = cache.as_ref() {
                let still_valid = state
                    .expires_at
                    .map_or(true, |exp| exp > Instant::now() + Duration::from_secs(30));
                if still_valid {
                    return Ok(state.access_token.clone());
                }
            }
        }

        if self.settings.oauth2_token_url.is_empty() {
            return Err("oauth2_token_url is empty".to_string());
        }
        if self.settings.oauth2_client_id.is_empty() {
            return Err("oauth2_client_id is empty".to_string());
        }

        let client = reqwest::Client::new();
        let mut params = vec![
            ("grant_type", "client_credentials".to_string()),
            ("client_id", self.settings.oauth2_client_id.clone()),
            ("client_secret", self.settings.oauth2_client_secret.clone()),
        ];
        if !self.settings.oauth2_scopes.is_empty() {
            params.push(("scope", self.settings.oauth2_scopes.join(" ")));
        }

        let resp = client
            .post(&self.settings.oauth2_token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("OAuth2 token request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("OAuth2 token endpoint returned HTTP {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse OAuth2 response: {}", e))?;

        let access_token = body
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "OAuth2 response missing access_token".to_string())?
            .to_string();

        let expires_at = body
            .get("expires_in")
            .and_then(|v| v.as_u64())
            .map(|secs| Instant::now() + Duration::from_secs(secs));

        {
            let mut cache = self.token_cache.lock().await;
            *cache = Some(TokenState {
                access_token: access_token.clone(),
                expires_at,
            });
        }

        Ok(access_token)
    }

    pub async fn apply_auth(&self, headers: &mut HashMap<String, String>) -> Result<(), String> {
        match self.settings.auth_type.as_str() {
            "none" => Ok(()),
            "bearer" | "oauth2" => {
                let token = self.get_token().await?;
                headers.insert("Authorization".to_string(), format!("Bearer {}", token));
                Ok(())
            }
            other => Err(format!("Unknown auth_type: {}", other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_settings_default() {
        let s = MCPAuthSettings::default();
        assert_eq!(s.auth_type, "none");
        assert!(s.bearer_token.is_empty());
    }

    #[test]
    fn test_auth_settings_serialization_roundtrip() {
        let settings = MCPAuthSettings {
            auth_type: "bearer".to_string(),
            bearer_token: "tok123".to_string(),
            oauth2_client_id: "".to_string(),
            oauth2_client_secret: "".to_string(),
            oauth2_token_url: "".to_string(),
            oauth2_scopes: vec![],
        };
        let json = serde_json::to_value(&settings).unwrap();
        let roundtrip: MCPAuthSettings = serde_json::from_value(json).unwrap();
        assert_eq!(settings, roundtrip);
    }

    #[test]
    fn test_auth_settings_deserialization_with_defaults() {
        let json = serde_json::json!({});
        let settings: MCPAuthSettings = serde_json::from_value(json).unwrap();
        assert_eq!(settings.auth_type, "none");
    }

    #[tokio::test]
    async fn test_bearer_token_injection() {
        let settings = MCPAuthSettings {
            auth_type: "bearer".to_string(),
            bearer_token: "my-secret-token".to_string(),
            ..Default::default()
        };
        let manager = MCPTokenManager::new(settings);
        let mut headers = HashMap::new();
        manager.apply_auth(&mut headers).await.unwrap();
        assert_eq!(headers.get("Authorization").unwrap(), "Bearer my-secret-token");
    }

    #[tokio::test]
    async fn test_none_auth_does_not_inject_headers() {
        let settings = MCPAuthSettings {
            auth_type: "none".to_string(),
            ..Default::default()
        };
        let manager = MCPTokenManager::new(settings);
        let mut headers = HashMap::new();
        let result = manager.apply_auth(&mut headers).await;
        assert!(result.is_ok());
        assert!(headers.is_empty());
    }

    #[tokio::test]
    async fn test_bearer_empty_token_returns_error() {
        let settings = MCPAuthSettings {
            auth_type: "bearer".to_string(),
            bearer_token: "".to_string(),
            ..Default::default()
        };
        let manager = MCPTokenManager::new(settings);
        let mut headers = HashMap::new();
        let result = manager.apply_auth(&mut headers).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Bearer token is empty"));
    }

    #[tokio::test]
    async fn test_oauth2_missing_token_url_returns_error() {
        let settings = MCPAuthSettings {
            auth_type: "oauth2".to_string(),
            oauth2_client_id: "client123".to_string(),
            oauth2_token_url: "".to_string(),
            ..Default::default()
        };
        let manager = MCPTokenManager::new(settings);
        let mut headers = HashMap::new();
        let result = manager.apply_auth(&mut headers).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("oauth2_token_url is empty"));
    }

    #[tokio::test]
    async fn test_oauth2_missing_client_id_returns_error() {
        let settings = MCPAuthSettings {
            auth_type: "oauth2".to_string(),
            oauth2_client_id: "".to_string(),
            oauth2_token_url: "https://example.com/token".to_string(),
            ..Default::default()
        };
        let manager = MCPTokenManager::new(settings);
        let mut headers = HashMap::new();
        let result = manager.apply_auth(&mut headers).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("oauth2_client_id is empty"));
    }

    #[tokio::test]
    async fn test_unknown_auth_type_returns_error() {
        let settings = MCPAuthSettings {
            auth_type: "digest".to_string(),
            ..Default::default()
        };
        let manager = MCPTokenManager::new(settings);
        let mut headers = HashMap::new();
        let result = manager.apply_auth(&mut headers).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown auth_type"));
    }
}
