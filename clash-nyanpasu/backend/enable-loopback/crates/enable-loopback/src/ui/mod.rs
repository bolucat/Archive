//! Reactor presentation and asynchronous NetIso wiring (§3/§4).
mod dialogs;
mod keys;
mod native;
mod table;
#[cfg(test)]
mod tests;

use crate::{
    args::GuiOptions,
    instance::WindowPublisher,
    model::{
        Change, Column, Direction, ListState, Modifiers, Placeholders, PointerButton, Selection,
        Status,
    },
    netiso::{self, AppContainer, Sid},
};
use dialogs::Dialog;
use snafu::Snafu;
use std::{
    collections::{BTreeMap, VecDeque},
    rc::Rc,
    time::Duration,
};
use windows::{
    Win32::{
        libloaderapi::GetModuleHandleW,
        winerror::{EPT_S_NOT_REGISTERED, ERROR_SUCCESS},
        winuser::*,
    },
    core::PCWSTR,
};
use windows_reactor::*;

const PAGE_MARGIN: f64 = 24.0;
const WINDOW_WIDTH: f64 = 960.0;
const WINDOW_HEIGHT: f64 = 600.0;
const SECTION_SPACING: f64 = 12.0;
const SEARCH_WIDTH: f64 = 280.0;
const TOOLBAR_WRAP_WIDTH: f64 = 900.0;

const LEARN_URL: &str =
    "https://learn.microsoft.com/previous-versions/windows/apps/hh780593(v=win.10)";

#[derive(Debug, Snafu)]
pub enum Error {
    #[snafu(display("Publishing the window for other instances failed: {source}"))]
    PublishWindow { source: crate::instance::Error },
    #[snafu(display("Starting the reactor window failed: {source}"))]
    StartWindow { source: windows::core::Error },
    #[snafu(display("Loading icon resource {resource_id} at {size}px failed: {source}"))]
    LoadWindowIcon {
        resource_id: u16,
        size: i32,
        source: windows::core::Error,
    },
    #[snafu(display("ShellExecuteW failed: 0x{code:x}"))]
    OpenHelp { code: usize },
    #[snafu(display("{operation} failed: {source}"))]
    Native {
        operation: &'static str,
        source: windows::core::Error,
    },
}

struct MainWindow {
    list: ListState,
    options: GuiOptions,
    placeholders: Placeholders,
    current_user: String,
    status: String,
    dialog: Option<Dialog>,
    dialog_open: bool,
    dialogs: VecDeque<Dialog>,
    keep_prompting: bool,
    dialog_timer: Option<ComponentTimer>,
    menu_timer: Option<ComponentTimer>,
    list_generation: u64,
    viewport_width: f64,
    table_rows: Rc<Vec<Rc<table::Row>>>,
    keyboard_focus: bool,
    app: Option<Rc<AppContext>>,
    list_focus: ElementRef<Border>,
    search_ref: ElementRef<AutoSuggestBox>,
    search_focused: bool,
    selection: Selection,
    hwnd: usize,
    /// Read only from the first declaration; None keeps the system default.
    initial_position: Option<ScreenPoint>,
    close: native::CloseState,
    hovering_help: bool,
    version: String,
}

#[derive(Clone)]
pub struct Startup {
    options: GuiOptions,
    publisher: Rc<WindowPublisher>,
    // None only in the recording adapter, which does not initialize WinUI.
    app: Option<Rc<AppContext>>,
}
impl PartialEq for Startup {
    fn eq(&self, other: &Self) -> bool {
        self.options == other.options
            && Rc::ptr_eq(&self.publisher, &other.publisher)
            && match (&self.app, &other.app) {
                (Some(a), Some(b)) => Rc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            }
    }
}

struct Loaded {
    apps: Vec<AppContainer>,
    exempted: Vec<Sid>,
    user: String,
}
enum Message {
    Search(String),
    FocusSearch,
    SearchFocused(bool),
    WindowPrepared(Result<usize, Error>),
    Refresh,
    BeginRefresh(bool),
    Loaded {
        registry: bool,
        force: bool,
        fallback_code: Option<u32>,
        result: Result<Loaded, netiso::Error>,
    },
    Sort(Column),
    Checked(Sid, bool),
    SetAll(bool),
    DialogClosed(ContentDialogResult),
    NextDialog,
    KeepPrompting(bool),
    Size(WindowSize),
    Pointer(Sid, Column, PointerEventInfo),
    ContextMenu((String, Column, ScreenPoint)),
    ShowContextMenu(ScreenPoint),
    ToggleKey(String, bool),
    PointerKey(String, Column, PointerEventInfo),
    FocusKey(String),
    RowFocused(Sid),
    ListKey(KeyEventInfo),
    Copy(bool),
    Save,
    Saved(Result<(), netiso::Error>),
    CloseRequested,
    Help,
    HoverHelp(bool),
}

