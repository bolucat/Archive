use super::*;

#[test]
#[ignore = "initializes WinUI; run alone on a Windows desktop (no injected input)"]
fn list_scroll_realizes_offscreen_items_with_custom_containers_and_no_selection() {
    App::run_with(|app| {
        let reference = ElementRef::<ListView>::new();
        let row_ref = ElementRef::<Border>::new();
        let options = ItemContainerOptions {
            padding: Some(Thickness::uniform(0.0)),
            min_height: Some(40.0),
            horizontal_content_alignment: Some(HorizontalAlignment::Stretch),
            suppress_selection_visuals: true,
        };
        let mut runtime = Runtime::new(WinUiAdapter::default());
        runtime
            .update(
                ListView::new()
                    .height(160.0)
                    .element_ref(&reference)
                    .selection_mode(ListViewSelectionMode::None)
                    .item_container(options)
                    .items((0usize..100).map(|i| {
                        let mut row = Border::new()
                            .height(40.0)
                            .is_tab_stop(true)
                            .content(i.to_string());
                        if i == 80 {
                            row = row.element_ref(&row_ref);
                        }
                        DataItem::new(i, i.to_string()).content(row)
                    })),
            )
            .unwrap();
        let root = runtime.graph().root().unwrap();
        let window = runtime
            .adapter_mut()
            .create_window_with_policy(root, &WindowPolicy::new().client_size(300.0, 200.0))?;
        window.activate()?;
        let runtime = RefCell::new(runtime);
        let app = app.clone();
        let step = Cell::new(0);
        let timer = native::DispatcherQueue::GetForCurrentThread()?.CreateTimer()?;
        timer.SetInterval(TimeSpan::try_from(Duration::from_millis(50)).unwrap())?;
        let tick = timer.Tick(move |sender, _| {
            let mut runtime = runtime.borrow_mut();
            if step.replace(1) == 0 {
                assert!(reference.request_scroll_into_view_result(
                    80,
                    ScrollIntoViewAlignment::Leading,
                    |result| assert_eq!(result, Ok(true))
                ));
                runtime.dispatch_native_events().unwrap();
                return;
            }
            sender.as_ref().unwrap().Stop().unwrap();
            let element = runtime.adapter().ui_element(root).unwrap();
            element.UpdateLayout().unwrap();
            let container = element
                .cast::<native::IItemContainerMapping>()
                .unwrap()
                .ContainerFromIndex(80)
                .unwrap();
            assert_eq!(
                container
                    .cast::<native::IControl>()
                    .unwrap()
                    .Padding()
                    .unwrap(),
                native::Thickness::default()
            );
            assert_eq!(
                container
                    .cast::<native::IFrameworkElement>()
                    .unwrap()
                    .MinHeight()
                    .unwrap(),
                40.0
            );
            assert!(row_ref.request_focus_result(|result| assert_eq!(result, Ok(true))));
            runtime.dispatch_native_events().unwrap();
            window.close().unwrap();
            app.exit().unwrap();
        })?;
        timer.Start()?;
        Ok((timer, tick))
    })
    .unwrap();
}

#[test]
#[ignore = "initializes WinUI; run alone on a Windows desktop (no injected input)"]
fn list_horizontal_scroll_reports_offsets_and_clears() {
    App::run_with(|app| {
        let offsets = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&offsets);
        let view = move |scroll: bool| {
            let offsets = Rc::clone(&recorded);
            ListView::new()
                .height(160.0)
                .horizontal_scroll(scroll)
                .on_horizontal_offset_changed(move |offset| offsets.borrow_mut().push(offset))
                .items((0usize..30).map(|i| {
                    DataItem::new(i, i.to_string()).content(
                        TextBlock::new()
                            .text(i.to_string())
                            .width(600.0)
                            .height(40.0),
                    )
                }))
        };
        let mut runtime = Runtime::new(WinUiAdapter::default());
        runtime.update(view(true)).unwrap();
        let root = runtime.graph().root().unwrap();
        let window = runtime
            .adapter_mut()
            .create_window_with_policy(root, &WindowPolicy::new().client_size(300.0, 200.0))?;
        window.activate()?;
        let element = runtime.adapter().ui_element(root)?;
        let runtime = RefCell::new(runtime);
        let app = app.clone();
        let step = Cell::new(0);
        let timer = native::DispatcherQueue::GetForCurrentThread()?.CreateTimer()?;
        timer.SetInterval(TimeSpan::try_from(Duration::from_millis(50)).unwrap())?;
        let tick = timer.Tick(move |sender, _| {
            // The template applies on a layout pass after the window shows.
            let Some(viewer) = template_scroll_viewer(&element.cast().unwrap()).unwrap() else {
                return;
            };
            let mut runtime = runtime.borrow_mut();
            if step.replace(1) == 0 {
                assert!(
                    viewer
                        .ChangeViewWithOptionalAnimation(Some(100.0), None, None, true)
                        .unwrap()
                );
                return;
            }
            sender.as_ref().unwrap().Stop().unwrap();
            runtime.dispatch_native_events().unwrap();
            assert_eq!(*offsets.borrow(), [100.0]);
            runtime.update(view(false)).unwrap();
            element.UpdateLayout().unwrap();
            // Disabled horizontal scrolling constrains content to the viewport.
            assert_eq!(viewer.HorizontalOffset().unwrap(), 0.0);
            window.close().unwrap();
            app.exit().unwrap();
        })?;
        timer.Start()?;
        Ok((timer, tick))
    })
    .unwrap();
}

