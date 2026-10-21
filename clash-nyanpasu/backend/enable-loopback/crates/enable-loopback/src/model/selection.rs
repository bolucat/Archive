//! SID-based multi-selection over the current sorted order (§6.2).

use super::Column;
use crate::netiso::Sid;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub control: bool,
    pub shift: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub selected: BTreeSet<Sid>,
    pub anchor: Option<Sid>,
    /// The focused row is independent of the selected set and native checkbox focus.
    pub focused: Option<Sid>,
    pub context_column: Option<Column>,
}

impl Selection {
    pub fn click(
        &mut self,
        order: &[Sid],
        sid: &Sid,
        modifiers: Modifiers,
        button: PointerButton,
        column: Column,
    ) {
        if !order.contains(sid) {
            return;
        }
        if button == PointerButton::Right {
            self.context_column = Some(column);
            if !self.selected.contains(sid) {
                self.select_one(sid);
            }
        } else if modifiers.shift {
            // Shift takes precedence when both modifiers are held (§6.2's range rule).
            self.select_range(order, sid);
        } else if modifiers.control {
            if !self.selected.remove(sid) {
                self.selected.insert(sid.clone());
            }
            self.anchor = Some(sid.clone());
        } else {
            self.select_one(sid);
        }
        self.focused = Some(sid.clone());
    }

    fn select_one(&mut self, sid: &Sid) {
        self.selected = BTreeSet::from([sid.clone()]);
        self.anchor = Some(sid.clone());
    }

    fn select_range(&mut self, order: &[Sid], sid: &Sid) {
        let Some(end) = order.iter().position(|candidate| candidate == sid) else {
            return;
        };
        let start = self
            .anchor
            .as_ref()
            .and_then(|anchor| order.iter().position(|candidate| candidate == anchor))
            .or_else(|| {
                self.focused
                    .as_ref()
                    .and_then(|focused| order.iter().position(|candidate| candidate == focused))
            })
            .unwrap_or(end);
        self.anchor = Some(order[start].clone());
        self.selected = order[start.min(end)..=start.max(end)]
            .iter()
            .cloned()
            .collect();
    }

    /// Handle plain and Shift+Up/Down in display order. The returned SID can be revealed.
    pub fn arrow(&mut self, order: &[Sid], direction: Direction, shift: bool) -> Option<Sid> {
        if order.is_empty() {
            return None;
        }
        let current = self
            .focused
            .as_ref()
            .and_then(|sid| order.iter().position(|candidate| candidate == sid));
        let next = match (current, direction) {
            (Some(index), Direction::Up) => index.saturating_sub(1),
            (Some(index), Direction::Down) => (index + 1).min(order.len() - 1),
            (None, Direction::Up) => order.len() - 1,
            (None, Direction::Down) => 0,
        };
        let sid = &order[next];
        if shift {
            self.select_range(order, sid);
        } else {
            self.select_one(sid);
        }
        self.focused = Some(sid.clone());
        Some(sid.clone())
    }

    pub fn select_all(&mut self, order: &[Sid]) {
        self.selected = order.iter().cloned().collect();
        if self.anchor.is_none() {
            self.anchor = order.first().cloned();
        }
        if self.focused.is_none() {
            self.focused = order.first().cloned();
        }
    }

    /// Refresh keeps surviving SID identities, discarding stale anchors and focus too.
    pub fn prune(&mut self, order: &[Sid]) {
        self.selected.retain(|sid| order.contains(sid));
        if self.anchor.as_ref().is_some_and(|sid| !order.contains(sid)) {
            self.anchor = None;
        }
        if self
            .focused
            .as_ref()
            .is_some_and(|sid| !order.contains(sid))
        {
            self.focused = None;
        }
    }

