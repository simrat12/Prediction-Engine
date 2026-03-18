use serde::{Deserialize, Serialize};
use std::fmt;

/// Type-safe token identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TokenId(String);

impl TokenId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for TokenId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for TokenId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for TokenId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl AsRef<str> for TokenId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Type-safe condition identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ConditionId(String);

impl ConditionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for ConditionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for ConditionId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for ConditionId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl AsRef<str> for ConditionId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Type-safe order identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrderId(String);

impl OrderId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for OrderId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for OrderId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl AsRef<str> for OrderId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Type-safe market slug
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MarketSlug(String);

impl MarketSlug {
    pub fn new(slug: impl Into<String>) -> Self {
        Self(slug.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for MarketSlug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for MarketSlug {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for MarketSlug {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl AsRef<str> for MarketSlug {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
