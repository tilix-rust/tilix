use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::model::theme::{ColorScheme, RgbColor};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CursorShapePreference {
    #[default]
    Block,
    IBeam,
    Underline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum CursorBlinkPreference {
    #[default]
    System,
    On,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EraseBindingPreference {
    #[default]
    Auto,
    AsciiDelete,
    AsciiBackspace,
    DeleteSequence,
    Tty,
}

impl From<EraseBindingPreference> for vte4::EraseBinding {
    fn from(pref: EraseBindingPreference) -> Self {
        match pref {
            EraseBindingPreference::Auto => vte4::EraseBinding::Auto,
            EraseBindingPreference::AsciiDelete => vte4::EraseBinding::AsciiDelete,
            EraseBindingPreference::AsciiBackspace => vte4::EraseBinding::AsciiBackspace,
            EraseBindingPreference::DeleteSequence => vte4::EraseBinding::DeleteSequence,
            EraseBindingPreference::Tty => vte4::EraseBinding::Tty,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TextBlinkModePreference {
    #[default]
    Never,
    Focused,
    Unfocused,
    Always,
}

impl From<TextBlinkModePreference> for vte4::TextBlinkMode {
    fn from(pref: TextBlinkModePreference) -> Self {
        match pref {
            TextBlinkModePreference::Never => vte4::TextBlinkMode::Never,
            TextBlinkModePreference::Focused => vte4::TextBlinkMode::Focused,
            TextBlinkModePreference::Unfocused => vte4::TextBlinkMode::Unfocused,
            TextBlinkModePreference::Always => vte4::TextBlinkMode::Always,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TerminalBellPreference {
    None,
    #[default]
    Sound,
    Icon,
    IconSound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExitActionPreference {
    #[default]
    Close,
    Restart,
    Hold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CjkWidthPreference {
    #[default]
    Narrow,
    Wide,
}

impl CjkWidthPreference {
    pub fn to_width(self) -> i32 {
        match self {
            CjkWidthPreference::Narrow => 1,
            CjkWidthPreference::Wide => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum BadgePosition {
    Northwest,
    #[default]
    Northeast,
    Southwest,
    Southeast,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileSwitchRule {
    pub hostname: String,
    pub directory: String,
    pub profile_id: String,
}

impl ProfileSwitchRule {
    pub fn matches(&self, hostname: &str, directory: &Path) -> bool {
        matches_switch_rule(self, hostname, directory)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CustomHyperlinkRule {
    pub name: String,
    pub pattern: String,
    pub uri: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerAction {
    Notification { text: String },
    SetBadge { badge: String },
    RunCommand { command: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerRule {
    pub name: String,
    pub pattern: String,
    pub action: TriggerAction,
}

#[derive(Debug, Clone)]
pub struct TitleTokenContext<'a> {
    pub id: u64,
    pub title: &'a str,
    pub profile_name: &'a str,
    pub directory: Option<&'a Path>,
    pub app_name: &'a str,
}

pub fn expand_tokens(format_str: &str, ctx: &TitleTokenContext) -> String {
    let mut token_ctx = crate::model::title::TokenContext::new_terminal(ctx.title);
    token_ctx.id = Some(ctx.id);
    token_ctx.profile_name = Some(ctx.profile_name.to_string());
    token_ctx.directory = ctx.directory.map(|d| d.to_path_buf());
    if !ctx.app_name.is_empty() {
        token_ctx.app_name = Some(ctx.app_name.to_string());
    }
    crate::model::title::expand_title_tokens_scoped(
        format_str,
        crate::model::title::TitleEditScope::Terminal,
        &token_ctx,
    )
}

pub fn expand_title_format(format_str: &str, ctx: &TitleTokenContext) -> String {
    expand_tokens(format_str, ctx)
}

pub fn expand_badge_format(format_str: &str, ctx: &TitleTokenContext) -> String {
    expand_tokens(format_str, ctx)
}

pub fn matches_switch_rule(rule: &ProfileSwitchRule, hostname: &str, directory: &Path) -> bool {
    let host_matches = rule.hostname.is_empty()
        || rule.hostname == "*"
        || rule.hostname.eq_ignore_ascii_case(hostname);
    if !host_matches {
        return false;
    }

    if rule.directory.is_empty() || rule.directory == "*" {
        return true;
    }

    let dir_str = directory.to_string_lossy();
    if dir_str == rule.directory {
        return true;
    }

    let rule_dir_trimmed = rule.directory.trim_end_matches('/');
    let dir_trimmed = dir_str.trim_end_matches('/');

    if dir_trimmed == rule_dir_trimmed {
        return true;
    }

    if let Some(remainder) = dir_trimmed.strip_prefix(rule_dir_trimmed) {
        if remainder.starts_with('/') {
            return true;
        }
    }

    false
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    // Identity
    pub id: String,
    pub name: String,

    // General Tab
    pub terminal_title: String,
    pub default_size_columns: u32,
    pub default_size_rows: u32,
    pub cell_width_scale: f64,
    pub cell_height_scale: f64,
    pub draw_margin: u32,
    pub text_blink_mode: TextBlinkModePreference,
    pub allow_bold: bool,
    pub rewrap_on_resize: bool,
    pub use_system_font: bool,
    pub font: Option<String>,
    pub select_by_word_chars: String,
    pub cursor_shape: CursorShapePreference,
    pub cursor_blink: CursorBlinkPreference,
    pub terminal_bell: TerminalBellPreference,

    // Command Tab
    pub login_shell: bool,
    pub use_custom_command: bool,
    pub custom_command: String,
    pub exit_action: ExitActionPreference,

    // Color Tab
    pub color_scheme: ColorScheme,
    pub use_theme_colors: bool,
    pub background_transparency_percent: u32,
    pub dim_transparency_percent: u32,
    pub bold_color_set: bool,
    pub bold_color: Option<RgbColor>,
    pub bold_is_bright: bool,
    pub cursor_colors_set: bool,
    pub cursor_background_color: Option<RgbColor>,
    pub cursor_foreground_color: Option<RgbColor>,
    pub highlight_colors_set: bool,
    pub highlight_background_color: Option<RgbColor>,
    pub highlight_foreground_color: Option<RgbColor>,

    // Scrolling Tab
    pub show_scrollbar: bool,
    pub scroll_on_output: bool,
    pub scroll_on_keystroke: bool,
    pub scrollback_unlimited: bool,
    pub scrollback_lines: Option<i64>,

    // Compatibility Tab
    pub backspace_binding: EraseBindingPreference,
    pub delete_binding: EraseBindingPreference,
    pub encoding: String,
    pub cjk_utf8_ambiguous_width: CjkWidthPreference,

    // Badge Tab
    pub badge_text: String,
    pub badge_position: BadgePosition,
    pub badge_color_set: bool,
    pub badge_color: Option<RgbColor>,
    pub badge_use_system_font: bool,
    pub badge_font: Option<String>,

    // Advanced Tab
    pub automatic_switch: Vec<ProfileSwitchRule>,
    pub custom_hyperlinks: Vec<CustomHyperlinkRule>,
    pub triggers: Vec<TriggerRule>,
    pub notify_silence_enabled: bool,
    pub notify_silence_threshold: u32,

    // Clipboard & OSC 52 Settings
    pub copy_on_select: bool,
    pub enable_osc52: bool,
    pub osc52_allow_query: bool,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            id: "default".into(),
            name: "Default".into(),

            // General Tab
            terminal_title: "${title}".into(),
            default_size_columns: 80,
            default_size_rows: 24,
            cell_width_scale: 1.0,
            cell_height_scale: 1.0,
            draw_margin: 0,
            text_blink_mode: TextBlinkModePreference::Never,
            allow_bold: true,
            rewrap_on_resize: true,
            use_system_font: true,
            font: Some("Monospace 11".into()),
            select_by_word_chars: "-A-Za-z0-9,./?%&#:_~".into(),
            cursor_shape: CursorShapePreference::Block,
            cursor_blink: CursorBlinkPreference::System,
            terminal_bell: TerminalBellPreference::Sound,

            // Command Tab
            login_shell: false,
            use_custom_command: false,
            custom_command: String::new(),
            exit_action: ExitActionPreference::Close,

            // Color Tab
            color_scheme: ColorScheme::tilix_dark(),
            use_theme_colors: false,
            background_transparency_percent: 0,
            dim_transparency_percent: 0,
            bold_color_set: false,
            bold_color: None,
            bold_is_bright: false,
            cursor_colors_set: false,
            cursor_background_color: None,
            cursor_foreground_color: None,
            highlight_colors_set: false,
            highlight_background_color: None,
            highlight_foreground_color: None,

            // Scrolling Tab
            show_scrollbar: true,
            scroll_on_output: true,
            scroll_on_keystroke: true,
            scrollback_unlimited: false,
            scrollback_lines: Some(5000),

            // Compatibility Tab
            backspace_binding: EraseBindingPreference::Auto,
            delete_binding: EraseBindingPreference::Auto,
            encoding: "UTF-8".into(),
            cjk_utf8_ambiguous_width: CjkWidthPreference::Narrow,

            // Badge Tab
            badge_text: String::new(),
            badge_position: BadgePosition::Northeast,
            badge_color_set: false,
            badge_color: None,
            badge_use_system_font: true,
            badge_font: None,

            // Advanced Tab
            automatic_switch: Vec::new(),
            custom_hyperlinks: Vec::new(),
            triggers: Vec::new(),
            notify_silence_enabled: false,
            notify_silence_threshold: 10,

            // Clipboard & OSC 52 Settings
            copy_on_select: false,
            enable_osc52: true,
            osc52_allow_query: false,
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

    #[test]
    fn test_profile_extended_default_values() {
        let p = Profile::default();
        assert_eq!(p.terminal_title, "${title}");
        assert_eq!(p.default_size_columns, 80);
        assert_eq!(p.default_size_rows, 24);
        assert_eq!(p.cell_width_scale, 1.0);
        assert_eq!(p.cell_height_scale, 1.0);
        assert_eq!(p.draw_margin, 0);
        assert_eq!(p.text_blink_mode, TextBlinkModePreference::Never);
        assert!(p.allow_bold);
        assert!(p.rewrap_on_resize);
        assert!(p.use_system_font);
        assert_eq!(p.select_by_word_chars, "-A-Za-z0-9,./?%&#:_~");
        assert_eq!(p.terminal_bell, TerminalBellPreference::Sound);
        assert!(!p.login_shell);
        assert!(!p.use_custom_command);
        assert!(p.custom_command.is_empty());
        assert_eq!(p.exit_action, ExitActionPreference::Close);
        assert!(!p.use_theme_colors);
        assert_eq!(p.background_transparency_percent, 0);
        assert_eq!(p.dim_transparency_percent, 0);
        assert!(!p.bold_color_set);
        assert!(p.bold_color.is_none());
        assert!(!p.bold_is_bright);
        assert!(!p.cursor_colors_set);
        assert!(p.cursor_background_color.is_none());
        assert!(p.cursor_foreground_color.is_none());
        assert!(!p.highlight_colors_set);
        assert!(p.highlight_background_color.is_none());
        assert!(p.highlight_foreground_color.is_none());
        assert!(p.show_scrollbar);
        assert!(p.scroll_on_output);
        assert!(p.scroll_on_keystroke);
        assert!(!p.scrollback_unlimited);
        assert_eq!(p.scrollback_lines, Some(5000));
        assert_eq!(p.backspace_binding, EraseBindingPreference::Auto);
        assert_eq!(p.delete_binding, EraseBindingPreference::Auto);
        assert_eq!(p.encoding, "UTF-8");
        assert_eq!(p.cjk_utf8_ambiguous_width, CjkWidthPreference::Narrow);
        assert!(p.badge_text.is_empty());
        assert_eq!(p.badge_position, BadgePosition::Northeast);
        assert!(!p.badge_color_set);
        assert!(p.badge_color.is_none());
        assert!(p.badge_use_system_font);
        assert!(p.badge_font.is_none());
        assert!(p.automatic_switch.is_empty());
        assert!(p.custom_hyperlinks.is_empty());
        assert!(p.triggers.is_empty());
        assert!(!p.notify_silence_enabled);
        assert_eq!(p.notify_silence_threshold, 10);
    }

    #[test]
    fn test_profile_serde_roundtrip() {
        let mut p = Profile {
            id: "custom-id".into(),
            name: "Custom Profile".into(),
            terminal_title: "${appName}: ${title}".into(),
            cell_width_scale: 1.2,
            cell_height_scale: 1.1,
            draw_margin: 80,
            text_blink_mode: TextBlinkModePreference::Focused,
            allow_bold: false,
            login_shell: true,
            use_custom_command: true,
            custom_command: "/usr/bin/htop".into(),
            exit_action: ExitActionPreference::Hold,
            background_transparency_percent: 15,
            dim_transparency_percent: 20,
            bold_color_set: true,
            bold_color: Some(RgbColor::from_hex("#ff0000").unwrap()),
            bold_is_bright: true,
            show_scrollbar: false,
            scrollback_unlimited: true,
            backspace_binding: EraseBindingPreference::AsciiBackspace,
            delete_binding: EraseBindingPreference::DeleteSequence,
            cjk_utf8_ambiguous_width: CjkWidthPreference::Wide,
            badge_text: "PROD".into(),
            badge_position: BadgePosition::Southwest,
            ..Default::default()
        };
        p.automatic_switch.push(ProfileSwitchRule {
            hostname: "prod.internal".into(),
            directory: "/var/log".into(),
            profile_id: "prod-profile".into(),
        });
        p.custom_hyperlinks.push(CustomHyperlinkRule {
            name: "BugTracker".into(),
            pattern: "BUG-[0-9]+".into(),
            uri: "https://jira.internal/browse/$0".into(),
        });
        p.triggers.push(TriggerRule {
            name: "Error Alert".into(),
            pattern: "ERROR".into(),
            action: TriggerAction::Notification {
                text: "Error detected in terminal".into(),
            },
        });

        let json = serde_json::to_string_pretty(&p).expect("serialization should succeed");
        let deserialized: Profile = serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(p, deserialized);
    }

    #[test]
    fn test_profile_serde_backwards_compatibility() {
        let legacy_json = r##"{
            "id": "legacy",
            "name": "Legacy",
            "color_scheme": {
                "name": "Tilix Dark",
                "comment": "Default theme",
                "foreground-color": "#ffffff",
                "background-color": "#000000",
                "palette": ["#000000", "#111111", "#222222", "#333333", "#444444", "#555555", "#666666", "#777777", "#888888", "#999999", "#aaaaaa", "#bbbbbb", "#cccccc", "#dddddd", "#eeeeee", "#ffffff"]
            },
            "font": "Monospace 12",
            "scrollback_lines": 2000,
            "cursor_shape": "Block",
            "cursor_blink": "System"
        }"##;

        let p: Profile = serde_json::from_str(legacy_json).expect("legacy JSON must deserialize");
        assert_eq!(p.id, "legacy");
        assert_eq!(p.name, "Legacy");
        assert_eq!(p.font.as_deref(), Some("Monospace 12"));
        assert_eq!(p.scrollback_lines, Some(2000));
        assert_eq!(p.cursor_shape, CursorShapePreference::Block);
        assert_eq!(p.cursor_blink, CursorBlinkPreference::System);

        // Verify newly added fields took default values
        assert_eq!(p.terminal_title, "${title}");
        assert_eq!(p.cell_width_scale, 1.0);
        assert!(p.show_scrollbar);
        assert_eq!(p.exit_action, ExitActionPreference::Close);
        assert_eq!(p.badge_position, BadgePosition::Northeast);
        assert!(p.automatic_switch.is_empty());
    }

    #[test]
    fn test_expand_title_tokens() {
        let ctx = TitleTokenContext {
            id: 42,
            title: "bash",
            profile_name: "MyProfile",
            directory: Some(Path::new("/home/user/work")),
            app_name: "Tilix",
        };
        let res = expand_title_format("${appName}: ${profile} - ${title} [${id}] in ${directory}", &ctx);
        assert_eq!(res, "Tilix: MyProfile - bash [42] in /home/user/work");

        // Edge case: no directory
        let ctx_no_dir = TitleTokenContext {
            id: 1,
            title: "zsh",
            profile_name: "Default",
            directory: None,
            app_name: "",
        };
        let res_no_dir = expand_title_format("${appName} ${directory} [${id}]", &ctx_no_dir);
        assert_eq!(res_no_dir, "Tilix  [1]");
    }

    #[test]
    fn test_expand_badge_tokens() {
        let ctx = TitleTokenContext {
            id: 7,
            title: "vim",
            profile_name: "Server",
            directory: Some(Path::new("/var/log")),
            app_name: "Tilix",
        };
        let res = expand_badge_format("${profile} (${id})", &ctx);
        assert_eq!(res, "Server (7)");
    }

    #[test]
    fn test_automatic_switch_rule_matching() {
        let rule = ProfileSwitchRule {
            hostname: "prod.server".into(),
            directory: "/var/log".into(),
            profile_id: "prod-profile".into(),
        };

        assert!(rule.matches("prod.server", Path::new("/var/log")));
        assert!(rule.matches("prod.server", Path::new("/var/log/nginx")));
        assert!(rule.matches("PROD.SERVER", Path::new("/var/log")));
        assert!(!rule.matches("dev.server", Path::new("/var/log")));
        assert!(!rule.matches("prod.server", Path::new("/etc")));
        assert!(!rule.matches("prod.server", Path::new("/var/log_other")));

        // Wildcard rule
        let wildcard_rule = ProfileSwitchRule {
            hostname: "*".into(),
            directory: "/projects".into(),
            profile_id: "proj-profile".into(),
        };
        assert!(wildcard_rule.matches("any-host", Path::new("/projects/rust")));
        assert!(!wildcard_rule.matches("any-host", Path::new("/home/user")));
    }

    #[test]
    fn test_profile_clipboard_defaults_and_backwards_compatibility() {
        let default_profile = Profile::default();
        assert!(!default_profile.copy_on_select);
        assert!(default_profile.enable_osc52);
        assert!(!default_profile.osc52_allow_query);

        // Deserialization without Phase 14 keys
        let legacy_json = r#"{
            "id": "legacy_clip",
            "name": "Legacy Clipboard Test"
        }"#;
        let deserialized: Profile = serde_json::from_str(legacy_json).expect("Should deserialize legacy JSON");
        assert!(!deserialized.copy_on_select);
        assert!(deserialized.enable_osc52);
        assert!(!deserialized.osc52_allow_query);

        // Roundtrip with custom values
        let custom = Profile {
            copy_on_select: true,
            enable_osc52: false,
            osc52_allow_query: true,
            ..Default::default()
        };

        let serialized = serde_json::to_string(&custom).expect("Serialization should succeed");
        let roundtrip: Profile = serde_json::from_str(&serialized).expect("Deserialization should succeed");
        assert!(roundtrip.copy_on_select);
        assert!(!roundtrip.enable_osc52);
        assert!(roundtrip.osc52_allow_query);
    }
}
