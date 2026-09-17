pub mod config;
pub mod keybindings;
pub mod layout;
pub mod profile;
pub mod session;
pub mod template;
pub mod theme;

pub use config::{AppConfig, PaneTitleStyle, WindowStyle};
pub use keybindings::{
    ActionCategory, ActionShortcutDef, ConflictInfo, KeybindingsConfig, ACTION_CATALOG,
};
pub use layout::{
    calculate_dock_position, Direction, DockPosition, LayoutError, LayoutNode, LayoutTree, PaneId,
    SplitId, SplitOrientation,
};
pub use profile::{CursorBlinkPreference, CursorShapePreference, Profile};
pub use session::{SessionModel, SyncGroupId};
pub use template::SessionLayoutTemplate;
pub use theme::{ColorScheme, RgbColor, ThemeError};

