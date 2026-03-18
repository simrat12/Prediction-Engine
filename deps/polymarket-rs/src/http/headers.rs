use crate::error::Result;
use crate::signing::{sign_clob_auth_message, EthSigner};
use crate::types::ApiCreds;
use crate::utils::{build_hmac_signature, get_current_unix_time_secs};
use alloy_primitives::hex::encode_prefixed;
use alloy_primitives::U256;
use serde::Serialize;
use std::collections::HashMap;

const POLY_ADDR_HEADER: &str = "POLY_ADDRESS";
const POLY_SIG_HEADER: &str = "POLY_SIGNATURE";
const POLY_TS_HEADER: &str = "POLY_TIMESTAMP";
const POLY_NONCE_HEADER: &str = "POLY_NONCE";
const POLY_API_KEY_HEADER: &str = "POLY_API_KEY";
const POLY_PASS_HEADER: &str = "POLY_PASSPHRASE";

pub type Headers = HashMap<&'static str, String>;

/// Create L1 headers for authentication (EIP-712 based)
///
/// These headers are used for operations that require wallet signature,
/// such as creating API keys.
pub fn create_l1_headers<S: EthSigner>(
    signer: &S,
    chain_id: u64,
    nonce: Option<U256>,
) -> Result<Headers> {
    let timestamp = get_current_unix_time_secs()?.to_string();
    let nonce = nonce.unwrap_or(U256::ZERO);
    let signature = sign_clob_auth_message(signer, timestamp.clone(), nonce, chain_id)?;
    let address = encode_prefixed(signer.address().as_slice());

    Ok(HashMap::from([
        (POLY_ADDR_HEADER, address),
        (POLY_SIG_HEADER, signature),
        (POLY_TS_HEADER, timestamp),
        (POLY_NONCE_HEADER, nonce.to_string()),
    ]))
}

/// Create L2 headers for authenticated requests (HMAC based)
///
/// These headers are used for API operations that require API credentials,
/// such as creating orders, querying private data, etc.
pub fn create_l2_headers<S: EthSigner, T>(
    signer: &S,
    api_creds: &ApiCreds,
    method: &str,
    req_path: &str,
    body: Option<&T>,
) -> Result<Headers>
where
    T: ?Sized + Serialize,
{
    let address = encode_prefixed(signer.address().as_slice());
    let timestamp = get_current_unix_time_secs()?;

    let hmac_signature =
        build_hmac_signature(&api_creds.secret, timestamp, method, req_path, body)?;

    Ok(HashMap::from([
        (POLY_ADDR_HEADER, address),
        (POLY_SIG_HEADER, hmac_signature),
        (POLY_TS_HEADER, timestamp.to_string()),
        (POLY_API_KEY_HEADER, api_creds.api_key.clone()),
        (POLY_PASS_HEADER, api_creds.passphrase.clone()),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_constants() {
        assert_eq!(POLY_ADDR_HEADER, "POLY_ADDRESS");
        assert_eq!(POLY_SIG_HEADER, "POLY_SIGNATURE");
        assert_eq!(POLY_TS_HEADER, "POLY_TIMESTAMP");
    }
}
