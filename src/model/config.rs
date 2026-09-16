use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::model::Profile;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    pub default_profile: Profile,
    pub quake_height_percent: u32,
    pub quake_hide_on_unfocus: bool,
    pub notifications_enabled: bool,
    pub bell_notifications: bool,
    pub process_exit_notifications: bool,
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
}
