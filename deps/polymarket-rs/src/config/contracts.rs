use crate::error::{Error, Result};

/// Contract addresses for a specific chain and market type
#[derive(Debug, Clone)]
pub struct ContractConfig {
    pub exchange: String,
    pub collateral: String,
    pub conditional_tokens: String,
}

/// Chain IDs for supported networks
pub mod chains {
    pub const POLYGON_MAINNET: u64 = 137;
    pub const POLYGON_AMOY_TESTNET: u64 = 80002;
}

/// Get contract configuration for a specific chain and market type
///
/// # Arguments
/// * `chain_id` - The chain ID (137 for Polygon, 80002 for Amoy testnet)
/// * `neg_risk` - Whether to use negative risk contracts
///
/// # Returns
/// * `Ok(ContractConfig)` - The contract configuration
/// * `Err(Error::Config)` - If the chain/config combination is not supported
pub fn get_contract_config(chain_id: u64, neg_risk: bool) -> Result<ContractConfig> {
    match (chain_id, neg_risk) {
        // Polygon Mainnet - NEG_RISK
        (chains::POLYGON_MAINNET, true) => Ok(ContractConfig {
            exchange: "0xC5d563A36AE78145C45a50134d48A1215220f80a".to_owned(),
            collateral: "0x2791bca1f2de4661ed88a30c99a7a9449aa84174".to_owned(),
            conditional_tokens: "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045".to_owned(),
        }),
        // Polygon Mainnet - Standard
        (chains::POLYGON_MAINNET, false) => Ok(ContractConfig {
            exchange: "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E".to_owned(),
            collateral: "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174".to_owned(),
            conditional_tokens: "0x4D97DCd97eC945f40cF65F87097ACe5EA0476045".to_owned(),
        }),
        // Polygon Amoy Testnet - NEG_RISK
        (chains::POLYGON_AMOY_TESTNET, true) => Ok(ContractConfig {
            exchange: "0xd91E80cF2E7be2e162c6513ceD06f1dD0dA35296".to_owned(),
            collateral: "0x9c4e1703476e875070ee25b56a58b008cfb8fa78".to_owned(),
            conditional_tokens: "0x69308FB512518e39F9b16112fA8d994F4e2Bf8bB".to_owned(),
        }),
        // Polygon Amoy Testnet - Standard
        (chains::POLYGON_AMOY_TESTNET, false) => Ok(ContractConfig {
            exchange: "0xdFE02Eb6733538f8Ea35D585af8DE5958AD99E40".to_owned(),
            collateral: "0x9c4e1703476e875070ee25b56a58b008cfb8fa78".to_owned(),
            conditional_tokens: "0x69308FB512518e39F9b16112fA8d994F4e2Bf8bB".to_owned(),
        }),
        // Unsupported chain
        _ => Err(Error::Config(format!(
            "Unsupported chain_id {} with neg_risk {}",
            chain_id, neg_risk
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_polygon_mainnet_standard() {
        let config = get_contract_config(chains::POLYGON_MAINNET, false).unwrap();
        assert_eq!(
            config.exchange,
            "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"
        );
    }

    #[test]
    fn test_polygon_mainnet_neg_risk() {
        let config = get_contract_config(chains::POLYGON_MAINNET, true).unwrap();
        assert_eq!(
            config.exchange,
            "0xC5d563A36AE78145C45a50134d48A1215220f80a"
        );
    }

    #[test]
    fn test_unsupported_chain() {
        let result = get_contract_config(999, false);
        assert!(result.is_err());
    }
}
