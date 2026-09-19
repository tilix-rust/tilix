use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionCategory {
    SessionAndTabs,
    SplitsAndLayout,
    Navigation,
    ViewAndSettings,
    Clipboard,
}

impl ActionCategory {
    pub fn title(&self) -> &'static str {
        match self {
            Self::SessionAndTabs => "Session & Tabs",
            Self::SplitsAndLayout => "Splits & Layout",
            Self::Navigation => "Navigation",
            Self::ViewAndSettings => "View & Settings",
            Self::Clipboard => "Clipboard & Edit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionShortcutDef {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub category: ActionCategory,
    pub default_accels: &'static [&'static str],
}

impl ActionShortcutDef {
    pub fn primary_default_accel(&self) -> &'static str {
        self.default_accels.first().copied().unwrap_or("")
    }
}

pub static ACTION_CATALOG: &[ActionShortcutDef] = &[
    // Session & Tabs (14 actions)
    ActionShortcutDef {
        id: "win.new-tab",
        title: "New Tab",
        description: "Create a new terminal tab",
        category: ActionCategory::SessionAndTabs,
        default_accels: &["<Primary><Shift>t"],
    },
    ActionShortcutDef {
        id: "win.close-pane",
        title: "Close Terminal Pane",
        description: "Close active split pane, or close tab if only one pane",
        category: ActionCategory::SessionAndTabs,
        default_accels: &["<Primary><Shift>w"],
    },
    ActionShortcutDef {
        id: "win.close-tab",
        title: "Close Tab",
        description: "Close current tab and all of its split panes",
        category: ActionCategory::SessionAndTabs,
        default_accels: &[],
    },
    ActionShortcutDef {
        id: "win.tab-next",
        title: "Next Tab",
        description: "Switch to the next tab",
        category: ActionCategory::SessionAndTabs,
        default_accels: &["<Primary>Page_Down"],
    },
    ActionShortcutDef {
        id: "win.tab-prev",
        title: "Previous Tab",
        description: "Switch to the previous tab",
        category: ActionCategory::SessionAndTabs,
        default_accels: &["<Primary>Page_Up"],
    },
    ActionShortcutDef {
        id: "win.switch-tab-1",
        title: "Switch to Tab 1",
        description: "Jump directly to tab 1",
        category: ActionCategory::SessionAndTabs,
        default_accels: &["<Alt>1"],
    },
    ActionShortcutDef {
        id: "win.switch-tab-2",
        title: "Switch to Tab 2",
        description: "Jump directly to tab 2",
        category: ActionCategory::SessionAndTabs,
        default_accels: &["<Alt>2"],
    },
    ActionShortcutDef {
        id: "win.switch-tab-3",
        title: "Switch to Tab 3",
        description: "Jump directly to tab 3",
        category: ActionCategory::SessionAndTabs,
        default_accels: &["<Alt>3"],
    },
    ActionShortcutDef {
        id: "win.switch-tab-4",
        title: "Switch to Tab 4",
        description: "Jump directly to tab 4",
        category: ActionCategory::SessionAndTabs,
        default_accels: &["<Alt>4"],
    },
    ActionShortcutDef {
        id: "win.switch-tab-5",
        title: "Switch to Tab 5",
        description: "Jump directly to tab 5",
        category: ActionCategory::SessionAndTabs,
        default_accels: &["<Alt>5"],
    },
    ActionShortcutDef {
        id: "win.switch-tab-6",
        title: "Switch to Tab 6",
        description: "Jump directly to tab 6",
        category: ActionCategory::SessionAndTabs,
        default_accels: &["<Alt>6"],
    },
    ActionShortcutDef {
        id: "win.switch-tab-7",
        title: "Switch to Tab 7",
        description: "Jump directly to tab 7",
        category: ActionCategory::SessionAndTabs,
        default_accels: &["<Alt>7"],
    },
    ActionShortcutDef {
        id: "win.switch-tab-8",
        title: "Switch to Tab 8",
        description: "Jump directly to tab 8",
        category: ActionCategory::SessionAndTabs,
        default_accels: &["<Alt>8"],
    },
    ActionShortcutDef {
        id: "win.switch-tab-9",
        title: "Switch to Tab 9",
        description: "Jump directly to tab 9",
        category: ActionCategory::SessionAndTabs,
        default_accels: &["<Alt>9"],
    },
    // Splits & Layout (4 actions)
    ActionShortcutDef {
        id: "win.split-right",
        title: "Split Right",
        description: "Split active terminal vertically (side by side)",
        category: ActionCategory::SplitsAndLayout,
        default_accels: &["<Primary><Shift>r"],
    },
    ActionShortcutDef {
        id: "win.split-down",
        title: "Split Down",
        description: "Split active terminal horizontally (stacked)",
        category: ActionCategory::SplitsAndLayout,
        default_accels: &["<Primary><Shift>d"],
    },
    ActionShortcutDef {
        id: "win.balance-layout",
        title: "Balance Layout",
        description: "Equalize dimensions of all split panes in session",
        category: ActionCategory::SplitsAndLayout,
        default_accels: &["<Primary><Shift>b"],
    },
    ActionShortcutDef {
        id: "win.toggle-sync-input",
        title: "Toggle Input Sync",
        description: "Broadcast keystrokes to all terminal panes in tab",
        category: ActionCategory::SplitsAndLayout,
        default_accels: &["<Primary><Shift>i"],
    },
    // Navigation (4 actions)
    ActionShortcutDef {
        id: "win.focus-up",
        title: "Focus Terminal Above",
        description: "Move focus to terminal pane above active pane",
        category: ActionCategory::Navigation,
        default_accels: &["<Alt>Up"],
    },
    ActionShortcutDef {
        id: "win.focus-down",
        title: "Focus Terminal Below",
        description: "Move focus to terminal pane below active pane",
        category: ActionCategory::Navigation,
        default_accels: &["<Alt>Down"],
    },
    ActionShortcutDef {
        id: "win.focus-left",
        title: "Focus Terminal Left",
        description: "Move focus to terminal pane to the left",
        category: ActionCategory::Navigation,
        default_accels: &["<Alt>Left"],
    },
    ActionShortcutDef {
        id: "win.focus-right",
        title: "Focus Terminal Right",
        description: "Move focus to terminal pane to the right",
        category: ActionCategory::Navigation,
        default_accels: &["<Alt>Right"],
    },
    // View & Settings (5 actions)
    ActionShortcutDef {
        id: "win.toggle-tab-bar",
        title: "Toggle Tab Bar",
        description: "Toggle tab bar visibility",
        category: ActionCategory::ViewAndSettings,
        default_accels: &["F12", "<Primary><Shift>F12"],
    },
    ActionShortcutDef {
        id: "win.preferences",
        title: "Preferences",
        description: "Open application preferences dialog",
        category: ActionCategory::ViewAndSettings,
        default_accels: &["<Primary>comma"],
    },
    ActionShortcutDef {
        id: "win.zoom-in",
        title: "Zoom In",
        description: "Increase terminal font size",
        category: ActionCategory::ViewAndSettings,
        default_accels: &["<Primary>plus", "<Primary>equal", "<Primary>KP_Add"],
    },
    ActionShortcutDef {
        id: "win.zoom-out",
        title: "Zoom Out",
        description: "Decrease terminal font size",
        category: ActionCategory::ViewAndSettings,
        default_accels: &["<Primary>minus", "<Primary>KP_Subtract"],
    },
    ActionShortcutDef {
        id: "win.zoom-normal",
        title: "Normal Size",
        description: "Reset terminal font size to default",
        category: ActionCategory::ViewAndSettings,
        default_accels: &["<Primary>0", "<Primary>KP_0"],
    },
    // Clipboard & Edit (6 actions)
    ActionShortcutDef {
        id: "win.copy",
        title: "Copy",
        description: "Copy selected text to clipboard",
        category: ActionCategory::Clipboard,
        default_accels: &["<Primary><Shift>c", "<Primary>Insert"],
    },
    ActionShortcutDef {
        id: "win.copy-html",
        title: "Copy as HTML",
        description: "Copy selected text with formatting as HTML",
        category: ActionCategory::Clipboard,
        default_accels: &[],
    },
    ActionShortcutDef {
        id: "win.paste",
        title: "Paste",
        description: "Paste clipboard text into active terminal",
        category: ActionCategory::Clipboard,
        default_accels: &["<Primary><Shift>v", "<Shift>Insert"],
    },
    ActionShortcutDef {
        id: "win.paste-primary",
        title: "Paste Primary Selection",
        description: "Paste primary selection text into active terminal",
        category: ActionCategory::Clipboard,
        default_accels: &[],
    },
    ActionShortcutDef {
        id: "win.select-all",
        title: "Select All",
        description: "Select all text in terminal buffer",
        category: ActionCategory::Clipboard,
        default_accels: &["<Primary><Shift>a"],
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictInfo {
    pub action_id: String,
    pub action_title: String,
    pub conflicting_accel: String,
}

pub fn normalize_accelerator(accel: &str) -> String {
    let trimmed = accel.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut primary = false;
    let mut shift = false;
    let mut alt = false;
    let mut super_mod = false;

    let mut remaining = String::new();
    let mut in_bracket = false;
    let mut tag_buf = String::new();

    for ch in trimmed.chars() {
        if ch == '<' {
            in_bracket = true;
            tag_buf.clear();
        } else if ch == '>' && in_bracket {
            in_bracket = false;
            let tag_lower = tag_buf.trim().to_ascii_lowercase();
            match tag_lower.as_str() {
                "primary" | "control" | "ctrl" => primary = true,
                "shift" => shift = true,
                "alt" | "mod1" => alt = true,
                "super" | "meta" | "mod4" => super_mod = true,
                _ => {}
            }
            tag_buf.clear();
        } else if in_bracket {
            tag_buf.push(ch);
        } else {
            remaining.push(ch);
        }
    }

    let key = remaining.trim();
    if key.is_empty() {
        return String::new();
    }

    let canonical_key = match key.to_ascii_lowercase().as_str() {
        "up" => "Up".to_string(),
        "down" => "Down".to_string(),
        "left" => "Left".to_string(),
        "right" => "Right".to_string(),
        "page_up" | "pageup" => "Page_Up".to_string(),
        "page_down" | "pagedown" => "Page_Down".to_string(),
        "comma" => "comma".to_string(),
        "return" | "enter" => "Return".to_string(),
        "escape" | "esc" => "Escape".to_string(),
        "tab" => "Tab".to_string(),
        "space" => "space".to_string(),
        "backspace" => "BackSpace".to_string(),
        "delete" => "Delete".to_string(),
        "insert" => "Insert".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        other => {
            if other.len() > 1 && other.starts_with('f') && other[1..].chars().all(|c| c.is_ascii_digit()) {
                format!("F{}", &other[1..])
            } else if other.len() == 1 {
                other.to_ascii_lowercase()
            } else {
                key.to_string()
            }
        }
    };

    let mut result = String::new();
    if primary {
        result.push_str("<Primary>");
    }
    if shift {
        result.push_str("<Shift>");
    }
    if alt {
        result.push_str("<Alt>");
    }
    if super_mod {
        result.push_str("<Super>");
    }
    result.push_str(&canonical_key);
    result
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub custom: HashMap<String, String>,
}

impl KeybindingsConfig {
    pub fn new() -> Self {
        Self {
            custom: HashMap::new(),
        }
    }

    pub fn get_effective_accel(&self, action_id: &str) -> Option<String> {
        if let Some(custom_accel) = self.custom.get(action_id) {
            return Some(custom_accel.clone());
        }
        ACTION_CATALOG
            .iter()
            .find(|def| def.id == action_id)
            .map(|def| def.primary_default_accel().to_string())
    }

    pub fn get_all_effective_accels(&self, action_id: &str) -> Vec<String> {
        if let Some(custom_accel) = self.custom.get(action_id) {
            if custom_accel.trim().is_empty() {
                return Vec::new();
            }
            return vec![custom_accel.clone()];
        }
        ACTION_CATALOG
            .iter()
            .find(|def| def.id == action_id)
            .map(|def| {
                def.default_accels
                    .iter()
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn is_customized(&self, action_id: &str) -> bool {
        self.custom.contains_key(action_id)
    }

    pub fn set_custom_accel(&mut self, action_id: &str, accel: impl Into<String>) {
        let accel_str = accel.into();
        let is_same_as_default = if let Some(def) = ACTION_CATALOG.iter().find(|def| def.id == action_id) {
            if def.default_accels.is_empty() {
                accel_str.trim().is_empty()
            } else {
                def.default_accels.iter().any(|d| {
                    accel_str == *d
                        || (!accel_str.trim().is_empty()
                            && normalize_accelerator(&accel_str) == normalize_accelerator(d))
                })
            }
        } else {
            false
        };

        if is_same_as_default {
            self.custom.remove(action_id);
        } else {
            self.custom.insert(action_id.to_string(), accel_str);
        }
    }

    pub fn reset_action(&mut self, action_id: &str) {
        self.custom.remove(action_id);
    }

    pub fn reset_all(&mut self) {
        self.custom.clear();
    }

    pub fn check_conflict(&self, target_action_id: &str, candidate_accel: &str) -> Option<ConflictInfo> {
        let norm_candidate = normalize_accelerator(candidate_accel);
        if norm_candidate.is_empty() {
            return None;
        }

        for def in ACTION_CATALOG {
            if def.id == target_action_id {
                continue;
            }
            for effective in self.get_all_effective_accels(def.id) {
                let norm_effective = normalize_accelerator(&effective);
                if !norm_effective.is_empty() && norm_candidate == norm_effective {
                    return Some(ConflictInfo {
                        action_id: def.id.to_string(),
                        action_title: def.title.to_string(),
                        conflicting_accel: effective,
                    });
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_keybindings_empty() {
        let config = KeybindingsConfig::default();
        assert!(config.custom.is_empty());
        assert_eq!(KeybindingsConfig::new().custom.len(), 0);
    }

    #[test]
    fn test_action_catalog_completeness() {
        assert_eq!(ACTION_CATALOG.len(), 32);
        let ids: std::collections::HashSet<&str> =
            ACTION_CATALOG.iter().map(|d| d.id).collect();
        assert_eq!(ids.len(), 32);
        assert!(ids.contains(&"win.new-tab"));
        assert!(ids.contains(&"win.close-pane"));
        assert!(ids.contains(&"win.close-tab"));
        assert!(ids.contains(&"win.tab-next"));
        assert!(ids.contains(&"win.tab-prev"));
        assert!(ids.contains(&"win.split-right"));
        assert!(ids.contains(&"win.split-down"));
        assert!(ids.contains(&"win.balance-layout"));
        assert!(ids.contains(&"win.toggle-sync-input"));
        assert!(ids.contains(&"win.preferences"));
        assert!(ids.contains(&"win.focus-up"));
        assert!(ids.contains(&"win.focus-down"));
        assert!(ids.contains(&"win.focus-left"));
        assert!(ids.contains(&"win.focus-right"));
        assert!(ids.contains(&"win.toggle-tab-bar"));
        assert!(ids.contains(&"win.zoom-in"));
        assert!(ids.contains(&"win.zoom-out"));
        assert!(ids.contains(&"win.zoom-normal"));
        assert!(ids.contains(&"win.copy"));
        assert!(ids.contains(&"win.copy-html"));
        assert!(ids.contains(&"win.paste"));
        assert!(ids.contains(&"win.paste-primary"));
        assert!(ids.contains(&"win.select-all"));
        for i in 1..=9 {
            let action_id = format!("win.switch-tab-{}", i);
            assert!(ids.contains(&action_id.as_str()));
        }

        // Verify categories
        for def in ACTION_CATALOG {
            assert!(!def.title.is_empty());
            assert!(!def.description.is_empty());
            assert!(!def.category.title().is_empty());
        }
    }

    #[test]
    fn test_effective_accel_zoom_actions() {
        let config = KeybindingsConfig::default();
        assert_eq!(
            config.get_effective_accel("win.zoom-in"),
            Some("<Primary>plus".to_string())
        );
        assert_eq!(
            config.get_effective_accel("win.zoom-out"),
            Some("<Primary>minus".to_string())
        );
        assert_eq!(
            config.get_effective_accel("win.zoom-normal"),
            Some("<Primary>0".to_string())
        );
        assert_eq!(
            config.get_all_effective_accels("win.zoom-in"),
            vec![
                "<Primary>plus".to_string(),
                "<Primary>equal".to_string(),
                "<Primary>KP_Add".to_string()
            ]
        );
        assert_eq!(
            config.get_all_effective_accels("win.zoom-out"),
            vec![
                "<Primary>minus".to_string(),
                "<Primary>KP_Subtract".to_string()
            ]
        );
        assert_eq!(
            config.get_all_effective_accels("win.zoom-normal"),
            vec!["<Primary>0".to_string(), "<Primary>KP_0".to_string()]
        );
    }

    #[test]
    fn test_effective_accel_default_resolution() {
        let config = KeybindingsConfig::default();
        assert_eq!(
            config.get_effective_accel("win.new-tab"),
            Some("<Primary><Shift>t".to_string())
        );
        assert_eq!(
            config.get_effective_accel("win.split-right"),
            Some("<Primary><Shift>r".to_string())
        );
        assert_eq!(
            config.get_effective_accel("win.close-tab"),
            Some("".to_string())
        );
        assert_eq!(
            config.get_all_effective_accels("win.close-tab"),
            Vec::<String>::new()
        );
        assert_eq!(
            config.get_all_effective_accels("win.toggle-tab-bar"),
            vec!["F12".to_string(), "<Primary><Shift>F12".to_string()]
        );
        assert_eq!(config.get_effective_accel("nonexistent"), None);
    }

    #[test]
    fn test_effective_accel_custom_override() {
        let mut config = KeybindingsConfig::default();
        config.set_custom_accel("win.new-tab", "<Primary>t");
        assert_eq!(
            config.get_effective_accel("win.new-tab"),
            Some("<Primary>t".to_string())
        );
        assert_eq!(
            config.get_all_effective_accels("win.new-tab"),
            vec!["<Primary>t".to_string()]
        );
    }

    #[test]
    fn test_effective_accel_disabled() {
        let mut config = KeybindingsConfig::default();
        config.set_custom_accel("win.new-tab", "");
        assert_eq!(
            config.get_effective_accel("win.new-tab"),
            Some("".to_string())
        );
        assert_eq!(
            config.get_all_effective_accels("win.new-tab"),
            Vec::<String>::new()
        );
        assert!(config.is_customized("win.new-tab"));
    }

    #[test]
    fn test_is_customized() {
        let mut config = KeybindingsConfig::default();
        assert!(!config.is_customized("win.new-tab"));
        config.set_custom_accel("win.new-tab", "<Alt>t");
        assert!(config.is_customized("win.new-tab"));
        config.reset_action("win.new-tab");
        assert!(!config.is_customized("win.new-tab"));
    }

    #[test]
    fn test_reset_action_and_reset_all() {
        let mut config = KeybindingsConfig::default();
        config.set_custom_accel("win.new-tab", "<Alt>t");
        config.set_custom_accel("win.close-pane", "<Alt>w");
        assert_eq!(config.custom.len(), 2);

        config.reset_action("win.new-tab");
        assert!(!config.is_customized("win.new-tab"));
        assert!(config.is_customized("win.close-pane"));
        assert_eq!(config.custom.len(), 1);

        config.reset_all();
        assert!(config.custom.is_empty());
        assert!(!config.is_customized("win.close-pane"));
    }

    #[test]
    fn test_normalize_accelerator_canonicalization() {
        assert_eq!(
            normalize_accelerator("<Shift><Primary>t"),
            "<Primary><Shift>t"
        );
        assert_eq!(
            normalize_accelerator("<Control><Shift>w"),
            "<Primary><Shift>w"
        );
        assert_eq!(
            normalize_accelerator("<ctrl><alt>up"),
            "<Primary><Alt>Up"
        );
        assert_eq!(
            normalize_accelerator("<super><alt>x"),
            "<Alt><Super>x"
        );
    }

    #[test]
    fn test_normalize_accelerator_f_key() {
        assert_eq!(normalize_accelerator("<Primary>f"), "<Primary>f");
        assert_eq!(normalize_accelerator("<Primary>F1"), "<Primary>F1");
    }

    #[test]
    fn test_normalize_accelerator_empty() {
        assert_eq!(normalize_accelerator(""), "");
        assert_eq!(normalize_accelerator("   "), "");
        assert_eq!(normalize_accelerator("<Primary><Shift>"), "");
    }

    #[test]
    fn test_check_conflict_detects_collision() {
        let config = KeybindingsConfig::default();
        let conflict = config.check_conflict("win.split-right", "<Primary><Shift>t");
        assert!(conflict.is_some());
        let info = conflict.unwrap();
        assert_eq!(info.action_id, "win.new-tab");
        assert_eq!(info.action_title, "New Tab");
        assert_eq!(info.conflicting_accel, "<Primary><Shift>t");
    }

    #[test]
    fn test_check_conflict_self_ignored() {
        let config = KeybindingsConfig::default();
        let conflict = config.check_conflict("win.new-tab", "<Primary><Shift>t");
        assert!(conflict.is_none());
    }

    #[test]
    fn test_check_conflict_empty_candidate_no_conflict() {
        let config = KeybindingsConfig::default();
        assert!(config.check_conflict("win.new-tab", "").is_none());
        assert!(config.check_conflict("win.new-tab", "   ").is_none());
    }

    #[test]
    fn test_check_conflict_with_alias_modifiers() {
        let config = KeybindingsConfig::default();
        let conflict = config.check_conflict("win.split-right", "<Shift><Control>t");
        assert!(conflict.is_some());
        let info = conflict.unwrap();
        assert_eq!(info.action_id, "win.new-tab");
    }

    #[test]
    fn test_set_custom_accel_reverting_to_default_removes_entry() {
        let mut config = KeybindingsConfig::default();
        config.set_custom_accel("win.new-tab", "<Primary>t");
        assert!(config.is_customized("win.new-tab"));

        // Setting back to default string directly
        config.set_custom_accel("win.new-tab", "<Primary><Shift>t");
        assert!(!config.is_customized("win.new-tab"));

        // Setting back to default via alias
        config.set_custom_accel("win.new-tab", "<Primary>t");
        assert!(config.is_customized("win.new-tab"));
        config.set_custom_accel("win.new-tab", "<Shift><Control>t");
        assert!(!config.is_customized("win.new-tab"));
    }
}
