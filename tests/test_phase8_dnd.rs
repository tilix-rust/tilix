#![allow(dead_code)]
#![allow(unused_imports)]

use std::cell::RefCell;
use std::rc::Rc;

#[path = "../src/model/mod.rs"]
pub mod model;

#[path = "../src/pty/mod.rs"]
pub mod pty;

#[path = "../src/ui/mod.rs"]
pub mod ui;

#[path = "../src/app.rs"]
pub mod app;

use model::layout::{
    calculate_dock_position, DockPosition, LayoutError, LayoutNode, LayoutTree, PaneId, SplitId,
    SplitOrientation,
};
use model::session::{SessionModel, SyncGroupId};
use ui::session_view::SessionView;
use ui::terminal_pane::TerminalPane;
use ui::window::run_gtk_test;

// ============================================================================
// 1. Pure Mathematical Zone Calculation Tests
// ============================================================================

#[test]
fn test_calculate_dock_position_center_zone_comprehensive() {
    let w = 400.0;
    let h = 400.0;

    // Exact center
    assert_eq!(
        calculate_dock_position(200.0, 200.0, w, h),
        DockPosition::Center
    );

    // Inner center zone boundaries (0.25 to 0.75)
    // For 400x400: x in [100, 300], y in [100, 300]
    assert_eq!(
        calculate_dock_position(150.0, 150.0, w, h),
        DockPosition::Center
    );
    assert_eq!(
        calculate_dock_position(250.0, 250.0, w, h),
        DockPosition::Center
    );
    assert_eq!(
        calculate_dock_position(101.0, 200.0, w, h),
        DockPosition::Center
    );
    assert_eq!(
        calculate_dock_position(299.0, 200.0, w, h),
        DockPosition::Center
    );
    assert_eq!(
        calculate_dock_position(200.0, 101.0, w, h),
        DockPosition::Center
    );
    assert_eq!(
        calculate_dock_position(200.0, 299.0, w, h),
        DockPosition::Center
    );
}

#[test]
fn test_calculate_dock_position_directional_zones_comprehensive() {
    let w = 400.0;
    let h = 400.0;

    // Direct cardinal regions
    // Top border (y < 100, x centered)
    assert_eq!(calculate_dock_position(200.0, 20.0, w, h), DockPosition::Top);
    assert_eq!(calculate_dock_position(200.0, 0.0, w, h), DockPosition::Top);

    // Bottom border (y > 300, x centered)
    assert_eq!(
        calculate_dock_position(200.0, 380.0, w, h),
        DockPosition::Bottom
    );
    assert_eq!(
        calculate_dock_position(200.0, 400.0, w, h),
        DockPosition::Bottom
    );

    // Left border (x < 100, y centered)
    assert_eq!(calculate_dock_position(20.0, 200.0, w, h), DockPosition::Left);
    assert_eq!(calculate_dock_position(0.0, 200.0, w, h), DockPosition::Left);

    // Right border (x > 300, y centered)
    assert_eq!(
        calculate_dock_position(380.0, 200.0, w, h),
        DockPosition::Right
    );
    assert_eq!(
        calculate_dock_position(400.0, 200.0, w, h),
        DockPosition::Right
    );

    // Corner proximity disambiguation (distance to edge determines zone)
    // Near top-left corner, closer to top
    assert_eq!(calculate_dock_position(50.0, 20.0, w, h), DockPosition::Top);
    // Near top-left corner, closer to left
    assert_eq!(calculate_dock_position(20.0, 50.0, w, h), DockPosition::Left);

    // Near bottom-right corner, closer to bottom
    assert_eq!(
        calculate_dock_position(350.0, 390.0, w, h),
        DockPosition::Bottom
    );
    // Near bottom-right corner, closer to right
    assert_eq!(
        calculate_dock_position(390.0, 350.0, w, h),
        DockPosition::Right
    );
}

#[test]
fn test_calculate_dock_position_boundary_clamping_and_degenerate() {
    let w = 200.0;
    let h = 100.0;

    // Out of bounds / negative clamped to border
    assert_eq!(
        calculate_dock_position(-50.0, 50.0, w, h),
        DockPosition::Left
    );
    assert_eq!(
        calculate_dock_position(100.0, -50.0, w, h),
        DockPosition::Top
    );
    assert_eq!(
        calculate_dock_position(300.0, 50.0, w, h),
        DockPosition::Right
    );
    assert_eq!(
        calculate_dock_position(100.0, 200.0, w, h),
        DockPosition::Bottom
    );

    // Extreme negative coordinates
    assert_eq!(
        calculate_dock_position(-9999.0, -9999.0, w, h),
        DockPosition::Top // Clamped to (0, 0), top or left tie-break
    );

    // Zero and negative dimension degenerate cases default safely to Center
    assert_eq!(
        calculate_dock_position(50.0, 50.0, 0.0, 100.0),
        DockPosition::Center
    );
    assert_eq!(
        calculate_dock_position(50.0, 50.0, 100.0, 0.0),
        DockPosition::Center
    );
    assert_eq!(
        calculate_dock_position(50.0, 50.0, -10.0, 100.0),
        DockPosition::Center
    );
    assert_eq!(
        calculate_dock_position(50.0, 50.0, 100.0, -10.0),
        DockPosition::Center
    );
}

