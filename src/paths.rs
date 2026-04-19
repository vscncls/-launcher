use crate::errors::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::SystemTime;
use tokio::fs;

pub fn discord_launcher_path() -> Result<PathBuf> {
    let path = dirs::data_dir().context("Failed to detect data directory")?;
    Ok(path.join("discord-launcher"))
}

pub fn install_path() -> Result<PathBuf> {
    let path = discord_launcher_path()?;
    Ok(path.join("install"))
}

pub fn new_install_path() -> Result<PathBuf> {
    let path = discord_launcher_path()?;
    Ok(path.join("install-new"))
}

pub fn state_file_path() -> Result<PathBuf> {
    let path = discord_launcher_path()?;
    Ok(path.join("state.json"))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct State {
    pub version: String,
    pub last_update_check: SystemTime,
}

pub async fn load_state_file() -> Result<Option<State>> {
    let state_file_path = state_file_path()?;
    if fs::metadata(&state_file_path).await.is_ok() {
        debug!("Reading state file from {:?}...", state_file_path);
        let buf = fs::read(&state_file_path).await.with_context(|| {
            anyhow!(
                "Failed to read discord-launcher state file at {:?}",
                state_file_path
            )
        })?;
        let state = serde_json::from_slice::<State>(&buf);
        debug!("Loaded state: {:?}", state);
        Ok(state.ok())
    } else {
        debug!(
            "State file at {:?} does not exist, using empty state",
            state_file_path
        );
        Ok(None)
    }
}
