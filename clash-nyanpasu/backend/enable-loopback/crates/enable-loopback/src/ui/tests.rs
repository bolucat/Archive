//! Render the actual component through reactor's recording adapter. Background closures
//! are retained, never run: these tests cannot enumerate/write the real firewall table.
use super::*;
use std::sync::{Arc, Mutex};
use windows::Win32::winerror::{ERROR_ACCESS_DENIED, ERROR_INVALID_DATA};

type Timer = (Arc<TimerRegistration>, Box<dyn FnOnce() + Send>);
#[derive(Default)]
struct Services {
    background: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
    timers: Mutex<VecDeque<Timer>>,
}
#[derive(Default)]
struct TimerRegistration;
impl ComponentTimerRegistration for TimerRegistration {
    fn cancel(&self) {}
}
impl ComponentServices for Services {
    fn spawn_background(&self, work: Box<dyn FnOnce() + Send>) {
        self.background.lock().unwrap().push(work);
    }
    fn set_timeout(
        &self,
        _: Duration,
        callback: Box<dyn FnOnce() + Send>,
    ) -> Arc<dyn ComponentTimerRegistration> {
        let timer = Arc::new(TimerRegistration);
        self.timers
            .lock()
            .unwrap()
            .push_back((timer.clone(), callback));
        timer
    }
}
struct Fixture {
    host: ComponentHost<RecordingAdapter>,
    sender: ComponentSender<Message>,
    services: Arc<Services>,
    _primary: crate::instance::Primary,
}
impl Fixture {
    fn new(name: &str) -> Self {
        let primary = crate::instance::acquire_for_test(name);
        let services = Arc::new(Services::default());
        let host = ComponentHost::mount_with_services(
            RecordingAdapter::default(),
            services.clone(),
            [component::<MainWindow>(
                "main",
                Startup {
                    options: GuiOptions {
                        manual: true,
                        registry: false,
                    },
                    publisher: primary.publisher(),
                    app: None,
                },
            )],
        )
        .unwrap();
        let sender = host.sender::<MainWindow>(&Key::from("main")).unwrap();
        Self {
            host,
            sender,
            services,
            _primary: primary,
        }
    }
    /// Drains messages and timers, then prepares every row like a list showing them all.
    fn settle(&mut self, budget: usize) {
        self.host.drain(budget).unwrap();
        loop {
            let timer = self.services.timers.lock().unwrap().pop_front();
            let Some((_, callback)) = timer else { break };
            callback();
            self.host.drain(budget).unwrap();
        }
        let graph = self.host.runtime().graph();
        let Some(list) = find(graph, graph.root().unwrap(), "apps") else {
            return;
        };
        let count = graph.virtual_item_count(list).unwrap();
        let source_revision = graph.virtual_source_revision(list).unwrap();
        for index in 0..count {
            self.host
                .test_adapter_mut()
                .queue_realization(RealizationRequest::Realize {
                    collection: list,
                    container: RealizedContainer(index as u64),
                    index,
                    source_revision,
                });
        }
        self.host.drain(budget.max(count * 10)).unwrap();
    }
    fn send(&mut self, message: Message) {
        assert!(self.sender.send(message));
        self.settle(100);
    }
    fn send_table(&mut self, message: table::Message) {
        let sender = self
            .host
            .sender_at::<table::TableView>(&[Key::from("main"), Key::from("table")])
            .unwrap();
        assert!(sender.send(message));
        self.settle(1000);
    }
    fn emit(&mut self, id: &str, event: EventId, payload: EventPayload) {
        let graph = self.host.runtime().graph();
        let object = find(graph, graph.root().unwrap(), id).unwrap();
        let callback = graph
            .events(object)
            .unwrap()
            .iter()
            .find(|value| value.id == event)
            .unwrap()
            .value
            .clone();
        self.host
            .test_adapter_mut()
            .queue_event(EventDispatch::new(object, event, callback, payload));
        self.settle(1000);
    }
    fn close_dialog(&mut self, result: ContentDialogResult) {
        self.send(Message::DialogClosed(result));
    }
    fn load(&mut self, orphans: &[u32]) {
        self.send(Message::Loaded {
            registry: false,
            force: false,
            fallback_code: None,
            result: Ok(Loaded {
                apps: vec![app(1), app(2)],
                exempted: orphans.iter().map(|id| sid(*id)).collect(),
                user: "DOMAIN\\user".into(),
            }),
        });
    }
    fn property(&self, id: &str, property: PropertyId) -> PropertyValue {
        let graph = self.host.runtime().graph();
        let object = find(graph, graph.root().unwrap(), id).unwrap();
        graph
            .properties(object)
            .unwrap()
            .iter()
            .find(|p| p.id == property)
            .unwrap()
            .value
            .clone()
    }
    fn dialog(&self) -> (ObjectId, bool) {
        let graph = self.host.runtime().graph();
        let root = find(graph, graph.root().unwrap(), "main").unwrap();
        self.host.adapter().content_dialog(root).unwrap()
    }
    fn dialog_property(&self, property: PropertyId) -> PropertyValue {
        let (dialog, _) = self.dialog();
        self.host
            .runtime()
            .graph()
            .properties(dialog)
            .unwrap()
            .iter()
            .find(|p| p.id == property)
            .unwrap()
            .value
            .clone()
    }
}
fn find(graph: &RetainedGraph, root: ObjectId, id: &str) -> Option<ObjectId> {
    if graph
        .properties(root)
        .unwrap()
        .iter()
        .any(|p| p.id == PropertyId::AutomationId && p.value == PropertyValue::String(id.into()))
    {
        return Some(root);
    }
    for relation in [
        RelationId::Content,
        RelationId::Children,
        RelationId::PrimaryCommands,
        RelationId::SecondaryCommands,
        RelationId::Items,
        RelationId::VirtualItems,
    ] {
        if let Some(child) = graph.child(root, relation)
            && let Some(found) = find(graph, child, id)
        {
            return Some(found);
        }
        for child in graph.children(root, relation).unwrap_or_default() {
            if let Some(found) = find(graph, *child, id) {
                return Some(found);
            }
        }
    }
    None
}
fn sid(id: u32) -> Sid {
    format!("S-1-15-2-{id}").parse().unwrap()
}
fn app(id: u32) -> AppContainer {
    AppContainer {
        sid: sid(id),
        users: vec!["DOMAIN\\user".into()],
        name: Some(format!("app{id}")),
        display_name: Some(format!("app{id}")),
        description: None,
        package_full_name: None,
        binaries: vec![],
        exempt_at_last_check: false,
    }
}