#[test]
fn test_calculate_dock_position_asymmetric_aspect_ratios() {
    // Ultra-wide pane (800 x 200)
    let w_wide = 800.0;
    let h_wide = 200.0;
    // Normalized x in [0.25, 0.75] -> [200, 600]
    // Normalized y in [0.25, 0.75] -> [50, 150]
    assert_eq!(
        calculate_dock_position(400.0, 100.0, w_wide, h_wide),
        DockPosition::Center
    );
    assert_eq!(
        calculate_dock_position(100.0, 100.0, w_wide, h_wide),
        DockPosition::Left
    );
    assert_eq!(
        calculate_dock_position(700.0, 100.0, w_wide, h_wide),
        DockPosition::Right
    );
    assert_eq!(
        calculate_dock_position(400.0, 20.0, w_wide, h_wide),
        DockPosition::Top
    );
    assert_eq!(
        calculate_dock_position(400.0, 180.0, w_wide, h_wide),
        DockPosition::Bottom
    );

    // Tall pane (200 x 800)
    let w_tall = 200.0;
    let h_tall = 800.0;
    assert_eq!(
        calculate_dock_position(100.0, 400.0, w_tall, h_tall),
        DockPosition::Center
    );
    assert_eq!(
        calculate_dock_position(20.0, 400.0, w_tall, h_tall),
        DockPosition::Left
    );
    assert_eq!(
        calculate_dock_position(180.0, 400.0, w_tall, h_tall),
        DockPosition::Right
    );
    assert_eq!(
        calculate_dock_position(100.0, 50.0, w_tall, h_tall),
        DockPosition::Top
    );
    assert_eq!(
        calculate_dock_position(100.0, 750.0, w_tall, h_tall),
        DockPosition::Bottom
    );
}

// ============================================================================
// 2. Complex LayoutTree Multi-Step Drag & Docking Sequences
// ============================================================================

#[test]
fn test_layout_tree_complex_docking_and_repositioning_pipeline() {
    // Pipeline:
    // 1. Start with single Pane 1
    let mut tree = LayoutTree::new(PaneId(1));
    assert_eq!(tree.panes(), vec![PaneId(1)]);

    // 2. Insert Pane 2 to the Right of Pane 1 -> Horizontal split [1, 2]
    tree.insert_pane_dock(PaneId(2), PaneId(1), DockPosition::Right)
        .unwrap();
    assert_eq!(tree.panes(), vec![PaneId(1), PaneId(2)]);

    // 3. Insert Pane 3 to the Bottom of Pane 2 -> [1, [2 / 3]]
    tree.insert_pane_dock(PaneId(3), PaneId(2), DockPosition::Bottom)
        .unwrap();
    assert_eq!(tree.panes(), vec![PaneId(1), PaneId(2), PaneId(3)]);

    // 4. Insert Pane 4 to the Left of Pane 1 -> [4, [1, [2 / 3]]]
    tree.insert_pane_dock(PaneId(4), PaneId(1), DockPosition::Left)
        .unwrap();
    assert_eq!(
        tree.panes(),
        vec![PaneId(4), PaneId(1), PaneId(2), PaneId(3)]
    );

    // 5. Re-dock Pane 3 to the Top of Pane 1 (intra-tree docking)
    // Pane 3 should be excised from the right branch (promoting Pane 2)
    // and inserted into a vertical split above Pane 1.
    tree.dock_pane(PaneId(3), PaneId(1), DockPosition::Top)
        .unwrap();
    assert_eq!(
        tree.panes(),
        vec![PaneId(4), PaneId(3), PaneId(1), PaneId(2)]
    );

    // 6. Dock Pane 4 to Center of Pane 2 -> Swaps Pane 4 and Pane 2!
    tree.dock_pane(PaneId(4), PaneId(2), DockPosition::Center)
        .unwrap();
    assert_eq!(
        tree.panes(),
        vec![PaneId(2), PaneId(3), PaneId(1), PaneId(4)]
    );

    // 7. Dock pane onto itself -> Safe no-op
    tree.dock_pane(PaneId(3), PaneId(3), DockPosition::Left)
        .unwrap();
    assert_eq!(
        tree.panes(),
        vec![PaneId(2), PaneId(3), PaneId(1), PaneId(4)]
    );

    // 8. Re-dock Pane 1 to the Right of Pane 4
    tree.dock_pane(PaneId(1), PaneId(4), DockPosition::Right)
        .unwrap();
    assert_eq!(
        tree.panes(),
        vec![PaneId(2), PaneId(3), PaneId(4), PaneId(1)]
    );

    // 9. Error cases
    // Target does not exist
    assert!(matches!(
        tree.dock_pane(PaneId(1), PaneId(999), DockPosition::Top),
        Err(LayoutError::PaneNotFound(PaneId(999)))
    ));
    // Source does not exist
    assert!(matches!(
        tree.dock_pane(PaneId(888), PaneId(1), DockPosition::Top),
        Err(LayoutError::PaneNotFound(PaneId(888)))
    ));
    // External insertion with Center is invalid
    assert!(matches!(
        tree.insert_pane_dock(PaneId(5), PaneId(1), DockPosition::Center),
        Err(LayoutError::InvalidSplit)
    ));
    // External insertion with already-existing pane id
    assert!(matches!(
        tree.insert_pane_dock(PaneId(2), PaneId(1), DockPosition::Left),
        Err(LayoutError::InvalidSplit)
    ));
}