    pub fn first_selected_index(&self, order: &[Sid]) -> Option<usize> {
        order.iter().position(|sid| self.selected.contains(sid))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> Vec<Sid> {
        (1..=5)
            .map(|id| format!("S-1-15-2-{id}").parse().unwrap())
            .collect()
    }
    fn click(selection: &mut Selection, order: &[Sid], index: usize, control: bool, shift: bool) {
        selection.click(
            order,
            &order[index],
            Modifiers { control, shift },
            PointerButton::Left,
            Column::DisplayName,
        );
    }
    fn selected(order: &[Sid], indices: &[usize]) -> BTreeSet<Sid> {
        indices.iter().map(|index| order[*index].clone()).collect()
    }

    #[test]
    fn plain_control_shift_and_reverse_ranges() {
        let order = order();
        let mut s = Selection::default();
        click(&mut s, &order, 1, false, false);
        assert_eq!(s.selected, selected(&order, &[1]));
        assert_eq!(s.anchor, Some(order[1].clone()));
        click(&mut s, &order, 3, true, false);
        assert_eq!(s.selected, selected(&order, &[1, 3]));
        assert_eq!(s.anchor, Some(order[3].clone()));
        click(&mut s, &order, 3, true, false);
        assert_eq!(s.selected, selected(&order, &[1]));
        assert_eq!(s.focused, Some(order[3].clone()));
        click(&mut s, &order, 0, false, true);
        assert_eq!(s.selected, selected(&order, &[0, 1, 2, 3]));
        click(&mut s, &order, 2, false, true);
        assert_eq!(s.selected, selected(&order, &[2, 3])); // Shrink around the same anchor.
        click(&mut s, &order, 4, false, false);
        assert_eq!(s.selected, selected(&order, &[4]));
    }

    #[test]
    fn right_click_preserves_multi_selection_or_selects_target_and_records_column() {
        let order = order();
        let mut s = Selection::default();
        s.select_all(&order);
        let anchor = s.anchor.clone();
        s.click(
            &order,
            &order[2],
            Modifiers::default(),
            PointerButton::Right,
            Column::Users,
        );
        assert_eq!(s.selected.len(), 5);
        assert_eq!(s.anchor, anchor);
        assert_eq!(s.context_column, Some(Column::Users));
        click(&mut s, &order, 0, false, false);
        s.click(
            &order,
            &order[2],
            Modifiers {
                control: true,
                shift: true,
            },
            PointerButton::Right,
            Column::Sid,
        );
        assert_eq!(s.selected, selected(&order, &[2]));
        assert_eq!(s.anchor, Some(order[2].clone()));
        assert_eq!(s.context_column, Some(Column::Sid));
    }

    #[test]
    fn shift_arrows_extend_shrink_and_stop_at_edges() {
        let order = order();
        let mut s = Selection::default();
        click(&mut s, &order, 2, false, false);
        s.arrow(&order, Direction::Down, true);
        s.arrow(&order, Direction::Down, true);
        assert_eq!(s.selected, selected(&order, &[2, 3, 4]));
        s.arrow(&order, Direction::Down, true);
        assert_eq!(s.selected.len(), 3);
        s.arrow(&order, Direction::Up, true);
        assert_eq!(s.selected, selected(&order, &[2, 3]));
        s.arrow(&order, Direction::Up, true);
        s.arrow(&order, Direction::Up, true);
        assert_eq!(s.selected, selected(&order, &[1, 2]));
        s.arrow(&order, Direction::Up, false);
        assert_eq!(s.selected, selected(&order, &[0]));
        s.arrow(&order, Direction::Up, false);
        assert_eq!(s.selected, selected(&order, &[0]));
    }

    #[test]
    fn sorted_ranges_refresh_pruning_and_select_all_use_current_order() {
        let mut order = order();
        let mut s = Selection::default();
        click(&mut s, &order, 1, false, false);
        order.swap(0, 4);
        order.swap(2, 3);
        click(&mut s, &order, 4, false, true);
        assert_eq!(s.selected, selected(&order, &[1, 2, 3, 4]));
        assert_eq!(s.first_selected_index(&order), Some(1));
        let surviving = vec![order[2].clone(), order[3].clone()];
        s.prune(&surviving);
        assert_eq!(s.selected, surviving.iter().cloned().collect());
        assert_eq!(s.anchor, None);
        assert_eq!(s.focused, None);
        s.select_all(&surviving);
        assert_eq!(s.anchor, Some(surviving[0].clone()));
        s.prune(&[]);
        assert_eq!(s.selected.len(), 0);
        assert_eq!(s.arrow(&[], Direction::Down, true), None);
        assert_eq!(s.first_selected_index(&[]), None);
    }

    #[test]
    fn absent_anchor_and_stale_events_are_safe() {
        let order = order();
        let mut s = Selection::default();
        click(&mut s, &order, 2, false, true);
        assert_eq!(s.selected, selected(&order, &[2]));
        let before = s.clone();
        s.click(
            &[],
            &order[0],
            Modifiers::default(),
            PointerButton::Left,
            Column::Name,
        );
        assert_eq!(s, before);
        s.prune(&[]);
        assert_eq!(
            s.arrow(&order, Direction::Down, true),
            Some(order[0].clone())
        );
        assert_eq!(s.selected, selected(&order, &[0]));
    }
}
