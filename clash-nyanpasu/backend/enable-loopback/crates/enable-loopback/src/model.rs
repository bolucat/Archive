//! Pure rows, rendering, sorting, copy formats, save baselines and CLI filters (§3).

mod columns;
pub use columns::{CHECKBOX_WIDTH, ColumnWidths};

mod selection;
use crate::search::{Query, SearchFields};
pub use selection::{Direction, Modifiers, PointerButton, Selection};

use crate::{
    args::Command,
    netiso::{AppContainer, Sid},
};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub struct Row {
    pub app: AppContainer,
    pub checked: bool,
    /// Enumeration position, restored when sorting is cleared.
    pub source_index: usize,
    searchable: SearchFields,
}

/// In display/copy order; the checkbox is separate and is never a sort column.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(usize)]
pub enum Column {
    #[default]
    DisplayName,
    Description,
    Package,
    Name,
    Sid,
    Users,
    Binaries,
}

#[derive(Clone, Debug)]
pub struct Placeholders {
    pub display_name: String,
    pub description: String,
    pub package: String,
    pub name: String,
    pub users: String,
    pub binaries: String,
}

impl Placeholders {
    /// An explicit locale keeps rendering independent of the process's current locale.
    pub fn for_locale(locale: &str) -> Self {
        Self {
            display_name: rust_i18n::t!("placeholder.display_name", locale = locale).into_owned(),
            description: rust_i18n::t!("placeholder.description", locale = locale).into_owned(),
            package: rust_i18n::t!("placeholder.package", locale = locale).into_owned(),
            name: rust_i18n::t!("placeholder.name", locale = locale).into_owned(),
            users: rust_i18n::t!("placeholder.users", locale = locale).into_owned(),
            binaries: rust_i18n::t!("placeholder.binaries", locale = locale).into_owned(),
        }
    }
}

impl Row {
    fn new(app: AppContainer, source_index: usize) -> Self {
        let searchable = SearchFields::new([
            app.display_name.clone(),
            app.description.clone(),
            app.package_full_name.clone(),
            app.name.clone(),
            Some(app.sid.to_string()),
            (!app.users.is_empty()).then(|| app.users.join("; ")),
            (!app.binaries.is_empty()).then(|| app.binaries.join("; ")),
        ]);
        Self {
            checked: app.exempt_at_last_check,
            app,
            source_index,
            searchable,
        }
    }

    /// Render the values compared and copied by ListViewItemComparer/frmMain.
    /// Only null fields receive placeholders; present empty strings stay empty.
    pub fn columns(&self, placeholders: &Placeholders) -> [String; 7] {
        let app = &self.app;
        [
            app.display_name
                .as_ref()
                .unwrap_or(&placeholders.display_name)
                .clone(),
            app.description
                .as_ref()
                .unwrap_or(&placeholders.description)
                .clone(),
            app.package_full_name
                .as_ref()
                .unwrap_or(&placeholders.package)
                .clone(),
            app.name.as_ref().unwrap_or(&placeholders.name).clone(),
            app.sid.to_string(), // Sid is validated; missing AC SIDs cannot enter a Row.
            if app.users.is_empty() {
                placeholders.users.clone()
            } else {
                app.users.join("; ")
            },
            if app.binaries.is_empty() {
                placeholders.binaries.clone()
            } else {
                app.binaries.join("; ")
            },
        ]
    }

