use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyprMonitor {
    pub name: String,
    pub active_workspace: WorkspaceRef,
    pub focused: bool,
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
    #[serde(default)]
    pub at: (i32, i32),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveWindow {
    pub address: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HyprWorkspace {
    pub id: i64,
    #[serde(default)]
    pub tiled_layout: String,
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

fn get_monitors() -> Option<Vec<HyprMonitor>> {
    let json = hyprctl(&["monitors", "-j"])?;
    serde_json::from_str(&json).ok()
}

/// Get the active workspace ID for a monitor.
/// Empty string = focused monitor.
pub fn get_active_workspace(monitor: &str) -> Option<i64> {
    let monitors = get_monitors()?;
    let m = if monitor.is_empty() {
        monitors.into_iter().find(|m| m.focused)
    } else {
        monitors.into_iter().find(|m| m.name == monitor)
    };
    m.map(|m| m.active_workspace.id)
}

/// Get the name of the currently focused monitor
pub fn get_focused_monitor_name() -> Option<String> {
    let monitors = get_monitors()?;
    monitors.into_iter().find(|m| m.focused).map(|m| m.name)
}

/// Get the tiled layout of a workspace (e.g. "scrolling", "dwindle", "master")
pub fn get_workspace_layout(id: i64) -> Option<String> {
    let json = hyprctl(&["workspaces", "-j"])?;
    let workspaces: Vec<HyprWorkspace> = serde_json::from_str(&json).ok()?;
    workspaces
        .into_iter()
        .find(|w| w.id == id)
        .map(|w| w.tiled_layout)
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
    let mut filtered: Vec<HyprClient> = clients
        .into_iter()
        .filter(|c| c.workspace.id == workspace_id && c.mapped && !c.hidden)
        .collect();
    filtered.sort_by_key(|c| (c.at.0, c.at.1));
    filtered
}

/// Get the address of the currently active window
pub fn get_active_window_address() -> Option<String> {
    let json = hyprctl(&["activewindow", "-j"])?;
    let active: ActiveWindow = serde_json::from_str(&json).ok()?;
    Some(active.address)
}
