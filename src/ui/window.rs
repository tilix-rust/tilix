use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

use adw::prelude::*;
use gio::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use crate::model::{
    expand_title_tokens_scoped, Direction, PaneTitleStyle, Profile, SessionLayoutTemplate,
    SplitOrientation, TitleEditScope, WindowStyle,
};
use crate::ui::geometry::calculate_window_size_for_profile;
use crate::ui::preferences::TilixPreferencesWindow;
use crate::ui::session_view::{SessionAction, SessionView};
use crate::ui::terminal_pane::TerminalPane;

type SessionMap = Rc<RefCell<HashMap<adw::TabPage, Rc<RefCell<SessionView>>>>>;
type TabReference = (glib::WeakRef<adw::TabView>, glib::WeakRef<adw::TabPage>);
type WindowTitleUpdater = (glib::WeakRef<adw::ApplicationWindow>, Rc<dyn Fn()>);
type WindowTransparencyUpdater = (glib::WeakRef<adw::ApplicationWindow>, Rc<dyn Fn()>);

thread_local! {
    static WIDGET_TO_SESSION: RefCell<HashMap<gtk::Widget, Rc<RefCell<SessionView>>>> = RefCell::new(HashMap::new());
    static SESSION_TO_TAB: RefCell<HashMap<gtk::Widget, TabReference>> = RefCell::new(HashMap::new());
    static WINDOW_HEADER_BARS: RefCell<Vec<glib::WeakRef<adw::HeaderBar>>> = const { RefCell::new(Vec::new()) };
    static WINDOW_TAB_BARS: RefCell<Vec<glib::WeakRef<adw::TabBar>>> = const { RefCell::new(Vec::new()) };
    static WINDOW_INSTANCES: RefCell<Vec<glib::WeakRef<adw::ApplicationWindow>>> = const { RefCell::new(Vec::new()) };
    static WINDOW_TITLE_UPDATERS: RefCell<Vec<WindowTitleUpdater>> = const { RefCell::new(Vec::new()) };
    static WINDOW_TRANSPARENCY_UPDATERS: RefCell<Vec<WindowTransparencyUpdater>> = const { RefCell::new(Vec::new()) };
}

pub fn register_session_widget(widget: &gtk::Widget, session: Rc<RefCell<SessionView>>) {
    WIDGET_TO_SESSION.with(|m| m.borrow_mut().insert(widget.clone(), session));
}

pub fn unregister_session_widget(widget: &gtk::Widget) {
    WIDGET_TO_SESSION.with(|m| m.borrow_mut().remove(widget));
    SESSION_TO_TAB.with(|m| m.borrow_mut().remove(widget));
}

pub fn session_for_widget(widget: &gtk::Widget) -> Option<Rc<RefCell<SessionView>>> {
    let mut curr = Some(widget.clone());
    while let Some(w) = curr {
        if let Some(session) = WIDGET_TO_SESSION.with(|m| m.borrow().get(&w).cloned()) {
            return Some(session);
        }
        curr = w.parent();
    }
    None
}

pub fn find_session_widget(widget: &gtk::Widget) -> Option<gtk::Widget> {
    let mut curr = Some(widget.clone());
    while let Some(w) = curr {
        if WIDGET_TO_SESSION.with(|m| m.borrow().contains_key(&w)) {
            return Some(w);
        }
        curr = w.parent();
    }
    None
}

pub fn close_session_tab(session_widget: &gtk::Widget) {
    let tab_info = SESSION_TO_TAB.with(|m| m.borrow().get(session_widget).cloned());
    if let Some((tv_weak, page_weak)) = tab_info {
        if let (Some(tv), Some(page)) = (tv_weak.upgrade(), page_weak.upgrade()) {
            tv.close_page(&page);
        }
    }
}

pub fn detach_drag_to_new_window() -> bool {
    let Some(drag) = crate::ui::dnd::take_active_pane_drag() else { return false; };
    let Some(source_session_widget) = drag.source_session_widget.upgrade() else { return false; };
    let Some(source_session_rc) = session_for_widget(&source_session_widget) else { return false; };

    let total_panes = {
        let tab_info = SESSION_TO_TAB.with(|m| m.borrow().get(&source_session_widget).cloned());
        if let Some((tv_weak, _)) = tab_info {
            if let Some(tv) = tv_weak.upgrade() {
                let mut count = 0;
                for i in 0..tv.n_pages() {
                    let page = tv.nth_page(i);
                    let child = page.child();
                    if let Some(session) = WIDGET_TO_SESSION.with(|m| m.borrow().get(&child).cloned()) {
                        count += session.borrow().pane_count();
                    }
                }
                count
            } else {
                source_session_rc.borrow().pane_count()
            }
        } else {
            source_session_rc.borrow().pane_count()
        }
    };

    if total_panes <= 1 {
        // Single-pane window detach guard: preserve sole pane in window
        return false;
    }

    let Some(pane) = source_session_rc.borrow().remove_pane_for_transfer(drag.pane_id) else {
        return false;
    };
    source_session_rc.borrow().rebuild_projection();

    if source_session_rc.borrow().is_empty() {
        close_session_tab(&source_session_widget);
    }

    let Some(app) = gio::Application::default().and_then(|a| a.downcast::<adw::Application>().ok()) else {
        return false;
    };
    let profile = pane.current_profile();
    let new_win = TilixWindow::new_empty_with_profile(&app, &profile);
    new_win.create_tab_with_existing_pane(pane);
    new_win.present();
    true
}

pub fn register_transparency_updater(window: &adw::ApplicationWindow, updater: Rc<dyn Fn()>) {
    WINDOW_TRANSPARENCY_UPDATERS.with(|updaters| {
        updaters.borrow_mut().push((window.downgrade(), updater));
    });
}

pub fn apply_transparency_to_all_windows() {
    WINDOW_TRANSPARENCY_UPDATERS.with(|updaters| {
        updaters.borrow_mut().retain(|(win_weak, updater)| {
            if win_weak.upgrade().is_some() {
                updater();
                true
            } else {
                false
            }
        });
    });
}

pub fn apply_profile_to_all_sessions(profile: &Profile) {
    WIDGET_TO_SESSION.with(|m| {
        for session in m.borrow().values() {
            session.borrow().apply_profile(profile);
        }
    });
    apply_transparency_to_all_windows();
}

pub fn apply_window_style_to_all_windows(style: WindowStyle) {
    WINDOW_HEADER_BARS.with(|bars| {
        bars.borrow_mut().retain(|bar_weak| {
            if let Some(bar) = bar_weak.upgrade() {
                bar.set_visible(style != WindowStyle::HideToolbar);
                true
            } else {
                false
            }
        });
    });
}

pub fn apply_show_tab_bar_to_all_windows(show: bool) {
    WINDOW_TAB_BARS.with(|bars| {
        bars.borrow_mut().retain(|bar_weak| {
            if let Some(bar) = bar_weak.upgrade() {
                bar.set_visible(show);
                true
            } else {
                false
            }
        });
    });
}

pub fn register_window_instance(window: &adw::ApplicationWindow) {
    WINDOW_INSTANCES.with(|wins| wins.borrow_mut().push(window.downgrade()));
}

pub fn apply_compact_mode_to_all_windows(compact: bool) {
    WINDOW_INSTANCES.with(|wins| {
        wins.borrow_mut().retain(|win_weak| {
            if let Some(win) = win_weak.upgrade() {
                if compact {
                    win.add_css_class("compact");
                } else {
                    win.remove_css_class("compact");
                }
                true
            } else {
                false
            }
        });
    });

    WINDOW_HEADER_BARS.with(|bars| {
        bars.borrow_mut().retain(|bar_weak| {
            if let Some(bar) = bar_weak.upgrade() {
                if compact {
                    bar.add_css_class("compact");
                } else {
                    bar.remove_css_class("compact");
                }
                true
            } else {
                false
            }
        });
    });

    WINDOW_TAB_BARS.with(|bars| {
        bars.borrow_mut().retain(|bar_weak| {
            if let Some(bar) = bar_weak.upgrade() {
                if compact {
                    bar.add_css_class("compact");
                } else {
                    bar.remove_css_class("compact");
                }
                true
            } else {
                false
            }
        });
    });
}

pub fn apply_wide_handle_to_all_sessions(wide: bool) {
    WIDGET_TO_SESSION.with(|m| {
        for session in m.borrow().values() {
            session.borrow().set_wide_handle(wide);
        }
    });
}

pub fn apply_pane_title_settings_to_all_sessions(style: PaneTitleStyle, show_when_single: bool) {
    WIDGET_TO_SESSION.with(|m| {
        for session in m.borrow().values() {
            session.borrow().set_pane_title_settings(style, show_when_single);
        }
    });
}