#[test]
fn live_search_prunes_selection_scopes_bulk_to_results_and_renders_empty_template() {
    let mut f = Fixture::new("search");
    f.load(&[]);
    let key = |key| KeyEventInfo {
        key,
        original_key: key,
        modifiers: InputModifiers::CONTROL,
        status: PhysicalKeyStatus::default(),
    };
    f.send(Message::ListKey(key(VirtualKey::A)));
    f.emit(
        "search",
        EventId::TextChanged,
        EventPayload::String("app2".into()),
    );
    assert_eq!(
        f.property("search", PropertyId::Text),
        PropertyValue::String("app2".into())
    );
    let graph = f.host.runtime().graph();
    assert!(find(graph, graph.root().unwrap(), &format!("row_{}", sid(1))).is_none());
    assert_eq!(
        f.property(&format!("chrome_{}", sid(2)), PropertyId::Background),
        PropertyValue::Brush(ThemeBrush::SubtleFillTertiary.into())
    );
    // Visible index 0 now refers to SID 2, not the first row in the full model.
    f.send(Message::ToggleKey(sid(2).to_string(), true));
    f.send(Message::Search(String::new()));
    assert_eq!(
        f.property(&format!("toggle_{}", sid(2)), PropertyId::IsChecked),
        PropertyValue::OptionalBool(Some(true))
    );
    assert_eq!(
        f.property(&format!("chrome_{}", sid(1)), PropertyId::Background),
        PropertyValue::Brush(Color::transparent().into())
    );
    f.send(Message::Search("app2".into()));
    // Only the visible match is exempted; the hidden first row stays unchecked.
    f.send(Message::SetAll(true));
    let expected = format!(
        "{} · {}",
        rust_i18n::t!("toolbar.counter", exempt = 1, total = 2),
        rust_i18n::t!("search.results", count = 1)
    );
    assert_eq!(
        f.property("counter", PropertyId::Text),
        PropertyValue::String(expected.into())
    );
    f.send(Message::Search("nonsense-query".into()));
    assert_eq!(
        f.property("search_empty", PropertyId::Text),
        PropertyValue::String(rust_i18n::t!("search.empty").into_owned().into())
    );
    f.send(Message::ListKey(key(VirtualKey::A)));
    f.emit(
        "search_host",
        EventId::PreviewKeyDown,
        EventPayload::KeyEventInfo(KeyEventInfo {
            key: VirtualKey::ESCAPE,
            original_key: VirtualKey::ESCAPE,
            modifiers: InputModifiers::NONE,
            status: PhysicalKeyStatus::default(),
        }),
    );
    assert_eq!(
        f.property("search", PropertyId::Text),
        PropertyValue::String("".into())
    );
    for id in [1, 2] {
        assert_eq!(
            f.property(&format!("chrome_{}", sid(id)), PropertyId::Background),
            PropertyValue::Brush(Color::transparent().into())
        );
    }
    let graph = f.host.runtime().graph();
    assert!(find(graph, graph.root().unwrap(), "search_empty").is_none());
    assert!(f.services.background.lock().unwrap().is_empty());
}