fn load(registry: bool, force: bool) -> Result<Loaded, netiso::Error> {
    let apps = if registry {
        netiso::enumerate_from_registry()?
    } else {
        netiso::enumerate(force)?
    };
    if apps.is_empty() && !registry {
        return Err(netiso::Error::Enumerate {
            force_binaries: force,
            code: ERROR_SUCCESS as u32,
        });
    }
    Ok(Loaded {
        apps,
        exempted: netiso::exempted_sids()?,
        user: netiso::current_user()?,
    })
}

impl MainWindow {
    fn toolbar(&self, cx: &ViewContext<Self>) -> Grid {
        let available = self.list.availability();
        // Reactor cannot measure text: the widest locale (en) needs about 280 + 544 DIP of
        // search box and buttons plus spacing and page margins.
        let wrap = self.viewport_width < TOOLBAR_WRAP_WIDTH;
        let (exempt_key, exempt_none_key) = self.list.bulk_label_keys();
        let actions = Grid::new()
            .grid_column(if wrap { 0 } else { 1 })
            .grid_row(i32::from(wrap))
            .horizontal_alignment(HorizontalAlignment::Right)
            .columns(vec![GridLength::Auto; 4])
            .column_spacing(8.0)
            .children((
                toolbar_button("toolbar.refresh", Symbol::Refresh)
                    .automation_id("refresh")
                    .is_enabled(available.refresh)
                    .on_click(cx.callback(|()| Message::Refresh)),
                toolbar_button(exempt_key, Symbol::SelectAll)
                    .grid_column(1)
                    .is_enabled(available.bulk)
                    .on_click(cx.callback(|()| Message::SetAll(true))),
                toolbar_button(exempt_none_key, Symbol::ClearSelection)
                    .grid_column(2)
                    .is_enabled(available.bulk)
                    .on_click(cx.callback(|()| Message::SetAll(false))),
                toolbar_button("toolbar.save", Symbol::Save)
                    .grid_column(3)
                    .automation_id("save")
                    .style(ButtonStyle::Accent)
                    .is_enabled(available.save)
                    .on_click(cx.callback(|()| Message::Save)),
            ));
        let sender = cx.sender();
        let search = Border::new()
            .automation_id("search_host")
            .horizontal_alignment(HorizontalAlignment::Left)
            .on_got_focus(cx.callback(|_| Message::SearchFocused(true)))
            .on_lost_focus(cx.callback(|_| Message::SearchFocused(false)))
            .on_preview_key_down(RoutedCallback::new(move |event: KeyEventInfo| {
                event.key == VirtualKey::ESCAPE
                    && event.modifiers == InputModifiers::NONE
                    && sender.send(Message::Search(String::new()))
            }))
            .content(
                AutoSuggestBox::new()
                    .automation_id("search")
                    .automation_name(rust_i18n::t!("search.placeholder"))
                    .element_ref(&self.search_ref)
                    .width(SEARCH_WIDTH)
                    .placeholder_text(rust_i18n::t!("search.placeholder"))
                    .query_icon(Symbol::Find)
                    .text(&self.list.query)
                    .is_enabled(!self.list.refreshing && !self.list.saving && !self.dialog_open)
                    .on_text_changed(
                        cx.callback(|text: Rc<str>| Message::Search(text.to_string())),
                    ),
            );
        Grid::new()
            .columns(if wrap {
                vec![GridLength::STAR]
            } else {
                vec![GridLength::STAR, GridLength::Auto]
            })
            .rows(vec![GridLength::Auto; if wrap { 2 } else { 1 }])
            .column_spacing(16.0)
            .row_spacing(8.0)
            .children((search, actions))
    }

    /// Global exemption count, plus the match count while searching (status bar, right).
    fn counter_text(&self) -> String {
        let (exempt, total) = self.list.counter();
        let mut counter =
            rust_i18n::t!("toolbar.counter", exempt = exempt, total = total).into_owned();
        if self.list.search_active() {
            counter.push_str(" · ");
            counter.push_str(&rust_i18n::t!(
                "search.results",
                count = self.list.visible_count()
            ));
        }
        counter
    }

