#![allow(unused)]
use std::collections::HashMap;
use tracing::trace;

use crate::action::Action;
use crate::command::{Command, CommandQueue};
use crate::ext_event::{ExtEventQueue, ExtEventSink};
use crate::widget::{WidgetMut, WidgetRef};
use crate::{
    Env, Event, Handled, SingleUse, Target, Widget, WidgetId, WindowDesc, WindowId, WindowRoot,
};

/// A context passed in to [`AppDelegate`] functions.
///
/// [`AppDelegate`]: trait.AppDelegate.html
pub struct DelegateCtx<'a, 'b> {
    //pub(crate) command_queue: &'a mut CommandQueue,
    pub(crate) ext_event_queue: &'a ExtEventQueue,
    // FIXME - Ideally, we'd like to get a hashmap of all root widgets,
    // but that creates "aliasing mutable references" problems
    pub(crate) main_root_widget: WidgetMut<'a, 'b, dyn Widget>,
    //pub(crate) active_windows: &'a mut HashMap<WindowId, WindowRoot>,
}

impl<'a, 'b> DelegateCtx<'a, 'b> {
    #[cfg(FALSE)]
    pub fn submit_command(&mut self, command: impl Into<Command>) {
        self.command_queue
            .push_back(command.into().default_to(Target::Global))
    }

    /// Returns an [`ExtEventSink`] that can be moved between threads,
    /// and can be used to submit commands back to the application.
    ///
    /// [`ExtEventSink`]: struct.ExtEventSink.html
    pub fn get_external_handle(&self) -> ExtEventSink {
        self.ext_event_queue.make_sink()
    }

    #[cfg(FALSE)]
    pub fn new_window(&mut self, desc: WindowDesc) {
        trace!("new_window");
        self.submit_command(
            crate::command::NEW_WINDOW
                .with(SingleUse::new(Box::new(desc)))
                .to(Target::Global),
        );
    }

    pub fn try_get_root<W: Widget>(&mut self) -> Option<WidgetMut<'_, 'b, W>> {
        self.main_root_widget.downcast()
    }

    pub fn get_root<W: Widget>(&mut self) -> WidgetMut<'_, 'b, W> {
        // root_widgets.get_mut(&window_id).expect("could not find window")
        self.main_root_widget.downcast().expect("wrong widget type")
    }
}

// TODO - remove all other methods, only keep on_event | on_command ?
pub trait AppDelegate {
    fn on_event(
        &mut self,
        ctx: &mut DelegateCtx,
        window_id: WindowId,
        event: &Event,
        env: &Env,
    ) -> Handled {
        #![allow(unused)]
        Handled::No
    }

    fn on_command(&mut self, ctx: &mut DelegateCtx, cmd: &Command, env: &Env) -> Handled {
        #![allow(unused)]
        Handled::No
    }

    fn on_action(
        &mut self,
        ctx: &mut DelegateCtx,
        window_id: WindowId,
        widget_id: WidgetId,
        action: Action,
        env: &Env,
    ) {
        #![allow(unused)]
    }

    /// The handler for window creation events.
    /// This function is called after a window has been added,
    /// allowing you to customize the window creation behavior of your app.
    fn on_window_added(&mut self, ctx: &mut DelegateCtx, id: WindowId, env: &Env) {
        #![allow(unused)]
    }

    /// The handler for window deletion events.
    /// This function is called after a window has been removed.
    fn on_window_removed(&mut self, ctx: &mut DelegateCtx, id: WindowId, env: &Env) {
        #![allow(unused)]
    }
}

// TODO - document
pub(crate) struct NullDelegate;

impl AppDelegate for NullDelegate {}