// ============================================================================
// 3. Headless Verification of SessionModel Focus History & Sync State
// ============================================================================

#[test]
fn test_session_model_docking_removal_and_focus_history_preservation() {
    let mut session = SessionModel::new(PaneId(1));
    assert_eq!(session.active_pane, Some(PaneId(1)));

    // Split horizontally: Pane 1 -> Pane 2
    let p2 = session.split_active(SplitOrientation::Horizontal).unwrap();
    assert_eq!(p2, PaneId(2));
    assert_eq!(session.active_pane, Some(PaneId(2)));

    // Split vertically: Pane 2 -> Pane 3
    let p3 = session.split_active(SplitOrientation::Vertical).unwrap();
    assert_eq!(p3, PaneId(3));
    assert_eq!(session.active_pane, Some(PaneId(3)));

    // Setup sync groups: Group 1 contains Pane 1 and Pane 2
    let sync_gid = SyncGroupId(10);
    session.sync_groups.insert(PaneId(1), sync_gid);
    session.sync_groups.insert(PaneId(2), sync_gid);
    assert_eq!(session.sync_groups.get(&PaneId(1)), Some(&sync_gid));
    assert_eq!(session.sync_groups.get(&PaneId(2)), Some(&sync_gid));
    assert_eq!(session.sync_groups.get(&PaneId(3)), None);

    // Switch focus: P3 -> P1 -> P2
    session.set_active_pane(PaneId(1));
    assert_eq!(session.active_pane, Some(PaneId(1)));
    session.set_active_pane(PaneId(2));
    assert_eq!(session.active_pane, Some(PaneId(2)));

    // Remove active Pane 2 (simulating cross-window/cross-session transfer)
    let removed = session.remove_pane(PaneId(2)).unwrap();
    assert_eq!(removed, Some(PaneId(1))); // Fallback active pane returned

    // Verify Pane 2 is removed from layout
    assert_eq!(session.layout.panes(), vec![PaneId(1), PaneId(3)]);

    // Verify Pane 2 is removed from sync groups
    assert_eq!(session.sync_groups.get(&PaneId(2)), None);
    assert_eq!(session.sync_groups.get(&PaneId(1)), Some(&sync_gid));

    // Verify active pane falls back to previous MRU focused pane (Pane 1)
    assert_eq!(session.active_pane, Some(PaneId(1)));

    // Adopt a new external Pane 4 into the session docked to the Right of Pane 3
    session
        .adopt_pane(PaneId(4), PaneId(3), DockPosition::Right)
        .unwrap();

    // Verify Pane 4 exists in layout and is automatically activated
    assert_eq!(
        session.layout.panes(),
        vec![PaneId(1), PaneId(3), PaneId(4)]
    );
    assert_eq!(session.active_pane, Some(PaneId(4)));

    // Perform intra-session dock: Move Pane 4 to the Top of Pane 1
    session
        .dock_pane(PaneId(4), PaneId(1), DockPosition::Top)
        .unwrap();
    assert_eq!(
        session.layout.panes(),
        vec![PaneId(4), PaneId(1), PaneId(3)]
    );
    assert_eq!(session.active_pane, Some(PaneId(4)));

    // Close remaining panes down to empty
    session.remove_pane(PaneId(4)).unwrap();
    session.remove_pane(PaneId(1)).unwrap();
    session.remove_pane(PaneId(3)).unwrap();
    assert!(session.layout.panes().is_empty());
    assert_eq!(session.active_pane, None);
}

