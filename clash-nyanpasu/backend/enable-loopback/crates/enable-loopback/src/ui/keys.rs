//! Routed WinUI key handlers; top-level HWND keyboard messages do not reach XAML.
use super::Message;
use windows_reactor::*;

pub(super) fn global(sender: LocalSender<Message>) -> RoutedCallback<KeyEventInfo> {
    RoutedCallback::new(move |event: KeyEventInfo| {
        global_action(event.key, event.modifiers).is_some_and(|action| sender.send(action))
    })
}

fn global_action(key: VirtualKey, modifiers: InputModifiers) -> Option<Message> {
    use windows::Win32::winuser::{VK_F1, VK_F5};
    if modifiers == InputModifiers::NONE {
        if key == VirtualKey(VK_F1 as u32) {
            return Some(Message::Help);
        }
        if key == VirtualKey(VK_F5 as u32) {
            return Some(Message::Refresh);
        }
    } else if modifiers == InputModifiers::CONTROL {
        return match key {
            VirtualKey::S => Some(Message::Save),
            VirtualKey::F => Some(Message::FocusSearch),
            _ => None,
        };
    } else if modifiers == InputModifiers::ALT {
        return match key {
            VirtualKey::L => Some(Message::Help),
            VirtualKey::R => Some(Message::Refresh),
            VirtualKey::A => Some(Message::SetAll(true)),
            VirtualKey::N => Some(Message::SetAll(false)),
            VirtualKey::S => Some(Message::Save),
            _ => None,
        };
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::winuser::{VK_F1, VK_F5};
    #[test]
    fn shortcuts_use_exact_modifiers_and_leave_list_keys_to_the_list() {
        assert!(matches!(
            global_action(VirtualKey::F, InputModifiers::CONTROL),
            Some(Message::FocusSearch)
        ));
        assert!(matches!(
            global_action(VirtualKey::S, InputModifiers::CONTROL),
            Some(Message::Save)
        ));
        assert!(matches!(
            global_action(VirtualKey(VK_F5 as u32), InputModifiers::NONE),
            Some(Message::Refresh)
        ));
        assert!(matches!(
            global_action(VirtualKey(VK_F1 as u32), InputModifiers::NONE),
            Some(Message::Help)
        ));
        assert!(matches!(
            global_action(VirtualKey::L, InputModifiers::ALT),
            Some(Message::Help)
        ));
        for (key, checked) in [(VirtualKey::A, true), (VirtualKey::N, false)] {
            assert!(
                matches!(global_action(key,InputModifiers::ALT), Some(Message::SetAll(value)) if value == checked)
            );
        }
        assert!(global_action(VirtualKey::A, InputModifiers::CONTROL).is_none());
        assert!(global_action(VirtualKey::C, InputModifiers::CONTROL).is_none());
        let mut altgr = InputModifiers::CONTROL;
        altgr |= InputModifiers::ALT;
        assert!(global_action(VirtualKey::S, altgr).is_none());
        assert!(global_action(VirtualKey::S, InputModifiers::NONE).is_none());
    }
}