#[test]
fn actual_view_retains_dirty_edits_after_access_denied_and_commits_only_success() {
    let mut f = Fixture::new("save");
    f.load(&[]);
    assert_eq!(
        f.property("save", PropertyId::IsEnabled),
        PropertyValue::Bool(false)
    );
    f.send(Message::Checked(sid(1), true));
    assert_eq!(
        f.property("save", PropertyId::Style),
        PropertyValue::ButtonStyle(ButtonStyle::Accent)
    );
    f.send(Message::Save);
    assert_eq!(f.services.background.lock().unwrap().len(), 1);
    assert_eq!(
        f.property("save", PropertyId::IsEnabled),
        PropertyValue::Bool(false)
    );
    f.send(Message::Saved(Err(
        netiso::Error::SetExemptionsAccessDenied {
            sid_count: 1,
            code: ERROR_ACCESS_DENIED as u32,
        },
    )));
    assert!(f.dialog().1);
    assert_eq!(
        f.dialog_property(PropertyId::Title),
        PropertyValue::String(rust_i18n::t!("dialog.admin_title").into_owned().into())
    );
    assert_eq!(
        f.property("save", PropertyId::IsEnabled),
        PropertyValue::Bool(true)
    );
    assert_eq!(
        f.property("counter", PropertyId::Text),
        PropertyValue::String(
            rust_i18n::t!("toolbar.counter", exempt = 1, total = 2)
                .into_owned()
                .into()
        )
    );
    f.close_dialog(ContentDialogResult::None);
    f.send(Message::Save);
    assert_eq!(f.services.background.lock().unwrap().len(), 2);
    f.send(Message::Saved(Err(netiso::Error::SetExemptions {
        sid_count: 1,
        code: ERROR_INVALID_DATA as u32,
    })));
    assert_eq!(
        f.property("save", PropertyId::IsEnabled),
        PropertyValue::Bool(true)
    );
    assert_eq!(
        f.dialog_property(PropertyId::Title),
        PropertyValue::String(rust_i18n::t!("dialog.save_title").into_owned().into())
    );
    f.close_dialog(ContentDialogResult::None);
    f.send(Message::Save);
    assert_eq!(f.services.background.lock().unwrap().len(), 3);
    f.send(Message::Saved(Ok(())));
    assert_eq!(
        f.property("save", PropertyId::IsEnabled),
        PropertyValue::Bool(false)
    );
    assert_eq!(
        f.property("status", PropertyId::Text),
        PropertyValue::String(rust_i18n::t!("status.saved").into_owned().into())
    );
}

#[test]
fn confirmations_default_to_cancel_and_do_not_drop_edits() {
    let mut f = Fixture::new("confirm");
    f.load(&[]);
    f.send(Message::Checked(sid(1), true));
    for message in [Message::Refresh, Message::CloseRequested] {
        f.send(message);
        assert!(f.dialog().1);
        assert_eq!(
            f.dialog_property(PropertyId::DefaultButton),
            PropertyValue::Enum {
                kind: "ContentDialogButton",
                variant: "Close"
            }
        );
        f.close_dialog(ContentDialogResult::None);
        assert!(!f.dialog().1);
        assert_eq!(
            f.property("save", PropertyId::IsEnabled),
            PropertyValue::Bool(true)
        );
        assert!(f.services.background.lock().unwrap().is_empty());
    }
    f.send(Message::Refresh);
    f.close_dialog(ContentDialogResult::Primary);
    assert_eq!(f.services.background.lock().unwrap().len(), 1);
    assert_eq!(
        f.property("apps", PropertyId::IsEnabled),
        PropertyValue::Bool(false)
    );
}