    fn order(&self) -> Vec<Sid> {
        self.list
            .visible_rows()
            .map(|r| r.app.sid.clone())
            .collect()
    }
    fn key_sid(&self, key: &str) -> Option<Sid> {
        self.table_rows
            .iter()
            .find(|row| row.key == key)
            .and_then(|_| {
                self.list
                    .rows
                    .iter()
                    .find(|row| row.app.sid.to_string() == key)
            })
            .map(|row| row.app.sid.clone())
    }

    fn sync_table_rows(&mut self) {
        let old: BTreeMap<_, _> = self
            .table_rows
            .iter()
            .map(|row| (row.key.as_str(), row))
            .collect();
        let rows: Vec<_> = self
            .list
            .visible_rows()
            .map(|row| {
                let cells = row.columns(&self.placeholders);
                let item = table::Row {
                    key: row.app.sid.to_string(),
                    checked: row.checked,
                    checkbox_label: rust_i18n::t!("column.exempt", name = cells[0].clone())
                        .into_owned(),
                    cells,
                    highlight: !self.current_user.is_empty()
                        && row.highlights_user(&self.current_user),
                };
                old.get(item.key.as_str())
                    .filter(|old| ****old == item)
                    .map(|old| (*old).clone())
                    .unwrap_or_else(|| Rc::new(item))
            })
            .collect();
        if self.table_rows.as_ref() != &rows {
            self.table_rows = Rc::new(rows);
        }
    }

    fn table(&self, cx: &ViewContext<Self>) -> ComponentNode {
        let labels = [
            "column.display_name",
            "column.description",
            "column.package",
            "column.name",
            "column.sid",
            "column.users",
            "column.binaries",
        ];
        let widths = [220.0, 220.0, 200.0, 160.0, 140.0, 140.0, 200.0];
        component::<table::TableView>(
            "table",
            table::Input {
                columns: std::array::from_fn(|index| table::ColumnSpec {
                    header: rust_i18n::t!(labels[index]).into_owned(),
                    initial_width: widths[index],
                    min_width: 48.0,
                    sortable: true,
                    resizable: true,
                }),
                rows: self.table_rows.clone(),
                empty_text: (self.list.loaded
                    && self.list.search_active()
                    && self.table_rows.is_empty())
                .then(|| rust_i18n::t!("search.empty").into_owned()),
                restore_focus: !self.search_focused,
                sort: self.list.sort,
                selected: self
                    .selection
                    .selected
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
                focused: self.selection.focused.as_ref().map(ToString::to_string),
                keyboard_focus: self.keyboard_focus,
                enabled: self.list.availability().list && !self.dialog_open,
                viewport_width: self.viewport_width,
                generation: self.list_generation,
                focus_ref: self.list_focus.clone(),
                on_sort: cx.callback(Message::Sort),
                on_toggle: cx.callback(|(key, checked)| Message::ToggleKey(key, checked)),
                on_pointer: cx
                    .callback(|(key, column, event)| Message::PointerKey(key, column, event)),
                on_key: cx.callback(Message::ListKey),
                on_context_menu: cx.callback(Message::ContextMenu),
                on_focus: cx.callback(Message::FocusKey),
            },
        )
    }
    fn operation_failed(&mut self, error: Error) {
        let detail = error.to_string();
        self.status = rust_i18n::t!("status.failed", detail = detail.clone()).into_owned();
        self.enqueue(Dialog::Error {
            title: rust_i18n::t!("dialog.error_title").into_owned(),
            body: detail,
        });
    }
    fn copy(&mut self, column: bool) {
        let value = if column {
            crate::model::copy_column(
                self.list.visible_rows(),
                &self.selection.selected,
                self.selection.context_column.unwrap_or(Column::DisplayName),
                &self.placeholders,
            )
        } else {
            crate::model::copy_rows(
                self.list.visible_rows(),
                &self.selection.selected,
                &self.placeholders,
            )
        };
        if !value.is_empty()
            && let Err(error) = native::clipboard(self.hwnd, &value)
        {
            self.operation_failed(error);
        }
    }
    fn enqueue(&mut self, dialog: Dialog) {
        self.dialogs.push_back(dialog);
        if self.dialog.is_none() {
            self.next_dialog();
        }
    }
    fn next_dialog(&mut self) {
        self.dialog = self.dialogs.pop_front();
        self.dialog_open = self.dialog.is_some();
        if !self.dialog_open && self.list.loaded {
            let _ = self.list_focus.request_focus();
        }
    }
    fn start_load(
        &self,
        registry: bool,
        force: bool,
        fallback_code: Option<u32>,
        cx: &ComponentContext<Self>,
    ) {
        cx.spawn_background(move |_| Message::Loaded {
            registry,
            force,
            fallback_code,
            result: load(registry, force),
        });
    }
    fn refresh(&mut self, force: bool, cx: &ComponentContext<Self>) {
        self.list.begin_refresh();
        self.status = rust_i18n::t!("status.refreshing").into_owned();
        self.start_load(self.options.registry, force, None, cx);
    }
    fn change_status(&mut self, status: Status) {
        self.status = status
            .key
            .map(|key| rust_i18n::t!(key).into_owned())
            .unwrap_or_default();
        if status.orphan_removal_count > 0 {
            self.status.push(' ');
            self.status.push_str(&rust_i18n::t!(
                "status.orphan_removal",
                count = status.orphan_removal_count
            ));
        }
    }
    fn load_failed(&mut self, error: netiso::Error, fallback_code: Option<u32>) {
        self.list.refreshing = false;
        let detail = error.to_string();
        let code = match error {
            netiso::Error::Enumerate { code, .. } | netiso::Error::ReadExemptions { code } => {
                Some(code)
            }
            _ => fallback_code,
        };
        let (title, body) = if let Some(code) = code {
            self.status =
                rust_i18n::t!("status.load_failed", code = format!("0x{code:x}")).into_owned();
            (
                rust_i18n::t!(if code == EPT_S_NOT_REGISTERED as u32 {
                    "dialog.firewall_title"
                } else {
                    "dialog.error_title"
                })
                .into_owned(),
                rust_i18n::t!(
                    if code == EPT_S_NOT_REGISTERED as u32 {
                        "dialog.firewall_body"
                    } else {
                        "dialog.load_failed"
                    },
                    detail = detail
                )
                .into_owned(),
            )
        } else {
            self.status = rust_i18n::t!("status.failed", detail = detail.clone()).into_owned();
            (rust_i18n::t!("dialog.error_title").into_owned(), detail)
        };
        self.enqueue(Dialog::Error { title, body });
    }
}