#[test]
#[ignore = "initializes WinUI; run alone on a Windows desktop (no injected input)"]
fn list_scroll_to_before_load_applies_both_axes_and_reports_offsets() {
    App::run_with(|app| {
        let reference = ElementRef::<ListView>::new();
        let vertical = Rc::new(RefCell::new(Vec::new()));
        let recorded = Rc::clone(&vertical);
        let mut runtime = Runtime::new(WinUiAdapter::default());
        runtime
            .update(
                ListView::new()
                    .height(160.0)
                    .element_ref(&reference)
                    .horizontal_scroll(true)
                    .on_horizontal_offset_changed(|_| {})
                    .on_vertical_offset_changed(move |offset| recorded.borrow_mut().push(offset))
                    .items((0usize..100).map(|i| {
                        DataItem::new(i, i.to_string()).content(
                            TextBlock::new()
                                .text(i.to_string())
                                .width(600.0)
                                .height(40.0),
                        )
                    })),
            )
            .unwrap();
        // Queued before the list has a template or an extent.
        assert!(reference.request_scroll_to(Some(100.0), Some(800.0)));
        runtime.dispatch_native_events().unwrap();
        let root = runtime.graph().root().unwrap();
        let window = runtime
            .adapter_mut()
            .create_window_with_policy(root, &WindowPolicy::new().client_size(300.0, 200.0))?;
        window.activate()?;
        let element = runtime.adapter().ui_element(root)?;
        let runtime = RefCell::new(runtime);
        let app = app.clone();
        let step = Cell::new(0);
        let timer = native::DispatcherQueue::GetForCurrentThread()?.CreateTimer()?;
        timer.SetInterval(TimeSpan::try_from(Duration::from_millis(50)).unwrap())?;
        let tick = timer.Tick(move |sender, _| {
            // The template applies on a layout pass after the window shows.
            let Some(viewer) = template_scroll_viewer(&element.cast().unwrap()).unwrap() else {
                return;
            };
            let mut runtime = runtime.borrow_mut();
            if step.replace(step.get() + 1) == 0 {
                return;
            }
            sender.as_ref().unwrap().Stop().unwrap();
            assert_eq!(viewer.HorizontalOffset().unwrap(), 100.0);
            assert_eq!(viewer.VerticalOffset().unwrap(), 800.0);
            runtime.dispatch_native_events().unwrap();
            assert_eq!(*vertical.borrow(), [800.0]);
            window.close().unwrap();
            app.exit().unwrap();
        })?;
        timer.Start()?;
        Ok((timer, tick))
    })
    .unwrap();
}