#[test]
fn fallback_warning_precedes_registry_read_and_firewall_failure_dialog() {
    let mut f = Fixture::new("fallback");
    f.send(Message::BeginRefresh(false));
    assert_eq!(f.services.background.lock().unwrap().len(), 1);
    f.send(Message::Loaded {
        registry: false,
        force: false,
        fallback_code: None,
        result: Err(netiso::Error::Enumerate {
            force_binaries: false,
            code: EPT_S_NOT_REGISTERED as u32,
        }),
    });
    assert_eq!(
        f.dialog_property(PropertyId::Title),
        PropertyValue::String(rust_i18n::t!("dialog.warning").into_owned().into())
    );
    assert_eq!(f.services.background.lock().unwrap().len(), 1); // no fallback until acknowledged
    f.close_dialog(ContentDialogResult::None);
    assert_eq!(f.services.background.lock().unwrap().len(), 2);
    f.send(Message::Loaded {
        registry: true,
        force: false,
        fallback_code: Some(EPT_S_NOT_REGISTERED as u32),
        result: Err(netiso::Error::ReadRegistry {
            step: "open",
            path: "test".into(),
            code: ERROR_INVALID_DATA as u32,
        }),
    });
    assert_eq!(
        f.dialog_property(PropertyId::Title),
        PropertyValue::String(rust_i18n::t!("dialog.firewall_title").into_owned().into())
    );
    assert_eq!(
        f.property("refresh", PropertyId::IsEnabled),
        PropertyValue::Bool(true)
    );
    assert_eq!(
        f.property("apps", PropertyId::IsEnabled),
        PropertyValue::Bool(false)
    );
}

#[test]
fn orphan_prompts_follow_config_order_and_opt_out_lasts_for_the_session() {
    let mut f = Fixture::new("orphans");
    f.load(&[9, 8, 7]);
    assert!(f.dialog().1);
    let first = f.dialog().0;
    let content = f
        .host
        .runtime()
        .graph()
        .child(first, RelationId::Content)
        .unwrap();
    let children = f
        .host
        .runtime()
        .graph()
        .children(content, RelationId::Children)
        .unwrap();
    assert!(
        f.host
            .runtime()
            .graph()
            .properties(children[1])
            .unwrap()
            .iter()
            .any(|p| p.id == PropertyId::Text
                && p.value == PropertyValue::String(sid(9).to_string().into()))
    );
    f.close_dialog(ContentDialogResult::None);
    let second = f.dialog().0;
    let content = f
        .host
        .runtime()
        .graph()
        .child(second, RelationId::Content)
        .unwrap();
    let children = f
        .host
        .runtime()
        .graph()
        .children(content, RelationId::Children)
        .unwrap();
    assert!(
        f.host
            .runtime()
            .graph()
            .properties(children[1])
            .unwrap()
            .iter()
            .any(|p| p.id == PropertyId::Text
                && p.value == PropertyValue::String(sid(8).to_string().into()))
    );
    f.send(Message::KeepPrompting(false));
    f.close_dialog(ContentDialogResult::None);
    assert!(!f.dialog().1);
    f.load(&[9, 8]);
    assert!(!f.dialog().1);
    f.send(Message::Checked(sid(1), true));
    if let PropertyValue::String(status) = f.property("status", PropertyId::Text) {
        assert!(status.contains(rust_i18n::t!("status.orphan_removal", count = 2).as_ref()));
    } else {
        panic!("status text expected");
    }
}

#[test]
fn sorting_rebuilds_the_native_list_and_restores_the_sid_focus_after_mount() {
    let mut f = Fixture::new("sort");
    f.load(&[]);
    f.send(Message::Pointer(
        sid(1),
        Column::DisplayName,
        PointerEventInfo {
            is_left_button_pressed: true,
            ..Default::default()
        },
    ));
    f.send(Message::Sort(Column::DisplayName));
    assert!(matches!(
        f.host
            .adapter()
            .imperatives()
            .iter()
            .rev()
            .find(|request| matches!(request, ImperativeRequest::ScrollIntoView { .. })),
        Some(ImperativeRequest::ScrollIntoView { index: 1, .. })
    ));
    assert_eq!(
        f.property("apps", PropertyId::SelectionMode),
        PropertyValue::Enum {
            kind: "ListViewSelectionMode",
            variant: "None"
        }
    );
    f.send(Message::Checked(sid(1), true));
    assert_eq!(
        f.property("counter", PropertyId::Text),
        PropertyValue::String(
            rust_i18n::t!("toolbar.counter", exempt = 1, total = 2)
                .into_owned()
                .into()
        )
    );
}