impl Drop for MainWindow {
    fn drop(&mut self) {
        if let Some(app) = &self.app
            && let Err(error) = app.exit()
        {
            crate::debug(&format!("Exiting the UI application failed: {error}"));
        }
    }
}

fn context_menu(sender: LocalSender<Message>, (exempt_key, exempt_none_key): (&str, &str)) -> Menu {
    Menu::new(
        [
            MenuItem::item("copy_rows", rust_i18n::t!("menu.copy_rows").into_owned()),
            MenuItem::item(
                "copy_column",
                rust_i18n::t!("menu.copy_column").into_owned(),
            ),
            MenuItem::separator("separator"),
            MenuItem::item("exempt_none", rust_i18n::t!(exempt_none_key).into_owned()),
            MenuItem::item("exempt_all", rust_i18n::t!(exempt_key).into_owned()),
        ],
        move |key: Key| {
            let message = match key.as_str() {
                Some("copy_rows") => Message::Copy(false),
                Some("copy_column") => Message::Copy(true),
                Some("exempt_none") => Message::SetAll(false),
                Some("exempt_all") => Message::SetAll(true),
                _ => return,
            };
            let _ = sender.send(message);
        },
    )
}

impl Component for MainWindow {
    type Message = Message;
    type Input = Startup;
    fn create(input: &Startup, cx: &ComponentContext<Self>) -> Self {
        let publisher = input.publisher.clone();
        let close = native::CloseState::default();
        let subclass_state = close.clone();
        let sender = cx.sender();
        if !cx.run_window(move |window| {
            // SAFETY: run_window supplies this process's live HWND on the UI thread.
            let hwnd = window.as_raw() as usize;
            let result = unsafe { publisher.publish_window(window.as_raw().cast()) }
                .map_err(|source| Error::PublishWindow { source })
                .and_then(|()| native::install_close(&window, subclass_state, sender))
                .and_then(|()| set_window_icons(&window))
                .map(|()| hwnd);
            Message::WindowPrepared(result)
        }) {
            crate::debug("Native window callback could not be scheduled");
        }
        if !input.options.manual {
            let _ = cx.sender().send(Message::BeginRefresh(false));
        }
        Self {
            list: ListState::default(),
            options: input.options,
            placeholders: Placeholders::for_locale(&rust_i18n::locale()),
            current_user: String::new(),
            status: rust_i18n::t!("status.ready").into_owned(),
            dialog: None,
            dialog_open: false,
            dialogs: VecDeque::new(),
            keep_prompting: true,
            dialog_timer: None,
            menu_timer: None,
            list_generation: 0,
            viewport_width: WINDOW_WIDTH - 2.0 * PAGE_MARGIN,
            table_rows: Rc::new(Vec::new()),
            keyboard_focus: false,
            app: input.app.clone(),
            list_focus: ElementRef::new(),
            search_ref: ElementRef::new(),
            search_focused: false,
            selection: Selection::default(),
            hwnd: 0,
            initial_position: native::centered_position(WINDOW_WIDTH, WINDOW_HEIGHT)
                .inspect_err(|error| crate::debug(&error.to_string()))
                .ok(),
            close,
            hovering_help: false,
            version: native::version().unwrap_or_else(|error| {
                crate::debug(&error.to_string());
                format!("enable-loopback v{}", env!("CARGO_PKG_VERSION"))
            }),
        }
    }
    fn update(&mut self, message: Message, cx: &ComponentContext<Self>) {
        let message = match message {
            Message::ToggleKey(key, checked) => {
                let Some(sid) = self.key_sid(&key) else {
                    return;
                };
                Message::Checked(sid, checked)
            }
            Message::PointerKey(key, column, event) => {
                let Some(sid) = self.key_sid(&key) else {
                    return;
                };
                Message::Pointer(sid, column, event)
            }
            Message::FocusKey(key) => {
                let Some(sid) = self.key_sid(&key) else {
                    return;
                };
                Message::RowFocused(sid)
            }
            message => message,
        };
        let rows_changed = matches!(
            &message,
            Message::Loaded { .. }
                | Message::Sort(_)
                | Message::Checked(..)
                | Message::SetAll(_)
                | Message::ListKey(_)
                | Message::Search(_)
                | Message::BeginRefresh(_)
                | Message::Refresh
                | Message::DialogClosed(_)
        );
        match message {
            Message::FocusSearch
                if !self.dialog_open && !self.list.refreshing && !self.list.saving =>
            {
                let _ = self.search_ref.request_focus();
            }
            Message::SearchFocused(focused) => self.search_focused = focused,
            Message::Search(query) if query != self.list.query => {
                self.list.set_query(query);
                self.selection.prune(&self.order());
                self.keyboard_focus = false;
                self.list_generation += 1;
            }
            Message::WindowPrepared(Err(error)) => {
                crate::debug(&error.to_string());
                // Do not allow editing in a window whose native integration failed.
                let _ = cx.close_window();
            }
            Message::WindowPrepared(Ok(hwnd)) => self.hwnd = hwnd,
            Message::BeginRefresh(force) => self.refresh(force, cx),
            Message::Refresh if self.list.availability().refresh && !self.dialog_open => {
                if self.list.is_dirty() {
                    self.enqueue(Dialog::Refresh);
                } else {
                    self.refresh(true, cx);
                }
            }
            Message::Loaded {
                registry,
                force,
                fallback_code,
                result,
            } => match result {
                Ok(loaded) => {
                    self.list
                        .replace_apps(loaded.apps, &loaded.exempted, &self.placeholders);
                    let order = self.order();
                    self.selection.prune(&order);
                    self.list_generation += 1;
                    self.current_user = loaded.user;
                    match native::time() {
                        Ok(time) => {
                            self.status =
                                rust_i18n::t!("status.refreshed", time = time).into_owned()
                        }
                        Err(error) => {
                            crate::debug(&error.to_string());
                            self.status.clear();
                        }
                    }
                    if self.keep_prompting {
                        for sid in self.list.orphans.clone() {
                            self.enqueue(Dialog::Orphan {
                                sid: sid.to_string(),
                            });
                        }
                    }
                    let _ = self.list_focus.request_focus();
                }
                Err(netiso::Error::Enumerate { code, .. }) if !registry => {
                    self.enqueue(Dialog::Fallback { code, force })
                }
                Err(error) => self.load_failed(error, fallback_code),
            },
            Message::Sort(column) if self.list.availability().list => {
                self.list.sort_by(column, &self.placeholders);
                self.list_generation += 1;
                let order = self.order();
                if let Some(index) = self.selection.first_selected_index(&order) {
                    self.selection.focused = Some(order[index].clone());
                }
            }
            Message::Pointer(sid, column, event) => {
                self.keyboard_focus = false;
                let button = if event.is_right_button_pressed {
                    PointerButton::Right
                } else if event.is_left_button_pressed {
                    PointerButton::Left
                } else {
                    return;
                };
                self.selection.click(
                    &self.order(),
                    &sid,
                    Modifiers {
                        control: event.modifiers.contains(InputModifiers::CONTROL),
                        shift: event.modifiers.contains(InputModifiers::SHIFT),
                    },
                    button,
                    column,
                );
            }
            Message::ContextMenu((key, column, point)) if self.key_sid(&key).is_some() => {
                self.selection.context_column = Some(column);
                self.menu_timer = Some(cx.set_local_timeout(Duration::from_millis(1), move || {
                    Message::ShowContextMenu(point)
                }));
            }
            Message::ShowContextMenu(point) => {
                self.menu_timer = None;
                if let Some(app) = &self.app
                    && let Err(source) = app.show_menu_at(
                        point,
                        context_menu(cx.sender(), self.list.bulk_label_keys()),
                    )
                {
                    self.operation_failed(Error::Native {
                        operation: "AppContext::show_menu_at",
                        source,
                    });
                }
            }
            Message::RowFocused(sid) if self.list.rows.iter().any(|row| row.app.sid == sid) => {
                // A checkbox may take focus without changing the app's selected set.
                if self.selection.focused.as_ref() != Some(&sid) {
                    self.selection.focused = Some(sid);
                }
            }
            Message::ListKey(event) if self.list.availability().list => {
                self.keyboard_focus = true;
                let ctrl = event.modifiers.contains(InputModifiers::CONTROL);
                if ctrl && event.key == VirtualKey::A {
                    self.selection.select_all(&self.order());
                } else if ctrl && event.key == VirtualKey::C {
                    self.copy(false);
                } else if event.key == VirtualKey::UP || event.key == VirtualKey::DOWN {
                    self.selection.arrow(
                        &self.order(),
                        if event.key == VirtualKey::UP {
                            Direction::Up
                        } else {
                            Direction::Down
                        },
                        event.modifiers.contains(InputModifiers::SHIFT),
                    );
                } else if event.key == VirtualKey::SPACE
                    && let Some(sid) = &self.selection.focused
                {
                    if let Some(row) = self.list.rows.iter_mut().find(|r| r.app.sid == *sid) {
                        row.checked = !row.checked;
                    }
                    self.change_status(self.list.status_after_change(Change::Checked));
                }
            }
            Message::Copy(column) => self.copy(column),
            Message::CloseRequested => {
                if !self.list.saving {
                    if self.list.is_dirty() {
                        self.enqueue(Dialog::Close);
                    } else {
                        self.close.confirmed.set(true);
                        let _ = cx.close_window();
                    }
                } else {
                    self.close.requested.set(false);
                }
            }
            Message::Help => {
                if let Err(error) = native::open_help(self.hwnd, LEARN_URL) {
                    self.operation_failed(error);
                }
            }
            Message::HoverHelp(value) => self.hovering_help = value,
            Message::Save if !self.dialog_open => {
                if let Some(sids) = self.list.begin_save() {
                    cx.spawn_background(move |_| Message::Saved(netiso::set_exempted(&sids)));
                }
            }
            Message::Saved(result) => {
                self.list.finish_save(result.is_ok());
                match result {
                    Ok(()) => self.status = rust_i18n::t!("status.saved").into_owned(),
                    Err(netiso::Error::SetExemptionsAccessDenied { .. }) => {
                        self.status = rust_i18n::t!("status.not_saved_admin").into_owned();
                        self.enqueue(Dialog::Error {
                            title: rust_i18n::t!("dialog.admin_title").into_owned(),
                            body: rust_i18n::t!("dialog.admin_body").into_owned(),
                        });
                    }
                    Err(error) => {
                        self.status = rust_i18n::t!("status.not_saved").into_owned();
                        self.enqueue(Dialog::Error {
                            title: rust_i18n::t!("dialog.save_title").into_owned(),
                            body: error.to_string(),
                        });
                    }
                }
            }
            Message::Checked(sid, checked) if self.list.availability().list => {
                if let Some(row) = self.list.rows.iter_mut().find(|row| row.app.sid == sid) {
                    row.checked = checked;
                }
                self.change_status(self.list.status_after_change(Change::Checked));
            }
            Message::SetAll(checked) if self.list.availability().bulk => {
                let status = self.list.set_all(checked);
                self.change_status(status);
            }
            Message::KeepPrompting(value) => self.keep_prompting = value,
            Message::DialogClosed(result) => {
                self.dialog_open = false;
                match &self.dialog {
                    Some(Dialog::Close) => {
                        self.close.requested.set(false);
                        if result == ContentDialogResult::Primary {
                            self.close.confirmed.set(true);
                            let _ = cx.close_window();
                        }
                    }
                    Some(Dialog::Refresh) if result == ContentDialogResult::Primary => {
                        self.refresh(true, cx)
                    }
                    Some(Dialog::Fallback { code, force }) => {
                        self.start_load(true, *force, Some(*code), cx)
                    }
                    Some(Dialog::Orphan { .. }) if !self.keep_prompting => {
                        self.dialogs.retain(|d| !matches!(d, Dialog::Orphan { .. }))
                    }
                    _ => {}
                }
                // Publish a closed frame before opening another ContentDialog.
                self.dialog_timer =
                    Some(cx.set_local_timeout(Duration::from_millis(1), || Message::NextDialog));
            }
            Message::NextDialog => {
                self.dialog_timer = None;
                self.next_dialog();
            }
            Message::Size(size) => self.viewport_width = (size.width - 2.0 * PAGE_MARGIN).max(0.0),
            _ => {}
        }
        if rows_changed {
            self.sync_table_rows();
        }
        self.close.dirty.set(self.list.is_dirty());
        self.close.saving.set(self.list.saving);
    }
    fn view(&self, _: &Startup, cx: &mut ViewContext<Self>) -> View {
        cx.window_title(rust_i18n::t!("window.title"));
        let mut visuals = WindowVisuals::new()
            .backdrop(WindowBackdrop::Mica)
            .client_size(WINDOW_WIDTH, WINDOW_HEIGHT)
            .constraints(WindowConstraints {
                min_width: Some(640.0),
                min_height: Some(400.0),
                ..Default::default()
            });
        if let Some(position) = self.initial_position {
            visuals = visuals.initial_position(position);
        }
        cx.window_visuals(visuals);
        cx.on_window_size(cx.callback(Message::Size));
        let mut status: Vec<View> = Vec::new();
        if self.list.refreshing {
            status.push(
                ProgressRing::new()
                    .width(16.0)
                    .height(16.0)
                    .is_active(true)
                    .into(),
            );
        }
        status.push(
            TextBlock::new()
                .automation_id("status")
                .text(if self.hovering_help {
                    &self.version
                } else {
                    &self.status
                })
                .text_wrapping(TextWrapping::Wrap)
                .into(),
        );
        Border::new()
            .automation_id("main")
            .on_preview_key_down(keys::global(cx.sender()))
            .content(
                Grid::new()
                    .margin(Thickness::uniform(PAGE_MARGIN))
                    .row_spacing(SECTION_SPACING)
                    .rows([
                        GridLength::Auto,
                        GridLength::Auto,
                        GridLength::STAR,
                        GridLength::Auto,
                    ])
                    .children((
                        explanation_bar(cx).grid_row(0),
                        self.toolbar(cx).grid_row(1),
                        Border::new().grid_row(2).content(self.table(cx)),
                        Grid::new()
                            .grid_row(3)
                            .columns([GridLength::STAR, GridLength::Auto])
                            .column_spacing(SECTION_SPACING)
                            .children((
                                StackPanel::new()
                                    .orientation(Orientation::Horizontal)
                                    .spacing(8.0)
                                    .children(status),
                                TextBlock::new()
                                    .grid_column(1)
                                    .automation_id("counter")
                                    .text(self.counter_text())
                                    .vertical_alignment(VerticalAlignment::Center),
                            )),
                    )),
            )
            .content_dialog(dialogs::view(
                self.dialog.as_ref(),
                self.dialog_open,
                self.keep_prompting,
                cx,
            ))
    }
}

