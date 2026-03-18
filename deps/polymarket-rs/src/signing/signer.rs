use alloy_signer::{Signer, SignerSync};

/// Trait for Ethereum signers used in Polymarket operations
///
/// This trait combines the required traits for signing EIP-712 messages
/// both synchronously and asynchronously.
pub trait EthSigner: Signer + SignerSync + Send + Sync {}

// Blanket implementation for any type that meets the requirements
impl<T: Signer + SignerSync + Send + Sync> EthSigner for T {}
