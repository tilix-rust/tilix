use serde::{Deserialize, Serialize};
use crate::model::theme::ColorScheme;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorShapePreference {
    Block,
    IBeam,
    Underline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CursorBlinkPreference {
    System,
    On,
    Off,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub color_scheme: ColorScheme,
    pub font: Option<String>,
    pub scrollback_lines: Option<i64>,
    pub cursor_shape: CursorShapePreference,
    pub cursor_blink: CursorBlinkPreference,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: "default".into(),
            name: "Default".into(),
            color_scheme: ColorScheme::tilix_dark(),
            font: Some("Monospace 11".into()),
            scrollback_lines: Some(5000),
            cursor_shape: CursorShapePreference::Block,
            cursor_blink: CursorBlinkPreference::System,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_default_settings() {
        let profile = Profile::default();
        assert_eq!(profile.id, "default");
        assert_eq!(profile.name, "Default");
        assert_eq!(profile.font.as_deref(), Some("Monospace 11"));
        assert_eq!(profile.scrollback_lines, Some(5000));
        assert_eq!(profile.cursor_shape, CursorShapePreference::Block);
        assert_eq!(profile.cursor_blink, CursorBlinkPreference::System);
        assert_eq!(profile.color_scheme.name, "Tilix Dark");

        // Serde test
        let json = serde_json::to_string(&profile).unwrap();
        let deserialized: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(profile, deserialized);
    }
}