fn toolbar_button(key: &str, symbol: Symbol) -> Button {
    Button::new()
        .automation_name(rust_i18n::t!(key))
        .horizontal_alignment(HorizontalAlignment::Stretch)
        .style(ButtonStyle::Default)
        .content(
            StackPanel::new()
                .orientation(Orientation::Horizontal)
                .spacing(8.0)
                .children((
                    SymbolIcon::new().symbol(symbol),
                    TextBlock::new().text(rust_i18n::t!(key)),
                )),
        )
}

/// Informational InfoBar look-alike (§4.2). InfoBar has no Content/ActionButton relation in
/// a29e064, so the Learn more link would otherwise sit on a row of its own.
fn explanation_bar(cx: &ViewContext<MainWindow>) -> Border {
    // Glyphs and sizes follow the WinUI InfoBar template: a status circle behind an "i".
    const ICON_BACKGROUND_GLYPH: &str = "\u{F136}";
    const INFORMATIONAL_ICON_GLYPH: &str = "\u{F13F}";
    const ICON_SIZE: f64 = 16.0;
    Border::new()
        .background(ThemeBrush::CardBackground)
        .border_brush(ThemeBrush::CardStroke)
        .border_thickness(Thickness::uniform(1.0))
        .corner_radius(CornerRadius::uniform(4.0))
        .padding(Thickness::new(16.0, 6.0, 8.0, 6.0))
        .content(
            Grid::new()
                .columns([GridLength::Auto, GridLength::STAR, GridLength::Auto])
                .children((
                    FontIcon::new()
                        .glyph(ICON_BACKGROUND_GLYPH)
                        .font_size(ICON_SIZE)
                        .foreground(ThemeBrush::Accent),
                    // The window base fill contrasts with the accent in both themes.
                    FontIcon::new()
                        .glyph(INFORMATIONAL_ICON_GLYPH)
                        .font_size(ICON_SIZE)
                        .foreground(ThemeBrush::SolidBackground),
                    TextBlock::new()
                        .grid_column(1)
                        .margin(Thickness::new(14.0, 6.0, 12.0, 6.0))
                        .vertical_alignment(VerticalAlignment::Center)
                        .text_wrapping(TextWrapping::Wrap)
                        .text(rust_i18n::t!("window.explanation")),
                    Border::new()
                        .grid_column(2)
                        .vertical_alignment(VerticalAlignment::Center)
                        .on_pointer_entered(cx.callback(|_| Message::HoverHelp(true)))
                        .on_pointer_exited(cx.callback(|_| Message::HoverHelp(false)))
                        .content(
                            HyperlinkButton::new()
                                .vertical_alignment(VerticalAlignment::Center)
                                .content(rust_i18n::t!("window.learn_more").into_owned())
                                .navigate_uri(LEARN_URL)
                                .expect("fixed Microsoft Learn URI"),
                        ),
                )),
        )
}

