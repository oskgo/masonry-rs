// Copyright 2020 The Druid Authors.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Tools and infrastructure for testing widgets.

use image::io::Reader as ImageReader;
use instant::Duration;
use shell::text::Selection;
use std::collections::{HashMap, VecDeque};
use std::panic::Location;
use std::path::Path;
use std::sync::Arc;

use crate::action::{Action, ActionQueue};
//use crate::ext_event::ExtEventHost;
use crate::command::CommandQueue;
use crate::contexts::GlobalPassCtx;
use crate::debug_logger::DebugLogger;
use crate::ext_event::ExtEventQueue;
use crate::piet::{BitmapTarget, Device, Error, ImageFormat, Piet};
use crate::platform::PendingWindow;
use crate::widget::WidgetState;
use crate::widget::{WidgetMut, WidgetRef};
use crate::*;
use druid_shell::{KeyEvent, Modifiers, MouseButton, MouseButtons};
pub use druid_shell::{
    RawMods, Region, Scalable, Scale, Screen, SysMods, TimerToken, WindowHandle, WindowLevel,
    WindowState,
};

use super::screenshots::{get_image_diff, get_rgba_image};
use super::snapshot_utils::get_cargo_workspace;
use super::MockTimerQueue;

pub const HARNESS_DEFAULT_SIZE: Size = Size::new(400., 400.);

// TODO - Rewrite this doc
// - Explain timers
/// A type that tries very hard to provide a comforting and safe environment
/// for widgets who are trying to find their way.
///
/// You create a `Harness` with some widget and its initial data; then you
/// can send events to that widget and verify that expected conditions are met.
///
/// Harness tries to act like the normal druid environment; for instance, it will
/// attempt to dispatch any `Command`s that are sent during event handling, and
/// it will call `update` automatically after an event.
///
/// That said, it _is_ missing a bunch of logic that would normally be handled
/// in `AppState`: for instance it does not clear the `needs_inval` and
/// `children_changed` flags on the window after an update.
///
/// In addition, layout and paint **are not called automatically**. This is
/// because paint is triggered by druid-shell, and there is no druid-shell here;
///
/// if you want those functions run you will need to call them yourself.
pub struct Harness {
    mock_app: MockAppRoot,
    mouse_state: MouseEvent,
    window_size: Size,
}

// TODO - merge
/// All of the state except for the `Piet` (render context). We need to pass
/// that in to get around some lifetime issues.
struct MockAppRoot {
    env: Env,
    window: WindowRoot,
    command_queue: CommandQueue,
    action_queue: ActionQueue,
    debug_logger: DebugLogger,
}

#[allow(missing_docs)]
impl Harness {
    pub fn create(root: impl Widget) -> Self {
        Self::create_with_size(root, HARNESS_DEFAULT_SIZE)
    }

    pub fn create_with_size(root: impl Widget, window_size: Size) -> Self {
        //let ext_host = ExtEventHost::default();
        //let ext_handle = ext_host.make_sink();

        // FIXME
        let event_queue = ExtEventQueue::new();

        let pending = PendingWindow::new(root);
        let window = WindowRoot::new(
            WindowId::next(),
            Default::default(),
            event_queue.make_sink(),
            pending,
            Some(MockTimerQueue::new()),
        );

        let mouse_state = MouseEvent {
            pos: Point::ZERO,
            window_pos: Point::ZERO,
            buttons: MouseButtons::default(),
            mods: Modifiers::default(),
            count: 0,
            focus: false,
            button: MouseButton::None,
            wheel_delta: Vec2::ZERO,
        };

        let mut harness = Harness {
            mock_app: MockAppRoot {
                env: Env::with_theme(),
                window,
                command_queue: VecDeque::new(),
                action_queue: VecDeque::new(),
                debug_logger: DebugLogger::new(true),
            },
            mouse_state,
            window_size,
        };

        // verify that all widgets are marked as having children_changed
        // (this should always be true for a new widget)
        harness.inspect_widgets(|widget| assert!(widget.state().children_changed));

        harness.process_event(Event::WindowConnected);
        harness.process_event(Event::WindowSize(window_size));

        harness
    }

    /// Send an event to the widget.
    ///
    /// If this event triggers lifecycle events, they will also be dispatched,
    /// as will any resulting commands. Commands created as a result of this event
    /// will also be dispatched.
    pub fn process_event(&mut self, event: Event) {
        self.mock_app.event(event);

        self.post_event_stuff();
    }

