//! Dialog presentation; sequencing and decisions belong to the window component.
use super::{MainWindow, Message};
use windows_reactor::*;

pub(super) enum Dialog {
    Refresh,
    Close,
    Fallback { code: u32, force: bool },
    Orphan { sid: String },
    Error { title: String, body: String },
}

pub(super) fn view(
    dialog: Option<&Dialog>,
    open: bool,
    keep_prompting: bool,
    cx: &ViewContext<MainWindow>,
) -> ContentDialog {
    let base = ContentDialog::new()
        .is_open(open)
        .on_closed(cx.callback(Message::DialogClosed));
    match dialog {
        Some(Dialog::Close) => base
            .title(rust_i18n::t!("dialog.close_title"))
            .content(text(&rust_i18n::t!("dialog.close_body")))
            .primary_button_text(rust_i18n::t!("dialog.exit"))
            .close_button_text(rust_i18n::t!("dialog.cancel"))
            .default_button(ContentDialogButton::Close),
        Some(Dialog::Refresh) => base
            .title(rust_i18n::t!("dialog.refresh_title"))
            .content(text(&rust_i18n::t!("dialog.refresh_body")))
            .primary_button_text(rust_i18n::t!("toolbar.refresh"))
            .close_button_text(rust_i18n::t!("dialog.cancel"))
            .default_button(ContentDialogButton::Close),
        Some(Dialog::Orphan { sid }) => base
            .title(rust_i18n::t!("dialog.orphan_title"))
            .content(
                StackPanel::new().spacing(12.0).children((
                    text(&rust_i18n::t!("dialog.orphan_body")),
                    TextBlock::new()
                        .text(sid)
                        .text_wrapping(TextWrapping::Wrap)
                        .is_text_selection_enabled(true),
                    CheckBox::new()
                        .is_checked(keep_prompting)
                        .content(text(&rust_i18n::t!("dialog.keep_prompting")))
                        .on_is_checked_changed(
                            cx.callback(|v| Message::KeepPrompting(v == Some(true))),
                        ),
                )),
            )
            .close_button_text(rust_i18n::t!("dialog.ok"))
            .default_button(ContentDialogButton::Close),
        Some(Dialog::Fallback { code, .. }) => base
            .title(rust_i18n::t!("dialog.warning"))
            .content(text(&rust_i18n::t!(
                "dialog.fallback",
                code = format!("0x{code:x}")
            )))
            .close_button_text(rust_i18n::t!("dialog.ok"))
            .default_button(ContentDialogButton::Close),
        Some(Dialog::Error { title, body }) => base
            .title(title)
            .content(text(body).is_text_selection_enabled(true))
            .close_button_text(rust_i18n::t!("dialog.ok"))
            .default_button(ContentDialogButton::Close),
        None => base,
    }
}

fn text(value: &str) -> TextBlock {
    TextBlock::new()
        .text(value)
        .text_wrapping(TextWrapping::Wrap)
}
