//! A nested, controlled table. Only column geometry and pointer drags belong here;
//! callers own data, sorting, selection and commands. Row components own transient chrome.
mod row;

use crate::model::{Column, ColumnWidths, Sort};
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    time::Duration,
};
use windows_reactor::*;

const SORT_GLYPH_SIZE: f64 = 10.0;

pub(super) const COLUMNS: [Column; 7] = [
    Column::DisplayName,
    Column::Description,
    Column::Package,
    Column::Name,
    Column::Sid,
    Column::Users,
    Column::Binaries,
];

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ColumnSpec {
    pub header: String,
    pub initial_width: f64,
    pub min_width: f64,
    pub sortable: bool,
    pub resizable: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Row {
    pub key: String,
    pub checked: bool,
    pub checkbox_label: String,
    pub cells: [String; 7],
    pub highlight: bool,
}

#[derive(Clone, PartialEq)]
pub(super) struct Input {
    pub columns: [ColumnSpec; 7],
    pub rows: Rc<Vec<Rc<Row>>>,
    /// Optional centered empty template, supplied by the caller.
    pub empty_text: Option<String>,
    pub restore_focus: bool,
    pub sort: Sort,
    pub selected: BTreeSet<String>,
    pub focused: Option<String>,
    pub keyboard_focus: bool,
    pub enabled: bool,
    pub viewport_width: f64,
    /// Replace the native ListView when its item order changes (upstream #5146).
    pub generation: u64,
    pub focus_ref: ElementRef<Border>,
    pub on_sort: Callback<Column>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_pointer: Callback<(String, Column, PointerEventInfo)>,
    pub on_key: Callback<KeyEventInfo>,
    pub on_context_menu: Callback<(String, Column, ScreenPoint)>,
    pub on_focus: Callback<String>,
}

pub(super) struct TableView {
    input: Input,
    widths: ColumnWidths,
    row_widths: ColumnWidths,
    drag: Option<Drag>,
    resize_timer: Option<ComponentTimer>,
    focus_timer: Option<ComponentTimer>,
    list_ref: ElementRef<ListView>,
    row_refs: BTreeMap<String, ElementRef<Border>>,
    checkbox_focus: Rc<Cell<bool>>,
    restore_focus: bool,
}

struct Drag {
    column: Column,
    pointer_id: u32,
    origin: f64,
    width: f64,
}

pub(super) enum Message {
    Sort(Column),
    ResizeStart(Column, PointerEventInfo),
    ResizeMove(PointerEventInfo),
    ResizeEnd(PointerEventInfo),
    ResizeLost,
    FinishResize,
    Pointer((String, Column, PointerEventInfo)),
    Context((String, Column, PointerEventInfo)),
    ContextPoint(String, Column, Result<ScreenPoint, super::Error>),
    Scrolled(u64, String, bool, Result<bool, IntegrationError>),
    FocusReady(u64, String),
}

impl TableView {
    fn content_width(&self, widths: ColumnWidths) -> f64 {
        (self.input.viewport_width - 2.0).max(widths.total())
    }

    fn commit_resize(&mut self) {
        self.drag = None;
        self.resize_timer = None;
        self.row_widths = self.widths;
    }

    fn rows(&self, cx: &ViewContext<Self>) -> ListView {
        let input = &self.input;
        ListView::new()
            .grid_row(1)
            .automation_id("apps")
            .element_ref(&self.list_ref)
            .is_enabled(input.enabled)
            .selection_mode(ListViewSelectionMode::None)
            .item_container(ItemContainerOptions {
                padding: Some(Thickness::uniform(0.0)),
                min_height: Some(40.0),
                horizontal_content_alignment: Some(HorizontalAlignment::Stretch),
                suppress_selection_visuals: true,
            })
            .items(input.rows.iter().map(|item| {
                DataItem::new(item.key.clone(), &item.cells[0]).content(component::<row::TableRow>(
                    item.key.clone(),
                    row::Input {
                        row: item.clone(),
                        widths: self.row_widths,
                        width: self.content_width(self.row_widths),
                        selected: input.selected.contains(&item.key),
                        focused: input.focused.as_ref() == Some(&item.key),
                        keyboard_focus: input.keyboard_focus,
                        focus_ref: self.row_refs[&item.key].clone(),
                        checkbox_focus: self.checkbox_focus.clone(),
                        on_toggle: input.on_toggle.clone(),
                        on_pointer: cx.callback(Message::Pointer),
                        on_context: cx.callback(Message::Context),
                        on_focus: input.on_focus.clone(),
                    },
                ))
            }))
    }

    fn headers(&self, cx: &ViewContext<Self>) -> Border {
        let input = &self.input;
        let mut cells: Vec<View> = vec![
            Border::new()
                .border_brush(ThemeBrush::DividerStroke)
                .border_thickness(Thickness::new(0.0, 0.0, 1.0, 0.0))
                .into(),
        ];
        for (index, spec) in input.columns.iter().enumerate() {
            let column = COLUMNS[index];
            // WinUI.TableView's Segoe Fluent sort glyphs: F0AD ascending, F0AE descending.
            // Unsorted columns keep an empty glyph so the child tuple stays fixed.
            let glyph = match input.sort.column {
                Some(sorted) if sorted == column && input.sort.ascending => "\u{F0AD}",
                Some(sorted) if sorted == column => "\u{F0AE}",
                _ => "",
            };
            let label = &spec.header;
            cells.push(
                Border::new()
                    .grid_column(index as i32 + 1)
                    .border_brush(ThemeBrush::DividerStroke)
                    .border_thickness(Thickness::new(0.0, 0.0, 1.0, 0.0))
                    .content(
                        Button::new()
                            .height(31.0)
                            .horizontal_alignment(HorizontalAlignment::Stretch)
                            .horizontal_content_alignment(HorizontalAlignment::Stretch)
                            .min_width(0.0)
                            .style(ButtonStyle::Subtle)
                            .is_enabled(input.enabled && spec.sortable)
                            .automation_name(label)
                            .content(
                                Grid::new()
                                    .columns([GridLength::STAR, GridLength::Auto])
                                    .children((
                                        TextBlock::new()
                                            .text(label)
                                            .font_weight(FontWeight::SEMI_BOLD)
                                            .max_lines(1)
                                            .text_trimming(TextTrimming::CharacterEllipsis),
                                        FontIcon::new()
                                            .grid_column(1)
                                            .margin(Thickness::new(8.0, 0.0, 0.0, 0.0))
                                            .vertical_alignment(VerticalAlignment::Center)
                                            .glyph(glyph)
                                            .font_size(SORT_GLYPH_SIZE)
                                            .foreground(ThemeBrush::TextSecondary),
                                    )),
                            )
                            .on_click(cx.callback(move |()| Message::Sort(column))),
                    )
                    .into(),
            );
        }
        // Sibling grips sit above both adjacent buttons. Capture never invokes sort.
        for (index, spec) in input.columns.iter().enumerate() {
            if !spec.resizable {
                continue;
            }
            let column = COLUMNS[index];
            cells.push(
                Border::new()
                    .grid_column(index as i32 + 1)
                    .automation_id(format!("resize_{index}"))
                    .width(8.0)
                    .margin(Thickness::new(
                        0.0,
                        0.0,
                        if index == 6 { 0.0 } else { -4.0 },
                        0.0,
                    ))
                    .horizontal_alignment(HorizontalAlignment::Right)
                    .background(Color::transparent())
                    .cursor(CursorShape::SizeWestEast)
                    .capture_pointer_on_press(input.enabled)
                    .on_pointer_pressed(
                        cx.callback(move |event| Message::ResizeStart(column, event)),
                    )
                    .on_pointer_moved(cx.callback(Message::ResizeMove))
                    .on_pointer_released(cx.callback(Message::ResizeEnd))
                    .on_pointer_capture_lost(cx.callback(|()| Message::ResizeLost))
                    .on_pointer_canceled(cx.callback(|()| Message::ResizeLost))
                    .into(),
            );
        }
        Border::new()
            .height(32.0)
            // Border does not clip children: round the header into the card's top corners
            // (card radius 8 minus its 1 DIP stroke).
            .corner_radius(CornerRadius::new(7.0, 7.0, 0.0, 0.0))
            .background(ThemeBrush::SubtleFillSecondary)
            .border_brush(ThemeBrush::DividerStroke)
            .border_thickness(Thickness::new(0.0, 0.0, 0.0, 1.0))
            .content(
                Grid::new()
                    .height(31.0)
                    .automation_id("header_columns")
                    .columns(
                        self.widths
                            .resolved(self.content_width(self.widths))
                            .map(GridLength::Pixel),
                    )
                    .children(cells),
            )
    }
}

impl Component for TableView {
    type Input = Input;
    type Message = Message;

    fn create(input: &Input, _: &ComponentContext<Self>) -> Self {
        let widths = ColumnWidths::new(
            input.columns.each_ref().map(|c| c.initial_width),
            input.columns.each_ref().map(|c| c.min_width),
        );
        Self {
            input: input.clone(),
            widths,
            row_widths: widths,
            drag: None,
            resize_timer: None,
            focus_timer: None,
            list_ref: ElementRef::new(),
            row_refs: input
                .rows
                .iter()
                .map(|row| (row.key.clone(), ElementRef::new()))
                .collect(),
            checkbox_focus: Rc::new(Cell::new(false)),
            restore_focus: true,
        }
    }

    fn input_changed(&mut self, input: &Input, _: &ComponentContext<Self>) {
        if input.columns.each_ref().map(|c| c.min_width)
            != self.input.columns.each_ref().map(|c| c.min_width)
        {
            self.widths = ColumnWidths::new(
                COLUMNS.map(|column| self.widths.get(column)),
                input.columns.each_ref().map(|c| c.min_width),
            );
            self.commit_resize();
        }
        self.restore_focus = input.generation != self.input.generation;
        if input.generation != self.input.generation {
            self.checkbox_focus.set(false);
        }
        if input.generation != self.input.generation || input.focused != self.input.focused {
            self.focus_timer = None;
        }
        self.row_refs
            .retain(|key, _| input.rows.iter().any(|row| &row.key == key));
        for row in input.rows.iter() {
            self.row_refs.entry(row.key.clone()).or_default();
        }
        if !input.enabled {
            self.commit_resize();
        }
        self.input = input.clone();
    }

    fn update(&mut self, message: Message, cx: &ComponentContext<Self>) {
        match message {
            Message::Sort(column)
                if self.drag.is_none()
                    && self.input.enabled
                    && self.input.restore_focus
                    && self.input.columns[column as usize].sortable =>
            {
                self.input.on_sort.call(column)
            }
            Message::ResizeStart(column, event)
                if self.input.enabled
                    && self.input.columns[column as usize].resizable
                    && event.is_left_button_pressed
                    && event.capture_succeeded == Some(true) =>
            {
                self.resize_timer = None;
                self.drag = Some(Drag {
                    column,
                    pointer_id: event.pointer_id,
                    origin: event.window_x,
                    width: self.widths.resolved(self.content_width(self.widths))
                        [column as usize + 1],
                });
            }
            Message::ResizeMove(event) | Message::ResizeEnd(event) => {
                if let Some(drag) = &self.drag
                    && drag.pointer_id == event.pointer_id
                {
                    self.widths
                        .drag(drag.column, drag.width, event.window_x - drag.origin);
                    if !event.is_left_button_pressed {
                        self.commit_resize();
                    }
                }
            }
            Message::ResizeLost if self.drag.is_some() => {
                // WinUI can deliver capture-lost before the final release coordinate.
                self.resize_timer =
                    Some(cx.set_local_timeout(Duration::from_millis(1), || Message::FinishResize));
            }
            Message::FinishResize => self.commit_resize(),
            Message::Pointer((key, column, event)) if self.input.enabled && self.drag.is_none() => {
                self.input.on_pointer.call((key, column, event));
            }
            Message::Context((key, column, event)) if self.input.enabled && self.drag.is_none() => {
                let _ = cx.run_window(move |window| {
                    Message::ContextPoint(
                        key,
                        column,
                        super::native::screen_point(window.as_raw() as usize, event),
                    )
                });
            }
            Message::ContextPoint(key, column, Ok(point)) if self.input.enabled => {
                self.input.on_context_menu.call((key, column, point))
            }
            Message::ContextPoint(_, _, Err(error)) => crate::debug(&error.to_string()),
            Message::Scrolled(generation, key, true, Ok(true))
                if generation == self.input.generation
                    && self.input.focused.as_ref() == Some(&key) =>
            {
                self.focus_timer =
                    Some(cx.set_local_timeout(Duration::from_millis(16), move || {
                        Message::FocusReady(generation, key)
                    }));
            }
            Message::FocusReady(generation, key)
                if generation == self.input.generation
                    && self.input.focused.as_ref() == Some(&key)
                    && self.input.enabled
                    && !self.checkbox_focus.get() =>
            {
                self.focus_timer = None;
                self.checkbox_focus.set(false);
                if let Some(reference) = self.row_refs.get(&key) {
                    let _ = reference.request_focus();
                }
            }
            Message::Scrolled(_, _, _, Err(error)) => {
                crate::debug(&format!("Table scroll request failed: {error:?}"))
            }
            _ => {}
        }
    }

    fn view(&self, input: &Input, cx: &mut ViewContext<Self>) -> View {
        let reference = self.list_ref.clone();
        let sender = cx.sender();
        let focused = input.focused.clone();
        let generation = input.generation;
        let index = focused
            .as_ref()
            .and_then(|key| input.rows.iter().position(|row| &row.key == key));
        let enabled = input.enabled && !self.checkbox_focus.get();
        let restore_focus = input.restore_focus && (self.restore_focus || input.keyboard_focus);
        cx.use_effect(
            "reveal",
            (generation, focused.clone(), enabled),
            move || {
                if enabled && let (Some(index), Some(key)) = (index, focused) {
                    let _ = reference.request_scroll_into_view_result(
                        index,
                        ScrollIntoViewAlignment::Default,
                        move |result| {
                            let _ = sender.send(Message::Scrolled(
                                generation,
                                key.clone(),
                                restore_focus,
                                result,
                            ));
                        },
                    );
                }
                None
            },
        );
        let on_key = input.on_key.clone();
        let check_focus = self.checkbox_focus.clone();
        let enabled = input.enabled;
        let mut content: Vec<View> = vec![
            ScrollViewer::new()
                .horizontal_scroll_bar_visibility(ScrollBarVisibility::Auto)
                .vertical_scroll_bar_visibility(ScrollBarVisibility::Disabled)
                .content(
                    Grid::new()
                        .width(self.content_width(self.widths))
                        .rows([GridLength::Auto, GridLength::STAR])
                        .keyed_children([
                            keyed("headers", self.headers(cx)),
                            keyed(input.generation, self.rows(cx)),
                        ]),
                )
                .into(),
        ];
        if let Some(text) = &input.empty_text {
            content.push(
                TextBlock::new()
                    .automation_id("search_empty")
                    .text(text)
                    .foreground(ThemeBrush::TextSecondary)
                    .text_wrapping(TextWrapping::Wrap)
                    .horizontal_alignment(HorizontalAlignment::Center)
                    .vertical_alignment(VerticalAlignment::Center)
                    .margin(Thickness::new(24.0, 32.0, 24.0, 0.0))
                    .into(),
            );
        }
        Border::new()
            .automation_id("table")
            .element_ref(&input.focus_ref)
            .is_tab_stop(true)
            .background(ThemeBrush::CardBackground)
            .border_brush(ThemeBrush::CardStroke)
            .border_thickness(Thickness::uniform(1.0))
            .corner_radius(CornerRadius::uniform(8.0))
            // No top inset: the header meets the card edge; the bottom inset keeps the last
            // row's fill off the rounded corners.
            .padding(Thickness::new(0.0, 0.0, 0.0, 4.0))
            .on_preview_key_down(RoutedCallback::new(move |event: KeyEventInfo| {
                let control = event.modifiers.contains(InputModifiers::CONTROL);
                let handled = enabled
                    && !event.modifiers.contains(InputModifiers::ALT)
                    && ((control && (event.key == VirtualKey::A || event.key == VirtualKey::C))
                        || (!control
                            && (event.key == VirtualKey::UP || event.key == VirtualKey::DOWN))
                        || (!control && event.key == VirtualKey::SPACE && !check_focus.get()));
                if handled {
                    if event.key == VirtualKey::UP || event.key == VirtualKey::DOWN {
                        check_focus.set(false);
                    }
                    on_key.call(event);
                }
                handled
            }))
            .content(Grid::new().children(content))
            .into()
    }
}
