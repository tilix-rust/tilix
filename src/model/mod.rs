pub mod config;
pub mod keybindings;
pub mod layout;
pub mod profile;
pub mod session;
pub mod template;
pub mod theme;
pub mod title;

pub use config::{AppConfig, PaneTitleStyle, ProfileError, WindowStyle};
pub use keybindings::{
    ActionCategory, ActionShortcutDef, ConflictInfo, KeybindingsConfig, ACTION_CATALOG,
};
pub use layout::{
    calculate_dock_position, Direction, DockPosition, LayoutError, LayoutNode, LayoutTree, PaneId,
    SplitId, SplitOrientation,
};
pub use profile::{
    expand_badge_format, expand_title_format, expand_tokens, matches_switch_rule, BadgePosition,
    CjkWidthPreference, CursorBlinkPreference, CursorShapePreference, CustomHyperlinkRule,
    EraseBindingPreference, ExitActionPreference, Profile, ProfileSwitchRule,
    TerminalBellPreference, TextBlinkModePreference, TitleTokenContext, TriggerAction,
    TriggerRule,
};
pub use session::{SessionModel, SyncGroupId};
pub use template::SessionLayoutTemplate;
pub use theme::{ColorScheme, RgbColor, ThemeError};
pub use title::{
    expand_title_tokens, expand_title_tokens_scoped, TitleEditScope, TokenContext, TokenDef,
    SESSION_TOKEN_DEFS, TERMINAL_TOKEN_DEFS, WINDOW_TOKEN_DEFS,
};