#[test]
fn captured_resize_with_260_rows_keeps_geometry_selection_and_sorting() {
    let mut f = Fixture::new("resize");
    f.send(Message::Loaded {
        registry: false,
        force: false,
        fallback_code: None,
        result: Ok(Loaded {
            apps: (1..=260).map(app).collect(),
            exempted: vec![],
            user: String::new(),
        }),
    });
    f.send(Message::Pointer(
        sid(131),
        Column::DisplayName,
        PointerEventInfo {
            is_left_button_pressed: true,
            ..Default::default()
        },
    ));
    let focus = f.property(&format!("chrome_{}", sid(131)), PropertyId::Background);
    let columns = |width: f64| {
        PropertyValue::GridLengths(
            vec![
                GridLength::Pixel(40.0),
                GridLength::Pixel(width),
                GridLength::Pixel(220.0),
                GridLength::Pixel(200.0),
                GridLength::Pixel(160.0),
                GridLength::Pixel(140.0),
                GridLength::Pixel(140.0),
                GridLength::Pixel(200.0),
            ]
            .into(),
        )
    };
    // Failed capture and unrelated pointer IDs must not start/move a resize.
    f.send_table(table::Message::ResizeStart(
        Column::DisplayName,
        PointerEventInfo {
            is_left_button_pressed: true,
            capture_succeeded: Some(false),
            ..Default::default()
        },
    ));
    f.send_table(table::Message::ResizeMove(PointerEventInfo {
        window_x: 300.0,
        ..Default::default()
    }));
    assert_eq!(
        f.property("header_columns", PropertyId::GridColumns),
        columns(220.0)
    );
    let press = PointerEventInfo {
        pointer_id: 5,
        window_x: 260.0,
        is_left_button_pressed: true,
        capture_succeeded: Some(true),
        ..Default::default()
    };
    f.send_table(table::Message::ResizeStart(Column::DisplayName, press));
    f.send_table(table::Message::Sort(Column::DisplayName)); // capture may never sort
    f.send_table(table::Message::ResizeMove(PointerEventInfo {
        pointer_id: 6,
        window_x: 900.0,
        ..press
    }));
    assert_eq!(
        f.property("header_columns", PropertyId::GridColumns),
        columns(220.0)
    );
    let start = std::time::Instant::now();
    for delta in 1..=20 {
        f.send_table(table::Message::ResizeMove(PointerEventInfo {
            window_x: 260.0 + delta as f64,
            ..press
        }));
    }
    eprintln!(
        "260 rows: 20 resize updates {:?} ({:?}/update, recording adapter; excludes native layout)",
        start.elapsed(),
        start.elapsed() / 20
    );
    assert_eq!(
        f.property("header_columns", PropertyId::GridColumns),
        columns(240.0)
    );
    // Rows follow on a coalescing frame timer; its tick may already have run.
    f.send_table(table::Message::FollowResize);
    assert_eq!(
        f.property(&format!("columns_{}", sid(131)), PropertyId::GridColumns),
        columns(240.0)
    );
    assert!(
        f.host
            .sender_at::<table::TableView>(&[Key::from("main"), Key::from("table")])
            .unwrap()
            .send(table::Message::ResizeLost)
    );
    f.send_table(table::Message::ResizeEnd(PointerEventInfo {
        window_x: 300.0,
        is_left_button_pressed: false,
        ..press
    }));
    assert_eq!(
        f.property("header_columns", PropertyId::GridColumns),
        columns(260.0)
    );
    assert_eq!(
        f.property(&format!("columns_{}", sid(131)), PropertyId::GridColumns),
        columns(260.0)
    );
    assert_eq!(
        f.property(&format!("chrome_{}", sid(131)), PropertyId::Background),
        focus
    );
    assert_eq!(
        f.property("save", PropertyId::IsEnabled),
        PropertyValue::Bool(false)
    );
    f.send_table(table::Message::ResizeStart(Column::DisplayName, press));
    f.send_table(table::Message::ResizeLost);
    f.send_table(table::Message::ResizeMove(PointerEventInfo {
        window_x: 1000.0,
        ..press
    }));
    assert_eq!(
        f.property("header_columns", PropertyId::GridColumns),
        columns(260.0)
    );
    let mut release_time = Duration::ZERO;
    for _ in 0..20 {
        f.send_table(table::Message::ResizeStart(Column::DisplayName, press));
        f.send_table(table::Message::ResizeMove(PointerEventInfo {
            window_x: press.window_x + 1.0,
            ..press
        }));
        let start = std::time::Instant::now();
        f.send_table(table::Message::ResizeEnd(PointerEventInfo {
            window_x: press.window_x + 1.0,
            is_left_button_pressed: false,
            ..press
        }));
        release_time += start.elapsed();
    }
    eprintln!(
        "260 rows: committing all row widths {:?}/release (20 samples, recording adapter; excludes native layout)",
        release_time / 20
    );
}