#[allow(
    clippy::manual_dangling_ptr,
    reason = "MAKEINTRESOURCEW(1) is an integer resource identifier"
)]
fn set_window_icons(window: &WindowHandle<'_>) -> Result<(), Error> {
    // SAFETY: null requests our module; resource ID 1 is embedded by build.rs.
    // LR_SHARED ties icon lifetime to the module, so no icon handle needs freeing.
    let module = unsafe { GetModuleHandleW(PCWSTR::null()) };
    // SAFETY: run_window supplies a live HWND; use its current monitor DPI.
    let dpi = unsafe { GetDpiForWindow(window.as_raw().cast()) };
    for (kind, metric) in [(ICON_BIG, SM_CXICON), (ICON_SMALL, SM_CXSMICON)] {
        // SAFETY: both metrics are valid icon dimensions at this window DPI.
        let size = unsafe { GetSystemMetricsForDpi(metric, dpi) };
        let icon = unsafe {
            LoadImageW(
                Some(module),
                PCWSTR(1usize as *const u16),
                IMAGE_ICON as u32,
                size,
                size,
                LR_SHARED as u32,
            )
        };
        if icon.is_null() {
            return Err(Error::LoadWindowIcon {
                resource_id: 1,
                size,
                source: windows::core::Error::from_thread(),
            });
        }
        // SAFETY: run_window supplies a live HWND on its UI thread; icon is module-owned.
        unsafe {
            SendMessageW(
                window.as_raw().cast(),
                WM_SETICON as u32,
                kind as usize,
                icon as isize,
            );
        }
    }
    Ok(())
}

pub fn run(options: GuiOptions, publisher: Rc<WindowPublisher>) -> Result<(), Error> {
    App::run_with(move |app| {
        app.open_component_window_with_policy::<MainWindow>(
            Startup {
                options,
                publisher,
                app: Some(Rc::new(app.clone())),
            },
            WindowPolicy::new(),
        )
    })
    .map_err(|source| Error::StartWindow { source })
}
