use std::path::Path;
use serde::Deserialize;

use crate::read_config::Config;

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct Entry {
    pub name: Option<String>,
    pub r#type: Option<String>,
    pub mtime: Option<String>,
    pub size: Option<u64>,
}


#[tokio::main]
pub async fn list_dir(config: Config, path: &std::path::Path) -> Result<Vec<Entry>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/{}", config.server, path.display()).as_str())
        .header("Authorization", format!("Basic {}", config.basic_auth))
        .send()
        .await?
        .json::<Vec<Entry>>()
        .await?;
    debug!("Found {} entries in {}.", resp.len(), path.display());
    Ok(resp)
}