#[test]
fn nested_table_routes_selection_checkbox_keys_hover_and_keyboard_focus() {
    let mut f = Fixture::new("table-events");
    f.load(&[]);
    let row1 = format!("row_{}", sid(1));
    let row2 = format!("row_{}", sid(2));
    let chrome1 = format!("chrome_{}", sid(1));
    let chrome2 = format!("chrome_{}", sid(2));
    assert_eq!(
        f.property(&format!("current_user_{}", sid(1)), PropertyId::Foreground),
        PropertyValue::Brush(Brush::Theme(ThemeBrush::TextSecondary))
    );
    let pointer = PointerEventInfo {
        is_left_button_pressed: true,
        x: 52.0,
        ..Default::default()
    };
    f.emit(
        &row1,
        EventId::PointerPressed,
        EventPayload::PointerEventInfo(pointer),
    );
    f.emit(
        &row2,
        EventId::PointerPressed,
        EventPayload::PointerEventInfo(PointerEventInfo {
            modifiers: InputModifiers::CONTROL,
            ..pointer
        }),
    );
    let selected = PropertyValue::Brush(Brush::Theme(ThemeBrush::SubtleFillTertiary));
    assert_eq!(f.property(&chrome1, PropertyId::Background), selected);
    assert_eq!(f.property(&chrome2, PropertyId::Background), selected);
    f.emit(
        &format!("toggle_{}", sid(1)),
        EventId::IsCheckedChanged,
        EventPayload::OptionalBool(Some(true)),
    );
    assert_eq!(f.property(&chrome1, PropertyId::Background), selected);
    assert_eq!(f.property(&chrome2, PropertyId::Background), selected);
    assert_eq!(
        f.property("save", PropertyId::IsEnabled),
        PropertyValue::Bool(true)
    );
    f.emit(
        "table",
        EventId::PreviewKeyDown,
        EventPayload::KeyEventInfo(KeyEventInfo {
            key: VirtualKey::UP,
            original_key: VirtualKey::UP,
            status: PhysicalKeyStatus::default(),
            modifiers: InputModifiers::NONE,
        }),
    );
    // The recording adapter accepts imperative focus without emitting native GotFocus.
    f.emit(
        &row1,
        EventId::GotFocus,
        EventPayload::FocusEventInfo(FocusEventInfo {
            state: ElementFocusState::Programmatic,
            is_direct: true,
        }),
    );
    assert_eq!(
        f.property(&chrome1, PropertyId::BorderThickness),
        PropertyValue::Thickness(Thickness::uniform(2.0))
    );
    f.emit(
        &row1,
        EventId::PointerPressed,
        EventPayload::PointerEventInfo(pointer),
    );
    assert_eq!(
        f.property(&chrome1, PropertyId::BorderThickness),
        PropertyValue::Thickness(Thickness::uniform(0.0))
    );
    // Only the hovered row carries its seven cell tooltips.
    let tooltips = |f: &Fixture| {
        let graph = f.host.runtime().graph();
        let columns = find(graph, graph.root().unwrap(), &format!("columns_{}", sid(2))).unwrap();
        graph
            .children(columns, RelationId::Children)
            .unwrap()
            .iter()
            .filter(|cell| f.host.adapter().tooltip(**cell).is_some())
            .count()
    };
    assert_eq!(tooltips(&f), 0);
    f.emit(
        &row2,
        EventId::PointerEntered,
        EventPayload::PointerEventInfo(pointer),
    );
    assert_eq!(
        f.property(&chrome2, PropertyId::Background),
        PropertyValue::Brush(Brush::Theme(ThemeBrush::SubtleFillSecondary))
    );
    assert_eq!(tooltips(&f), 7);
    f.emit(
        &row2,
        EventId::PointerExited,
        EventPayload::PointerEventInfo(pointer),
    );
    assert_eq!(
        f.property(&chrome2, PropertyId::Background),
        PropertyValue::Brush(Color::transparent().into())
    );
    assert_eq!(tooltips(&f), 0);
}
