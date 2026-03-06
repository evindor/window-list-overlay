use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyprMonitor {
    pub name: String,
    pub active_workspace: WorkspaceRef,
}

#[derive(Debug, Deserialize)]
pub struct WorkspaceRef {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyprClient {
    pub address: String,
    pub title: String,
    pub class: String,
    pub workspace: WorkspaceRef,
    pub mapped: bool,
    pub hidden: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveWindow {
    pub address: String,
}

fn hyprctl(args: &[&str]) -> Option<String> {
    let output = Command::new("hyprctl")
        .args(args)
        .output()
        .ok()?;
    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        None
    }
}

/// Get the active workspace ID on DP-1
pub fn get_dp1_active_workspace() -> Option<i64> {
    let json = hyprctl(&["monitors", "-j"])?;
    let monitors: Vec<HyprMonitor> = serde_json::from_str(&json).ok()?;
    monitors
        .into_iter()
        .find(|m| m.name == "DP-1")
        .map(|m| m.active_workspace.id)
}

/// Get all visible clients on a given workspace
pub fn get_workspace_clients(workspace_id: i64) -> Vec<HyprClient> {
    let json = match hyprctl(&["clients", "-j"]) {
        Some(j) => j,
        None => return Vec::new(),
    };
    let clients: Vec<HyprClient> = match serde_json::from_str(&json) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    clients
        .into_iter()
        .filter(|c| c.workspace.id == workspace_id && c.mapped && !c.hidden)
        .collect()
}

/// Get the address of the currently active window
pub fn get_active_window_address() -> Option<String> {
    let json = hyprctl(&["activewindow", "-j"])?;
    let active: ActiveWindow = serde_json::from_str(&json).ok()?;
    Some(active.address)
}