pub fn apply_title_settings_to_all_windows() {
    WIDGET_TO_SESSION.with(|m| {
        for session in m.borrow().values() {
            session.borrow().notify_title_changed();
        }
    });
    WINDOW_TITLE_UPDATERS.with(|updaters| {
        updaters.borrow_mut().retain(|(win_weak, updater)| {
            if win_weak.upgrade().is_some() {
                updater();
                true
            } else {
                false
            }
        });
    });
}

fn compute_and_apply_window_title(
    window: &adw::ApplicationWindow,
    tab_view: &adw::TabView,
    title_widget: &adw::WindowTitle,
) {
    let n_pages = tab_view.n_pages() as usize;

    for i in 0..tab_view.n_pages() {
        let page = tab_view.nth_page(i);
        let child = page.child();
        if let Some(session_rc) = WIDGET_TO_SESSION.with(|m| m.borrow().get(&child).cloned()) {
            if let Ok(s) = session_rc.try_borrow() {
                let session_title = s.active_title();
                page.set_title(&session_title);
            }
        }
    }

    let (selected_session_title, selected_page_idx, session_rc_opt) =
        if let Some(page) = tab_view.selected_page() {
            let idx = tab_view.page_position(&page) as usize + 1;
            let child = page.child();
            let session = WIDGET_TO_SESSION.with(|m| m.borrow().get(&child).cloned());
            let title = session
                .as_ref()
                .and_then(|s| s.try_borrow().ok().map(|s_ref| s_ref.active_title()))
                .unwrap_or_else(|| page.title().to_string());
            (title, idx, session)
        } else {
            ("Terminal".to_string(), 1, None)
        };

    let mut ctx = if let Some(session_rc) = session_rc_opt {
        if let Ok(s) = session_rc.try_borrow() {
            s.build_token_context()
        } else {
            crate::model::title::TokenContext::new_window(
                &selected_session_title,
                n_pages.max(1),
                selected_page_idx,
            )
        }
    } else {
        crate::model::title::TokenContext::new_window(
            &selected_session_title,
            n_pages.max(1),
            selected_page_idx,
        )
    };

    ctx.app_name = Some("Tilix".to_string());
    ctx.session_name = Some(selected_session_title);
    ctx.session_number = Some(selected_page_idx);
    ctx.session_count = Some(n_pages.max(1));

    let cfg = crate::model::AppConfig::load();
    let resolved = expand_title_tokens_scoped(
        &cfg.app_title,
        TitleEditScope::Window,
        &ctx,
    );

    window.set_title(Some(&resolved));
    title_widget.set_title(&resolved);
}