#[test]
#[ignore = "initializes WinUI; run alone on a Windows desktop (no injected input)"]
fn list_virtual_source_realizes_only_prepared_containers() {
    App::run_with(|app| {
        let reference = ElementRef::<ListView>::new();
        let created = Rc::new(Cell::new(0usize));
        let view = {
            let created = Rc::clone(&created);
            let reference = reference.clone();
            move |revision: u64, len: usize| {
                let created = Rc::clone(&created);
                ListView::new()
                    .height(160.0)
                    .element_ref(&reference)
                    .item_container(ItemContainerOptions {
                        padding: Some(Thickness::uniform(0.0)),
                        min_height: Some(40.0),
                        horizontal_content_alignment: Some(HorizontalAlignment::Stretch),
                        suppress_selection_visuals: true,
                    })
                    .virtual_source(VirtualSource::new(
                        revision,
                        len,
                        move |index| format!("{revision}:{index}"),
                        move |index| {
                            created.set(created.get() + 1);
                            TextBlock::new()
                                .text(format!("{revision}:{index}"))
                                .automation_name(format!("item {revision}:{index}"))
                                .height(40.0)
                        },
                    ))
            }
        };
        let mut runtime = Runtime::new(WinUiAdapter::default());
        runtime.update(view(0, 1000)).unwrap();
        let root = runtime.graph().root().unwrap();
        let window = runtime
            .adapter_mut()
            .create_window_with_policy(root, &WindowPolicy::new().client_size(300.0, 200.0))?;
        window.activate()?;
        let element = runtime.adapter().ui_element(root)?;
        let name_at = {
            let element = element.clone();
            move |index: i32| {
                native::AutomationProperties::GetName(
                    &element
                        .cast::<native::IItemContainerMapping>()
                        .unwrap()
                        .ContainerFromIndex(index)
                        .unwrap(),
                )
                .unwrap()
            }
        };
        // The text of the view in the container the list prepared for an index.
        let text_at = move |runtime: &Runtime<WinUiAdapter>, index: i32| -> Option<String> {
            element.UpdateLayout().unwrap();
            let mut pending = VecDeque::from([element
                .cast::<native::IItemContainerMapping>()
                .unwrap()
                .ContainerFromIndex(index)
                .ok()?]);
            while let Some(parent) = pending.pop_front() {
                if parent.cast::<native::TextBlock>().is_ok() {
                    let text = parent.cast::<native::UIElement>().unwrap();
                    let object = runtime.graph().objects().find(|object| {
                        runtime.adapter().ui_element(*object).ok().as_ref() == Some(&text)
                    })?;
                    return runtime
                        .graph()
                        .properties(object)?
                        .iter()
                        .find_map(|property| match &property.value {
                            PropertyValue::String(text) if property.id == PropertyId::Text => {
                                Some(text.to_string())
                            }
                            _ => None,
                        });
                }
                for child in 0..native::VisualTreeHelper::GetChildrenCount(&parent).unwrap() {
                    pending.push_back(native::VisualTreeHelper::GetChild(&parent, child).unwrap());
                }
            }
            None
        };
        let runtime = RefCell::new(runtime);
        let app = app.clone();
        let step = Cell::new(0);
        let timer = native::DispatcherQueue::GetForCurrentThread()?.CreateTimer()?;
        timer.SetInterval(TimeSpan::try_from(Duration::from_millis(50)).unwrap())?;
        let tick = timer.Tick(move |sender, _| {
            let mut runtime = runtime.borrow_mut();
            runtime.dispatch_native_events().unwrap();
            match step.replace(step.get() + 1) {
                0 => {}
                1 => {
                    assert_eq!(text_at(&runtime, 0).as_deref(), Some("0:0"));
                    // Items are named by their views, not their indices.
                    assert_eq!(name_at(0), "item 0:0");
                    // Only containers the list prepared hold views, not all 1000 items.
                    assert!(created.get() < 100, "{} views", created.get());
                    assert!(reference.request_scroll_into_view_result(
                        900,
                        ScrollIntoViewAlignment::Leading,
                        |result| assert_eq!(result, Ok(true))
                    ));
                    runtime.dispatch_native_events().unwrap();
                }
                2 => {}
                3 => {
                    assert_eq!(text_at(&runtime, 900).as_deref(), Some("0:900"));
                    assert!(created.get() < 200, "{} views", created.get());
                    // New keys reset the source; prepared containers show the new items.
                    runtime.update(view(1, 3)).unwrap();
                }
                4 => {}
                _ => {
                    sender.as_ref().unwrap().Stop().unwrap();
                    assert_eq!(text_at(&runtime, 0).as_deref(), Some("1:0"));
                    assert_eq!(text_at(&runtime, 2).as_deref(), Some("1:2"));
                    assert_eq!(name_at(2), "item 1:2");
                    window.close().unwrap();
                    app.exit().unwrap();
                }
            }
        })?;
        timer.Start()?;
        Ok((timer, tick))
    })
    .unwrap();
}

#[test]
#[ignore = "initializes WinUI; run alone on a Windows desktop (no injected input)"]
fn list_item_container_style_applies_to_realized_items_and_clears() {
    App::run_with(|app| {
        let options = ItemContainerOptions {
            padding: Some(Thickness::uniform(0.0)),
            min_height: Some(40.0),
            horizontal_content_alignment: Some(HorizontalAlignment::Stretch),
            suppress_selection_visuals: true,
        };
        let view = || {
            ListView::new()
                .height(160.0)
                .item_container(options)
                .items((0usize..30).map(|i| {
                    DataItem::new(i, i.to_string())
                        .content(TextBlock::new().text(i.to_string()).height(40.0))
                }))
        };
        let mut runtime = Runtime::new(WinUiAdapter::default());
        runtime.update(view()).unwrap();
        let root = runtime.graph().root().unwrap();
        let window = runtime
            .adapter_mut()
            .create_window_with_policy(root, &WindowPolicy::new().client_size(300.0, 200.0))?;
        window.activate()?;
        let element = runtime.adapter().ui_element(root)?;
        let runtime = RefCell::new(runtime);
        let app = app.clone();
        let timer = native::DispatcherQueue::GetForCurrentThread()?.CreateTimer()?;
        timer.SetInterval(TimeSpan::try_from(Duration::from_millis(50)).unwrap())?;
        let tick = timer.Tick(move |sender, _| {
            sender.as_ref().unwrap().Stop().unwrap();
            element.UpdateLayout().unwrap();
            let mapping = element.cast::<native::IItemContainerMapping>().unwrap();
            let container = mapping.ContainerFromIndex(0).unwrap();
            assert_eq!(
                container
                    .cast::<native::IControl>()
                    .unwrap()
                    .Padding()
                    .unwrap(),
                native::Thickness::default()
            );
            assert_eq!(
                container
                    .cast::<native::IControl>()
                    .unwrap()
                    .HorizontalContentAlignment()
                    .unwrap(),
                native::HorizontalAlignment::Stretch
            );
            assert_eq!(
                container
                    .cast::<native::IFrameworkElement>()
                    .unwrap()
                    .MinHeight()
                    .unwrap(),
                40.0
            );
            runtime
                .borrow_mut()
                .update(ListView::new().items([]))
                .unwrap();
            assert!(
                element
                    .cast::<native::IItemsControl>()
                    .unwrap()
                    .ItemContainerStyle()
                    .is_err()
            );
            window.close().unwrap();
            app.exit().unwrap();
        })?;
        timer.Start()?;
        Ok((timer, tick))
    })
    .unwrap();
}

