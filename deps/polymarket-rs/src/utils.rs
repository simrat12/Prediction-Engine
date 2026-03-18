use crate::error::{Error, Result};
use base64::{engine::general_purpose::URL_SAFE, Engine};
use hmac::{Hmac, Mac};
use serde::Serialize;
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

/// Get current Unix timestamp in seconds
pub fn get_current_unix_time_secs() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .map_err(|e| Error::Config(format!("System time error: {}", e)))
}

/// Build HMAC-SHA256 signature for L2 authentication
///
/// This generates the signature required for authenticated API requests
/// using the API secret key.
pub fn build_hmac_signature<T>(
    secret: &str,
    timestamp: u64,
    method: &str,
    req_path: &str,
    body: Option<&T>,
) -> Result<String>
where
    T: ?Sized + Serialize,
{
    // Decode the base64-encoded secret
    let decoded = URL_SAFE
        .decode(secret)
        .map_err(|e| Error::Config(format!("Failed to decode secret: {}", e)))?;

    // Build the message to sign
    let message = match body {
        None => format!("{timestamp}{method}{req_path}"),
        Some(s) => {
            // Use compact JSON (no spaces) like standard JSON.stringify
            let serialized = serde_json::to_string(&s)?;
            format!("{timestamp}{method}{req_path}{serialized}")
        }
    };

    // Create HMAC
    let mut mac = HmacSha256::new_from_slice(&decoded)
        .map_err(|e| Error::Config(format!("HMAC initialization error: {}", e)))?;

    mac.update(message.as_bytes());

    // Finalize and encode result
    let result = mac.finalize();
    Ok(URL_SAFE.encode(&result.into_bytes()[..]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_build_hmac_signature() {
        let body = HashMap::from([("hash", "0x123")]);
        let signature = build_hmac_signature(
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
            1000000,
            "test-sign",
            "/orders",
            Some(&body),
        )
        .unwrap();

        assert_eq!(signature, "4gJVbox-R6XlDK4nlaicig0_ANVL1qdcahiL8CXfXLM=");
    }

    #[test]
    fn test_get_current_unix_time() {
        let timestamp = get_current_unix_time_secs().unwrap();
        // Should be a reasonable timestamp (after 2020)
        assert!(timestamp > 1577836800);
    }
}