pub fn setup_css() {
    let css_provider = gtk::CssProvider::new();
    css_provider.load_from_string(
        "
        .terminal-pane {
            border: none;
            border-radius: 0;
            padding: 0;
            margin: 0;
        }
        .terminal-pane.active-pane {
            border: none;
        }
        .terminal-pane-header {
            background-color: alpha(@window_bg_color, 0.7);
            border-bottom: 1px solid alpha(@borders, 0.4);
            padding: 2px 6px;
            opacity: 0.8;
            transition: opacity 150ms ease-in-out;
        }
        .terminal-pane.active-pane .terminal-pane-header {
            opacity: 1.0;
        }
        paned > separator {
            background-color: alpha(@borders, 0.75);
            transition: background-color 150ms ease-in-out;
        }
        paned.horizontal > separator:not(.wide) {
            min-width: 1px;
            margin: 0;
            padding: 0;
        }
        paned.vertical > separator:not(.wide) {
            min-height: 1px;
            margin: 0;
            padding: 0;
        }
        paned > separator:hover,
        paned > separator:active {
            background-color: @accent_color;
        }
        .drop-indicator-overlay {
            background-color: alpha(@accent_color, 0.35);
            border: 2px solid @accent_color;
            border-radius: 4px;
            transition: all 120ms ease-in-out;
        }
        .terminal-badge {
            font-size: 2.2em;
            font-weight: bold;
            opacity: 0.25;
            margin: 16px;
            color: @window_fg_color;
        }
        .terminal-margin-line {
            background-color: alpha(@borders, 0.4);
            min-width: 1px;
            margin-left: 640px;
        }

        /* Compact Mode Density Overrides */
        /* 1. Eliminate 3px gap between toolbarview top-bar and terminal content */
        window.compact toolbarview > .top-bar,
        window.compact toolbarview > .top-bar .collapse-spacing,
        .compact toolbarview > .top-bar,
        .compact toolbarview > .top-bar .collapse-spacing {
            padding-top: 0;
            padding-bottom: 0;
        }

        /* 2. Compact HeaderBar & WindowTitle */
        window.compact toolbarview > .top-bar .collapse-spacing headerbar,
        window.compact headerbar,
        .compact headerbar,
        headerbar.compact {
            min-height: 28px;
            padding-top: 0;
            padding-bottom: 0;
        }
        window.compact headerbar > windowhandle > box,
        .compact headerbar > windowhandle > box {
            padding-top: 0;
            padding-bottom: 0;
        }
        window.compact windowtitle,
        .compact windowtitle {
            min-height: 20px;
            padding: 0;
        }
        window.compact windowtitle .title,
        .compact windowtitle .title {
            font-size: 13px;
            line-height: 14px;
        }
        window.compact headerbar button,
        window.compact headerbar menubutton,
        window.compact headerbar menubutton > button,
        .compact headerbar button,
        .compact headerbar menubutton,
        .compact headerbar menubutton > button {
            min-height: 24px;
            min-width: 24px;
            padding: 0;
            margin-top: 2px;
            margin-bottom: 2px;
            margin-left: 1px;
            margin-right: 1px;
            border-radius: 3px;
        }
        window.compact headerbar button image,
        .compact headerbar button image {
            -gtk-icon-size: 13px;
            opacity: 0.82;
            transition: opacity 150ms ease-in-out;
        }
        window.compact headerbar button:hover image,
        .compact headerbar button:hover image {
            opacity: 1.0;
        }
        window.compact headerbar windowcontrols button,
        .compact headerbar windowcontrols button {
            min-height: 20px;
            min-width: 20px;
            padding: 1px;
            margin-top: 0;
            margin-bottom: 0;
            border-radius: 3px;
        }
        window.compact headerbar windowcontrols button image,
        .compact headerbar windowcontrols button image {
            -gtk-icon-size: 12px;
            opacity: 0.82;
            transition: opacity 150ms ease-in-out;
        }
        window.compact headerbar windowcontrols button:hover image,
        .compact headerbar windowcontrols button:hover image {
            opacity: 1.0;
        }

        /* 3. Compact TabBar & TabBox */
        window.compact toolbarview > .top-bar .collapse-spacing tabbar tabbox,
        window.compact tabbar tabbox,
        .compact tabbar tabbox {
            min-height: 22px;
            padding-top: 0;
            padding-bottom: 0;
        }
        window.compact tabbar,
        window.compact tabbar .box,
        .compact tabbar,
        .compact tabbar .box {
            min-height: 22px;
            margin-bottom: 0;
            padding-top: 0;
            padding-bottom: 0;
            box-shadow: none;
        }
        window.compact tabbar tab,
        window.compact tabbar tabbox > tabboxchild,
        .compact tabbar tab,
        .compact tabbar tabbox > tabboxchild {
            min-height: 20px;
            padding: 0 6px;
            border-radius: 0;
        }
        window.compact tabbar tab button.image-button,
        window.compact tabbar tab button.image-button:hover,
        .compact tabbar tab button.image-button,
        .compact tabbar tab button.image-button:hover {
            min-width: 18px;
            min-height: 18px;
            padding: 0;
            border-radius: 9999px;
        }
        window.compact tabbar tab button.image-button image,
        .compact tabbar tab button.image-button image {
            -gtk-icon-size: 16px;
        }

        /* 4. Compact TerminalPane Header */
        window.compact .terminal-pane-header,
        .compact .terminal-pane-header,
        .terminal-pane-header.compact {
            padding: 0 4px;
            min-height: 22px;
        }
        window.compact .terminal-pane-header button,
        .compact .terminal-pane-header button {
            min-height: 18px;
            min-width: 18px;
            padding: 0 1px;
            border-radius: 3px;
        }
        window.compact .terminal-pane-header button image,
        .compact .terminal-pane-header button image {
            -gtk-icon-size: 12px;
            opacity: 0.82;
            transition: opacity 150ms ease-in-out;
        }
        window.compact .terminal-pane-header button:hover image,
        .compact .terminal-pane-header button:hover image {
            opacity: 1.0;
        }
        window.compact .terminal-pane-header label,
        .compact .terminal-pane-header label {
            font-size: 0.85em;
        }

        /* Window Transparency & Container Passthrough (Phase 17) */
        window.transparent-window,
        window.transparent-window.background,
        window.transparent-window > contents {
            background-color: transparent;
            background: transparent;
        }

        window.quake-window.transparent-window,
        window.quake-window.transparent-window.background,
        window.quake-window.transparent-window > contents {
            background-color: transparent;
            background: transparent;
        }

        window.transparent-window toolbarview {
            background-color: transparent;
            background: transparent;
        }
        window.transparent-window toolbarview > stack,
        window.transparent-window toolbarview > stack > * {
            background-color: transparent;
            background: transparent;
        }

        window.transparent-window tabview,
        window.transparent-window tabview > stack,
        window.transparent-window tabview > stack > * {
            background-color: transparent;
            background: transparent;
        }

        window.transparent-window .terminal-pane,
        window.transparent-window .terminal-pane > box,
        window.transparent-window .terminal-pane overlay {
            background-color: transparent;
            background: transparent;
        }

        /* Readability Safeguards: HeaderBar and TabBar must remain solid/opaque */
        window.transparent-window headerbar,
        window.transparent-window toolbarview > .top-bar,
        window.transparent-window toolbarview > .top-bar headerbar {
            background-color: @headerbar_bg_color;
            color: @headerbar_fg_color;
        }

        window.transparent-window tabbar,
        window.transparent-window tabbar .box,
        window.transparent-window tabbar tabbox {
            background-color: @headerbar_bg_color;
        }

        /* Paned Separator Safeguard: Must remain solid/opaque to cleanly divide panes */
        window.transparent-window paned > separator {
            background-color: mix(@headerbar_bg_color, @headerbar_fg_color, 0.15);
        }
        window.transparent-window paned > separator:hover,
        window.transparent-window paned > separator:active {
            background-color: @accent_color;
        }
        ",
    );
    if let Some(display) = gtk::gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &css_provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

pub fn setup_accels(app: &adw::Application) {
    let config = crate::model::AppConfig::load();
    apply_keybindings_to_app(app, &config.keybindings);
}

pub fn apply_keybindings_to_app(app: &adw::Application, keybindings: &crate::model::KeybindingsConfig) {
    for def in crate::model::ACTION_CATALOG {
        let accels = keybindings.get_all_effective_accels(def.id);
        let refs: Vec<&str> = accels.iter().map(|s| s.as_str()).collect();
        app.set_accels_for_action(def.id, &refs);
    }
}

pub fn apply_keybindings_globally(keybindings: &crate::model::KeybindingsConfig) {
    if let Some(app) = gio::Application::default() {
        if let Ok(adw_app) = app.downcast::<adw::Application>() {
            apply_keybindings_to_app(&adw_app, keybindings);
        }
    }
}


pub struct TilixWindow {
    window: adw::ApplicationWindow,
    tab_view: adw::TabView,
    sessions: SessionMap,
    next_session_id: Rc<RefCell<u64>>,
    title_widget: adw::WindowTitle,
    preferences_window: Rc<RefCell<Option<TilixPreferencesWindow>>>,
}

fn refocus_active_pane(tab_view: &adw::TabView, sessions: &SessionMap) {
    if let Some(page) = tab_view.selected_page() {
        if let Some(session) = sessions.borrow().get(&page).cloned() {
            session.borrow().grab_focus();
        }
    }
}

impl TilixWindow {
    pub fn new_empty(app: &adw::Application) -> Self {
        let cfg = crate::model::AppConfig::load();
        Self::new_empty_with_profile(app, cfg.get_default_profile())
    }

    pub fn new_empty_with_profile(app: &adw::Application, profile: &Profile) -> Self {
        let window = adw::ApplicationWindow::new(app);
        let cfg = crate::model::AppConfig::load();

        let monitor = gtk::gdk::Display::default().and_then(|d| {
            d.monitors().item(0).and_then(|o| o.downcast::<gtk::gdk::Monitor>().ok())
        });
        let (width, height) = calculate_window_size_for_profile(profile, &cfg, monitor.as_ref());
        window.set_default_size(width, height);
        window.set_title(Some("Tilix"));

        let header_bar = adw::HeaderBar::new();
        header_bar.set_visible(cfg.window_style != WindowStyle::HideToolbar);
        WINDOW_HEADER_BARS.with(|bars| bars.borrow_mut().push(header_bar.downgrade()));

        let title_widget = adw::WindowTitle::new("Tilix", "");
        header_bar.set_title_widget(Some(&title_widget));

        let new_tab_btn = gtk::Button::from_icon_name("tab-new-symbolic");
        new_tab_btn.set_tooltip_text(Some("New Tab (Ctrl+Shift+T)"));
        new_tab_btn.set_action_name(Some("win.new-tab"));
        new_tab_btn.add_css_class("flat");
        new_tab_btn.set_valign(gtk::Align::Center);
        new_tab_btn.set_focusable(false);
        header_bar.pack_start(&new_tab_btn);

        let split_h_btn = gtk::Button::from_icon_name("object-flip-horizontal-symbolic");
        split_h_btn.set_tooltip_text(Some("Split Right (Ctrl+Shift+R)"));
        split_h_btn.set_action_name(Some("win.split-right"));
        split_h_btn.add_css_class("flat");
        split_h_btn.set_valign(gtk::Align::Center);
        split_h_btn.set_focusable(false);
        header_bar.pack_start(&split_h_btn);

        let split_v_btn = gtk::Button::from_icon_name("object-flip-vertical-symbolic");
        split_v_btn.set_tooltip_text(Some("Split Down (Ctrl+Shift+D)"));
        split_v_btn.set_action_name(Some("win.split-down"));
        split_v_btn.add_css_class("flat");
        split_v_btn.set_valign(gtk::Align::Center);
        split_v_btn.set_focusable(false);
        header_bar.pack_start(&split_v_btn);

        let sync_btn = gtk::ToggleButton::new();
        sync_btn.set_icon_name("network-transmit-receive-symbolic");
        sync_btn.set_tooltip_text(Some("Toggle Synchronized Input (Ctrl+Shift+I)"));
        sync_btn.set_action_name(Some("win.toggle-sync-input"));
        sync_btn.add_css_class("flat");
        sync_btn.set_valign(gtk::Align::Center);
        sync_btn.set_focusable(false);
        header_bar.pack_start(&sync_btn);

        let menu_btn = gtk::MenuButton::new();
        menu_btn.set_icon_name("open-menu-symbolic");
        menu_btn.set_tooltip_text(Some("Main Menu"));
        menu_btn.set_primary(true);
        menu_btn.set_valign(gtk::Align::Center);
        menu_btn.set_focusable(false);
        header_bar.pack_end(&menu_btn);

        let app_menu = gio::Menu::new();
        app_menu.append(Some("Balance Layout"), Some("win.balance-layout"));
        app_menu.append(Some("Preferences"), Some("win.preferences"));
        app_menu.append(Some("Save Layout..."), Some("win.save-layout"));
        app_menu.append(Some("Shortcuts"), Some("win.shortcuts"));
        app_menu.append(Some("About Tilix"), Some("win.about"));
        menu_btn.set_menu_model(Some(&app_menu));

        let tab_view = adw::TabView::new();
        let tab_bar = adw::TabBar::new();
        tab_bar.set_view(Some(&tab_view));
        tab_bar.set_autohide(false);
        tab_bar.set_visible(cfg.show_tab_bar);
        WINDOW_TAB_BARS.with(|bars| bars.borrow_mut().push(tab_bar.downgrade()));

        if cfg.compact_mode {
            window.add_css_class("compact");
            header_bar.add_css_class("compact");
            tab_bar.add_css_class("compact");
        }
        if profile.background_transparency_percent > 0 {
            window.add_css_class("transparent-window");
        }
        WINDOW_INSTANCES.with(|wins| wins.borrow_mut().push(window.downgrade()));

        let sessions: SessionMap = Rc::new(RefCell::new(HashMap::new()));
        let next_session_id = Rc::new(RefCell::new(1u64));
        let preferences_window: Rc<RefCell<Option<TilixPreferencesWindow>>> = Rc::new(RefCell::new(None));

        let win_weak = window.downgrade();
        let tv_weak = tab_view.downgrade();
        let tw_weak = title_widget.downgrade();

        let update_titles: Rc<dyn Fn()> = Rc::new(move || {
            if let (Some(w), Some(tv), Some(tw)) = (win_weak.upgrade(), tv_weak.upgrade(), tw_weak.upgrade()) {
                compute_and_apply_window_title(&w, &tv, &tw);
            }
        });

        WINDOW_TITLE_UPDATERS.with(|updaters| {
            updaters.borrow_mut().push((window.downgrade(), Rc::clone(&update_titles)));
        });

        let win_weak_trans = window.downgrade();
        let tv_weak_trans = tab_view.downgrade();
        let sess_trans = Rc::clone(&sessions);
        let update_transparency: Rc<dyn Fn()> = Rc::new(move || {
            if let (Some(w), Some(tv)) = (win_weak_trans.upgrade(), tv_weak_trans.upgrade()) {
                let is_trans = tv.selected_page()
                    .and_then(|p| sess_trans.borrow().get(&p).cloned())
                    .map(|s| s.borrow().has_transparent_pane())
                    .unwrap_or(false);
                if is_trans {
                    w.add_css_class("transparent-window");
                } else {
                    w.remove_css_class("transparent-window");
                }
            }
        });
        register_transparency_updater(&window, Rc::clone(&update_transparency));

        let u_sel = Rc::clone(&update_titles);
        let u_trans_sel = Rc::clone(&update_transparency);
        tab_view.connect_selected_page_notify(move |_| {
            u_sel();
            u_trans_sel();
        });

        let u_att = Rc::clone(&update_titles);
        let u_trans_att = Rc::clone(&update_transparency);
        tab_view.connect_page_attached(move |_, _, _| {
            u_att();
            u_trans_att();
        });

        let u_det = Rc::clone(&update_titles);
        let u_trans_det = Rc::clone(&update_transparency);
        tab_view.connect_page_detached(move |_, _, _| {
            u_det();
            u_trans_det();
        });

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header_bar);
        toolbar_view.add_top_bar(&tab_bar);
        toolbar_view.set_content(Some(&tab_view));
        window.set_content(Some(&toolbar_view));

        if let Some(popover) = menu_btn.popover() {
            let tv_weak_pop = tab_view.downgrade();
            let sess_pop = Rc::clone(&sessions);
            let pref_holder_pop = Rc::clone(&preferences_window);
            popover.connect_closed(move |_| {
                if let Ok(holder) = pref_holder_pop.try_borrow() {
                    if let Some(ref pref) = *holder {
                        if pref.window().is_visible() {
                            return;
                        }
                    }
                }
                let tv_opt = tv_weak_pop.upgrade();
                let s_map = Rc::clone(&sess_pop);
                glib::idle_add_local_once(move || {
                    if let Some(tv) = tv_opt {
                        refocus_active_pane(&tv, &s_map);
                    }
                });
            });
        }

        let tv_weak_active = tab_view.downgrade();
        let sess_active = Rc::clone(&sessions);
        let pref_holder_active = Rc::clone(&preferences_window);
        window.connect_is_active_notify(move |win| {
            if win.is_active() {
                if let Ok(holder) = pref_holder_active.try_borrow() {
                    if let Some(ref pref) = *holder {
                        if pref.window().is_visible() {
                            return;
                        }
                    }
                }
                let tv_opt = tv_weak_active.upgrade();
                let s_map = Rc::clone(&sess_active);
                glib::idle_add_local_once(move || {
                    if let Some(tv) = tv_opt {
                        refocus_active_pane(&tv, &s_map);
                    }
                });
            }
        });

        let tilix_win = Self {
            window,
            tab_view,
            sessions,
            next_session_id,
            title_widget,
            preferences_window,
        };

        tilix_win.setup_tab_close_handler();
        tilix_win.setup_tab_detaching(app);
        tilix_win.setup_actions();

        tilix_win
    }

    pub fn new(app: &adw::Application) -> Self {
        let tilix_win = Self::new_empty(app);
        // Open initial tab
        tilix_win.create_tab();
        tilix_win
    }

    pub fn new_with_profile(app: &adw::Application, profile: &Profile) -> Self {
        let tilix_win = Self::new_empty_with_profile(app, profile);
        tilix_win.create_tab();
        tilix_win
    }

    fn setup_tab_detaching(&self, app: &adw::Application) {
        let app_weak = app.downgrade();
        self.tab_view.connect_create_window(move |_tv| {
            let app = app_weak.upgrade()?;
            let new_win = TilixWindow::new_empty(&app);
            new_win.present();
            Some(new_win.tab_view().clone())
        });

        let sessions_attached = Rc::clone(&self.sessions);
        let tab_view_weak = self.tab_view.downgrade();
        self.tab_view.connect_page_attached(move |_tv, page, _pos| {
            let child = page.child();
            if let Some(session) = WIDGET_TO_SESSION.with(|m| m.borrow().get(&child).cloned()) {
                sessions_attached.borrow_mut().insert(page.clone(), session);
                SESSION_TO_TAB.with(|m| {
                    m.borrow_mut().insert(child.clone(), (tab_view_weak.clone(), page.downgrade()));
                });
            }
        });

        let sessions_detached = Rc::clone(&self.sessions);
        self.tab_view.connect_page_detached(move |_tv, page, _pos| {
            let child = page.child();
            sessions_detached.borrow_mut().remove(page);
            SESSION_TO_TAB.with(|m| m.borrow_mut().remove(&child));
        });
    }

    fn create_tab_internal(
        tab_view: &adw::TabView,
        sessions: &SessionMap,
        next_session_id: &Rc<RefCell<u64>>,
        model: Option<crate::model::SessionModel>,
        initial_directory: Option<&Path>,
    ) -> (adw::TabPage, Rc<RefCell<SessionView>>) {
        let session_view = match model {
            Some(m) => Rc::new(RefCell::new(SessionView::with_model_and_dir(m, initial_directory))),
            None => {
                let initial_pane_id = {
                    let mut id = next_session_id.borrow_mut();
                    let cur = *id;
                    *id += 100; // Offset pane IDs by 100 per tab to prevent any pane ID collisions
                    crate::model::PaneId(cur)
                };
                Rc::new(RefCell::new(SessionView::with_id_and_dir(initial_pane_id, initial_directory)))
            }
        };
        let widget = session_view.borrow().widget().clone();
        WIDGET_TO_SESSION.with(|m| m.borrow_mut().insert(widget.clone(), Rc::clone(&session_view)));
        let tab_page = tab_view.append(&widget);
        SESSION_TO_TAB.with(|m| {
            m.borrow_mut().insert(
                widget.clone(),
                (tab_view.downgrade(), tab_page.downgrade()),
            )
        });
        sessions.borrow_mut().insert(tab_page.clone(), Rc::clone(&session_view));

        // Set initial title and bind title changes
        let initial_title = session_view.borrow().active_title();
        tab_page.set_title(&initial_title);

        let page_weak = tab_page.downgrade();
        session_view.borrow().connect_title_changed(move |title| {
            if let Some(page) = page_weak.upgrade() {
                page.set_title(title);
            }
            WINDOW_TITLE_UPDATERS.with(|updaters| {
                for (_, u) in updaters.borrow().iter() {
                    u();
                }
            });
        });

        // Wire session view action handler
        let session_weak = Rc::downgrade(&session_view);
        let tab_page_weak = tab_page.downgrade();
        let tab_view_weak = tab_view.downgrade();
        session_view.borrow().set_action_handler(move |action| {
            let s_weak = session_weak.clone();
            let p_weak = tab_page_weak.clone();
            let tv_weak = tab_view_weak.clone();
            glib::idle_add_local_once(move || {
                let Some(session) = s_weak.upgrade() else { return; };
                match action {
                    SessionAction::Split(id, orientation) => {
                        session.borrow().set_active_pane(id);
                        session.borrow().split_active(orientation);
                    }
                    SessionAction::Close(id) => {
                        session.borrow().close_pane(id);
                        if session.borrow().is_empty() {
                            if let (Some(tv), Some(p)) = (tv_weak.upgrade(), p_weak.upgrade()) {
                                tv.close_page(&p);
                            }
                        }
                    }
                    SessionAction::Focus(id) => {
                        session.borrow().set_active_pane(id);
                    }
                }
            });
        });

        sessions
            .borrow_mut()
            .insert(tab_page.clone(), Rc::clone(&session_view));
        tab_view.set_selected_page(&tab_page);
        session_view.borrow().grab_focus();

        WINDOW_TITLE_UPDATERS.with(|updaters| {
            for (_, u) in updaters.borrow().iter() {
                u();
            }
        });
        apply_transparency_to_all_windows();

        (tab_page, session_view)
    }

    pub fn create_tab(&self) -> (adw::TabPage, Rc<RefCell<SessionView>>) {
        let initial_dir = self
            .tab_view
            .selected_page()
            .and_then(|page| {
                self.sessions
                    .borrow()
                    .get(&page)
                    .and_then(|s| s.borrow().active_current_directory())
            })
            .or_else(|| std::env::current_dir().ok());
        Self::create_tab_internal(
            &self.tab_view,
            &self.sessions,
            &self.next_session_id,
            None,
            initial_dir.as_deref(),
        )
    }

    pub fn import_session_template(
        &self,
        template: &SessionLayoutTemplate,
    ) -> (adw::TabPage, Rc<RefCell<SessionView>>) {
        let (start_pane_id, start_split_id) = {
            let mut id = self.next_session_id.borrow_mut();
            let p_id = *id;
            let s_id = *id;
            *id += 100;
            (p_id, s_id)
        };
        let session_model = template.instantiate_session(start_pane_id, start_split_id);
        Self::create_tab_internal(
            &self.tab_view,
            &self.sessions,
            &self.next_session_id,
            Some(session_model),
            None,
        )
    }

    pub fn import_session_layout_from_json(
        &self,
        json: &str,
    ) -> Result<adw::TabPage, serde_json::Error> {
        let template = SessionLayoutTemplate::from_json(json)?;
        let (page, _) = self.import_session_template(&template);
        Ok(page)
    }

    pub fn export_active_session_layout(&self) -> Option<String> {
        let session_rc = self.active_session()?;
        let session = session_rc.borrow();
        let title = session.active_title();
        let model = session.model();
        let template = SessionLayoutTemplate::from_session(&title, &model);
        template.to_json().ok()
    }

    fn setup_tab_close_handler(&self) {
        let sessions_clone = Rc::clone(&self.sessions);
        let win_weak = self.window.downgrade();

        self.tab_view.connect_close_page(move |tv, page| {
            let child = page.child();
            tv.close_page_finish(page, true);
            let session = sessions_clone.borrow_mut().remove(page);
            if let Some(session) = session {
                session.borrow().close();
            }
            WIDGET_TO_SESSION.with(|m| m.borrow_mut().remove(&child));
            SESSION_TO_TAB.with(|m| m.borrow_mut().remove(&child));

            if tv.n_pages() == 0 {
                if let Some(win) = win_weak.upgrade() {
                    win.close();
                }
            }

            glib::Propagation::Stop
        });
    }

    pub fn create_tab_with_existing_pane(
        &self,
        pane: TerminalPane,
    ) -> (adw::TabPage, Rc<RefCell<SessionView>>) {
        let session_view = Rc::new(RefCell::new(SessionView::with_existing_pane(pane)));
        let widget = session_view.borrow().widget().clone();
        WIDGET_TO_SESSION.with(|m| m.borrow_mut().insert(widget.clone(), Rc::clone(&session_view)));
        let tab_page = self.tab_view.append(&widget);
        SESSION_TO_TAB.with(|m| {
            m.borrow_mut().insert(
                widget.clone(),
                (self.tab_view.downgrade(), tab_page.downgrade()),
            )
        });
        self.sessions
            .borrow_mut()
            .insert(tab_page.clone(), Rc::clone(&session_view));

        let initial_title = session_view.borrow().active_title();
        tab_page.set_title(&initial_title);

        let page_weak = tab_page.downgrade();
        session_view.borrow().connect_title_changed(move |title| {
            if let Some(page) = page_weak.upgrade() {
                page.set_title(title);
            }
            WINDOW_TITLE_UPDATERS.with(|updaters| {
                for (_, u) in updaters.borrow().iter() {
                    u();
                }
            });
        });

        // Wire session view action handler
        let session_weak = Rc::downgrade(&session_view);
        let tab_page_weak = tab_page.downgrade();
        let tab_view_weak = self.tab_view.downgrade();
        session_view.borrow().set_action_handler(move |action| {
            let s_weak = session_weak.clone();
            let p_weak = tab_page_weak.clone();
            let tv_weak = tab_view_weak.clone();
            glib::idle_add_local_once(move || {
                let Some(session) = s_weak.upgrade() else { return; };
                match action {
                    SessionAction::Split(id, orientation) => {
                        session.borrow().set_active_pane(id);
                        session.borrow().split_active(orientation);
                    }
                    SessionAction::Close(id) => {
                        session.borrow().close_pane(id);
                        if session.borrow().is_empty() {
                            if let (Some(tv), Some(p)) = (tv_weak.upgrade(), p_weak.upgrade()) {
                                tv.close_page(&p);
                            }
                        }
                    }
                    SessionAction::Focus(id) => {
                        session.borrow().set_active_pane(id);
                    }
                }
            });
        });

        self.tab_view.set_selected_page(&tab_page);
        session_view.borrow().grab_focus();

        WINDOW_TITLE_UPDATERS.with(|updaters| {
            for (_, u) in updaters.borrow().iter() {
                u();
            }
        });
        apply_transparency_to_all_windows();

        (tab_page, session_view)
    }

    fn active_session(&self) -> Option<Rc<RefCell<SessionView>>> {
        let page = self.tab_view.selected_page()?;
        self.sessions.borrow().get(&page).cloned()
    }

    pub fn update_transparency(&self) {
        let is_trans = self.active_session()
            .map(|s| s.borrow().has_transparent_pane())
            .unwrap_or(false);
        if is_trans {
            self.window.add_css_class("transparent-window");
        } else {
            self.window.remove_css_class("transparent-window");
        }
    }

    fn setup_actions(&self) {
        // New Tab
        {
            let action = gio::SimpleAction::new("new-tab", None);
            let tv_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            let next_id = Rc::clone(&self.next_session_id);

            action.connect_activate(move |_, _| {
                let Some(tv) = tv_weak.upgrade() else { return; };

                let initial_dir = tv
                    .selected_page()
                    .and_then(|page| {
                        sessions.borrow().get(&page).and_then(|s| s.borrow().active_current_directory())
                    })
                    .or_else(|| std::env::current_dir().ok());

                Self::create_tab_internal(&tv, &sessions, &next_id, None, initial_dir.as_deref());
            });
            self.window.add_action(&action);
        }

        // Close Pane / Tab
        {
            let action = gio::SimpleAction::new("close-pane", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            let win_weak = self.window.downgrade();

            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };

                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    if session.borrow().pane_count() > 1 {
                        session.borrow().close_active();
                    } else {
                        tv.close_page(&page);
                    }
                } else if let Some(win) = win_weak.upgrade() {
                    win.close();
                }
            });
            self.window.add_action(&action);
        }

        // Close Tab
        {
            let action = gio::SimpleAction::new("close-tab", None);
            let tab_view_weak = self.tab_view.downgrade();
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                if let Some(page) = tv.selected_page() {
                    tv.close_page(&page);
                }
            });
            self.window.add_action(&action);
        }

        // Export Session Layout
        {
            let action = gio::SimpleAction::new("export-session-layout", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);

            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    let s = session.borrow();
                    let title = s.active_title();
                    let model = s.model();
                    let template = SessionLayoutTemplate::from_session(&title, &model);
                    if let Ok(json) = template.to_json() {
                        let path = crate::model::AppConfig::config_dir().join("templates");
                        let _ = std::fs::create_dir_all(&path);
                        let file_name = format!("{}.json", title.replace(['/', '\\', ' '], "_"));
                        let _ = std::fs::write(path.join(&file_name), &json);
                        let _ = std::fs::write(path.join("latest.json"), &json);
                        if let Some(display) = gtk::gdk::Display::default() {
                            display.clipboard().set_text(&json);
                        }
                    }
                }
            });
            self.window.add_action(&action);
        }

        // Import Session Layout
        {
            let action = gio::SimpleAction::new("import-session-layout", None);
            let tv_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            let next_id = Rc::clone(&self.next_session_id);

            action.connect_activate(move |_, _| {
                let Some(tv) = tv_weak.upgrade() else { return; };

                let mut template_opt = None;
                let path = crate::model::AppConfig::config_dir().join("templates").join("latest.json");
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(tpl) = SessionLayoutTemplate::from_json(&content) {
                        template_opt = Some(tpl);
                    }
                }

                let template = template_opt.unwrap_or_else(|| {
                    let mut tree = crate::model::LayoutTree::new(crate::model::PaneId(1));
                    let _ = tree.split(
                        crate::model::PaneId(1),
                        SplitOrientation::Horizontal,
                        crate::model::PaneId(2),
                    );
                    SessionLayoutTemplate::new("Default Layout", None, tree)
                });

                let (start_pane_id, start_split_id) = {
                    let mut id = next_id.borrow_mut();
                    let p_id = *id;
                    let s_id = *id;
                    *id += 100;
                    (p_id, s_id)
                };
                let session_model = template.instantiate_session(start_pane_id, start_split_id);
                Self::create_tab_internal(&tv, &sessions, &next_id, Some(session_model), None);
            });
            self.window.add_action(&action);
        }

        // Preferences
        {
            let action = gio::SimpleAction::new("preferences", None);
            let win_weak = self.window.downgrade();
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            let pref_holder = Rc::clone(&self.preferences_window);

            action.connect_activate(move |_, _| {
                let Some(win) = win_weak.upgrade() else { return; };
                if let Ok(mut holder) = pref_holder.try_borrow_mut() {
                    if let Some(ref pref) = *holder {
                        if pref.window().is_visible() {
                            pref.present();
                            return;
                        }
                    }
                    let pref = TilixPreferencesWindow::new(Some(&win), move |profile| {
                        apply_profile_to_all_sessions(profile);
                    });

                    let tv_weak = tab_view_weak.clone();
                    let sess_map = Rc::clone(&sessions);
                    let holder_weak = Rc::clone(&pref_holder);

                    pref.window().connect_close_request(move |_| {
                        if let Ok(mut h) = holder_weak.try_borrow_mut() {
                            *h = None;
                        }
                        let tv_opt = tv_weak.upgrade();
                        let s_map = Rc::clone(&sess_map);
                        glib::idle_add_local_once(move || {
                            if let Some(tv) = tv_opt {
                                refocus_active_pane(&tv, &s_map);
                            }
                        });
                        glib::Propagation::Proceed
                    });

                    let tv_weak2 = tab_view_weak.clone();
                    let sess_map2 = Rc::clone(&sessions);
                    pref.window().connect_destroy(move |_| {
                        let tv_opt = tv_weak2.upgrade();
                        let s_map = Rc::clone(&sess_map2);
                        glib::idle_add_local_once(move || {
                            if let Some(tv) = tv_opt {
                                refocus_active_pane(&tv, &s_map);
                            }
                        });
                    });

                    pref.present();
                    *holder = Some(pref);
                }
            });
            self.window.add_action(&action);
        }

        // Split Right
        {
            let action = gio::SimpleAction::new("split-right", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().split_active(SplitOrientation::Horizontal);
                }
            });
            self.window.add_action(&action);
        }

        // Split Down
        {
            let action = gio::SimpleAction::new("split-down", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().split_active(SplitOrientation::Vertical);
                }
            });
            self.window.add_action(&action);
        }

        // Balance Layout
        {
            let action = gio::SimpleAction::new("balance-layout", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().balance_layout();
                    session.borrow().grab_focus();
                    let s_weak = Rc::downgrade(&session);
                    glib::idle_add_local_once(move || {
                        if let Some(s) = s_weak.upgrade() {
                            s.borrow().grab_focus();
                        }
                    });
                }
            });
            self.window.add_action(&action);
        }

        // Toggle Sync Input
        {
            let action = gio::SimpleAction::new_stateful(
                "toggle-sync-input",
                None,
                &false.to_variant(),
            );
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);

            action.connect_activate(move |act, _| {
                let current = act.state().and_then(|s| s.get::<bool>()).unwrap_or(false);
                let new_state = !current;
                act.set_state(&new_state.to_variant());

                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().set_sync_input_enabled(new_state);
                    session.borrow().grab_focus();
                    let s_weak = Rc::downgrade(&session);
                    glib::idle_add_local_once(move || {
                        if let Some(s) = s_weak.upgrade() {
                            s.borrow().grab_focus();
                        }
                    });
                }
            });
            self.window.add_action(&action);

            // Save Layout ("win.save-layout" from menu)
            {
                let action = gio::SimpleAction::new("save-layout", None);
                let tab_view_weak = self.tab_view.downgrade();
                let sessions = Rc::clone(&self.sessions);

                action.connect_activate(move |_, _| {
                    let Some(tv) = tab_view_weak.upgrade() else { return; };
                    let Some(page) = tv.selected_page() else { return; };
                    let session_opt = sessions.borrow().get(&page).cloned();
                    if let Some(session) = session_opt {
                        let s = session.borrow();
                        let title = s.active_title();
                        let model = s.model();
                        let template = SessionLayoutTemplate::from_session(&title, &model);
                        if let Ok(json) = template.to_json() {
                            let path = crate::model::AppConfig::config_dir().join("templates");
                            let _ = std::fs::create_dir_all(&path);
                            let file_name = format!("{}.json", title.replace(['/', '\\', ' '], "_"));
                            let _ = std::fs::write(path.join(&file_name), &json);
                            let _ = std::fs::write(path.join("latest.json"), &json);
                            if let Some(display) = gtk::gdk::Display::default() {
                                display.clipboard().set_text(&json);
                            }
                        }
                        drop(model);
                        drop(s);
                        session.borrow().grab_focus();
                        let s_weak = Rc::downgrade(&session);
                        glib::idle_add_local_once(move || {
                            if let Some(s) = s_weak.upgrade() {
                                s.borrow().grab_focus();
                            }
                        });
                    }
                });
                self.window.add_action(&action);
            }

            // Shortcuts ("win.shortcuts" from menu)
            {
                let action = gio::SimpleAction::new("shortcuts", None);
                let win_weak = self.window.downgrade();
                let tab_view_weak = self.tab_view.downgrade();
                let sessions = Rc::clone(&self.sessions);
                let pref_holder = Rc::clone(&self.preferences_window);

                action.connect_activate(move |_, _| {
                    let Some(win) = win_weak.upgrade() else { return; };
                    if let Ok(mut holder) = pref_holder.try_borrow_mut() {
                        if let Some(ref pref) = *holder {
                            if pref.window().is_visible() {
                                pref.set_visible_page_name("shortcuts");
                                pref.present();
                                return;
                            }
                        }
                        let pref = TilixPreferencesWindow::new(Some(&win), move |profile| {
                            apply_profile_to_all_sessions(profile);
                        });
                        pref.set_visible_page_name("shortcuts");

                        let tv_weak = tab_view_weak.clone();
                        let sess_map = Rc::clone(&sessions);
                        let holder_weak = Rc::clone(&pref_holder);

                        pref.window().connect_close_request(move |_| {
                            if let Ok(mut h) = holder_weak.try_borrow_mut() {
                                *h = None;
                            }
                            let tv_opt = tv_weak.upgrade();
                            let s_map = Rc::clone(&sess_map);
                            glib::idle_add_local_once(move || {
                                if let Some(tv) = tv_opt {
                                    refocus_active_pane(&tv, &s_map);
                                }
                            });
                            glib::Propagation::Proceed
                        });

                        let tv_weak2 = tab_view_weak.clone();
                        let sess_map2 = Rc::clone(&sessions);
                        pref.window().connect_destroy(move |_| {
                            let tv_opt = tv_weak2.upgrade();
                            let s_map = Rc::clone(&sess_map2);
                            glib::idle_add_local_once(move || {
                                if let Some(tv) = tv_opt {
                                    refocus_active_pane(&tv, &s_map);
                                }
                            });
                        });

                        pref.present();
                        *holder = Some(pref);
                    }
                });
                self.window.add_action(&action);
            }

            // About Tilix ("win.about" from menu)
            {
                let action = gio::SimpleAction::new("about", None);
                let win_weak = self.window.downgrade();
                let tab_view_weak = self.tab_view.downgrade();
                let sessions = Rc::clone(&self.sessions);

                action.connect_activate(move |_, _| {
                    let Some(win) = win_weak.upgrade() else { return; };
                    let about = gtk::AboutDialog::builder()
                        .transient_for(&win)
                        .modal(true)
                        .program_name("Tilix")
                        .version("0.1.0")
                        .comments("A tiling terminal emulator for Linux")
                        .website("https://github.com/gnunn1/tilix")
                        .license_type(gtk::License::Gpl30)
                        .build();

                    let tv_weak = tab_view_weak.clone();
                    let sess_map = Rc::clone(&sessions);
                    about.connect_close_request(move |_| {
                        let tv_opt = tv_weak.upgrade();
                        let s_map = Rc::clone(&sess_map);
                        glib::idle_add_local_once(move || {
                            if let Some(tv) = tv_opt {
                                refocus_active_pane(&tv, &s_map);
                            }
                        });
                        glib::Propagation::Proceed
                    });

                    about.present();
                });
                self.window.add_action(&action);
            }

            // Sync action state with active tab when tab selection changes
            let action_weak = action.downgrade();
            let sessions_for_sync = Rc::clone(&self.sessions);
            self.tab_view.connect_selected_page_notify(move |tv| {
                let Some(page) = tv.selected_page() else { return; };
                let Some(act) = action_weak.upgrade() else { return; };
                let session_opt = sessions_for_sync.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    let is_sync = session.borrow().sync_input_enabled();
                    act.set_state(&is_sync.to_variant());
                    session.borrow().grab_focus();
                }
            });
        }

        // Tab Next
        {
            let action = gio::SimpleAction::new("tab-next", None);
            let tab_view_weak = self.tab_view.downgrade();
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let n = tv.n_pages();
                if n > 1 {
                    if let Some(page) = tv.selected_page() {
                        let pos = tv.page_position(&page);
                        let next_pos = (pos + 1) % n;
                        let next_page = tv.nth_page(next_pos);
                        tv.set_selected_page(&next_page);
                    }
                }
            });
            self.window.add_action(&action);
        }

        // Tab Prev
        {
            let action = gio::SimpleAction::new("tab-prev", None);
            let tab_view_weak = self.tab_view.downgrade();
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let n = tv.n_pages();
                if n > 1 {
                    if let Some(page) = tv.selected_page() {
                        let pos = tv.page_position(&page);
                        let prev_pos = if pos == 0 { n - 1 } else { pos - 1 };
                        let prev_page = tv.nth_page(prev_pos);
                        tv.set_selected_page(&prev_page);
                    }
                }
            });
            self.window.add_action(&action);
        }

        // Switch Tab 1..9
        for i in 1..=9 {
            let action_name = format!("switch-tab-{}", i);
            let action = gio::SimpleAction::new(&action_name, None);
            let tab_view_weak = self.tab_view.downgrade();
            let idx = i - 1;
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                if tv.n_pages() > idx {
                    let page = tv.nth_page(idx);
                    tv.set_selected_page(&page);
                }
            });
            self.window.add_action(&action);
        }

        // Directional focus
        let directions = [
            ("focus-up", Direction::Up),
            ("focus-down", Direction::Down),
            ("focus-left", Direction::Left),
            ("focus-right", Direction::Right),
        ];

        for (name, dir) in directions {
            let action = gio::SimpleAction::new(name, None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().focus_adjacent(dir);
                }
            });
            self.window.add_action(&action);
        }

        // Toggle Tab Bar
        {
            let action = gio::SimpleAction::new("toggle-tab-bar", None);
            action.connect_activate(move |_, _| {
                let mut cfg = crate::model::AppConfig::load();
                cfg.show_tab_bar = !cfg.show_tab_bar;
                let _ = cfg.save();
                apply_show_tab_bar_to_all_windows(cfg.show_tab_bar);
            });
            self.window.add_action(&action);
        }

        // Zoom In
        {
            let action = gio::SimpleAction::new("zoom-in", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().zoom_in_active();
                }
            });
            self.window.add_action(&action);
        }

        // Zoom Out
        {
            let action = gio::SimpleAction::new("zoom-out", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().zoom_out_active();
                }
            });
            self.window.add_action(&action);
        }

        // Zoom Normal
        {
            let action = gio::SimpleAction::new("zoom-normal", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().zoom_normal_active();
                }
            });
            self.window.add_action(&action);
        }

        // Copy
        {
            let action = gio::SimpleAction::new("copy", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().copy_clipboard_active();
                }
            });
            self.window.add_action(&action);
        }

        // Copy as HTML
        {
            let action = gio::SimpleAction::new("copy-html", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().copy_html_active();
                }
            });
            self.window.add_action(&action);
        }

        // Paste
        {
            let action = gio::SimpleAction::new("paste", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().paste_clipboard_active();
                }
            });
            self.window.add_action(&action);
        }

        // Paste Primary Selection
        {
            let action = gio::SimpleAction::new("paste-primary", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().paste_primary_active();
                }
            });
            self.window.add_action(&action);
        }

        // Cut
        {
            let action = gio::SimpleAction::new("cut", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().copy_clipboard_active();
                }
            });
            self.window.add_action(&action);
        }

        // Select All
        {
            let action = gio::SimpleAction::new("select-all", None);
            let tab_view_weak = self.tab_view.downgrade();
            let sessions = Rc::clone(&self.sessions);
            action.connect_activate(move |_, _| {
                let Some(tv) = tab_view_weak.upgrade() else { return; };
                let Some(page) = tv.selected_page() else { return; };
                let session_opt = sessions.borrow().get(&page).cloned();
                if let Some(session) = session_opt {
                    session.borrow().select_all_active();
                }
            });
            self.window.add_action(&action);
        }
    }

    pub fn window(&self) -> &adw::ApplicationWindow {
        &self.window
    }

    pub fn tab_view(&self) -> &adw::TabView {
        &self.tab_view
    }

    pub fn title_widget(&self) -> &adw::WindowTitle {
        &self.title_widget
    }

    pub fn update_window_and_tab_titles(&self) {
        compute_and_apply_window_title(&self.window, &self.tab_view, &self.title_widget);
    }

    pub fn session_view(&self) -> Option<Rc<RefCell<SessionView>>> {
        self.active_session()
    }

    pub fn grab_focus(&self) {
        refocus_active_pane(&self.tab_view, &self.sessions);
    }

    pub fn present(&self) {
        self.window.present();
        self.grab_focus();
    }

    pub fn apply_profile(&self, profile: &Profile) {
        for session in self.sessions.borrow().values() {
            session.borrow().apply_profile(profile);
        }
        self.update_transparency();
    }
}