#[test]
#[ignore = "initializes WinUI; run alone on a Windows desktop (no injected input)"]
fn border_cursor_round_trips_through_the_protected_interface() {
    App::run_with(|app| {
        let mut runtime = Runtime::new(WinUiAdapter::default());
        runtime
            .update(Border::new().cursor(CursorShape::SizeWestEast))
            .unwrap();
        let root = runtime.graph().root().unwrap();
        let element = runtime
            .adapter()
            .ui_element(root)?
            .cast::<native::IUIElementProtected>()?;
        assert_eq!(
            element
                .ProtectedCursor()?
                .cast::<native::InputSystemCursor>()?
                .CursorShape()?,
            native::InputSystemCursorShape::SizeWestEast
        );
        runtime
            .update(Border::new().cursor(CursorShape::Hand))
            .unwrap();
        assert_eq!(
            element
                .ProtectedCursor()?
                .cast::<native::InputSystemCursor>()?
                .CursorShape()?,
            native::InputSystemCursorShape::Hand
        );
        runtime.update(Border::new()).unwrap();
        assert!(element.ProtectedCursor().is_err()); // WinRT null interface result.
        app.proxy().exit()?;
        Ok(runtime)
    })
    .unwrap();
}

#[test]
fn image_load_subscriptions_complete_once_and_do_not_retain_controls() {
    let state = ImageLoadState::default();
    let values = Rc::new(RefCell::new(Vec::new()));
    let result = Rc::clone(&values);
    let callback: Rc<dyn Fn(bool)> = Rc::new(move |value| result.borrow_mut().push(value));
    let removed: Rc<dyn Fn(bool)> = Rc::new(|_| panic!("removed subscription"));
    assert_eq!(state.subscribe(&removed), None);
    drop(removed);
    assert_eq!(state.subscribe(&callback), None);
    state.complete(true);
    state.complete(false);
    assert_eq!(*values.borrow(), [true]);
    assert_eq!(state.subscribe(&callback), Some(true));
    assert_eq!(*values.borrow(), [true]);
    assert!(state.subscribers.borrow().is_empty());
}

#[test]
fn native_queue_preview_uses_current_callback_and_preserves_order() {
    let object = ObjectId::test(1);
    let first = Callback::new(|_: Key| panic!("preview must not invoke callbacks"));
    let second = Callback::new(|_: Key| panic!("preview must not invoke callbacks"));
    let mut adapter = WinUiAdapter::default();
    adapter.menus.insert(
        object,
        NativeMenu {
            menu: Menu::new([], first),
            revision: 1,
            flyout: None,
            _revokers: Vec::new(),
        },
    );
    let observation = Observation::SetProperty {
        object,
        property: Property {
            id: PropertyId::Text,
            value: PropertyValue::String(Rc::from("Before")),
        },
    };
    adapter.event_queue.queue(
        None,
        Some(QueuedEvent {
            object,
            event: EventId::MenuItemInvoked,
            revision: 0,
            payload: EventPayload::Key("stale".into()),
        }),
    );
    adapter.event_queue.queue(Some(observation.clone()), None);
    adapter.event_queue.queue(
        None,
        Some(QueuedEvent {
            object,
            event: EventId::MenuItemInvoked,
            revision: 1,
            payload: EventPayload::Key("current".into()),
        }),
    );
    adapter.event_queue.queue(Some(observation.clone()), None);

    let mut preview = Vec::new();
    adapter.preview_native_events(&mut preview);
    assert_eq!(preview.len(), 3);
    assert_eq!(adapter.event_queue.events.borrow().len(), 4);

    // Callback replacement preserves the menu presentation revision.
    adapter.menus.get_mut(&object).unwrap().menu.on_click = second.clone();
    preview.clear();
    adapter.preview_native_events(&mut preview);
    assert_eq!(
        preview,
        [
            NativeEvent::new(None, None),
            NativeEvent::new(Some(observation.clone()), None),
            NativeEvent::new(
                None,
                Some(EventDispatch::new(
                    object,
                    EventId::MenuItemInvoked,
                    EventValue::Key(second),
                    EventPayload::Key("current".into()),
                )),
            ),
        ]
    );
    for expected in preview {
        assert_eq!(adapter.pop_native_event(), Some(expected));
    }
    assert_eq!(
        adapter.pop_native_event(),
        Some(NativeEvent::new(Some(observation), None))
    );
    assert_eq!(adapter.pop_native_event(), None);
}

