#![windows_subsystem = "windows"]

use widget_cruncher::action::Action;
use widget_cruncher::app_delegate::{AppDelegate, DelegateCtx};
use widget_cruncher::widget::prelude::*;
use widget_cruncher::widget::widget_view::WidgetView;
use widget_cruncher::widget::{Button, Flex, Label, Portal};
use widget_cruncher::{AppLauncher, WindowDesc, WindowId};

const VERTICAL_WIDGET_SPACING: f64 = 20.0;

struct Delegate;

impl AppDelegate for Delegate {
    fn on_action(
        &mut self,
        ctx: &mut DelegateCtx,
        _window_id: WindowId,
        _widget_id: WidgetId,
        action: Action,
        _env: &Env,
    ) {
        match action {
            Action::ButtonPressed => {
                let mut root: WidgetView<Portal<Flex>> = ctx.get_root();
                let mut flex = root.get_child_view();
                flex.add_child(Label::new("Hello"));
            }
            Action::TextChanged(_) => todo!(),
            _ => {}
        }
    }
}

fn main() {
    // describe the main window
    let main_window = WindowDesc::new(build_root_widget())
        .title("To-do list")
        .window_size((400.0, 400.0));

    // start the application. Here we pass in the application state.
    AppLauncher::with_window(main_window)
        .with_delegate(Delegate)
        .log_to_console()
        .launch()
        .expect("Failed to launch application");
}

fn build_root_widget() -> impl Widget {
    let button = Button::new("Add task");

    // arrange the two widgets vertically, with some padding
    Portal::new(
        Flex::column()
            .with_child(button)
            .with_spacer(VERTICAL_WIDGET_SPACING),
    )
}