// ============================================================================
// 4. GTK Integration Tests: Reparenting, Drop Overlays, & Session Views
// ============================================================================

#[test]
fn test_integration_pane_overlay_and_session_view_docking() {
    run_gtk_test(|| {
        // --- Test 4.1: TerminalPane Drop Indicator Overlay ---
        let pane = TerminalPane::new(PaneId(10), None);
        assert!(!pane.is_drop_indicator_visible());

        // Showing overlay activates visibility and positions correctly
        pane.show_drop_indicator(DockPosition::Top);
        assert!(pane.is_drop_indicator_visible());

        pane.show_drop_indicator(DockPosition::Bottom);
        assert!(pane.is_drop_indicator_visible());

        pane.show_drop_indicator(DockPosition::Left);
        assert!(pane.is_drop_indicator_visible());

        pane.show_drop_indicator(DockPosition::Right);
        assert!(pane.is_drop_indicator_visible());

        pane.show_drop_indicator(DockPosition::Center);
        assert!(pane.is_drop_indicator_visible());

        // Hiding overlay clears visibility
        pane.hide_drop_indicator();
        assert!(!pane.is_drop_indicator_visible());

        // Clear callbacks without error
        pane.clear_callbacks();

        // --- Test 4.2: SessionView Intra-Session Docking ---
        let session = SessionView::new();
        assert_eq!(session.pane_count(), 1);
        let p1 = session.active_pane_id().unwrap();

        // Split to get second pane
        session.split_active(SplitOrientation::Horizontal);
        assert_eq!(session.pane_count(), 2);
        let p2 = session.active_pane_id().unwrap();
        assert_ne!(p1, p2);

        // Perform intra-session dock: dock p2 to the Bottom of p1
        session.dock_pane(p2, p1, DockPosition::Bottom);
        assert_eq!(session.pane_count(), 2);
        assert_eq!(session.active_pane_id(), Some(p2));

        // Perform intra-session swap: dock p2 to Center of p1
        session.dock_pane(p2, p1, DockPosition::Center);
        assert_eq!(session.pane_count(), 2);

        // Clean up
        session.close();
    });
}

#[test]
fn test_integration_cross_session_transfer_zero_process_interruption() {
    run_gtk_test(|| {
        // Setup Session A with 2 panes
        let session_a = SessionView::new();
        let p1 = session_a.active_pane_id().unwrap();
        session_a.split_active(SplitOrientation::Horizontal);
        let p2 = session_a.active_pane_id().unwrap();
        assert_eq!(session_a.pane_count(), 2);

        // Setup Session B with 1 pane
        let session_b = SessionView::new();
        let p3 = session_b.active_pane_id().unwrap();
        assert_eq!(session_b.pane_count(), 1);

        // Excise Pane 2 from Session A for transfer
        let transferred_pane = session_a
            .remove_pane_for_transfer(p2)
            .expect("Pane 2 must be extractable from Session A");

        // Assert Session A state immediately reflects excision
        assert_eq!(session_a.pane_count(), 1);
        assert_eq!(session_a.active_pane_id(), Some(p1));

        // Assert transferred pane preserves its identity
        assert_eq!(transferred_pane.pane_id(), p2);

        // Adopt Pane 2 into Session B docked to the Right of Pane 3
        session_b.adopt_pane(transferred_pane, p3, DockPosition::Right);

        // Assert Session B now contains both panes
        assert_eq!(session_b.pane_count(), 2);
        let b_panes = session_b.panes();
        assert!(b_panes.borrow().contains_key(&p3));
        assert!(b_panes.borrow().contains_key(&p2));
        assert_eq!(session_b.active_pane_id(), Some(p2));

        // Rebuilding projections on both sessions works without panics
        session_a.rebuild_projection();
        session_b.rebuild_projection();

        // Clean up both sessions
        session_a.close();
        session_b.close();
    });
}

#[test]
fn test_integration_session_view_with_existing_pane_constructor() {
    run_gtk_test(|| {
        // Create an existing standalone pane
        let pane = TerminalPane::new(PaneId(42), None);

        // Construct SessionView directly around the existing pane
        let session = SessionView::with_existing_pane(pane);
        assert_eq!(session.pane_count(), 1);
        assert_eq!(session.active_pane_id(), Some(PaneId(42)));
        assert!(session.panes().borrow().contains_key(&PaneId(42)));

        // Can split from this existing pane
        session.split_active(SplitOrientation::Vertical);
        assert_eq!(session.pane_count(), 2);

        session.close();
    });
}