#[test]
fn placement_coordinates_round_trip_negative_monitors_and_taskbar_offsets() {
    let screen = native::RECT {
        left: -1600,
        top: -200,
        right: -800,
        bottom: 400,
    };
    for (x, y) in [(0, 0), (48, 0), (0, 40), (48, 40)] {
        let workspace = window_placement::translate_rect(screen, -x, -y).unwrap();
        let restored = window_placement::translate_rect(workspace, x, y).unwrap();
        assert_eq!(restored, screen);
        assert_eq!(workspace.left, screen.left - x);
        assert_eq!(workspace.top, screen.top - y);
    }
    assert!(
        window_placement::translate_rect(
            native::RECT {
                left: i32::MAX,
                ..Default::default()
            },
            1,
            0,
        )
        .is_err()
    );
}

#[test]
fn initial_placement_rejects_invalid_extents_but_accepts_negative_positions() {
    let placement = WindowPlacement {
        x: -1200,
        y: -600,
        width: 800,
        height: 500,
        maximized: true,
    };
    let visuals = WindowVisuals::new().initial_placement(placement);
    assert_eq!(visuals.initial_placement, Some(placement));
    for invalid in [
        WindowPlacement {
            width: 0,
            ..placement
        },
        WindowPlacement {
            height: -1,
            ..placement
        },
        WindowPlacement {
            x: i32::MAX,
            ..placement
        },
        WindowPlacement {
            y: i32::MAX,
            ..placement
        },
    ] {
        assert!(
            std::panic::catch_unwind(|| WindowVisuals::new().initial_placement(invalid)).is_err()
        );
    }
}

#[test]
fn changed_initial_placement_does_not_become_a_reactive_resize() {
    let initial = WindowVisuals::new()
        .client_size(640.0, 480.0)
        .initial_position(ScreenPoint::new(50, 60));
    let updated = initial.clone().initial_placement(WindowPlacement {
        x: -1200,
        y: -600,
        width: 800,
        height: 500,
        maximized: true,
    });
    let changes = window_visual_changes(&initial, &updated);
    assert!(!changes.client_size);
    assert!(!changes.constraints);
    assert!(!changes.backdrop);
    assert!(!changes.icon);
    assert!(!changes.theme);
}

#[test]
fn content_dialog_schedule_is_fifo_and_cancellable() {
    let first = ObjectId::test(1);
    let second = ObjectId::test(2);
    let third = ObjectId::test(3);
    let mut schedule = ContentDialogSchedule::default();

    schedule.enqueue(first);
    schedule.enqueue(second);
    schedule.enqueue(second);
    schedule.enqueue(third);
    schedule.cancel(second);

    assert_eq!(schedule.next(), Some(first));
    assert_eq!(schedule.next(), Some(third));
    assert_eq!(schedule.next(), None);
}

#[test]
fn content_dialog_schedule_ignores_stale_completion_generation() {
    let dialog = ObjectId::test(1);
    let mut schedule = ContentDialogSchedule::default();
    schedule.begin(dialog, 7);

    assert!(!schedule.complete(dialog, 6));
    assert_eq!(schedule.active, Some((dialog, 7)));
    assert!(schedule.complete(dialog, 7));
    assert_eq!(schedule.active, None);
}

#[test]
fn active_slot_reorder_preserves_interleaved_retiring_indices() {
    let mut current = vec![(1, 1), (99, 99), (2, 2), (3, 3), (4, 4)];
    let active = HashSet::from([1, 2, 3, 4]);

    let swaps = simulate_active_slot_reorder(&mut current, &active, &[4, 3, 2, 1]).unwrap();

    assert_eq!(current, [(4, 4), (99, 99), (3, 3), (2, 2), (1, 1)]);
    assert_eq!(swaps.len(), 2);
    assert!(swaps.iter().all(|(left, right, _, _)| left < right));
}

#[test]
fn active_slot_reorder_rejects_missing_or_duplicate_active_identity() {
    let mut current = vec![(1, ()), (99, ()), (2, ())];
    let active = HashSet::from([1, 2]);

    assert!(simulate_active_slot_reorder(&mut current, &active, &[2, 3]).is_none());
    assert!(simulate_active_slot_reorder(&mut current, &active, &[2, 2]).is_none());
}

