pub mod cloud;
pub mod diff;
pub mod provisioner;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CloudProvider {
    Aws,
    Gcp,
    Azure,
    Fly,
}

impl CloudProvider {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "aws" => Ok(Self::Aws),
            "gcp" => Ok(Self::Gcp),
            "azure" => Ok(Self::Azure),
            "fly" => Ok(Self::Fly),
            _ => Err(anyhow::anyhow!("Unknown cloud provider: {}. Use aws, gcp, azure, or fly.", s)),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DeployPlan {
    pub creates: Vec<ResourceChange>,
    pub updates: Vec<ResourceChange>,
    pub deletes: Vec<ResourceChange>,
    pub estimated_monthly_cost: f64,
}

#[derive(Debug, Serialize)]
pub struct ResourceChange {
    pub resource_type: String,
    pub name: String,
    pub detail: String,
    pub estimated_cost: Option<f64>,
}
