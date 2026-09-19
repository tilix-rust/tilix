use std::path::PathBuf;
use serde::{Deserialize, Serialize};
use crate::model::keybindings::KeybindingsConfig;
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

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ProfileError {
    #[error("Profile not found: {0}")]
    ProfileNotFound(String),
    #[error("Cannot delete the last remaining profile")]
    CannotDeleteLastProfile,
    #[error("Duplicate profile ID: {0}")]
    DuplicateProfileId(String),
}

pub fn default_session_name_default() -> String {
    "${title}".to_string()
}

pub fn default_app_title_default() -> String {
    "${appName}: ${sessionName}".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub default_profile: Profile,
    pub profiles: Vec<Profile>,
    pub default_profile_id: String,
    pub quake_height_percent: u32,
    pub quake_hide_on_unfocus: bool,
    pub notifications_enabled: bool,
    pub bell_notifications: bool,
    pub process_exit_notifications: bool,
    pub window_style: WindowStyle,
    pub use_wide_handle: bool,
    pub pane_title_style: PaneTitleStyle,
    pub pane_title_show_when_single: bool,
    pub show_tab_bar: bool,
    pub keybindings: KeybindingsConfig,
    #[serde(default = "default_session_name_default")]
    pub default_session_name: String,
    #[serde(default = "default_app_title_default")]
    pub app_title: String,
    #[serde(default)]
    pub compact_mode: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let default_profile = Profile::default();
        let default_profile_id = default_profile.id.clone();
        let profiles = vec![default_profile.clone()];
        Self {
            default_profile,
            profiles,
            default_profile_id,
            quake_height_percent: 40,
            quake_hide_on_unfocus: false,
            notifications_enabled: true,
            bell_notifications: true,
            process_exit_notifications: true,
            window_style: WindowStyle::Normal,
            use_wide_handle: false,
            pane_title_style: PaneTitleStyle::Normal,
            pane_title_show_when_single: true,
            show_tab_bar: true,
            keybindings: KeybindingsConfig::default(),
            default_session_name: default_session_name_default(),
            app_title: default_app_title_default(),
            compact_mode: false,
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
        Self::from_json(&content)
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
        let val: serde_json::Value = serde_json::from_str(json)?;
        let mut config: Self = serde_json::from_value(val.clone())?;

        let has_profiles = val
            .get("profiles")
            .is_some_and(|p| p.as_array().is_some_and(|a| !a.is_empty()));
        let has_default_profile = val.get("default_profile").is_some();

        if !has_profiles && has_default_profile {
            config.profiles = vec![config.default_profile.clone()];
            config.default_profile_id = config.default_profile.id.clone();
        } else {
            if config.profiles.is_empty() {
                config.profiles = vec![config.default_profile.clone()];
            }
            if config.default_profile_id.is_empty() {
                config.default_profile_id = config.profiles[0].id.clone();
            }
            if !has_default_profile {
                if let Some(def) = config.profiles.iter().find(|p| p.id == config.default_profile_id) {
                    config.default_profile = def.clone();
                } else if !config.profiles.is_empty() {
                    config.default_profile_id = config.profiles[0].id.clone();
                    config.default_profile = config.profiles[0].clone();
                }
            }
        }
        Ok(config)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn get_profile(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn get_profile_mut(&mut self, id: &str) -> Option<&mut Profile> {
        self.profiles.iter_mut().find(|p| p.id == id)
    }

    pub fn get_default_profile(&self) -> &Profile {
        self.get_profile(&self.default_profile_id)
            .unwrap_or(&self.default_profile)
    }

    pub fn add_profile(&mut self, mut profile: Profile) -> Result<String, ProfileError> {
        if profile.id.is_empty() {
            profile.id = format!("profile-{}", glib::uuid_string_random());
        } else if self.profiles.iter().any(|p| p.id == profile.id) {
            return Err(ProfileError::DuplicateProfileId(profile.id));
        }
        let id = profile.id.clone();
        self.profiles.push(profile);
        Ok(id)
    }

    pub fn duplicate_profile(&mut self, source_id: &str) -> Result<Profile, ProfileError> {
        let source = self
            .get_profile(source_id)
            .ok_or_else(|| ProfileError::ProfileNotFound(source_id.to_string()))?;
        let mut clone = source.clone();
        clone.id = format!("profile-{}", glib::uuid_string_random());
        clone.name = format!("{} (Copy)", clone.name);
        self.profiles.push(clone.clone());
        Ok(clone)
    }

    pub fn delete_profile(&mut self, id: &str) -> Result<(), ProfileError> {
        if self.profiles.len() <= 1 {
            return Err(ProfileError::CannotDeleteLastProfile);
        }
        let pos = self
            .profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| ProfileError::ProfileNotFound(id.to_string()))?;
        self.profiles.remove(pos);
        if self.default_profile_id == id {
            self.default_profile_id = self.profiles[0].id.clone();
            self.default_profile = self.profiles[0].clone();
        }
        Ok(())
    }

    pub fn set_default_profile(&mut self, id: &str) -> Result<(), ProfileError> {
        let profile = self
            .get_profile(id)
            .ok_or_else(|| ProfileError::ProfileNotFound(id.to_string()))?
            .clone();
        self.default_profile_id = id.to_string();
        self.default_profile = profile;
        Ok(())
    }

    pub fn update_profile(&mut self, profile: Profile) -> Result<(), ProfileError> {
        let pos = self
            .profiles
            .iter()
            .position(|p| p.id == profile.id)
            .ok_or_else(|| ProfileError::ProfileNotFound(profile.id.clone()))?;
        if profile.id == self.default_profile_id {
            self.default_profile = profile.clone();
        }
        self.profiles[pos] = profile;
        Ok(())
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
        assert!(deserialized.show_tab_bar);
    }

    #[test]
    fn test_app_config_roundtrip_with_show_tab_bar() {
        let config = AppConfig {
            show_tab_bar: false,
            ..Default::default()
        };
        let json = config.to_json().expect("to_json should succeed");
        assert!(json.contains("\"show_tab_bar\": false"));

        let deserialized = AppConfig::from_json(&json).expect("from_json should succeed");
        assert_eq!(config, deserialized);
        assert!(!deserialized.show_tab_bar);
    }

    #[test]
    fn test_app_config_with_keybindings_defaults() {
        let config = AppConfig::default();
        assert!(config.keybindings.custom.is_empty());
    }

    #[test]
    fn test_app_config_deserialize_legacy_v6_json() {
        let legacy_json = r#"{
            "quake_height_percent": 50,
            "window_style": "hide_toolbar",
            "use_wide_handle": true,
            "pane_title_style": "none",
            "pane_title_show_when_single": false,
            "show_tab_bar": false
        }"#;
        let config = AppConfig::from_json(legacy_json).expect("deserialize legacy v6 json should succeed");
        assert_eq!(config.quake_height_percent, 50);
        assert_eq!(config.window_style, WindowStyle::HideToolbar);
        assert!(config.use_wide_handle);
        assert_eq!(config.pane_title_style, PaneTitleStyle::None);
        assert!(!config.pane_title_show_when_single);
        assert!(!config.show_tab_bar);
        assert!(config.keybindings.custom.is_empty());
        assert_eq!(
            config.keybindings.get_effective_accel("win.new-tab"),
            Some("<Primary><Shift>t".to_string())
        );
    }

    #[test]
    fn test_app_config_roundtrip_with_custom_keybindings() {
        let mut config = AppConfig::default();
        config.keybindings.set_custom_accel("win.new-tab", "<Primary>t");
        config.keybindings.set_custom_accel("win.close-tab", "<Primary>w");
        config.keybindings.set_custom_accel("win.toggle-sync-input", "");

        let json = config.to_json().expect("to_json should succeed");
        assert!(json.contains("\"win.new-tab\": \"<Primary>t\""));
        assert!(json.contains("\"win.close-tab\": \"<Primary>w\""));
        assert!(json.contains("\"win.toggle-sync-input\": \"\""));

        let deserialized = AppConfig::from_json(&json).expect("from_json should succeed");
        assert_eq!(config, deserialized);
        assert_eq!(
            deserialized.keybindings.get_effective_accel("win.new-tab"),
            Some("<Primary>t".to_string())
        );
        assert_eq!(
            deserialized.keybindings.get_effective_accel("win.close-tab"),
            Some("<Primary>w".to_string())
        );
        assert_eq!(
            deserialized.keybindings.get_effective_accel("win.toggle-sync-input"),
            Some("".to_string())
        );
    }

    #[test]
    fn test_app_config_profiles_defaults() {
        let config = AppConfig::default();
        assert_eq!(config.profiles.len(), 1);
        assert_eq!(config.default_profile_id, "default");
        assert_eq!(config.default_profile.id, "default");
        assert_eq!(config.get_default_profile().id, "default");
    }

    #[test]
    fn test_app_config_add_profile() {
        let mut config = AppConfig::default();
        let new_prof = Profile {
            id: "my-profile".into(),
            name: "My Profile".into(),
            ..Default::default()
        };

        let id = config.add_profile(new_prof).expect("add profile should succeed");
        assert_eq!(id, "my-profile");
        assert_eq!(config.profiles.len(), 2);
        assert_eq!(config.get_profile("my-profile").unwrap().name, "My Profile");

        // Duplicate ID should error
        let dup = Profile {
            id: "my-profile".into(),
            ..Default::default()
        };
        assert_eq!(
            config.add_profile(dup),
            Err(ProfileError::DuplicateProfileId("my-profile".into()))
        );

        // Empty ID auto-generates unique ID
        let auto_id = Profile {
            id: "".into(),
            ..Default::default()
        };
        let gen_id = config.add_profile(auto_id).expect("empty id should auto-generate");
        assert!(!gen_id.is_empty());
        assert!(gen_id.starts_with("profile-"));
        assert_eq!(config.profiles.len(), 3);
    }

    #[test]
    fn test_app_config_duplicate_profile() {
        let mut config = AppConfig::default();
        let cloned = config
            .duplicate_profile("default")
            .expect("duplicate should succeed");

        assert_eq!(config.profiles.len(), 2);
        assert_eq!(cloned.name, "Default (Copy)");
        assert_ne!(cloned.id, "default");
        assert!(config.get_profile(&cloned.id).is_some());

        // Duplicate non-existent profile should error
        assert_eq!(
            config.duplicate_profile("non-existent"),
            Err(ProfileError::ProfileNotFound("non-existent".into()))
        );
    }

    #[test]
    fn test_app_config_delete_profile() {
        let mut config = AppConfig::default();
        let prof2 = Profile {
            id: "p2".into(),
            ..Default::default()
        };
        config.add_profile(prof2).unwrap();

        assert_eq!(config.profiles.len(), 2);
        config.delete_profile("p2").expect("delete should succeed");
        assert_eq!(config.profiles.len(), 1);
        assert!(config.get_profile("p2").is_none());

        assert_eq!(
            config.delete_profile("p2"),
            Err(ProfileError::CannotDeleteLastProfile)
        );
    }

    #[test]
    fn test_app_config_delete_default_profile_reassigns_default() {
        let mut config = AppConfig::default();
        let prof2 = Profile {
            id: "p2".into(),
            name: "Profile Two".into(),
            ..Default::default()
        };
        config.add_profile(prof2).unwrap();

        assert_eq!(config.default_profile_id, "default");
        config.delete_profile("default").expect("deleting default should succeed if other profiles exist");
        assert_eq!(config.profiles.len(), 1);
        assert_eq!(config.default_profile_id, "p2");
        assert_eq!(config.default_profile.id, "p2");
        assert_eq!(config.default_profile.name, "Profile Two");
    }

    #[test]
    fn test_app_config_cannot_delete_last_profile() {
        let mut config = AppConfig::default();
        assert_eq!(config.profiles.len(), 1);
        assert_eq!(
            config.delete_profile("default"),
            Err(ProfileError::CannotDeleteLastProfile)
        );
    }

    #[test]
    fn test_app_config_set_default_profile() {
        let mut config = AppConfig::default();
        let prof2 = Profile {
            id: "work".into(),
            name: "Work".into(),
            ..Default::default()
        };
        config.add_profile(prof2).unwrap();

        config.set_default_profile("work").expect("set default should succeed");
        assert_eq!(config.default_profile_id, "work");
        assert_eq!(config.default_profile.name, "Work");

        assert_eq!(
            config.set_default_profile("missing"),
            Err(ProfileError::ProfileNotFound("missing".into()))
        );
    }

    #[test]
    fn test_app_config_legacy_deserialization_populates_profiles() {
        let legacy_json = r#"{
            "notifications_enabled": false
        }"#;

        let config = AppConfig::from_json(legacy_json).expect("deserialize legacy json should succeed");
        assert_eq!(config.profiles.len(), 1);
        assert_eq!(config.profiles[0].id, "default");
        assert_eq!(config.default_profile_id, "default");
        assert_eq!(config.default_profile.id, "default");
        assert!(!config.notifications_enabled);
    }

    #[test]
    fn test_app_config_title_options_serialization_and_legacy() {
        let mut config = AppConfig::default();
        assert_eq!(config.default_session_name, "${title}");
        assert_eq!(config.app_title, "${appName}: ${sessionName}");

        config.default_session_name = "Session: ${activeTerminalTitle}".to_string();
        config.app_title = "${appName} [${sessionNumber}/${sessionCount}]".to_string();

        let json = config.to_json().expect("serialize to json");
        let loaded = AppConfig::from_json(&json).expect("deserialize from json");
        assert_eq!(loaded.default_session_name, "Session: ${activeTerminalTitle}");
        assert_eq!(loaded.app_title, "${appName} [${sessionNumber}/${sessionCount}]");

        // Legacy json missing title fields defaults properly
        let legacy_json = r#"{"notifications_enabled": true}"#;
        let legacy = AppConfig::from_json(legacy_json).expect("deserialize legacy json");
        assert_eq!(legacy.default_session_name, "${title}");
        assert_eq!(legacy.app_title, "${appName}: ${sessionName}");
    }

    #[test]
    fn test_app_config_compact_mode_default() {
        let config = AppConfig::default();
        assert!(!config.compact_mode, "Default compact_mode must be false");
    }

    #[test]
    fn test_app_config_compact_mode_serde_roundtrip() {
        let mut config = AppConfig::default();
        config.compact_mode = true;
        let json_true = config.to_json().expect("serialize compact_mode true");
        let loaded_true = AppConfig::from_json(&json_true).expect("deserialize compact_mode true");
        assert!(loaded_true.compact_mode);

        config.compact_mode = false;
        let json_false = config.to_json().expect("serialize compact_mode false");
        let loaded_false = AppConfig::from_json(&json_false).expect("deserialize compact_mode false");
        assert!(!loaded_false.compact_mode);
    }

    #[test]
    fn test_app_config_compact_mode_backwards_compatibility() {
        let legacy_json = r#"{
            "notifications_enabled": true,
            "window_style": "normal"
        }"#;
        let config = AppConfig::from_json(legacy_json).expect("deserialize legacy json missing compact_mode");
        assert!(!config.compact_mode, "Missing compact_mode field should default to false");
    }
}