#[test]
fn controlled_property_feedback_uses_exact_incumbent_clear_values() {
    let cases = [
        (
            ObjectType::CheckBox,
            PropertyId::IsChecked,
            EventId::IsCheckedChanged,
            PropertyValue::OptionalBool(Some(false)),
        ),
        (
            ObjectType::ToggleButton,
            PropertyId::IsChecked,
            EventId::IsCheckedChanged,
            PropertyValue::OptionalBool(Some(false)),
        ),
        (
            ObjectType::Expander,
            PropertyId::IsExpanded,
            EventId::IsExpandedChanged,
            PropertyValue::Bool(false),
        ),
        (
            ObjectType::NavigationView,
            PropertyId::IsPaneOpen,
            EventId::IsPaneOpenChanged,
            PropertyValue::Bool(true),
        ),
    ];

    for (kind, property, event, value) in cases {
        assert_eq!(
            GeneratedHandle::feedback_expectation(kind, property, None),
            Some((
                event,
                FeedbackExpectation::Exact(Property {
                    id: property,
                    value,
                }),
            ))
        );
    }
}

fn virtual_index(value: IInspectable) -> i32 {
    value
        .cast::<windows_reference::IReference<i32>>()
        .unwrap()
        .Value()
        .unwrap()
}

#[test]
fn recycled_physical_shell_keeps_old_token_until_acknowledged() {
    let mut shells = ShellPool::<usize>::default();
    let (old, physical) = shells.take(|| Ok::<_, windows_core::Error>(42)).unwrap();

    assert!(shells.retire(old, true));
    assert!(shells.retired.contains(&old));
    let (new, reused) = shells
        .take(|| panic!("retired physical shell was not reused"))
        .unwrap();

    assert_ne!(new, old);
    assert_eq!(reused, physical);
    assert!(shells.retired.contains(&old));
    assert!(shells.shells.contains_key(&new));

    shells.acknowledge_recycle(old);
    assert!(!shells.retired.contains(&old));
    assert!(shells.shells.contains_key(&new));
}

#[test]
fn adopted_shell_stays_with_its_container() {
    let mut shells = ShellPool::<usize>::default();
    let old = shells.adopt(7).unwrap();

    assert!(shells.retire(old, false));
    assert!(shells.retired.contains(&old));
    assert!(shells.available.is_empty());
    let new = shells.adopt(7).unwrap();
    assert_ne!(new, old);
    assert_eq!(shells.shells.get(&new), Some(&7));
}

#[test]
fn virtual_source_generates_indices_on_demand() {
    let source = ComObject::new(NativeVirtualSource::new(3).unwrap());
    let vector: IObservableVector<IInspectable> = source.to_interface();

    assert_eq!(vector.Size().unwrap(), 3);
    assert_eq!(virtual_index(vector.GetAt(0).unwrap()), 0);
    assert_eq!(virtual_index(vector.GetAt(2).unwrap()), 2);
    assert_eq!(vector.GetAt(3).unwrap_err().code(), E_BOUNDS);

    let mut index = u32::MAX;
    assert!(
        vector
            .IndexOf(&virtual_item_value(2).unwrap(), &mut index)
            .unwrap()
    );
    assert_eq!(index, 2);
    let missing: IInspectable = windows_reference::IReference::<i32>::from(9).into();
    assert!(!vector.IndexOf(&missing, &mut index).unwrap());
    assert_eq!(index, 0);
    let negative: IInspectable = windows_reference::IReference::<i32>::from(-1).into();
    assert!(!vector.IndexOf(&negative, &mut index).unwrap());
    let invalid: IInspectable =
        windows_reference::IReference::<HSTRING>::from(HSTRING::from("invalid")).into();
    assert!(vector.IndexOf(&invalid, &mut index).is_err());

    let mut values = vec![None; 4];
    assert_eq!(vector.GetMany(1, &mut values).unwrap(), 2);
    assert_eq!(virtual_index(values[0].take().unwrap()), 1);
    assert_eq!(virtual_index(values[1].take().unwrap()), 2);
    assert!(values[2].is_none());

    let view = vector.GetView().unwrap();
    assert_eq!(view.Size().unwrap(), 3);
    assert_eq!(virtual_index(view.GetAt(1).unwrap()), 1);

    let iterator = vector.First().unwrap();
    assert!(iterator.HasCurrent().unwrap());
    assert_eq!(virtual_index(iterator.Current().unwrap()), 0);
    assert!(iterator.MoveNext().unwrap());
    assert_eq!(virtual_index(iterator.Current().unwrap()), 1);
    let mut tail = vec![None; 3];
    assert_eq!(iterator.GetMany(&mut tail).unwrap(), 2);
    assert_eq!(virtual_index(tail[0].take().unwrap()), 1);
    assert_eq!(virtual_index(tail[1].take().unwrap()), 2);
    assert!(!iterator.HasCurrent().unwrap());
    assert_eq!(iterator.Current().unwrap_err().code(), E_BOUNDS);
}

