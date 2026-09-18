use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PaneId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitOrientation {
    /// Side-by-side panes (left and right), separated by a vertical divider.
    Horizontal,
    /// Stacked panes (top and bottom), separated by a horizontal divider.
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockPosition {
    Top,
    Bottom,
    Left,
    Right,
    Center,
}

pub fn calculate_dock_position(x: f64, y: f64, width: f64, height: f64) -> DockPosition {
    if width <= 0.0 || height <= 0.0 {
        return DockPosition::Center;
    }

    let nx = (x / width).clamp(0.0, 1.0);
    let ny = (y / height).clamp(0.0, 1.0);

    if (0.25..=0.75).contains(&nx) && (0.25..=0.75).contains(&ny) {
        return DockPosition::Center;
    }

    let d_top = ny;
    let d_bottom = 1.0 - ny;
    let d_left = nx;
    let d_right = 1.0 - nx;

    if d_top <= d_bottom && d_top <= d_left && d_top <= d_right {
        DockPosition::Top
    } else if d_bottom <= d_left && d_bottom <= d_right {
        DockPosition::Bottom
    } else if d_left <= d_right {
        DockPosition::Left
    } else {
        DockPosition::Right
    }
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum LayoutError {
    #[error("Pane ID {0:?} not found in layout tree")]
    PaneNotFound(PaneId),
    #[error("Cannot split: tree invariant violated")]
    InvalidSplit,
    #[error("Cannot close last remaining pane")]
    CannotCloseLastPane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SplitId(pub u64);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LayoutNode {
    Leaf(PaneId),
    Split {
        id: SplitId,
        orientation: SplitOrientation,
        ratio: f64,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

impl LayoutNode {
    pub fn panes(&self, list: &mut Vec<PaneId>) {
        match self {
            LayoutNode::Leaf(id) => list.push(*id),
            LayoutNode::Split { first, second, .. } => {
                first.panes(list);
                second.panes(list);
            }
        }
    }

    pub fn contains(&self, target: PaneId) -> bool {
        match self {
            LayoutNode::Leaf(id) => *id == target,
            LayoutNode::Split { first, second, .. } => {
                first.contains(target) || second.contains(target)
            }
        }
    }

    pub fn leaf_count(&self) -> usize {
        match self {
            LayoutNode::Leaf(_) => 1,
            LayoutNode::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
        }
    }

    pub fn first_leaf(&self) -> PaneId {
        match self {
            LayoutNode::Leaf(id) => *id,
            LayoutNode::Split { first, .. } => first.first_leaf(),
        }
    }

    pub fn last_leaf(&self) -> PaneId {
        match self {
            LayoutNode::Leaf(id) => *id,
            LayoutNode::Split { second, .. } => second.last_leaf(),
        }
    }

    fn split_leaf(
        &mut self,
        target: PaneId,
        orientation: SplitOrientation,
        new_pane: PaneId,
        split_id: SplitId,
    ) -> Result<bool, LayoutError> {
        match self {
            LayoutNode::Leaf(id) if *id == target => {
                let old_leaf = Box::new(LayoutNode::Leaf(*id));
                let new_leaf = Box::new(LayoutNode::Leaf(new_pane));
                *self = LayoutNode::Split {
                    id: split_id,
                    orientation,
                    ratio: 0.5,
                    first: old_leaf,
                    second: new_leaf,
                };
                Ok(true)
            }
            LayoutNode::Leaf(_) => Ok(false),
            LayoutNode::Split { first, second, .. } => {
                if first.split_leaf(target, orientation, new_pane, split_id)? {
                    return Ok(true);
                }
                second.split_leaf(target, orientation, new_pane, split_id)
            }
        }
    }

    fn insert_dock_leaf(
        &mut self,
        new_pane: PaneId,
        target: PaneId,
        position: DockPosition,
        split_id: SplitId,
    ) -> Result<bool, LayoutError> {
        match self {
            LayoutNode::Leaf(id) if *id == target => {
                let target_leaf = Box::new(LayoutNode::Leaf(*id));
                let new_leaf = Box::new(LayoutNode::Leaf(new_pane));
                let (orientation, first, second) = match position {
                    DockPosition::Left => (SplitOrientation::Horizontal, new_leaf, target_leaf),
                    DockPosition::Right => (SplitOrientation::Horizontal, target_leaf, new_leaf),
                    DockPosition::Top => (SplitOrientation::Vertical, new_leaf, target_leaf),
                    DockPosition::Bottom => (SplitOrientation::Vertical, target_leaf, new_leaf),
                    DockPosition::Center => return Err(LayoutError::InvalidSplit),
                };
                *self = LayoutNode::Split {
                    id: split_id,
                    orientation,
                    ratio: 0.5,
                    first,
                    second,
                };
                Ok(true)
            }
            LayoutNode::Leaf(_) => Ok(false),
            LayoutNode::Split { first, second, .. } => {
                if first.insert_dock_leaf(new_pane, target, position, split_id)? {
                    return Ok(true);
                }
                second.insert_dock_leaf(new_pane, target, position, split_id)
            }
        }
    }

    fn set_split_ratio(&mut self, target_id: SplitId, clamped_ratio: f64) -> bool {
        match self {
            LayoutNode::Leaf(_) => false,
            LayoutNode::Split {
                id,
                ratio,
                first,
                second,
                ..
            } => {
                if *id == target_id {
                    *ratio = clamped_ratio;
                    true
                } else {
                    first.set_split_ratio(target_id, clamped_ratio)
                        || second.set_split_ratio(target_id, clamped_ratio)
                }
            }
        }
    }

    pub fn orientation_weight(&self, target_orientation: SplitOrientation) -> usize {
        match self {
            LayoutNode::Leaf(_) => 1,
            LayoutNode::Split {
                orientation,
                first,
                second,
                ..
            } => {
                if *orientation == target_orientation {
                    first.orientation_weight(target_orientation)
                        + second.orientation_weight(target_orientation)
                } else {
                    1
                }
            }
        }
    }

    pub fn equalize_cluster(&mut self, target_orientation: SplitOrientation) {
        if let LayoutNode::Split {
            orientation,
            ratio,
            first,
            second,
            ..
        } = self
        {
            if *orientation == target_orientation {
                let w1 = first.orientation_weight(target_orientation);
                let w2 = second.orientation_weight(target_orientation);
                let total = (w1 + w2) as f64;
                if total > 0.0 {
                    *ratio = (w1 as f64 / total).clamp(0.05, 0.95);
                }
                first.equalize_cluster(target_orientation);
                second.equalize_cluster(target_orientation);
            }
        }
    }

    pub fn equalize_all_clusters(&mut self, target_orientation: SplitOrientation) {
        if let LayoutNode::Split {
            orientation,
            first,
            second,
            ..
        } = self
        {
            if *orientation == target_orientation {
                self.equalize_cluster(target_orientation);
            } else {
                first.equalize_all_clusters(target_orientation);
                second.equalize_all_clusters(target_orientation);
            }
        }
    }

    pub fn find_split_orientation(&self, target_id: SplitId) -> Option<SplitOrientation> {
        match self {
            LayoutNode::Leaf(_) => None,
            LayoutNode::Split {
                id,
                orientation,
                first,
                second,
                ..
            } => {
                if *id == target_id {
                    Some(*orientation)
                } else {
                    first
                        .find_split_orientation(target_id)
                        .or_else(|| second.find_split_orientation(target_id))
                }
            }
        }
    }

    pub fn contains_split(&self, target_id: SplitId) -> bool {
        match self {
            LayoutNode::Leaf(_) => false,
            LayoutNode::Split {
                id,
                first,
                second,
                ..
            } => {
                *id == target_id
                    || first.contains_split(target_id)
                    || second.contains_split(target_id)
            }
        }
    }

    pub fn equalize_split_cluster(
        &mut self,
        target_id: SplitId,
        target_orientation: SplitOrientation,
    ) -> bool {
        match self {
            LayoutNode::Leaf(_) => false,
            LayoutNode::Split {
                orientation,
                first,
                second,
                ..
            } => {
                if *orientation == target_orientation {
                    if self.contains_split(target_id) {
                        self.equalize_cluster(target_orientation);
                        return true;
                    }
                } else {
                    if first.contains_split(target_id) {
                        return first.equalize_split_cluster(target_id, target_orientation);
                    }
                    if second.contains_split(target_id) {
                        return second.equalize_split_cluster(target_id, target_orientation);
                    }
                }
                false
            }
        }
    }

    fn remap_ids(&mut self, next_pane: &mut u64, next_split: &mut u64) {
        match self {
            LayoutNode::Leaf(id) => {
                *id = PaneId(*next_pane);
                *next_pane += 1;
            }
            LayoutNode::Split {
                id,
                first,
                second,
                ..
            } => {
                *id = SplitId(*next_split);
                *next_split += 1;
                first.remap_ids(next_pane, next_split);
                second.remap_ids(next_pane, next_split);
            }
        }
    }

    fn close_leaf(&mut self, target: PaneId) -> Result<Option<PaneId>, LayoutError> {
        match self {
            LayoutNode::Leaf(_) => Err(LayoutError::PaneNotFound(target)),
            LayoutNode::Split { first, second, .. } => {
                if let LayoutNode::Leaf(id) = **first {
                    if id == target {
                        let focus = second.first_leaf();
                        let promoted = (**second).clone();
                        *self = promoted;
                        return Ok(Some(focus));
                    }
                }
                if let LayoutNode::Leaf(id) = **second {
                    if id == target {
                        let focus = first.last_leaf();
                        let promoted = (**first).clone();
                        *self = promoted;
                        return Ok(Some(focus));
                    }
                }

                if first.contains(target) {
                    first.close_leaf(target)
                } else if second.contains(target) {
                    second.close_leaf(target)
                } else {
                    Err(LayoutError::PaneNotFound(target))
                }
            }
        }
    }

    fn balance(&mut self) {
        if let LayoutNode::Split {
            ref mut ratio,
            ref mut first,
            ref mut second,
            ..
        } = self
        {
            first.balance();
            second.balance();
            let c1 = first.leaf_count() as f64;
            let c2 = second.leaf_count() as f64;
            *ratio = (c1 / (c1 + c2)).clamp(0.05, 0.95);
        }
    }

    fn swap_panes(&mut self, a: PaneId, b: PaneId) {
        match self {
            LayoutNode::Leaf(id) => {
                if *id == a {
                    *id = b;
                } else if *id == b {
                    *id = a;
                }
            }
            LayoutNode::Split { first, second, .. } => {
                first.swap_panes(a, b);
                second.swap_panes(a, b);
            }
        }
    }

    fn compute_rects(&self, rect: Rect, out: &mut Vec<(PaneId, Rect)>) {
        match self {
            LayoutNode::Leaf(id) => out.push((*id, rect)),
            LayoutNode::Split {
                orientation,
                ratio,
                first,
                second,
                ..
            } => {
                let r = ratio.clamp(0.05, 0.95);
                match orientation {
                    SplitOrientation::Horizontal => {
                        let first_rect = Rect {
                            x: rect.x,
                            y: rect.y,
                            w: rect.w * r,
                            h: rect.h,
                        };
                        let second_rect = Rect {
                            x: rect.x + rect.w * r,
                            y: rect.y,
                            w: rect.w * (1.0 - r),
                            h: rect.h,
                        };
                        first.compute_rects(first_rect, out);
                        second.compute_rects(second_rect, out);
                    }
                    SplitOrientation::Vertical => {
                        let first_rect = Rect {
                            x: rect.x,
                            y: rect.y,
                            w: rect.w,
                            h: rect.h * r,
                        };
                        let second_rect = Rect {
                            x: rect.x,
                            y: rect.y + rect.h * r,
                            w: rect.w,
                            h: rect.h * (1.0 - r),
                        };
                        first.compute_rects(first_rect, out);
                        second.compute_rects(second_rect, out);
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LayoutTree {
    root: Option<LayoutNode>,
    #[serde(default = "default_next_split_id")]
    next_split_id: u64,
}

fn default_next_split_id() -> u64 {
    1
}

impl LayoutTree {
    pub fn new(initial_pane: PaneId) -> Self {
        Self {
            root: Some(LayoutNode::Leaf(initial_pane)),
            next_split_id: 1,
        }
    }

    pub fn empty() -> Self {
        Self {
            root: None,
            next_split_id: 1,
        }
    }

    pub fn next_split_id(&mut self) -> SplitId {
        let id = SplitId(self.next_split_id);
        self.next_split_id += 1;
        id
    }

    pub fn root(&self) -> Option<&LayoutNode> {
        self.root.as_ref()
    }

    pub fn root_mut(&mut self) -> Option<&mut LayoutNode> {
        self.root.as_mut()
    }

    pub fn panes(&self) -> Vec<PaneId> {
        let mut list = Vec::new();
        if let Some(ref root) = self.root {
            root.panes(&mut list);
        }
        list
    }

    pub fn contains(&self, id: PaneId) -> bool {
        self.root.as_ref().is_some_and(|r| r.contains(id))
    }

    pub fn split(
        &mut self,
        target: PaneId,
        orientation: SplitOrientation,
        new_pane: PaneId,
    ) -> Result<(), LayoutError> {
        let split_id = self.next_split_id();
        self.split_with_id(target, orientation, new_pane, split_id)
    }

    pub fn split_with_id(
        &mut self,
        target: PaneId,
        orientation: SplitOrientation,
        new_pane: PaneId,
        split_id: SplitId,
    ) -> Result<(), LayoutError> {
        if self.contains(new_pane) {
            return Err(LayoutError::InvalidSplit);
        }
        if !self.contains(target) {
            return Err(LayoutError::PaneNotFound(target));
        }

        let Some(ref mut root) = self.root else {
            return Err(LayoutError::PaneNotFound(target));
        };

        if split_id.0 >= self.next_split_id {
            self.next_split_id = split_id.0 + 1;
        }

        let found = root.split_leaf(target, orientation, new_pane, split_id)?;
        if found {
            Ok(())
        } else {
            Err(LayoutError::PaneNotFound(target))
        }
    }

    pub fn set_split_ratio(&mut self, split_id: SplitId, ratio: f64) -> bool {
        let clamped = ratio.clamp(0.05, 0.95);
        if let Some(ref mut root) = self.root {
            root.set_split_ratio(split_id, clamped)
        } else {
            false
        }
    }

    pub fn equalize_split(&mut self, split_id: SplitId) -> bool {
        let Some(ref mut root) = self.root else {
            return false;
        };
        let Some(orientation) = root.find_split_orientation(split_id) else {
            return false;
        };
        root.equalize_split_cluster(split_id, orientation)
    }

    pub fn equalize_direction(&mut self, target_orientation: SplitOrientation) -> bool {
        let Some(ref mut root) = self.root else {
            return false;
        };
        root.equalize_all_clusters(target_orientation);
        true
    }

    pub fn remap_ids(&mut self, next_pane: &mut u64, next_split: &mut u64) {
        if let Some(ref mut root) = self.root {
            root.remap_ids(next_pane, next_split);
        }
        self.next_split_id = *next_split;
    }

    pub fn close(&mut self, target: PaneId) -> Result<Option<PaneId>, LayoutError> {
        if !self.contains(target) {
            return Err(LayoutError::PaneNotFound(target));
        }

        if let Some(LayoutNode::Leaf(id)) = self.root {
            if id == target {
                self.root = None;
                return Ok(None);
            }
        }

        let Some(ref mut root) = self.root else {
            return Err(LayoutError::PaneNotFound(target));
        };

        root.close_leaf(target)
    }

    pub fn swap_panes(&mut self, a: PaneId, b: PaneId) -> Result<(), LayoutError> {
        if !self.contains(a) {
            return Err(LayoutError::PaneNotFound(a));
        }
        if !self.contains(b) {
            return Err(LayoutError::PaneNotFound(b));
        }
        if a == b {
            return Ok(());
        }
        if let Some(ref mut root) = self.root {
            root.swap_panes(a, b);
        }
        Ok(())
    }

    pub fn insert_pane_dock(
        &mut self,
        new_pane: PaneId,
        target: PaneId,
        position: DockPosition,
    ) -> Result<(), LayoutError> {
        if position == DockPosition::Center {
            return Err(LayoutError::InvalidSplit);
        }
        if self.contains(new_pane) {
            return Err(LayoutError::InvalidSplit);
        }
        if !self.contains(target) {
            return Err(LayoutError::PaneNotFound(target));
        }

        let split_id = self.next_split_id();
        let Some(ref mut root) = self.root else {
            return Err(LayoutError::PaneNotFound(target));
        };

        let found = root.insert_dock_leaf(new_pane, target, position, split_id)?;
        if found {
            Ok(())
        } else {
            Err(LayoutError::PaneNotFound(target))
        }
    }

    pub fn dock_pane(
        &mut self,
        source: PaneId,
        target: PaneId,
        position: DockPosition,
    ) -> Result<(), LayoutError> {
        if source == target {
            return Ok(());
        }
        if !self.contains(source) {
            return Err(LayoutError::PaneNotFound(source));
        }
        if !self.contains(target) {
            return Err(LayoutError::PaneNotFound(target));
        }
        if position == DockPosition::Center {
            return self.swap_panes(source, target);
        }
        self.close(source)?;
        self.insert_pane_dock(source, target, position)
    }

    pub fn balance(&mut self) {
        if let Some(ref mut root) = self.root {
            root.balance();
        }
    }

    pub fn find_adjacent(&self, current: PaneId, direction: Direction) -> Option<PaneId> {
        let root = self.root.as_ref()?;
        let mut rects = Vec::new();
        root.compute_rects(
            Rect {
                x: 0.0,
                y: 0.0,
                w: 1.0,
                h: 1.0,
            },
            &mut rects,
        );

        let current_rect = rects.iter().find(|(id, _)| *id == current)?.1;

        const EPS: f64 = 1e-5;

        #[derive(Debug)]
        struct Candidate {
            id: PaneId,
            border_dist: f64,
            overlap: f64,
            center_dist: f64,
        }

        let mut candidates = Vec::new();

        for (id, r) in rects {
            if id == current {
                continue;
            }

            match direction {
                Direction::Right => {
                    if r.x >= current_rect.x + current_rect.w - EPS {
                        let border_dist = (r.x - (current_rect.x + current_rect.w)).max(0.0);
                        let overlap = (current_rect.y + current_rect.h).min(r.y + r.h)
                            - current_rect.y.max(r.y);
                        let center_dist = ((r.y + r.h / 2.0)
                            - (current_rect.y + current_rect.h / 2.0))
                            .abs();
                        candidates.push(Candidate {
                            id,
                            border_dist,
                            overlap,
                            center_dist,
                        });
                    }
                }
                Direction::Left => {
                    if r.x + r.w <= current_rect.x + EPS {
                        let border_dist = (current_rect.x - (r.x + r.w)).max(0.0);
                        let overlap = (current_rect.y + current_rect.h).min(r.y + r.h)
                            - current_rect.y.max(r.y);
                        let center_dist = ((r.y + r.h / 2.0)
                            - (current_rect.y + current_rect.h / 2.0))
                            .abs();
                        candidates.push(Candidate {
                            id,
                            border_dist,
                            overlap,
                            center_dist,
                        });
                    }
                }
                Direction::Down => {
                    if r.y >= current_rect.y + current_rect.h - EPS {
                        let border_dist = (r.y - (current_rect.y + current_rect.h)).max(0.0);
                        let overlap = (current_rect.x + current_rect.w).min(r.x + r.w)
                            - current_rect.x.max(r.x);
                        let center_dist = ((r.x + r.w / 2.0)
                            - (current_rect.x + current_rect.w / 2.0))
                            .abs();
                        candidates.push(Candidate {
                            id,
                            border_dist,
                            overlap,
                            center_dist,
                        });
                    }
                }
                Direction::Up => {
                    if r.y + r.h <= current_rect.y + EPS {
                        let border_dist = (current_rect.y - (r.y + r.h)).max(0.0);
                        let overlap = (current_rect.x + current_rect.w).min(r.x + r.w)
                            - current_rect.x.max(r.x);
                        let center_dist = ((r.x + r.w / 2.0)
                            - (current_rect.x + current_rect.w / 2.0))
                            .abs();
                        candidates.push(Candidate {
                            id,
                            border_dist,
                            overlap,
                            center_dist,
                        });
                    }
                }
            }
        }

        if candidates.is_empty() {
            return None;
        }

        // Prefer candidates with positive interval overlap along perpendicular axis
        let has_overlapping = candidates.iter().any(|c| c.overlap > EPS);
        if has_overlapping {
            candidates.retain(|c| c.overlap > EPS);
        }

        // Sort by:
        // 1. Min border distance (immediate neighbor)
        // 2. Max overlap length
        // 3. Min center distance
        candidates.sort_by(|a, b| {
            a.border_dist
                .partial_cmp(&b.border_dist)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    b.overlap
                        .partial_cmp(&a.overlap)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    a.center_dist
                        .partial_cmp(&b.center_dist)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
        });

        candidates.first().map(|c| c.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_layout_has_single_leaf() {
        let tree = LayoutTree::new(PaneId(1));
        assert_eq!(tree.panes(), vec![PaneId(1)]);
        assert!(tree.contains(PaneId(1)));
        assert_eq!(tree.root(), Some(&LayoutNode::Leaf(PaneId(1))));
    }

    #[test]
    fn test_split_horizontal_and_vertical() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        assert_eq!(tree.panes(), vec![PaneId(1), PaneId(2)]);
        assert!(tree.contains(PaneId(1)));
        assert!(tree.contains(PaneId(2)));

        tree.split(PaneId(2), SplitOrientation::Vertical, PaneId(3))
            .unwrap();
        assert_eq!(tree.panes(), vec![PaneId(1), PaneId(2), PaneId(3)]);
        assert!(tree.contains(PaneId(3)));
    }

    #[test]
    fn test_split_nested_deep_tree() {
        let mut tree = LayoutTree::new(PaneId(1));
        for i in 2..=8 {
            let orientation = if i % 2 == 0 {
                SplitOrientation::Horizontal
            } else {
                SplitOrientation::Vertical
            };
            tree.split(PaneId(i - 1), orientation, PaneId(i)).unwrap();
        }
        assert_eq!(tree.panes().len(), 8);
        for i in 1..=8 {
            assert!(tree.contains(PaneId(i)));
        }
    }

    #[test]
    fn test_close_leaf_promotes_sibling() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        let next = tree.close(PaneId(1)).unwrap();
        assert_eq!(next, Some(PaneId(2)));
        assert_eq!(tree.root(), Some(&LayoutNode::Leaf(PaneId(2))));
        assert_eq!(tree.panes(), vec![PaneId(2)]);
        assert!(!tree.contains(PaneId(1)));
    }

    #[test]
    fn test_close_nested_branch_promotes_subtree() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        tree.split(PaneId(2), SplitOrientation::Vertical, PaneId(3))
            .unwrap();
        // Tree: Split(Horizontal, Leaf(1), Split(Vertical, Leaf(2), Leaf(3)))
        let next = tree.close(PaneId(1)).unwrap();
        assert!(next == Some(PaneId(2)) || next == Some(PaneId(3)));
        assert_eq!(tree.panes(), vec![PaneId(2), PaneId(3)]);
        assert!(!tree.contains(PaneId(1)));
    }

    #[test]
    fn test_close_last_pane_returns_none() {
        let mut tree = LayoutTree::new(PaneId(1));
        let next = tree.close(PaneId(1)).unwrap();
        assert_eq!(next, None);
        assert_eq!(tree.root(), None);
        assert_eq!(tree.panes(), Vec::<PaneId>::new());
        assert!(!tree.contains(PaneId(1)));
        assert_eq!(
            tree.close(PaneId(1)),
            Err(LayoutError::PaneNotFound(PaneId(1)))
        );
    }

    #[test]
    fn test_directional_navigation_2x2_grid() {
        // Grid:
        // [ 1 ] [ 3 ]
        // [ 2 ] [ 4 ]
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(3))
            .unwrap();
        tree.split(PaneId(1), SplitOrientation::Vertical, PaneId(2))
            .unwrap();
        tree.split(PaneId(3), SplitOrientation::Vertical, PaneId(4))
            .unwrap();

        assert_eq!(
            tree.find_adjacent(PaneId(1), Direction::Right),
            Some(PaneId(3))
        );
        assert_eq!(
            tree.find_adjacent(PaneId(1), Direction::Down),
            Some(PaneId(2))
        );
        assert_eq!(tree.find_adjacent(PaneId(1), Direction::Left), None);
        assert_eq!(tree.find_adjacent(PaneId(1), Direction::Up), None);

        assert_eq!(
            tree.find_adjacent(PaneId(2), Direction::Right),
            Some(PaneId(4))
        );
        assert_eq!(
            tree.find_adjacent(PaneId(2), Direction::Up),
            Some(PaneId(1))
        );
        assert_eq!(tree.find_adjacent(PaneId(2), Direction::Left), None);
        assert_eq!(tree.find_adjacent(PaneId(2), Direction::Down), None);

        assert_eq!(
            tree.find_adjacent(PaneId(3), Direction::Left),
            Some(PaneId(1))
        );
        assert_eq!(
            tree.find_adjacent(PaneId(3), Direction::Down),
            Some(PaneId(4))
        );
        assert_eq!(tree.find_adjacent(PaneId(3), Direction::Right), None);
        assert_eq!(tree.find_adjacent(PaneId(3), Direction::Up), None);

        assert_eq!(
            tree.find_adjacent(PaneId(4), Direction::Left),
            Some(PaneId(2))
        );
        assert_eq!(
            tree.find_adjacent(PaneId(4), Direction::Up),
            Some(PaneId(3))
        );
        assert_eq!(tree.find_adjacent(PaneId(4), Direction::Right), None);
        assert_eq!(tree.find_adjacent(PaneId(4), Direction::Down), None);
    }

    #[test]
    fn test_directional_navigation_asymmetrical() {
        // Asymmetrical:
        // [ 1 (tall) ] [ 2 (top) ]
        // [ 1 (tall) ] [ 3 (bottom) ]
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        tree.split(PaneId(2), SplitOrientation::Vertical, PaneId(3))
            .unwrap();

        assert_eq!(
            tree.find_adjacent(PaneId(2), Direction::Left),
            Some(PaneId(1))
        );
        assert_eq!(
            tree.find_adjacent(PaneId(3), Direction::Left),
            Some(PaneId(1))
        );
        assert_eq!(
            tree.find_adjacent(PaneId(2), Direction::Down),
            Some(PaneId(3))
        );
        assert_eq!(
            tree.find_adjacent(PaneId(3), Direction::Up),
            Some(PaneId(2))
        );

        let right_of_1 = tree.find_adjacent(PaneId(1), Direction::Right);
        assert!(right_of_1 == Some(PaneId(2)) || right_of_1 == Some(PaneId(3)));
    }

    #[test]
    fn test_balance_split_ratios() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        // Artificially change ratio or add more splits
        tree.split(PaneId(2), SplitOrientation::Horizontal, PaneId(3))
            .unwrap();

        tree.balance();
        if let Some(LayoutNode::Split { ratio, .. }) = tree.root() {
            // First has 1 leaf (Pane 1), second has 2 leaves (Pane 2, Pane 3)
            // Ratio should be 1 / 3 ≈ 0.333333
            assert!((ratio - 1.0 / 3.0).abs() < 1e-4);
        } else {
            panic!("Expected split root");
        }
    }

    #[test]
    fn test_error_conditions() {
        let mut tree = LayoutTree::new(PaneId(1));
        // Target not found
        assert_eq!(
            tree.split(PaneId(99), SplitOrientation::Horizontal, PaneId(2)),
            Err(LayoutError::PaneNotFound(PaneId(99)))
        );
        // Duplicate new pane
        assert_eq!(
            tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(1)),
            Err(LayoutError::InvalidSplit)
        );
        // Close non-existent pane
        assert_eq!(
            tree.close(PaneId(99)),
            Err(LayoutError::PaneNotFound(PaneId(99)))
        );
    }

    #[test]
    fn test_split_assigns_unique_split_ids() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        tree.split(PaneId(2), SplitOrientation::Vertical, PaneId(3))
            .unwrap();

        let root = tree.root().unwrap();
        if let LayoutNode::Split { id: s1, second, .. } = root {
            assert_eq!(*s1, SplitId(1));
            if let LayoutNode::Split { id: s2, .. } = &**second {
                assert_eq!(*s2, SplitId(2));
            } else {
                panic!("Expected second node to be split");
            }
        } else {
            panic!("Expected root to be split");
        }
    }

    #[test]
    fn test_set_split_ratio_updates_target_node() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        assert!(tree.set_split_ratio(SplitId(1), 0.75));

        if let Some(LayoutNode::Split { ratio, .. }) = tree.root() {
            assert!((ratio - 0.75).abs() < 1e-6);
        } else {
            panic!("Expected root to be split");
        }

        assert!(!tree.set_split_ratio(SplitId(999), 0.5));
    }

    #[test]
    fn test_set_split_ratio_clamps_values() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();

        tree.set_split_ratio(SplitId(1), 0.01);
        if let Some(LayoutNode::Split { ratio, .. }) = tree.root() {
            assert!((ratio - 0.05).abs() < 1e-6);
        } else {
            panic!("Expected root to be split");
        }

        tree.set_split_ratio(SplitId(1), 0.99);
        if let Some(LayoutNode::Split { ratio, .. }) = tree.root() {
            assert!((ratio - 0.95).abs() < 1e-6);
        } else {
            panic!("Expected root to be split");
        }
    }

    #[test]
    fn test_equalize_split_two_panes() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        tree.set_split_ratio(SplitId(1), 0.8);

        assert!(tree.equalize_split(SplitId(1)));
        if let Some(LayoutNode::Split { ratio, .. }) = tree.root() {
            assert!((ratio - 0.5).abs() < 1e-6);
        } else {
            panic!("Expected root to be split");
        }
    }

    #[test]
    fn test_equalize_split_three_panes() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        tree.split(PaneId(2), SplitOrientation::Horizontal, PaneId(3))
            .unwrap();
        // Tree: Split1(first: Pane 1, second: Split2(first: Pane 2, second: Pane 3))
        tree.set_split_ratio(SplitId(1), 0.7);
        tree.set_split_ratio(SplitId(2), 0.2);

        assert!(tree.equalize_split(SplitId(2)));
        if let Some(LayoutNode::Split { ratio: r1, second, .. }) = tree.root() {
            // Root should have ratio 1/3
            assert!((r1 - (1.0 / 3.0)).abs() < 1e-6);
            if let LayoutNode::Split { ratio: r2, .. } = &**second {
                // Second split should have ratio 1/2
                assert!((r2 - 0.5).abs() < 1e-6);
            } else {
                panic!("Expected second node to be split");
            }
        } else {
            panic!("Expected root to be split");
        }
    }

    #[test]
    fn test_equalize_direction_grid() {
        let mut tree = LayoutTree::new(PaneId(1));
        // Vertical split: Top (Pane 1) and Bottom (Pane 2)
        tree.split(PaneId(1), SplitOrientation::Vertical, PaneId(2))
            .unwrap();
        // Split Top horizontally: Pane 1 and Pane 3
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(3))
            .unwrap();
        // Split Bottom horizontally: Pane 2 and Pane 4
        tree.split(PaneId(2), SplitOrientation::Horizontal, PaneId(4))
            .unwrap();

        // Mess up ratios
        tree.set_split_ratio(SplitId(1), 0.8); // Vertical
        tree.set_split_ratio(SplitId(2), 0.2); // Top horizontal
        tree.set_split_ratio(SplitId(3), 0.75); // Bottom horizontal

        // Equalize Horizontal direction only
        assert!(tree.equalize_direction(SplitOrientation::Horizontal));

        // Vertical ratio should remain unchanged (0.8)
        if let Some(LayoutNode::Split { ratio: r_v, first, second, .. }) = tree.root() {
            assert!((r_v - 0.8).abs() < 1e-6);
            if let LayoutNode::Split { ratio: r_top, .. } = &**first {
                assert!((r_top - 0.5).abs() < 1e-6);
            }
            if let LayoutNode::Split { ratio: r_bot, .. } = &**second {
                assert!((r_bot - 0.5).abs() < 1e-6);
            }
        }

        // Now equalize Vertical direction
        assert!(tree.equalize_direction(SplitOrientation::Vertical));
        if let Some(LayoutNode::Split { ratio: r_v, .. }) = tree.root() {
            assert!((r_v - 0.5).abs() < 1e-6);
        }
    }

    #[test]
    fn test_remap_ids_renumbers_panes_and_splits() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        tree.split(PaneId(2), SplitOrientation::Vertical, PaneId(3))
            .unwrap();

        let mut next_pane = 10;
        let mut next_split = 20;
        tree.remap_ids(&mut next_pane, &mut next_split);

        assert_eq!(next_pane, 13);
        assert_eq!(next_split, 22);

        assert_eq!(tree.panes(), vec![PaneId(10), PaneId(11), PaneId(12)]);

        // Next split should use updated next_split_id
        let next_s = tree.next_split_id();
        assert_eq!(next_s, SplitId(22));
    }

    #[test]
    fn test_swap_panes_adjacent_leaves() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        assert_eq!(tree.panes(), vec![PaneId(1), PaneId(2)]);

        tree.swap_panes(PaneId(1), PaneId(2)).unwrap();
        assert_eq!(tree.panes(), vec![PaneId(2), PaneId(1)]);
    }

    #[test]
    fn test_swap_panes_nested_leaves() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        tree.split(PaneId(2), SplitOrientation::Vertical, PaneId(3))
            .unwrap();
        assert_eq!(tree.panes(), vec![PaneId(1), PaneId(2), PaneId(3)]);

        tree.swap_panes(PaneId(1), PaneId(3)).unwrap();
        assert_eq!(tree.panes(), vec![PaneId(3), PaneId(2), PaneId(1)]);
    }

    #[test]
    fn test_swap_panes_same_pane_noop() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2))
            .unwrap();
        tree.swap_panes(PaneId(1), PaneId(1)).unwrap();
        assert_eq!(tree.panes(), vec![PaneId(1), PaneId(2)]);
    }

    #[test]
    fn test_swap_panes_nonexistent_returns_err() {
        let mut tree = LayoutTree::new(PaneId(1));
        assert_eq!(
            tree.swap_panes(PaneId(1), PaneId(99)),
            Err(LayoutError::PaneNotFound(PaneId(99)))
        );
        assert_eq!(
            tree.swap_panes(PaneId(99), PaneId(1)),
            Err(LayoutError::PaneNotFound(PaneId(99)))
        );
    }

    #[test]
    fn test_dock_position_variants() {
        assert_eq!(DockPosition::Top, DockPosition::Top);
        assert_eq!(DockPosition::Bottom, DockPosition::Bottom);
        assert_eq!(DockPosition::Left, DockPosition::Left);
        assert_eq!(DockPosition::Right, DockPosition::Right);
        assert_eq!(DockPosition::Center, DockPosition::Center);
        assert_ne!(DockPosition::Top, DockPosition::Bottom);
        assert_eq!(format!("{:?}", DockPosition::Top), "Top");
        assert_eq!(format!("{:?}", DockPosition::Center), "Center");
    }

    #[test]
    fn test_calculate_dock_position_center_zone() {
        assert_eq!(calculate_dock_position(50.0, 50.0, 100.0, 100.0), DockPosition::Center);
        assert_eq!(calculate_dock_position(30.0, 30.0, 100.0, 100.0), DockPosition::Center);
        assert_eq!(calculate_dock_position(70.0, 70.0, 100.0, 100.0), DockPosition::Center);
        assert_eq!(calculate_dock_position(25.0, 25.0, 100.0, 100.0), DockPosition::Center);
        assert_eq!(calculate_dock_position(75.0, 75.0, 100.0, 100.0), DockPosition::Center);
    }

    #[test]
    fn test_calculate_dock_position_directional_zones() {
        assert_eq!(calculate_dock_position(50.0, 5.0, 100.0, 100.0), DockPosition::Top);
        assert_eq!(calculate_dock_position(50.0, 95.0, 100.0, 100.0), DockPosition::Bottom);
        assert_eq!(calculate_dock_position(5.0, 50.0, 100.0, 100.0), DockPosition::Left);
        assert_eq!(calculate_dock_position(95.0, 50.0, 100.0, 100.0), DockPosition::Right);
    }

    #[test]
    fn test_calculate_dock_position_boundary_clamping() {
        // Negative coordinates outside box clamp to nearest boundary
        assert_eq!(calculate_dock_position(-10.0, 50.0, 100.0, 100.0), DockPosition::Left);
        assert_eq!(calculate_dock_position(50.0, -10.0, 100.0, 100.0), DockPosition::Top);
        // Beyond dimensions clamp to right / bottom
        assert_eq!(calculate_dock_position(150.0, 50.0, 100.0, 100.0), DockPosition::Right);
        assert_eq!(calculate_dock_position(50.0, 150.0, 100.0, 100.0), DockPosition::Bottom);
        // Zero or negative dimensions default safely to Center
        assert_eq!(calculate_dock_position(10.0, 10.0, 0.0, 100.0), DockPosition::Center);
        assert_eq!(calculate_dock_position(10.0, 10.0, 100.0, 0.0), DockPosition::Center);
        assert_eq!(calculate_dock_position(10.0, 10.0, -50.0, -50.0), DockPosition::Center);
    }

    #[test]
    fn test_layout_tree_dock_pane_intra_tree_directions() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2)).unwrap();
        assert_eq!(tree.panes(), vec![PaneId(1), PaneId(2)]);

        // Dock P1 to Right of P2 -> order should be [P2, P1] with Horizontal orientation
        tree.dock_pane(PaneId(1), PaneId(2), DockPosition::Right).unwrap();
        assert_eq!(tree.panes(), vec![PaneId(2), PaneId(1)]);
        if let Some(LayoutNode::Split { orientation, .. }) = tree.root() {
            assert_eq!(*orientation, SplitOrientation::Horizontal);
        } else {
            panic!("Expected split root");
        }

        // Dock P1 to Top of P2 -> order should be [P1, P2] with Vertical orientation
        tree.dock_pane(PaneId(1), PaneId(2), DockPosition::Top).unwrap();
        assert_eq!(tree.panes(), vec![PaneId(1), PaneId(2)]);
        if let Some(LayoutNode::Split { orientation, .. }) = tree.root() {
            assert_eq!(*orientation, SplitOrientation::Vertical);
        } else {
            panic!("Expected split root");
        }

        // Dock P1 to Bottom of P2 -> order should be [P2, P1] with Vertical orientation
        tree.dock_pane(PaneId(1), PaneId(2), DockPosition::Bottom).unwrap();
        assert_eq!(tree.panes(), vec![PaneId(2), PaneId(1)]);
        if let Some(LayoutNode::Split { orientation, .. }) = tree.root() {
            assert_eq!(*orientation, SplitOrientation::Vertical);
        } else {
            panic!("Expected split root");
        }

        // Dock P1 to Left of P2 -> order should be [P1, P2] with Horizontal orientation
        tree.dock_pane(PaneId(1), PaneId(2), DockPosition::Left).unwrap();
        assert_eq!(tree.panes(), vec![PaneId(1), PaneId(2)]);
        if let Some(LayoutNode::Split { orientation, .. }) = tree.root() {
            assert_eq!(*orientation, SplitOrientation::Horizontal);
        } else {
            panic!("Expected split root");
        }
    }

    #[test]
    fn test_layout_tree_dock_pane_center_swaps() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2)).unwrap();
        tree.dock_pane(PaneId(1), PaneId(2), DockPosition::Center).unwrap();
        assert_eq!(tree.panes(), vec![PaneId(2), PaneId(1)]);
    }

    #[test]
    fn test_layout_tree_dock_pane_self_noop() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.split(PaneId(1), SplitOrientation::Horizontal, PaneId(2)).unwrap();
        tree.dock_pane(PaneId(1), PaneId(1), DockPosition::Top).unwrap();
        assert_eq!(tree.panes(), vec![PaneId(1), PaneId(2)]);
    }

    #[test]
    fn test_layout_tree_insert_pane_dock_external() {
        let mut tree = LayoutTree::new(PaneId(1));
        tree.insert_pane_dock(PaneId(2), PaneId(1), DockPosition::Right).unwrap();
        assert_eq!(tree.panes(), vec![PaneId(1), PaneId(2)]);

        tree.insert_pane_dock(PaneId(3), PaneId(2), DockPosition::Top).unwrap();
        assert_eq!(tree.panes(), vec![PaneId(1), PaneId(3), PaneId(2)]);

        // Inserting existing pane should fail
        assert_eq!(
            tree.insert_pane_dock(PaneId(1), PaneId(2), DockPosition::Left),
            Err(LayoutError::InvalidSplit)
        );

        // Center is invalid for insert
        assert_eq!(
            tree.insert_pane_dock(PaneId(4), PaneId(2), DockPosition::Center),
            Err(LayoutError::InvalidSplit)
        );
    }
}
