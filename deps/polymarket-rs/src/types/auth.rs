use serde::{Deserialize, Serialize};

/// API credentials for L2 authentication
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApiCreds {
    #[serde(rename = "apiKey")]
    pub api_key: String,
    pub secret: String,
    pub passphrase: String,
}

impl ApiCreds {
    pub fn new(api_key: String, secret: String, passphrase: String) -> Self {
        Self {
            api_key,
            secret,
            passphrase,
        }
    }
}

/// Response from API keys list endpoint
#[derive(Debug, Deserialize)]
pub struct ApiKeysResponse {
    #[serde(rename = "apiKeys")]
    pub api_keys: Vec<String>,
}

/// Balance and allowance query parameters
#[derive(Debug, Default, Clone)]
pub struct BalanceAllowanceParams {
    pub asset_type: Option<super::AssetType>,
    pub token_id: Option<String>,
    pub signature_type: Option<u8>,
}

impl BalanceAllowanceParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn asset_type(mut self, asset_type: super::AssetType) -> Self {
        self.asset_type = Some(asset_type);
        self
    }

    pub fn token_id(mut self, token_id: impl Into<String>) -> Self {
        self.token_id = Some(token_id.into());
        self
    }

    pub fn signature_type(mut self, sig_type: u8) -> Self {
        self.signature_type = Some(sig_type);
        self
    }

    pub fn to_query_params(&self) -> Vec<(&str, String)> {
        let mut params = Vec::with_capacity(3);

        if let Some(ref asset_type) = self.asset_type {
            let type_str = match asset_type {
                super::AssetType::Collateral => "COLLATERAL",
                super::AssetType::Conditional => "CONDITIONAL",
            };
            params.push(("asset_type", type_str.to_string()));
        }

        if let Some(ref token_id) = self.token_id {
            params.push(("token_id", token_id.clone()));
        }

        if let Some(sig_type) = self.signature_type {
            params.push(("signature_type", sig_type.to_string()));
        }

        params
    }
}