#[test]
fn virtual_source_is_read_only_and_validates_index_limits() {
    let max_count = i32::MAX as usize + 1;
    let source = ComObject::new(NativeVirtualSource::new(max_count).unwrap());
    let vector: IObservableVector<IInspectable> = source.to_interface();

    assert_eq!(vector.Size().unwrap(), max_count as u32);
    assert_eq!(
        virtual_index(vector.GetAt(i32::MAX as u32).unwrap()),
        i32::MAX
    );
    assert_eq!(
        vector.GetAt(i32::MAX as u32 + 1).unwrap_err().code(),
        E_BOUNDS
    );
    assert!(NativeVirtualSource::new(max_count + 1).is_err());

    let value = virtual_item_value(0).unwrap();
    assert_eq!(
        vector.SetAt(0, &value).unwrap_err().code(),
        E_ILLEGAL_METHOD_CALL
    );
    assert_eq!(
        vector.Append(&value).unwrap_err().code(),
        E_ILLEGAL_METHOD_CALL
    );
    assert_eq!(vector.Clear().unwrap_err().code(), E_ILLEGAL_METHOD_CALL);
    assert_eq!(
        vector.ReplaceAll(&[Some(value)]).unwrap_err().code(),
        E_ILLEGAL_METHOD_CALL
    );
}

#[test]
fn virtual_source_reset_notifies_once_and_honors_event_removal() {
    let source = ComObject::new(NativeVirtualSource::new(0).unwrap());
    let vector: IObservableVector<IInspectable> = source.to_interface();
    let first = Arc::new(AtomicUsize::new(0));
    let second = Arc::new(AtomicUsize::new(0));
    let first_observed = Arc::clone(&first);
    let first_revoker = vector
        .VectorChanged(move |_, args| {
            let args = args.ok().unwrap();
            assert_eq!(args.CollectionChange().unwrap(), CollectionChange::Reset);
            assert_eq!(args.Index().unwrap(), 0);
            first_observed.fetch_add(1, Ordering::Relaxed);
        })
        .unwrap();
    let second_observed = Arc::clone(&second);
    let second_revoker = vector
        .VectorChanged(move |_, _| {
            second_observed.fetch_add(1, Ordering::Relaxed);
        })
        .unwrap();

    source.reset(10_000);
    assert_eq!(vector.Size().unwrap(), 10_000);
    assert_eq!(first.load(Ordering::Relaxed), 1);
    assert_eq!(second.load(Ordering::Relaxed), 1);

    drop(first_revoker);
    source.reset(10_000);
    assert_eq!(first.load(Ordering::Relaxed), 1);
    assert_eq!(second.load(Ordering::Relaxed), 2);

    drop(second_revoker);
    source.reset(0);
    assert_eq!(vector.Size().unwrap(), 0);
    assert_eq!(first.load(Ordering::Relaxed), 1);
    assert_eq!(second.load(Ordering::Relaxed), 2);
}

#[test]
fn policy_preserves_requested_window_state() {
    let policy = WindowPolicy::new()
        .title("Solitaire")
        .theme(WindowTheme::Dark)
        .client_size(800.0, 600.0)
        .minimum_client_size(800.0, 600.0);

    assert_eq!(policy.title.as_deref(), Some("Solitaire"));
    assert_eq!(policy.theme, WindowTheme::Dark);
    assert_eq!(policy.client_size, Some((800.0, 600.0)));
    assert_eq!(policy.minimum_client_size, Some((800.0, 600.0)));
}

#[test]
fn policy_rejects_invalid_window_sizes() {
    for (width, height) in [
        (0.0, 1.0),
        (1.0, -1.0),
        (f64::NAN, 1.0),
        (1.0, f64::INFINITY),
    ] {
        assert!(
            std::panic::catch_unwind(|| WindowPolicy::new().client_size(width, height)).is_err()
        );
        assert!(
            std::panic::catch_unwind(|| { WindowPolicy::new().minimum_client_size(width, height) })
                .is_err()
        );
    }
}

#[test]
fn window_visual_changes_only_apply_changed_contracts() {
    let previous = WindowVisuals::new()
        .theme(WindowTheme::System)
        .backdrop(WindowBackdrop::Mica)
        .client_size(800.0, 600.0);
    let next = WindowVisuals::new()
        .theme(WindowTheme::Dark)
        .backdrop(WindowBackdrop::Mica)
        .client_size(800.0, 600.0);

    let changes = window_visual_changes(&previous, &next);
    assert!(!changes.backdrop);
    assert!(!changes.client_size);
    assert!(!changes.constraints);
    assert!(!changes.icon);
    assert!(changes.theme);
    assert!(changes.any());
    assert!(!window_visual_changes(&next, &next).any());
}

