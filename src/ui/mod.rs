pub mod dnd;
pub mod geometry;
pub mod notifications;
pub mod preferences;
pub mod quake;
pub mod session_view;
pub mod terminal_pane;
pub mod window;

pub use dnd::{setup_pane_drag_source, setup_pane_drop_target};
pub use geometry::{
    calculate_window_size_for_profile, calculate_window_size_from_cell_size,
    calculate_window_size_with_bounds, measure_cell_size, DEFAULT_HEADER_BAR_HEIGHT,
    DEFAULT_PANE_HEADER_HEIGHT, DEFAULT_SCROLLBAR_WIDTH, DEFAULT_TAB_BAR_HEIGHT,
    DEFAULT_WINDOW_HEIGHT, DEFAULT_WINDOW_WIDTH, MAX_MONITOR_RATIO, MIN_WINDOW_HEIGHT,
    MIN_WINDOW_WIDTH,
};
pub use notifications::NotificationService;
pub use preferences::TilixPreferencesWindow;
pub use quake::TilixQuakeWindow;
pub use session_view::{SessionAction, SessionView};
pub use terminal_pane::TerminalPane;
pub use window::TilixWindow;
