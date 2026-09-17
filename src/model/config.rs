use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::model::Profile;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum WindowStyle {
    #[default]
    Normal,
    HideToolbar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PaneTitleStyle {
    #[default]
    Normal,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub default_profile: Profile,
    pub quake_height_percent: u32,
    pub quake_hide_on_unfocus: bool,
    pub notifications_enabled: bool,
    pub bell_notifications: bool,
    pub process_exit_notifications: bool,
    pub window_style: WindowStyle,
    pub use_wide_handle: bool,
    pub pane_title_style: PaneTitleStyle,
    pub pane_title_show_when_single: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            default_profile: Profile::default(),
            quake_height_percent: 40,
            quake_hide_on_unfocus: false,
            notifications_enabled: true,
            bell_notifications: true,
            process_exit_notifications: true,
            window_style: WindowStyle::Normal,
            use_wide_handle: false,
            pane_title_style: PaneTitleStyle::Normal,
            pane_title_show_when_single: true,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        glib::user_config_dir().join("tilix").join("config.json")
    }

    pub fn load() -> Self {
        Self::load_from_path(&Self::config_path()).unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        self.save_to_path(&Self::config_path())
    }

    pub fn load_from_path(path: &std::path::Path) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    pub fn save_to_path(&self, path: &std::path::Path) -> Result<(), std::io::Error> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = self
            .to_json()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_config_default_values() {
        let config = AppConfig::default();
        assert_eq!(config.quake_height_percent, 40);
        assert!(!config.quake_hide_on_unfocus);
        assert!(config.notifications_enabled);
        assert!(config.bell_notifications);
        assert!(config.process_exit_notifications);
        assert_eq!(config.default_profile.name, "Default");
        assert_eq!(config.window_style, WindowStyle::Normal);
        assert!(!config.use_wide_handle);
        assert_eq!(config.pane_title_style, PaneTitleStyle::Normal);
        assert!(config.pane_title_show_when_single);
    }

    #[test]
    fn test_window_style_default_is_normal() {
        assert_eq!(WindowStyle::default(), WindowStyle::Normal);
    }

    #[test]
    fn test_window_style_serde_serialization() {
        let normal_json = serde_json::to_string(&WindowStyle::Normal).unwrap();
        assert_eq!(normal_json, "\"normal\"");
        let hide_toolbar_json = serde_json::to_string(&WindowStyle::HideToolbar).unwrap();
        assert_eq!(hide_toolbar_json, "\"hide_toolbar\"");

        let deserialized_normal: WindowStyle = serde_json::from_str("\"normal\"").unwrap();
        assert_eq!(deserialized_normal, WindowStyle::Normal);
        let deserialized_hide: WindowStyle = serde_json::from_str("\"hide_toolbar\"").unwrap();
        assert_eq!(deserialized_hide, WindowStyle::HideToolbar);
    }

    #[test]
    fn test_app_config_with_window_style_and_wide_handle_defaults() {
        let config = AppConfig::default();
        assert_eq!(config.window_style, WindowStyle::Normal);
        assert!(!config.use_wide_handle);
    }

    #[test]
    fn test_app_config_deserialize_legacy_v4_json() {
        let legacy_v4_json = r#"{
            "quake_height_percent": 50,
            "quake_hide_on_unfocus": true,
            "notifications_enabled": true,
            "bell_notifications": false,
            "process_exit_notifications": true
        }"#;
        let config = AppConfig::from_json(legacy_v4_json).expect("legacy v4 json without new fields should deserialize");
        assert_eq!(config.quake_height_percent, 50);
        assert!(config.quake_hide_on_unfocus);
        assert!(!config.bell_notifications);
        assert_eq!(config.window_style, WindowStyle::Normal);
        assert!(!config.use_wide_handle);
    }

    #[test]
    fn test_app_config_roundtrip_with_window_style_and_wide_handle() {
        let config = AppConfig {
            window_style: WindowStyle::HideToolbar,
            use_wide_handle: true,
            ..Default::default()
        };

        let json = config.to_json().expect("to_json should succeed");
        assert!(json.contains("\"window_style\": \"hide_toolbar\""));
        assert!(json.contains("\"use_wide_handle\": true"));

        let deserialized = AppConfig::from_json(&json).expect("from_json should succeed");
        assert_eq!(config, deserialized);
        assert_eq!(deserialized.window_style, WindowStyle::HideToolbar);
        assert!(deserialized.use_wide_handle);
    }

    #[test]
    fn test_pane_title_style_default_is_normal() {
        assert_eq!(PaneTitleStyle::default(), PaneTitleStyle::Normal);
    }

    #[test]
    fn test_pane_title_style_serde_serialization() {
        let normal_json = serde_json::to_string(&PaneTitleStyle::Normal).unwrap();
        assert_eq!(normal_json, "\"normal\"");
        let none_json = serde_json::to_string(&PaneTitleStyle::None).unwrap();
        assert_eq!(none_json, "\"none\"");

        let deserialized_normal: PaneTitleStyle = serde_json::from_str("\"normal\"").unwrap();
        assert_eq!(deserialized_normal, PaneTitleStyle::Normal);
        let deserialized_none: PaneTitleStyle = serde_json::from_str("\"none\"").unwrap();
        assert_eq!(deserialized_none, PaneTitleStyle::None);
    }

    #[test]
    fn test_app_config_with_pane_title_defaults() {
        let config = AppConfig::default();
        assert_eq!(config.pane_title_style, PaneTitleStyle::Normal);
        assert!(config.pane_title_show_when_single);
    }

    #[test]
    fn test_app_config_deserialize_legacy_v5_json() {
        let legacy_v5_json = r#"{
            "quake_height_percent": 50,
            "quake_hide_on_unfocus": true,
            "notifications_enabled": true,
            "bell_notifications": false,
            "process_exit_notifications": true,
            "window_style": "hide_toolbar",
            "use_wide_handle": true
        }"#;
        let config = AppConfig::from_json(legacy_v5_json).expect("legacy v5 json without phase 6 fields should deserialize");
        assert_eq!(config.window_style, WindowStyle::HideToolbar);
        assert!(config.use_wide_handle);
        assert_eq!(config.pane_title_style, PaneTitleStyle::Normal);
        assert!(config.pane_title_show_when_single);
    }

    #[test]
    fn test_app_config_roundtrip_with_pane_title_settings() {
        let config = AppConfig {
            pane_title_style: PaneTitleStyle::None,
            pane_title_show_when_single: false,
            ..Default::default()
        };

        let json = config.to_json().expect("to_json should succeed");
        assert!(json.contains("\"pane_title_style\": \"none\""));
        assert!(json.contains("\"pane_title_show_when_single\": false"));

        let deserialized = AppConfig::from_json(&json).expect("from_json should succeed");
        assert_eq!(config, deserialized);
        assert_eq!(deserialized.pane_title_style, PaneTitleStyle::None);
        assert!(!deserialized.pane_title_show_when_single);
    }

    #[test]
    fn test_app_config_serialize_deserialize_json() {
        let config = AppConfig::default();
        let json = config.to_json().expect("to_json should succeed");
        let deserialized = AppConfig::from_json(&json).expect("from_json should succeed");
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_app_config_roundtrip_file_io() {
        let mut temp_path = std::env::temp_dir();
        let unique_name = format!("tilix_test_config_{}.json", std::process::id());
        temp_path.push(unique_name);

        let config = AppConfig::default();
        config.save_to_path(&temp_path).expect("save should succeed");

        let loaded = AppConfig::load_from_path(&temp_path).expect("load should succeed");
        assert_eq!(config, loaded);

        let _ = std::fs::remove_file(&temp_path);
    }

    #[test]
    fn test_app_config_deserialize_partial_json() {
        let partial_json = r#"{"notifications_enabled": false}"#;
        let deserialized = AppConfig::from_json(partial_json).expect("deserialize partial json should succeed");
        let expected = AppConfig {
            notifications_enabled: false,
            ..Default::default()
        };
        assert_eq!(deserialized, expected);
    }

    #[test]
    fn test_app_config_deserialize_empty_json() {
        let empty_json = "{}";
        let deserialized = AppConfig::from_json(empty_json).expect("deserialize empty json should succeed");
        assert_eq!(deserialized, AppConfig::default());
    }
}
