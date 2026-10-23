use super::*;
use crate::model::CHECKBOX_WIDTH;

#[derive(Clone, PartialEq)]
pub(super) struct Input {
    pub row: Rc<Row>,
    pub widths: ColumnWidths,
    pub width: f64,
    pub selected: bool,
    pub focused: bool,
    pub keyboard_focus: bool,
    pub focus_ref: ElementRef<Border>,
    pub checkbox_focus: Rc<Cell<bool>>,
    pub on_toggle: Callback<(String, bool)>,
    pub on_pointer: Callback<(String, Column, PointerEventInfo)>,
    /// Raised when a right-button press on this row is released, like WM_CONTEXTMENU.
    pub on_context: Callback<(String, Column, PointerEventInfo)>,
    pub on_focus: Callback<String>,
}

pub(super) struct TableRow {
    input: Input,
    hovered: bool,
    keyboard_focused: bool,
    has_focus: bool,
    /// A right-button press on this row awaits its release.
    right_pressed: bool,
}

pub(super) enum Message {
    Hover(bool),
    Released(PointerEventInfo),
    Focus(FocusEventInfo),
    Blur,
    Pointer(PointerEventInfo),
    Toggle(Option<bool>),
}

impl Component for TableRow {
    type Input = Input;
    type Message = Message;

    fn create(input: &Input, _: &ComponentContext<Self>) -> Self {
        Self {
            input: input.clone(),
            hovered: false,
            keyboard_focused: false,
            has_focus: false,
            right_pressed: false,
        }
    }

    fn input_changed(&mut self, input: &Input, _: &ComponentContext<Self>) {
        self.input = input.clone();
    }

    fn update(&mut self, message: Message, _: &ComponentContext<Self>) {
        let input = &self.input;
        match message {
            Message::Hover(value) => {
                self.hovered = value;
                // A release outside this row must not open its menu later.
                self.right_pressed &= value;
            }
            Message::Focus(event) => {
                self.has_focus = true;
                if event.is_direct {
                    input.checkbox_focus.set(false);
                }
                self.keyboard_focused = event.state == ElementFocusState::Keyboard;
                input.on_focus.call(input.row.key.clone());
            }
            Message::Blur => {
                self.keyboard_focused = false;
                self.has_focus = false;
            }
            Message::Pointer(event) => {
                self.right_pressed = event.is_right_button_pressed;
                self.keyboard_focused = false;
                input.checkbox_focus.set(false);
                input.on_pointer.call((
                    input.row.key.clone(),
                    input.widths.column_at(event.x),
                    event,
                ));
            }
            // Opening on press lets the matching release light-dismiss the new flyout.
            Message::Released(event) if self.right_pressed && !event.is_right_button_pressed => {
                self.right_pressed = false;
                input.on_context.call((
                    input.row.key.clone(),
                    input.widths.column_at(event.x),
                    event,
                ));
            }
            Message::Released(_) => {}
            Message::Toggle(checked) => input
                .on_toggle
                .call((input.row.key.clone(), checked == Some(true))),
        }
    }

    fn view(&self, input: &Input, cx: &mut ViewContext<Self>) -> View {
        let check_focus = input.checkbox_focus.clone();
        let mut cells: Vec<View> = vec![
            Border::new()
                .on_got_focus(move |_| check_focus.set(true))
                .content(
                    CheckBox::new()
                        .automation_id(format!("toggle_{}", input.row.key))
                        .width(CHECKBOX_WIDTH / 2.0)
                        .min_width(0.0)
                        .horizontal_alignment(HorizontalAlignment::Center)
                        .is_checked(input.row.checked)
                        .automation_name(&input.row.checkbox_label)
                        .on_is_checked_changed(cx.callback(Message::Toggle)),
                )
                .into(),
        ];
        for (index, text) in input.row.cells.iter().enumerate() {
            let label = TextBlock::new()
                .text(text)
                .foreground(ThemeBrush::PrimaryText)
                .max_lines(1)
                .text_trimming(TextTrimming::CharacterEllipsis)
                .vertical_alignment(VerticalAlignment::Center);
            let content: View = if index == Column::Users as usize && input.row.highlight {
                Grid::new()
                    .columns([GridLength::Auto, GridLength::STAR])
                    .column_spacing(6.0)
                    .children((
                        TextBlock::new()
                            .automation_id(format!("current_user_{}", input.row.key))
                            .text("●")
                            .font_size(10.0)
                            .foreground(ThemeBrush::TextSecondary)
                            .vertical_alignment(VerticalAlignment::Center),
                        label.grid_column(1),
                    ))
                    .into()
            } else {
                label.into()
            };
            let cell = Border::new()
                .grid_column(index as i32 + 1)
                .padding(Thickness::xy(12.0, 0.0))
                .content(content);
            // Only the hovered row carries tooltips, so rows do not each hold seven.
            cells.push(if self.hovered {
                cell.tooltip(text)
            } else {
                cell.into()
            });
        }
        let fill: Brush = if input.selected {
            ThemeBrush::SubtleFillTertiary.into()
        } else if self.hovered {
            ThemeBrush::SubtleFillSecondary.into()
        } else {
            Color::transparent().into()
        };
        let focus =
            input.focused && self.has_focus && (input.keyboard_focus || self.keyboard_focused);
        Border::new()
            .element_ref(&input.focus_ref)
            .is_tab_stop(true)
            .focus_on_pointer_release(true)
            .automation_id(format!("row_{}", input.row.key))
            // The list names its item after the row's view.
            .automation_name(&input.row.cells[0])
            .width(input.width)
            .height(super::ROW_HEIGHT)
            .background(Color::transparent())
            .border_brush(ThemeBrush::DividerStroke)
            .border_thickness(Thickness::new(0.0, 0.0, 0.0, 1.0))
            .on_pointer_entered(cx.callback(|_| Message::Hover(true)))
            .on_pointer_exited(cx.callback(|_| Message::Hover(false)))
            .on_pointer_pressed(cx.callback(Message::Pointer))
            .on_pointer_released(cx.callback(Message::Released))
            .on_got_focus(cx.callback(Message::Focus))
            .on_lost_focus(cx.callback(|_| Message::Blur))
            .content(
                Grid::new().children((
                    Border::new()
                        .automation_id(format!("chrome_{}", input.row.key))
                        .background(fill)
                        .border_brush(if focus {
                            ThemeBrush::PrimaryText.into()
                        } else {
                            Brush::from(Color::transparent())
                        })
                        .border_thickness(Thickness::uniform(if focus { 2.0 } else { 0.0 })),
                    Border::new()
                        .width(3.0)
                        .height(16.0)
                        .corner_radius(CornerRadius::uniform(1.5))
                        .horizontal_alignment(HorizontalAlignment::Left)
                        .vertical_alignment(VerticalAlignment::Center)
                        .background(if input.selected {
                            ThemeBrush::Accent.into()
                        } else {
                            Brush::from(Color::transparent())
                        }),
                    Grid::new()
                        .automation_id(format!("columns_{}", input.row.key))
                        .columns(input.widths.resolved(input.width).map(GridLength::Pixel))
                        .children(cells),
                )),
            )
            .into()
    }
}
