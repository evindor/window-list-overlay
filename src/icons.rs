use gio::prelude::*;
use gtk4::IconTheme;

const FALLBACK_ICON: &str = "application-x-executable";

/// Resolve an icon name from a window class string.
/// Tries multiple strategies to find a matching desktop file / icon.
pub fn resolve_icon(class: &str, icon_theme: &IconTheme) -> String {
    // 1. Exact class as desktop ID
    if let Some(name) = icon_name_from_desktop(&format!("{class}.desktop")) {
        return name;
    }

    // 2. Lowercase class as desktop ID
    let lower = class.to_lowercase();
    if let Some(name) = icon_name_from_desktop(&format!("{lower}.desktop")) {
        return name;
    }

    // 3. Last segment after '.' (e.g. "com.mitchellh.ghostty" -> "ghostty")
    if let Some(last) = class.rsplit('.').next() {
        let last_lower = last.to_lowercase();
        if let Some(name) = icon_name_from_desktop(&format!("{last_lower}.desktop")) {
            return name;
        }
        // Try as direct icon name
        if icon_theme.has_icon(&last_lower) {
            return last_lower;
        }
    }

    // 4. Direct icon theme lookup by class name
    if icon_theme.has_icon(&lower) {
        return lower;
    }

    // 5. Fallback
    FALLBACK_ICON.to_string()
}

fn icon_name_from_desktop(desktop_id: &str) -> Option<String> {
    let info = gio::DesktopAppInfo::new(desktop_id)?;
    let icon = info.icon()?;
    icon.to_string().map(|s| s.into())
}