    // TODO - rename
    fn post_event_stuff(&mut self) {
        loop {
            let cmd = self.mock_app.command_queue.pop_front();
            match cmd {
                Some(cmd) => self
                    .mock_app
                    .event(Event::Internal(InternalEvent::TargetedCommand(cmd))),
                None => break,
            };
        }

        // TODO - this might be too coarse
        if self.root_widget().state().needs_layout {
            self.mock_app.layout();
            *self.window_mut().invalid_mut() = Region::from(self.window_size.to_rect());
        }
    }

    fn render_to(&mut self, render_target: &mut BitmapTarget) {
        /// A way to clean up resources when our render context goes out of
        /// scope, even during a panic.
        pub struct RenderContextGuard<'a>(Piet<'a>);

        impl Drop for RenderContextGuard<'_> {
            fn drop(&mut self) {
                // We need to call finish even if a test assert failed
                if let Err(err) = self.0.finish() {
                    // We can't panic, because we might already be panicking
                    tracing::error!("piet finish failed: {}", err);
                }
            }
        }

        let mut piet = RenderContextGuard(render_target.render_context());

        // FIXME - this doesn't make sense given we might render to a fresh surface
        let invalid = std::mem::replace(self.window_mut().invalid_mut(), Region::EMPTY);
        self.mock_app.paint_region(&mut piet.0, &invalid);
    }

    /// Create a Piet bitmap render context (an array of pixels), paint the
    /// window and return the bitmap.
    pub fn render(&mut self) -> Arc<[u8]> {
        let mut device = Device::new().expect("harness failed to get device");
        let mut render_target = device
            .bitmap_target(
                self.window_size.width as usize,
                self.window_size.height as usize,
                1.0,
            )
            .expect("failed to create bitmap_target");

        self.render_to(&mut render_target);

        render_target
            .to_image_buf(ImageFormat::RgbaPremul)
            .unwrap()
            .raw_pixels_shared()
    }

    // --- Event helpers ---

    /// Move an internal mouse state, and send a MouseMove event to the window.
    pub fn mouse_move(&mut self, pos: impl Into<Point>) {
        let pos = pos.into();
        // FIXME - not actually the same
        self.mouse_state.pos = pos;
        self.mouse_state.window_pos = pos;
        self.mouse_state.button = MouseButton::None;

        self.process_event(Event::MouseMove(self.mouse_state.clone()));
    }

    /// Send a MouseDown event to the window.
    pub fn mouse_button_press(&mut self, button: MouseButton) {
        self.mouse_state.buttons.insert(button);
        self.mouse_state.button = button;

        self.process_event(Event::MouseDown(self.mouse_state.clone()));
    }

    /// Send a MouseUp event to the window.
    pub fn mouse_button_release(&mut self, button: MouseButton) {
        self.mouse_state.buttons.remove(button);
        self.mouse_state.button = button;

        self.process_event(Event::MouseUp(self.mouse_state.clone()));
    }

    /// Send a Wheel event to the window
    pub fn mouse_wheel(&mut self, wheel_delta: Vec2) {
        self.mouse_state.button = MouseButton::None;
        self.mouse_state.wheel_delta = wheel_delta;

        self.process_event(Event::Wheel(self.mouse_state.clone()));
        self.mouse_state.wheel_delta = Vec2::ZERO;
    }

    /// Send events that lead to a given widget being clicked.
    ///
    /// Combines [`mouse_move`](Self::mouse_move), [`mouse_button_press`](Self::mouse_button_press), and [`mouse_button_release`](Self::mouse_button_release).
    pub fn mouse_click_on(&mut self, id: WidgetId) {
        let widget_rect = self.get_widget(id).state().window_layout_rect();
        let widget_center = widget_rect.center();

        self.mouse_move(widget_center);
        self.mouse_button_press(MouseButton::Left);
        self.mouse_button_release(MouseButton::Left);
    }

    /// Use [`mouse_move`](Self::mouse_move) to set the internal mouse pos to the center of the given widget.
    pub fn mouse_move_to(&mut self, id: WidgetId) {
        // FIXME - handle case where the widget isn't visible
        // FIXME - assert that the widget correctly receives the event otherwise?
        let widget_rect = self.get_widget(id).state().window_layout_rect();
        let widget_center = widget_rect.center();

        self.mouse_move(widget_center);
    }

    // TODO - Handle complicated IME

    /// Simulate typing the given text.
    ///
    /// For every character in the input string (more specifically,
    /// for every Unicode Scalar Value), this sends a KeyDown and a
    /// KeyUp event to the window.
    ///
    /// Obviously this works better with ASCII text.
    ///
    /// IME mocking is a future feature.
    pub fn keyboard_type_chars(&mut self, text: &str) {
        // For each character
        for c in text.split("").filter(|s| !s.is_empty()) {
            let event = KeyEvent::for_test(RawMods::None, c);

            if self.mock_app.event(Event::KeyDown(event.clone())) == Handled::No {
                if let Some(mut input_handler) = self.mock_app.window.get_focused_ime_handler(true)
                {
                    // This is copy-pasted from druid-shell's simulate_input function
                    let selection = input_handler.selection();
                    input_handler.replace_range(selection.range(), &c);
                    let new_caret_index = selection.min() + c.len();
                    input_handler.set_selection(Selection::caret(new_caret_index));

                    let modified_widget = self.mock_app.window.release_focused_ime_handler();

                    if let Some(widget_id) = modified_widget {
                        let event = Event::Internal(InternalEvent::RouteImeStateChange(widget_id));
                        self.mock_app.event(event);
                    }
                }
            }
            self.mock_app.event(Event::KeyUp(event.clone()));
        }
        self.post_event_stuff();
    }

    // TODO - add doc alias "send_command"
    /// Send a command to a target.
    pub fn submit_command(&mut self, command: impl Into<Command>) {
        let command = command.into().default_to(self.mock_app.window.id.into());
        let event = Event::Internal(InternalEvent::TargetedCommand(command));
        self.process_event(event);
    }

    pub fn move_timers_forward(&mut self, duration: Duration) {
        // TODO - handle animations
        let tokens = self
            .mock_app
            .window
            .mock_timer_queue
            .as_mut()
            .unwrap()
            .move_forward(duration);
        for token in tokens {
            self.process_event(Event::Timer(token));
        }
    }

    // --- Getters ---

    pub fn window(&self) -> &WindowRoot {
        &self.mock_app.window
    }

    pub fn window_mut(&mut self) -> &mut WindowRoot {
        &mut self.mock_app.window
    }

    pub fn root_widget(&self) -> WidgetRef<'_, dyn Widget> {
        self.mock_app.window.root.as_dyn()
    }

    pub fn get_widget(&self, id: WidgetId) -> WidgetRef<'_, dyn Widget> {
        self.mock_app
            .window
            .find_widget_by_id(id)
            .expect("could not find widget")
    }

    pub fn try_get_widget(&self, id: WidgetId) -> Option<WidgetRef<'_, dyn Widget>> {
        self.mock_app.window.find_widget_by_id(id)
    }

    pub fn focused_widget(&self) -> Option<WidgetRef<'_, dyn Widget>> {
        self.mock_app.window.focused_widget()
    }

    pub fn inspect_widgets(&mut self, f: impl Fn(WidgetRef<'_, dyn Widget>) + 'static) {
        fn inspect(
            widget: WidgetRef<'_, dyn Widget>,
            f: &(impl Fn(WidgetRef<'_, dyn Widget>) + 'static),
        ) {
            f(widget);
            for child in widget.widget().children() {
                inspect(child, f);
            }
        }

        inspect(self.mock_app.window.root.as_dyn(), &f);
    }

    pub fn edit_root_widget<R>(
        &mut self,
        f: impl FnOnce(WidgetMut<'_, '_, dyn Widget>, &Env) -> R,
    ) -> R {
        // TODO - Move to MockAppRoot?
        let window = &mut self.mock_app.window;
        let mut fake_widget_state;
        let mut timers = HashMap::new();
        let res = {
            let mut global_state = GlobalPassCtx::new(
                window.ext_event_sink.clone(),
                &mut self.mock_app.debug_logger,
                &mut self.mock_app.command_queue,
                &mut self.mock_app.action_queue,
                &mut timers,
                window.mock_timer_queue.as_mut(),
                &window.handle,
                window.id,
                window.focus,
            );
            fake_widget_state = window.root.state.clone();

            let root_widget = WidgetMut {
                global_state: &mut global_state,
                parent_widget_state: &mut fake_widget_state,
                widget_state: &mut window.root.state,
                widget: &mut *window.root.inner,
            };

            f(root_widget, &self.mock_app.env)
        };

        // Timer creation should use mock_timer_queue instead
        assert!(timers.is_empty());

        // TODO - handle cursor and validation

        window.post_event_processing(
            &mut fake_widget_state,
            &mut self.mock_app.debug_logger,
            &mut self.mock_app.command_queue,
            &mut self.mock_app.action_queue,
            &self.mock_app.env,
            false,
        );
        self.post_event_stuff();

        res
    }

    pub fn pop_action(&mut self) -> Option<(Action, WidgetId)> {
        let (action, widget_id, _) = self.mock_app.action_queue.pop_front()?;
        Some((action, widget_id))
    }

    // --- Screenshots ---

    pub fn check_render_snapshot(
        &mut self,
        manifest_dir: &str,
        test_file_path: &str,
        test_module_path: &str,
        test_name: &str,
    ) {
        let mut device = Device::new().expect("harness failed to get device");
        let mut render_target = device
            .bitmap_target(
                self.window_size.width as usize,
                self.window_size.height as usize,
                1.0,
            )
            .expect("failed to create bitmap_target");

        self.render_to(&mut render_target);

        let new_image = get_rgba_image(&mut render_target, self.window_size);

        let workspace_path = get_cargo_workspace(manifest_dir);
        let test_file_path_abs = workspace_path.join(test_file_path);
        let folder_path = test_file_path_abs.parent().unwrap();

        let screenshots_folder = folder_path.join("screenshots");
        std::fs::create_dir_all(&screenshots_folder).unwrap();

        let module_str = test_module_path.replace("::", "__");

        let reference_path = screenshots_folder.join(format!("{module_str}__{test_name}.png"));
        let new_path = screenshots_folder.join(format!("{module_str}__{test_name}.new.png"));
        let diff_path = screenshots_folder.join(format!("{module_str}__{test_name}.diff.png"));

        if let Ok(reference_file) = ImageReader::open(&reference_path) {
            let ref_image = reference_file.decode().unwrap().to_rgba8();

            if let Some(diff_image) = get_image_diff(&ref_image, &new_image) {
                // Remove '<test_name>.new.png' '<test_name>.diff.png' files if they exist
                let _ = std::fs::remove_file(&new_path);
                let _ = std::fs::remove_file(&diff_path);
                new_image.save(&new_path).unwrap();
                diff_image.save(&diff_path).unwrap();
                panic!("Images are different");
            }
        } else {
            // Remove '<test_name>.new.png' file if it exists
            let _ = std::fs::remove_file(&new_path);
            new_image.save(&new_path).unwrap();
            panic!("No reference file");
        }
    }

    // --- Debug logger ---

    pub fn push_log(&mut self, message: &str) {
        self.mock_app
            .debug_logger
            .update_widget_state(self.mock_app.window.root.as_dyn());
        self.mock_app.debug_logger.push_log(false, message);
    }

    // ex: harness.write_debug_logs("test_log.json");
    pub fn write_debug_logs(&mut self, path: &str) {
        self.mock_app.debug_logger.write_to_file(path);
    }
}

#[allow(dead_code)]
impl MockAppRoot {
    fn event(&mut self, event: Event) -> Handled {
        self.window.event(
            event,
            &mut self.debug_logger,
            &mut self.command_queue,
            &mut self.action_queue,
            &self.env,
        )
    }

    fn lifecycle(&mut self, event: LifeCycle) {
        self.window.lifecycle(
            &event,
            &mut self.debug_logger,
            &mut self.command_queue,
            &mut self.action_queue,
            &self.env,
            false,
        );
    }

    fn layout(&mut self) {
        self.window.layout(
            &mut self.debug_logger,
            &mut self.command_queue,
            &mut self.action_queue,
            &self.env,
        );
    }

    fn paint_region(&mut self, piet: &mut Piet, invalid: &Region) {
        self.window.do_paint(
            piet,
            invalid,
            &mut self.debug_logger,
            &mut self.command_queue,
            &mut self.action_queue,
            &self.env,
        );
    }
}