#[test]
fn window_visual_changes_detect_icon_removal() {
    let previous = WindowVisuals::new().icon("test.ico");
    let changes = window_visual_changes(&previous, &WindowVisuals::new());
    assert!(changes.icon);
}

#[test]
fn pointer_event_queue_preserves_payload_and_revision() {
    let mut runtime = Runtime::new(RecordingAdapter::default());
    runtime.update(Border::new()).unwrap();
    let object = runtime.graph().root().unwrap();
    let event = Rc::new(RefCell::new(NativePointerEventInfoEvent {
        revision: 7,
        callback: Some(Callback::new(|_| {})),
    }));
    let event_queue = Rc::new(NativeEventQueue::default());
    let payload = PointerEventInfo {
        pointer_id: 42,
        modifiers: InputModifiers::CONTROL,
        is_captured: true,
        is_right_button_pressed: true,
        ..Default::default()
    };

    assert!(WinUiAdapter::queue_event(
        &event,
        &event_queue,
        object,
        EventId::PointerReleased,
        None,
        payload,
    ));
    let queued = event_queue.events.borrow();
    assert_eq!(queued.len(), 1);
    assert!(queued[0].observation.is_none());
    let event = queued[0].event.as_ref().unwrap();
    assert_eq!(event.object, object);
    assert_eq!(event.event, EventId::PointerReleased);
    assert_eq!(event.revision, 7);
    assert_eq!(event.payload, EventPayload::PointerEventInfo(payload));
}

#[test]
fn pointer_event_modifiers_map_native_flags() {
    let mut expected = InputModifiers::SHIFT;
    expected |= InputModifiers::CONTROL;
    expected |= InputModifiers::ALT;
    expected |= InputModifiers::WINDOWS;

    assert_eq!(
        WinUiAdapter::input_modifiers_from_virtual_keys(
            native::VirtualKeyModifiers::Shift
                | native::VirtualKeyModifiers::Control
                | native::VirtualKeyModifiers::Menu
                | native::VirtualKeyModifiers::Windows
        ),
        expected
    );
    assert_eq!(
        WinUiAdapter::input_modifiers_from_virtual_keys(native::VirtualKeyModifiers::None),
        InputModifiers::NONE
    );
}

#[test]
fn optional_numeric_adapters_use_distinct_sentinels() {
    assert!(native_number_box_value(None).is_nan());
    assert_eq!(number_box_value(f64::NAN), None);
    assert_eq!(number_box_value(-1.0), Some(-1.0));

    assert_eq!(native_rating_value(None), -1.0);
    assert_eq!(rating_value(-1.0), None);
    assert!(rating_value(f64::NAN).unwrap().is_nan());
}

#[test]
fn selection_index_conversion_reports_overflow() {
    assert_eq!(native_selection_index(None), Ok(-1));
    assert_eq!(
        native_selection_index(Some(i32::MAX as usize)),
        Ok(i32::MAX)
    );
    let overflow = i32::MAX as usize + 1;
    assert_eq!(
        native_selection_index(Some(overflow)),
        Err(WinUiError::IndexOverflow(overflow))
    );
}

#[test]
fn generated_feedback_contracts_distinguish_exact_normalized_and_clear() {
    let exact = GeneratedHandle::feedback_expectation(
        ObjectType::ToggleSwitch,
        PropertyId::IsOn,
        Some(&PropertyValue::Bool(true)),
    );
    assert_eq!(
        exact,
        Some((
            EventId::Toggled,
            FeedbackExpectation::Exact(Property {
                id: PropertyId::IsOn,
                value: PropertyValue::Bool(true),
            })
        ))
    );
    assert_eq!(
        GeneratedHandle::feedback_expectation(ObjectType::ToggleSwitch, PropertyId::IsOn, None,),
        Some((
            EventId::Toggled,
            FeedbackExpectation::Exact(Property {
                id: PropertyId::IsOn,
                value: PropertyValue::Bool(false),
            })
        ))
    );
    assert_eq!(
        GeneratedHandle::feedback_expectation(
            ObjectType::Slider,
            PropertyId::Value,
            Some(&PropertyValue::F64(0.5)),
        ),
        Some((
            EventId::ValueChanged,
            FeedbackExpectation::Normalized { observation: None }
        ))
    );
    assert_eq!(
        GeneratedHandle::feedback_expectation(
            ObjectType::ProgressBar,
            PropertyId::Value,
            Some(&PropertyValue::F64(0.5)),
        ),
        None
    );
    assert_eq!(
        GeneratedHandle::feedback_expectation(ObjectType::RichEditBox, PropertyId::Document, None,),
        Some((
            EventId::TextChanged,
            FeedbackExpectation::DeferredExact(Property {
                id: PropertyId::Document,
                value: PropertyValue::String(Rc::from("")),
            })
        ))
    );
}