#[cfg(test)]
pub(crate) fn run_gtk_test<F: FnOnce() + Send + 'static>(f: F) {
    static GTK_TEST_POOL: std::sync::OnceLock<Option<glib::ThreadPool>> = std::sync::OnceLock::new();
    let pool = GTK_TEST_POOL.get_or_init(|| {
        let (init_tx, init_rx) = std::sync::mpsc::sync_channel(1);
        let Ok(pool) = glib::ThreadPool::exclusive(1) else {
            return None;
        };
        if pool
            .push(move || {
                let ok = gtk::init().is_ok();
                let _ = init_tx.send(ok);
            })
            .is_err()
        {
            return None;
        }
        if init_rx.recv().unwrap_or(false) {
            Some(pool)
        } else {
            None
        }
    });

    if let Some(pool) = pool.as_ref() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        pool.push(move || {
            let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
            let _ = tx.send(res);
        })
        .expect("failed to push test");
        match rx.recv().expect("failed to wait for test") {
            Ok(()) => {}
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_new_tab_action_after_drop() {
        run_gtk_test(|| {
            let app = adw::Application::builder()
                .application_id("com.github.tilix_rust.test_window_tab")
                .flags(gio::ApplicationFlags::NON_UNIQUE)
                .build();

            let tilix_win = TilixWindow::new(&app);
            let window = tilix_win.window().clone();
            let tab_view = tilix_win.tab_view().clone();

            assert_eq!(tab_view.n_pages(), 1);

            // Drop the TilixWindow struct to simulate it leaving scope in connect_activate / connect_command_line
            drop(tilix_win);

            // Activate win.new-tab action (which the HeaderBar button and Ctrl+Shift+T invoke)
            let action = window
                .lookup_action("new-tab")
                .expect("new-tab action must exist");
            action.activate(None);

            // Verify that a second tab was created
            assert_eq!(tab_view.n_pages(), 2);

            // Activate new-tab action again
            action.activate(None);
            assert_eq!(tab_view.n_pages(), 3);

            // Test tab navigation
            let next_action = window
                .lookup_action("tab-next")
                .expect("tab-next action must exist");
            next_action.activate(None);

            let switch_action = window
                .lookup_action("switch-tab-1")
                .expect("switch-tab-1 action must exist");
            switch_action.activate(None);

            // Test split-right on the active tab
            let split_action = window
                .lookup_action("split-right")
                .expect("split-right action must exist");
            split_action.activate(None);

            // Test close-pane closes split first
            let close_action = window
                .lookup_action("close-pane")
                .expect("close-pane action must exist");
            close_action.activate(None);
            assert_eq!(tab_view.n_pages(), 3);

            // Close one of the tabs
            close_action.activate(None);
            assert_eq!(tab_view.n_pages(), 2);

            // Test preferences action and profile broadcast
            let pref_action = window
                .lookup_action("preferences")
                .expect("preferences action must exist");
            pref_action.activate(None);

            let test_profile = Profile {
                color_scheme: crate::model::ColorScheme::monokai(),
                ..Default::default()
            };
            apply_profile_to_all_sessions(&test_profile);
        });
    }

    #[test]
    fn test_window_style_header_bar_toggle() {
        run_gtk_test(|| {
            let app = adw::Application::builder()
                .application_id("com.github.tilix_rust.test_window_style")
                .flags(gio::ApplicationFlags::NON_UNIQUE)
                .build();
            let tilix_win = TilixWindow::new_empty(&app);

            let header_bar = WINDOW_HEADER_BARS
                .with(|bars| bars.borrow().last().and_then(|w| w.upgrade()))
                .expect("Header bar must be registered");

            apply_window_style_to_all_windows(WindowStyle::HideToolbar);
            assert!(!header_bar.get_visible());

            apply_window_style_to_all_windows(WindowStyle::Normal);
            assert!(header_bar.get_visible());

            drop(tilix_win);
        });
    }

    #[test]
    fn test_apply_pane_title_settings_to_all_sessions() {
        run_gtk_test(|| {
            let session = Rc::new(RefCell::new(SessionView::new()));
            let widget = session.borrow().widget().clone();
            register_session_widget(&widget, Rc::clone(&session));

            // Default: Normal, true
            assert_eq!(session.borrow().pane_title_style(), PaneTitleStyle::Normal);
            assert!(session.borrow().pane_title_show_when_single());

            // Broadcast None, false
            apply_pane_title_settings_to_all_sessions(PaneTitleStyle::None, false);
            assert_eq!(session.borrow().pane_title_style(), PaneTitleStyle::None);
            assert!(!session.borrow().pane_title_show_when_single());

            // Broadcast Normal, true
            apply_pane_title_settings_to_all_sessions(PaneTitleStyle::Normal, true);
            assert_eq!(session.borrow().pane_title_style(), PaneTitleStyle::Normal);
            assert!(session.borrow().pane_title_show_when_single());

            unregister_session_widget(&widget);
            session.borrow().close();
        });
    }

    #[test]
    fn test_show_tab_bar_toggle() {
        run_gtk_test(|| {
            let mut init_cfg = crate::model::AppConfig::load();
            init_cfg.show_tab_bar = true;
            let _ = init_cfg.save();

            let app = adw::Application::builder()
                .application_id("com.github.tilix_rust.test_show_tab_bar")
                .flags(gio::ApplicationFlags::NON_UNIQUE)
                .build();
            let tilix_win = TilixWindow::new_empty(&app);

            let tab_bar = WINDOW_TAB_BARS
                .with(|bars| bars.borrow().last().and_then(|w| w.upgrade()))
                .expect("Tab bar must be registered");

            // Initial state (default show_tab_bar is true)
            assert!(tab_bar.get_visible());

            // Broadcast hide
            apply_show_tab_bar_to_all_windows(false);
            assert!(!tab_bar.get_visible());

            // Broadcast show
            apply_show_tab_bar_to_all_windows(true);
            assert!(tab_bar.get_visible());

            // Test toggle-tab-bar action
            let action = tilix_win
                .window()
                .lookup_action("toggle-tab-bar")
                .expect("toggle-tab-bar action must exist");
            action.activate(None);

            let cfg = crate::model::AppConfig::load();
            assert_eq!(tab_bar.get_visible(), cfg.show_tab_bar);
            assert!(!tab_bar.get_visible());

            // Reset back to default true
            let mut reset_cfg = crate::model::AppConfig::load();
            reset_cfg.show_tab_bar = true;
            let _ = reset_cfg.save();

            drop(tilix_win);
        });
    }

    #[test]
    fn test_apply_keybindings_to_app_headless() {
        run_gtk_test(|| {
            let app = adw::Application::builder()
                .application_id("com.github.tilix_rust.test_keybindings")
                .flags(gio::ApplicationFlags::NON_UNIQUE)
                .build();
            let mut keybindings = crate::model::KeybindingsConfig::default();
            keybindings.set_custom_accel("win.new-tab", "<Primary>t");
            keybindings.set_custom_accel("win.close-tab", "<Primary>w");
            apply_keybindings_to_app(&app, &keybindings);
            apply_keybindings_globally(&keybindings);
        });
    }

    #[test]
    fn test_tilix_preferences_window_standalone_headless() {
        run_gtk_test(|| {
            let app = adw::Application::builder()
                .application_id("com.github.tilix_rust.test_pref_standalone")
                .flags(gio::ApplicationFlags::NON_UNIQUE)
                .build();
            let pref = crate::ui::preferences::TilixPreferencesWindow::new(None::<&gtk::Window>, |_| {});
            pref.window().set_application(Some(&app));
            assert!(!pref.window().is_modal());
            assert_eq!(pref.window().transient_for(), None);
        });
    }

    #[test]
    fn test_setup_css_loads_without_errors() {
        run_gtk_test(|| {
            setup_css();
        });
    }

    #[test]
    fn test_apply_compact_mode_to_all_windows() {
        run_gtk_test(|| {
            let app = adw::Application::builder()
                .application_id("com.github.tilix_rust.test_compact_projection")
                .flags(gio::ApplicationFlags::NON_UNIQUE)
                .build();
            let win = adw::ApplicationWindow::new(&app);
            register_window_instance(&win);

            assert!(!win.has_css_class("compact"));

            apply_compact_mode_to_all_windows(true);
            assert!(win.has_css_class("compact"));

            apply_compact_mode_to_all_windows(false);
            assert!(!win.has_css_class("compact"));
        });
    }

    #[test]
    fn test_window_headerbar_buttons_non_focusable() {
        run_gtk_test(|| {
            let app = adw::Application::builder()
                .application_id("com.github.tilix_rust.test_header_buttons_focus")
                .flags(gio::ApplicationFlags::NON_UNIQUE)
                .build();
            let _ = app.register(gio::Cancellable::NONE);
            let tilix_win = TilixWindow::new(&app);
            let win = tilix_win.window();

            // Find all Tilix custom headerbar action buttons and menu buttons
            let mut action_buttons = Vec::new();
            fn collect_action_buttons(widget: &gtk::Widget, buttons: &mut Vec<gtk::Widget>) {
                if let Some(btn) = widget.downcast_ref::<gtk::Button>() {
                    if let Some(action) = btn.action_name() {
                        if action.starts_with("win.") {
                            buttons.push(widget.clone());
                        }
                    }
                } else if let Some(tbtn) = widget.downcast_ref::<gtk::ToggleButton>() {
                    if let Some(action) = tbtn.action_name() {
                        if action.starts_with("win.") {
                            buttons.push(widget.clone());
                        }
                    }
                } else if widget.is::<gtk::MenuButton>() {
                    buttons.push(widget.clone());
                }
                let mut child = widget.first_child();
                while let Some(c) = child {
                    collect_action_buttons(&c, buttons);
                    child = c.next_sibling();
                }
            }

            collect_action_buttons(win.upcast_ref(), &mut action_buttons);

            assert!(
                action_buttons.len() >= 5,
                "Expected at least 5 header action buttons, found {}",
                action_buttons.len()
            );
            for btn in &action_buttons {
                assert!(
                    !btn.is_focusable(),
                    "Header action button of type {} must have focusable=false",
                    btn.type_().name()
                );
            }
        });
    }

    #[test]
    fn test_toggle_sync_input_and_balance_layout_refocus_pane() {
        run_gtk_test(|| {
            let app = adw::Application::builder()
                .application_id("com.github.tilix_rust.test_actions_refocus")
                .flags(gio::ApplicationFlags::NON_UNIQUE)
                .build();
            let _ = app.register(gio::Cancellable::NONE);
            let tilix_win = TilixWindow::new(&app);
            tilix_win.present();
            let session = tilix_win.session_view().expect("session must exist");

            let pane = session.borrow().active_pane().expect("pane must exist");

            // Trigger toggle-sync-input
            let action = tilix_win
                .window()
                .lookup_action("toggle-sync-input")
                .expect("toggle-sync-input action must exist");
            action.activate(None);

            // Process idle callbacks
            let ctx = glib::MainContext::default();
            while ctx.iteration(false) {}

            let is_focused = pane.terminal().is_focus() || pane.terminal().has_focus();
            assert!(
                is_focused,
                "Active terminal pane should have focus within window after toggle-sync-input"
            );

            // Trigger balance-layout
            let bal_action = tilix_win
                .window()
                .lookup_action("balance-layout")
                .expect("balance-layout action must exist");
            bal_action.activate(None);

            while ctx.iteration(false) {}

            let is_focused_after_bal = pane.terminal().is_focus() || pane.terminal().has_focus();
            assert!(
                is_focused_after_bal,
                "Active terminal pane should have focus within window after balance-layout"
            );
        });
    }

    #[test]
    fn test_preferences_window_close_refocuses_pane() {
        run_gtk_test(|| {
            let app = adw::Application::builder()
                .application_id("com.github.tilix_rust.test_pref_close_refocus")
                .flags(gio::ApplicationFlags::NON_UNIQUE)
                .build();
            let _ = app.register(gio::Cancellable::NONE);
            let tilix_win = TilixWindow::new(&app);
            tilix_win.present();
            let session = tilix_win.session_view().expect("session must exist");

            let pane = session.borrow().active_pane().expect("pane must exist");

            // Open preferences
            let pref_action = tilix_win
                .window()
                .lookup_action("preferences")
                .expect("preferences action must exist");
            pref_action.activate(None);

            let ctx = glib::MainContext::default();
            while ctx.iteration(false) {}

            let pref_window = tilix_win.preferences_window.borrow().clone();
            assert!(pref_window.is_some(), "Preferences window should be opened");
            let pref = pref_window.unwrap();

            // Simulate closing preferences window
            pref.window().close();

            while ctx.iteration(false) {}

            let is_focused = pane.terminal().is_focus() || pane.terminal().has_focus();
            assert!(
                is_focused,
                "Active terminal pane should regain focus within window after preferences window closes"
            );
        });
    }
}

