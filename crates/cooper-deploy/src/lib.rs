pub mod cloud;
pub mod diff;
pub mod providers;
pub mod provisioner;
pub mod state;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CloudProvider {
    Aws,
    Gcp,
    Azure,
    Fly,
    Cooper,
}

impl CloudProvider {
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "aws" => Ok(Self::Aws),
            "gcp" => Ok(Self::Gcp),
            "azure" => Ok(Self::Azure),
            "fly" => Ok(Self::Fly),
            "cooper" => Ok(Self::Cooper),
            _ => Err(anyhow::anyhow!(
                "Unknown cloud provider: {}. Use aws, gcp, azure, fly, or cooper.",
                s
            )),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployPlan {
    pub creates: Vec<ResourceChange>,
    pub updates: Vec<ResourceChange>,
    pub deletes: Vec<ResourceChange>,
    pub estimated_monthly_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceChange {
    pub resource_type: String,
    pub name: String,
    pub detail: String,
    pub estimated_cost: Option<f64>,
}

/// Result of a deployment — connection info for all provisioned resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployResult {
    pub env: String,
    pub provider: String,
    pub url: Option<String>,
    pub resources: Vec<ProvisionedResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionedResource {
    pub resource_type: String,
    pub name: String,
    pub status: String,
    pub connection_info: Option<String>,
}
