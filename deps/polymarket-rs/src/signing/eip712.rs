use crate::error::Result;
use alloy_primitives::{hex::encode_prefixed, Address, U256};
use alloy_sol_types::{eip712_domain, sol, SolStruct};

// EIP-712 struct for CLOB authentication
sol! {
    struct ClobAuth {
        address address;
        string timestamp;
        uint256 nonce;
        string message;
    }
}

// EIP-712 struct for order signing
sol! {
    struct Order {
        uint256 salt;
        address maker;
        address signer;
        address taker;
        uint256 tokenId;
        uint256 makerAmount;
        uint256 takerAmount;
        uint256 expiration;
        uint256 nonce;
        uint256 feeRateBps;
        uint8 side;
        uint8 signatureType;
    }
}

/// Signs a CLOB authentication message using EIP-712
///
/// This creates the L1 authentication signature required for
/// API key creation and other L1 operations.
pub fn sign_clob_auth_message<T>(
    signer: &T,
    timestamp: String,
    nonce: U256,
    chain_id: u64,
) -> Result<String>
where
    T: alloy_signer::Signer + alloy_signer::SignerSync,
{
    let message = "This message attests that I control the given wallet".to_owned();

    let auth_struct = ClobAuth {
        address: signer.address(),
        timestamp,
        nonce,
        message,
    };

    let domain = eip712_domain!(
        name: "ClobAuthDomain",
        version: "1",
        chain_id: chain_id,
    );

    let hash = auth_struct.eip712_signing_hash(&domain);
    let signature = signer
        .sign_hash_sync(&hash)
        .map_err(|e| crate::error::Error::Signing(format!("Failed to sign auth message: {}", e)))?;

    Ok(encode_prefixed(signature.as_bytes()))
}

/// Signs an order using EIP-712
///
/// This creates the signature for a limit or market order
/// that will be submitted to the exchange.
pub fn sign_order_message<T>(
    signer: &T,
    order: Order,
    chain_id: u64,
    verifying_contract: Address,
) -> Result<String>
where
    T: alloy_signer::Signer + alloy_signer::SignerSync,
{
    let domain = eip712_domain!(
        name: "Polymarket CTF Exchange",
        version: "1",
        chain_id: chain_id,
        verifying_contract: verifying_contract,
    );

    let hash = order.eip712_signing_hash(&domain);
    let signature = signer
        .sign_hash_sync(&hash)
        .map_err(|e| crate::error::Error::Signing(format!("Failed to sign order: {}", e)))?;

    Ok(encode_prefixed(signature.as_bytes()))
}