    /// Original highlighting tests the joined users for DOMAIN\user as a substring.
    pub fn highlights_user(&self, current_user: &str) -> bool {
        self.app.users.join("; ").contains(current_user)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sort {
    /// None uses search relevance, or enumeration order when the query is empty.
    pub column: Option<Column>,
    pub ascending: bool,
}

impl Default for Sort {
    /// The original tool starts sorted by display name, ascending.
    fn default() -> Self {
        Self {
            column: Some(Column::DisplayName),
            ascending: true,
        }
    }
}

impl Sort {
    /// Cycle ascending, descending, then enumeration order; another column starts ascending.
    pub fn click(&mut self, column: Column) {
        *self = match self.column {
            Some(current) if current == column && self.ascending => Self {
                column: Some(column),
                ascending: false,
            },
            Some(current) if current == column => Self {
                column: None,
                ascending: true,
            },
            _ => Self {
                column: Some(column),
                ascending: true,
            },
        };
    }

    pub fn apply(&self, rows: &mut [Row], placeholders: &Placeholders) {
        let Some(column) = self.column else {
            rows.sort_by_key(|row| row.source_index);
            return;
        };
        // .NET Ordinal compares UTF-16 code units, not Rust's UTF-8 byte order.
        if self.ascending {
            rows.sort_by_cached_key(|row| {
                row.columns(placeholders)[column as usize]
                    .encode_utf16()
                    .collect::<Vec<_>>()
            });
        } else {
            rows.sort_by_cached_key(|row| {
                std::cmp::Reverse(
                    row.columns(placeholders)[column as usize]
                        .encode_utf16()
                        .collect::<Vec<_>>(),
                )
            });
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Checked,
    ExemptAll,
    ExemptNone,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    /// None means clear the status bar after reverting all changes.
    pub key: Option<&'static str>,
    /// Append status.orphan_removal with this count when nonzero.
    pub orphan_removal_count: usize,
}

#[derive(Debug, Default)]
pub struct ListState {
    pub rows: Vec<Row>,
    pub query: String,
    visible: Vec<usize>,
    pub orphans: Vec<Sid>,
    pub sort: Sort,
    /// Suppresses dirty notifications while refresh replaces rows (§3.3).
    pub refreshing: bool,
    pub loaded: bool,
    pub saving: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Availability {
    pub refresh: bool,
    pub bulk: bool,
    pub save: bool,
    pub list: bool,
}

impl ListState {
    pub fn from_apps(
        apps: Vec<AppContainer>,
        exempted: &[Sid],
        placeholders: &Placeholders,
    ) -> Self {
        let mut rows: Vec<_> = apps
            .into_iter()
            .enumerate()
            .map(|(source_index, mut app)| {
                app.exempt_at_last_check = exempted.contains(&app.sid);
                Row::new(app, source_index)
            })
            .collect();
        // Exemption order is also the original orphan-prompt order.
        let orphans = exempted
            .iter()
            .filter(|sid| !rows.iter().any(|row| row.app.sid == **sid))
            .cloned()
            .collect();
        let sort = Sort::default();
        sort.apply(&mut rows, placeholders);
        let visible = (0..rows.len()).collect();
        Self {
            rows,
            query: String::new(),
            visible,
            orphans,
            sort,
            refreshing: false,
            loaded: true,
            saving: false,
        }
    }

    /// Keep UI enablement in the pure model, including the in-flight save snapshot.
    pub fn availability(&self) -> Availability {
        let idle = !self.refreshing && !self.saving;
        Availability {
            refresh: idle,
            // Bulk actions apply to the visible (searched) rows only.
            bulk: idle && self.loaded && !self.visible.is_empty(),
            save: idle && self.is_dirty(),
            list: idle && self.loaded,
        }
    }

    pub fn begin_refresh(&mut self) {
        self.refreshing = true;
        self.loaded = false;
        self.rows.clear();
        self.visible.clear();
        self.orphans.clear();
    }

    pub fn is_dirty(&self) -> bool {
        !self.refreshing
            && self
                .rows
                .iter()
                .any(|row| row.checked != row.app.exempt_at_last_check)
    }

    /// Refresh retains the column and direction chosen before the refresh.
    pub fn replace_apps(
        &mut self,
        apps: Vec<AppContainer>,
        exempted: &[Sid],
        placeholders: &Placeholders,
    ) {
        let sort = self.sort;
        let query = self.query.clone();
        *self = Self::from_apps(apps, exempted, placeholders);
        self.sort = sort;
        self.sort.apply(&mut self.rows, placeholders);
        self.set_query(query);
    }

    pub fn status_after_change(&self, change: Change) -> Status {
        if !self.is_dirty() {
            return Status {
                key: None,
                orphan_removal_count: 0,
            };
        }
        Status {
            key: Some(match (change, self.search_active()) {
                (Change::Checked, _) => "status.dirty",
                (Change::ExemptAll, false) => "status.exempt_all",
                (Change::ExemptNone, false) => "status.exempt_none",
                (Change::ExemptAll, true) => "status.exempt_results",
                (Change::ExemptNone, true) => "status.exempt_none_results",
            }),
            orphan_removal_count: self.orphans.len(),
        }
    }

    /// Exempt or unexempt every visible row; rows hidden by the search keep their state.
    pub fn set_all(&mut self, checked: bool) -> Status {
        for &index in &self.visible {
            self.rows[index].checked = checked;
        }
        self.status_after_change(if checked {
            Change::ExemptAll
        } else {
            Change::ExemptNone
        })
    }

    pub fn checked_sids(&self) -> Vec<Sid> {
        self.rows
            .iter()
            .filter(|row| row.checked)
            .map(|row| row.app.sid.clone())
            .collect()
    }

    /// Freeze edits while the writer owns this exact checked set.
    pub fn begin_save(&mut self) -> Option<Vec<Sid>> {
        if !self.availability().save {
            return None;
        }
        self.saving = true;
        Some(self.checked_sids())
    }

    pub fn finish_save(&mut self, succeeded: bool) {
        self.saving = false;
        if succeeded {
            self.mark_saved();
        }
    }

    /// Call only after SetAppContainerConfig succeeds. Failure leaves edits retryable.
    pub fn mark_saved(&mut self) {
        for row in &mut self.rows {
            row.app.exempt_at_last_check = row.checked;
        }
        self.orphans.clear();
    }

    pub fn counter(&self) -> (usize, usize) {
        (
            self.rows.iter().filter(|row| row.checked).count(),
            self.rows.len(),
        )
    }

    pub fn sort_by(&mut self, column: Column, placeholders: &Placeholders) {
        self.sort.click(column);
        self.sort.apply(&mut self.rows, placeholders);
        self.filter();
    }

    pub fn search_active(&self) -> bool {
        !self.query.trim().is_empty()
    }

    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.filter();
    }

    fn filter(&mut self) {
        let query = Query::new(&self.query);
        let mut matches: Vec<_> = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(index, row)| query.score(&row.searchable).map(|score| (index, score)))
            .collect();
        if self.sort.column.is_none() {
            matches.sort_unstable_by_key(|&(index, score)| {
                (std::cmp::Reverse(score), self.rows[index].source_index)
            });
        }
        self.visible = matches.into_iter().map(|(index, _)| index).collect();
    }

    pub fn visible_rows(&self) -> impl Iterator<Item = &Row> {
        self.visible.iter().map(|&index| &self.rows[index])
    }

    pub fn visible_count(&self) -> usize {
        self.visible.len()
    }

    /// Bulk action label keys: "all" without a search, "results" while searching.
    pub fn bulk_label_keys(&self) -> (&'static str, &'static str) {
        if self.search_active() {
            ("toolbar.exempt_results", "toolbar.exempt_none_results")
        } else {
            ("toolbar.exempt_all", "toolbar.exempt_none")
        }
    }
}

/// Iterate current display order, never BTreeSet SID order.
pub fn copy_rows<'a>(
    rows: impl IntoIterator<Item = &'a Row>,
    selected: &BTreeSet<Sid>,
    placeholders: &Placeholders,
) -> String {
    let selected: Vec<_> = rows
        .into_iter()
        .filter(|row| selected.contains(&row.app.sid))
        .collect();
    let mut result = String::new();
    for row in &selected {
        result.push_str(if row.checked {
            "LoopbackExempt\t"
        } else {
            "LoopbackBlocked\t"
        });
        result.push_str(&row.columns(placeholders).join("\t"));
        if selected.len() > 1 {
            result.push_str("\r\n");
        }
    }
    result
}

pub fn copy_column<'a>(
    rows: impl IntoIterator<Item = &'a Row>,
    selected: &BTreeSet<Sid>,
    column: Column,
    placeholders: &Placeholders,
) -> String {
    let mut result = String::new();
    for row in rows
        .into_iter()
        .filter(|row| selected.contains(&row.app.sid))
    {
        result.push_str(&row.columns(placeholders)[column as usize]);
        result.push_str("\r\n"); // StringBuilder.AppendLine uses Environment.NewLine on Windows.
    }
    result
}

/// `-none` and `-deny *` bypass both enumeration and configuration reads.
pub fn clears_without_reading(command: &Command) -> bool {
    matches!(command, Command::None) || matches!(command, Command::Deny { name } if name == "*")
}

/// Only named filters need the current exemption baseline; wildcard allow replaces it.
pub fn needs_exemptions(command: &Command) -> bool {
    matches!(command, Command::Allow { name } | Command::Deny { name } if name != "*")
}

fn ordinal_ignore_case(text: &str) -> String {
    // OrdinalIgnoreCase uses one-to-one invariant uppercase, never expansions (ß != ss).
    text.chars()
        .map(|ch| {
            let mut uppercase = ch.to_uppercase();
            let first = uppercase.next().unwrap_or(ch);
            if uppercase.next().is_none() && ch != '\u{131}' {
                first
            } else {
                ch
            }
        })
        .collect()
}

/// Preserve enumeration order and omit orphans, like the original array-based CLI.
pub fn filter_exemptions(command: &Command, apps: &[AppContainer], exempted: &[Sid]) -> Vec<Sid> {
    if clears_without_reading(command) || matches!(command, Command::Syntax) {
        return Vec::new();
    }
    apps.iter()
        .filter(|app| match command {
            Command::All => true,
            Command::Allow { name } if name == "*" => true,
            Command::Allow { name } => {
                exempted.contains(&app.sid)
                    || app.name.as_ref().is_none_or(|value| {
                        value.is_empty()
                            || ordinal_ignore_case(value).contains(&ordinal_ignore_case(name))
                    })
            }
            Command::Deny { name } => {
                exempted.contains(&app.sid)
                    && app.name.as_ref().is_none_or(|value| {
                        value.is_empty()
                            || !ordinal_ignore_case(value).contains(&ordinal_ignore_case(name))
                    })
            }
            _ => false,
        })
        .map(|app| app.sid.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_orders_by_relevance_only_when_unsorted_and_scopes_bulk_edits() {
        let p = Placeholders::for_locale("en");
        let mut first = app(1, Some("zStore"));
        first.display_name = Some("zStore".into());
        let mut second = app(2, Some("Store"));
        second.display_name = Some("Store".into());
        let mut third = app(3, Some("Store"));
        third.display_name = Some("Store".into());
        let mut hidden = app(4, None);
        hidden.display_name = None;
        let apps = vec![first, second, third, hidden];
        let mut list = ListState::from_apps(apps.clone(), &[], &p);
        let ids = |list: &ListState| {
            list.visible_rows()
                .map(|row| row.source_index)
                .collect::<Vec<_>>()
        };
        list.set_query("store".into());
        assert_eq!(ids(&list), [1, 2, 0]); // Active ascending column wins.
        list.sort_by(Column::DisplayName, &p);
        assert_eq!(ids(&list), [0, 1, 2]); // Active descending column wins.
        list.sort_by(Column::DisplayName, &p);
        assert_eq!(list.sort.column, None);
        assert_eq!(ids(&list), [1, 2, 0]); // Score, then enumeration on ties.
        // Bulk edits touch only the three matches; the hidden row keeps its state.
        list.set_all(true);
        assert_eq!(list.counter(), (3, 4));
        assert_eq!(list.checked_sids().len(), 3);
        let mut selection = Selection::default();
        selection.select_all(
            &list
                .rows
                .iter()
                .map(|row| row.app.sid.clone())
                .collect::<Vec<_>>(),
        );
        let visible = list
            .visible_rows()
            .map(|row| row.app.sid.clone())
            .collect::<Vec<_>>();
        selection.prune(&visible);
        assert_eq!(selection.selected.len(), 3);
        let copy = copy_rows(list.visible_rows(), &selection.selected, &p);
        assert_eq!(copy.lines().count(), 3);
        assert!(!copy.contains("S-1-15-2-4"));
        list.set_query("No DisplayName".into());
        assert_eq!(list.visible_count(), 0);
        list.set_query("store".into());
        list.begin_refresh();
        assert_eq!(list.visible_count(), 0);
        list.replace_apps(apps, &[], &p);
        assert_eq!(list.query, "store");
        assert_eq!(ids(&list), [1, 2, 0]);
        list.set_query("  \t".into());
        assert!(!list.search_active());
        assert_eq!(ids(&list), [0, 1, 2, 3]);
    }

    #[test]
    fn replacement_rebuilds_search_cache_and_copy_accepts_only_visible_selection() {
        let p = Placeholders::for_locale("en");
        let mut list = ListState::from_apps(vec![app(1, Some("old"))], &[], &p);
        list.set_query("new".into());
        assert_eq!(list.visible_count(), 0);
        list.replace_apps(vec![app(1, Some("new"))], &[], &p);
        assert_eq!(list.visible_count(), 1);
        let selected = list.rows.iter().map(|row| row.app.sid.clone()).collect();
        list.set_query("old".into());
        assert_eq!(
            copy_column(list.visible_rows(), &selected, Column::Name, &p),
            ""
        );
        assert_eq!(copy_rows(list.visible_rows(), &selected, &p), "");
    }

    fn app(id: u32, name: Option<&str>) -> AppContainer {
        AppContainer {
            sid: format!("S-1-15-2-{id}").parse().unwrap(),
            users: Vec::new(),
            name: name.map(str::to_owned),
            display_name: name.map(str::to_owned),
            description: None,
            package_full_name: None,
            binaries: Vec::new(),
            exempt_at_last_check: false,
        }
    }

    #[test]
    fn save_failure_keeps_edits_retryable_and_success_commits_the_snapshot() {
        let p = Placeholders::for_locale("en");
        let first = app(1, None);
        let second = app(2, None);
        let orphan = app(9, None).sid;
        let mut state = ListState::from_apps(
            vec![first.clone(), second.clone()],
            &[first.sid.clone(), orphan.clone()],
            &p,
        );
        assert!(state.begin_save().is_none());
        state.rows[0].checked = false;
        state.rows[1].checked = true;
        assert_eq!(
            state.begin_save().unwrap(),
            std::slice::from_ref(&second.sid)
        );
        assert!(state.begin_save().is_none()); // prevent duplicate writes
        state.finish_save(false);
        assert!(state.is_dirty());
        assert_eq!(state.orphans, [orphan]);
        assert!(state.rows[0].app.exempt_at_last_check);
        assert!(!state.rows[1].app.exempt_at_last_check);
        assert_eq!(state.begin_save().unwrap(), [second.sid]);
        state.finish_save(true);
        assert!(!state.is_dirty());
        assert!(!state.availability().save);
        assert!(state.orphans.is_empty());
        assert!(!state.rows[0].app.exempt_at_last_check);
        assert!(state.rows[1].app.exempt_at_last_check);
    }

    #[test]
    fn availability_covers_manual_loading_clean_dirty_failed_and_saving() {
        let mut state = ListState::default();
        assert_eq!(
            state.availability(),
            Availability {
                refresh: true,
                bulk: false,
                save: false,
                list: false
            }
        );
        state.begin_refresh();
        assert_eq!(
            state.availability(),
            Availability {
                refresh: false,
                bulk: false,
                save: false,
                list: false
            }
        );
        state.replace_apps(vec![app(1, None)], &[], &Placeholders::for_locale("en"));
        assert_eq!(
            state.availability(),
            Availability {
                refresh: true,
                bulk: true,
                save: false,
                list: true
            }
        );
        state.set_all(true);
        assert_eq!(
            state.availability(),
            Availability {
                refresh: true,
                bulk: true,
                save: true,
                list: true
            }
        );
        state.begin_save().unwrap();
        assert_eq!(
            state.availability(),
            Availability {
                refresh: false,
                bulk: false,
                save: false,
                list: false
            }
        );
        assert!(state.is_dirty());
        state.finish_save(false);
        state.begin_refresh();
        state.refreshing = false; // failed load
        assert_eq!(
            state.availability(),
            Availability {
                refresh: true,
                bulk: false,
                save: false,
                list: false
            }
        );
        state.replace_apps(vec![], &[], &Placeholders::for_locale("en"));
        assert_eq!(
            state.availability(),
            Availability {
                refresh: true,
                bulk: false,
                save: false,
                list: true
            }
        );
    }

    #[test]
    fn merge_keeps_first_metadata_and_position_and_sorts_all_users_ordinally() {
        let mut first = app(2, Some("first"));
        first.users = vec!["z".into()];
        let mut duplicate = app(2, Some("ignored"));
        duplicate.users = vec!["A".into(), "z".into()];
        let merged =
            crate::netiso::merge_records([first.clone(), app(1, Some("other")), duplicate]);
        assert_eq!(merged[0].name, first.name);
        assert_eq!(merged[0].users, ["A", "z", "z"]);
        assert_eq!(merged[1].sid, app(1, None).sid);
    }

    #[test]
    fn load_marks_baselines_sorts_and_collects_orphans_in_config_order() {
        let p = Placeholders::for_locale("en");
        let a = app(1, Some("z"));
        let b = app(2, Some("a"));
        let exemptions = vec![app(9, None).sid, b.sid.clone(), app(8, None).sid];
        let mut state = ListState::from_apps(vec![a, b.clone()], &exemptions, &p);
        assert_eq!(state.rows[0].app.sid, b.sid);
        assert!(state.rows[0].checked);
        assert!(!state.rows[1].checked);
        assert_eq!(
            state.orphans,
            [exemptions[0].clone(), exemptions[2].clone()]
        );
        assert!(!state.is_dirty());
        assert_eq!(state.counter(), (1, 2));
        state.rows[0].checked = false;
        assert!(state.is_dirty()); // No mark_saved on a failed API call.
        assert!(state.rows[0].app.exempt_at_last_check);
        state.mark_saved();
        assert!(!state.is_dirty());
        assert!(state.orphans.is_empty());
    }

    #[test]
    fn null_empty_and_joined_values_render_differently() {
        let p = Placeholders::for_locale("en");
        let mut row = Row::new(app(1, None), 0);
        assert_eq!(
            row.columns(&p),
            [
                "(No DisplayName)",
                "(No Description)",
                "(No Package Name)",
                "(No AC Name)",
                "S-1-15-2-1",
                "(No User SID)",
                "(None)"
            ]
        );
        row.app.display_name = Some(String::new());
        row.app.name = Some(String::new());
        row.app.users = vec![r"DOMAIN\user".into(), "b".into()];
        row.app.binaries = vec![String::new(), "c.exe".into()];
        let columns = row.columns(&p);
        assert_eq!(columns[0], "");
        assert_eq!(columns[3], "");
        assert_eq!(columns[5], r"DOMAIN\user; b");
        assert_eq!(columns[6], "; c.exe");
        assert!(row.highlights_user(r"DOMAIN\user"));
        assert!(!row.highlights_user(r"domain\user"));
    }

    #[test]
    fn status_bulk_dirty_clean_refresh_and_success_baselines() {
        let p = Placeholders::for_locale("en");
        let mut state =
            ListState::from_apps(vec![app(1, None), app(2, None)], &[app(9, None).sid], &p);
        assert_eq!(state.status_after_change(Change::Checked).key, None);
        assert_eq!(
            state.set_all(true),
            Status {
                key: Some("status.exempt_all"),
                orphan_removal_count: 1
            }
        );
        assert_eq!(state.counter(), (2, 2));
        assert_eq!(
            state.status_after_change(Change::Checked).key,
            Some("status.dirty")
        );
        state.refreshing = true;
        assert!(!state.is_dirty());
        state.refreshing = false;
        assert_eq!(state.set_all(false).key, None);
        state.set_all(true);
        state.mark_saved();
        assert_eq!(state.set_all(false).key, Some("status.exempt_none"));
        assert_eq!(state.set_all(true).key, None);
        assert_eq!(ListState::default().set_all(true).key, None);
    }

    #[test]
    fn sorting_compares_each_rendered_column_and_keeps_direction_on_column_change() {
        let p = Placeholders::for_locale("en");
        for column in [
            Column::DisplayName,
            Column::Description,
            Column::Package,
            Column::Name,
            Column::Sid,
            Column::Users,
            Column::Binaries,
        ] {
            let mut a = app(2, Some("z"));
            let mut b = app(1, Some("A"));
            a.description = Some("z".into());
            b.description = Some("A".into());
            a.package_full_name = Some("z".into());
            b.package_full_name = Some("A".into());
            a.users = vec!["z".into()];
            b.users = vec!["A".into()];
            a.binaries = vec!["z".into()];
            b.binaries = vec!["A".into()];
            let mut rows = vec![Row::new(a, 0), Row::new(b.clone(), 1)];
            let mut sort = Sort {
                column: Some(column),
                ascending: true,
            };
            sort.apply(&mut rows, &p);
            assert_eq!(rows[0].app.sid, b.sid);
            sort.click(column);
            sort.apply(&mut rows, &p);
            assert_eq!(rows[1].app.sid, b.sid);
            // The third click clears sorting and restores enumeration order.
            sort.click(column);
            assert_eq!(sort.column, None);
            sort.apply(&mut rows, &p);
            assert_eq!(rows[1].app.sid, b.sid);
            assert_eq!(rows[0].source_index, 0);
        }
        let mut sort = Sort::default();
        sort.click(Column::DisplayName);
        assert_eq!(
            (sort.column, sort.ascending),
            (Some(Column::DisplayName), false)
        );
        sort.click(Column::Binaries);
        assert_eq!(
            (sort.column, sort.ascending),
            (Some(Column::Binaries), true)
        );
        sort.click(Column::Binaries);
        sort.click(Column::Binaries);
        assert_eq!(sort.column, None);
        sort.click(Column::Binaries);
        assert_eq!(
            (sort.column, sort.ascending),
            (Some(Column::Binaries), true)
        );
        let state = ListState::from_apps(
            vec![app(1, Some("\u{e000}")), app(2, Some("\u{10000}"))],
            &[],
            &p,
        );
        assert_eq!(state.rows[0].app.sid, app(2, None).sid); // UTF-16 differs from UTF-8 here.
    }

    #[test]
    fn copy_uses_display_order_status_tokens_tabs_and_exact_crlf_rules() {
        let p = Placeholders::for_locale("en");
        let state = ListState::from_apps(
            vec![app(1, Some("z")), app(2, Some("a"))],
            &[app(2, None).sid],
            &p,
        );
        let mut selected = BTreeSet::new();
        assert_eq!(copy_rows(&state.rows, &selected, &p), "");
        selected.insert(app(2, None).sid);
        let expected = "LoopbackExempt\ta\t(No Description)\t(No Package Name)\ta\tS-1-15-2-2\t(No User SID)\t(None)";
        assert_eq!(copy_rows(&state.rows, &selected, &p), expected);
        assert_eq!(
            copy_column(&state.rows, &selected, Column::Name, &p),
            "a\r\n"
        );
        selected.insert(app(1, None).sid);
        assert_eq!(
            copy_rows(&state.rows, &selected, &p),
            format!(
                "{expected}\r\nLoopbackBlocked\tz\t(No Description)\t(No Package Name)\tz\tS-1-15-2-1\t(No User SID)\t(None)\r\n"
            )
        );
        assert_eq!(
            copy_column(&state.rows, &selected, Column::Name, &p),
            "a\r\nz\r\n"
        );
    }

    #[test]
    fn bulk_actions_touch_only_visible_rows_while_searching() {
        let p = Placeholders::for_locale("en");
        let mut state =
            ListState::from_apps(vec![app(1, Some("Store")), app(2, Some("Photos"))], &[], &p);
        assert_eq!(
            state.bulk_label_keys(),
            ("toolbar.exempt_all", "toolbar.exempt_none")
        );
        state.set_query("store".into());
        assert_eq!(
            state.bulk_label_keys(),
            ("toolbar.exempt_results", "toolbar.exempt_none_results")
        );
        assert_eq!(state.set_all(true).key, Some("status.exempt_results"));
        let checked = |name: &str| {
            state
                .rows
                .iter()
                .find(|row| row.app.display_name.as_deref() == Some(name))
                .unwrap()
                .checked
        };
        assert!(checked("Store"));
        assert!(!checked("Photos"));
        state.set_query("zzz".into());
        assert!(!state.availability().bulk);
        // Without a search the bulk action covers every row again.
        state.set_query(String::new());
        assert_eq!(state.set_all(false).key, None);
    }

    #[test]
    fn cli_filters_match_original_empty_names_wildcards_and_orphan_removal() {
        let apps = vec![
            app(1, Some("Store")),
            app(2, Some("other")),
            app(3, None),
            app(4, Some("")),
            app(5, Some("My STORE App")),
        ];
        let saved = vec![apps[0].sid.clone(), apps[1].sid.clone(), app(9, None).sid];
        let ids = |command| filter_exemptions(&command, &apps, &saved);
        assert_eq!(
            ids(Command::All),
            apps.iter().map(|a| a.sid.clone()).collect::<Vec<_>>()
        );
        assert_eq!(
            ids(Command::Allow {
                name: "sToRe".into()
            }),
            apps.iter().map(|a| a.sid.clone()).collect::<Vec<_>>()
        );
        assert_eq!(
            ids(Command::Deny {
                name: "STORE".into()
            }),
            [apps[1].sid.clone()]
        );
        assert!(ids(Command::None).is_empty());
        assert!(ids(Command::Deny { name: "*".into() }).is_empty());
        assert_eq!(ids(Command::Allow { name: "*".into() }).len(), 5);
        let all = apps.iter().map(|app| app.sid.clone()).collect::<Vec<_>>();
        assert_eq!(
            filter_exemptions(
                &Command::Deny {
                    name: "store".into()
                },
                &apps,
                &all
            ),
            all[1..4]
        );
        assert_eq!(ordinal_ignore_case("Straße"), "STRAßE");
        assert_eq!(ordinal_ignore_case("ёж"), "ЁЖ");
    }
}
