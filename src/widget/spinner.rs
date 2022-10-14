// Copyright 2020 The Druid Authors.

//! An animated spinner widget.

use std::f64::consts::PI;

use druid::kurbo::Line;
use druid::widget::prelude::*;
use druid::{theme, Color, KeyOrValue, Point, Vec2};
use smallvec::SmallVec;
use tracing::trace;

use crate::contexts::WidgetCtx;
use crate::widget::WidgetRef;

// TODO - Set color
/// An animated spinner widget for showing a loading state.
///
/// To customize the spinner's size, you can place it inside a [`SizedBox`]
/// that has a fixed width and height.
///
/// [`SizedBox`]: struct.SizedBox.html
pub struct Spinner {
    t: f64,
    color: KeyOrValue<Color>,
}

pub struct SpinnerMut<'a, 'b>(WidgetCtx<'a, 'b>, &'a mut Spinner);

impl Spinner {
    /// Create a spinner widget
    pub fn new() -> Spinner {
        Spinner::default()
    }

    /// Builder-style method for setting the spinner's color.
    ///
    /// The argument can be either a `Color` or a [`Key<Color>`].
    ///
    /// [`Key<Color>`]: ../struct.Key.html
    pub fn with_color(mut self, color: impl Into<KeyOrValue<Color>>) -> Self {
        self.color = color.into();
        self
    }
}

impl SpinnerMut<'_, '_> {
    /// Set the spinner's color.
    ///
    /// The argument can be either a `Color` or a [`Key<Color>`].
    ///
    /// [`Key<Color>`]: ../struct.Key.html
    pub fn set_color(&mut self, color: impl Into<KeyOrValue<Color>>) {
        self.1.color = color.into();
        self.0.request_paint();
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Spinner {
            t: 0.0,
            color: theme::TEXT_COLOR.into(),
        }
    }
}

impl Widget for Spinner {
    fn on_event(&mut self, ctx: &mut EventCtx, event: &Event, _env: &Env) {
        ctx.init();
        if let Event::AnimFrame(interval) = event {
            self.t += (*interval as f64) * 1e-9;
            if self.t >= 1.0 {
                self.t = 0.0;
            }
            ctx.request_anim_frame();
            ctx.request_paint();
        }
    }

    fn on_status_change(&mut self, _ctx: &mut LifeCycleCtx, _event: &StatusChange, _env: &Env) {}

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, _env: &Env) {
        ctx.init();
        if let LifeCycle::WidgetAdded = event {
            ctx.request_anim_frame();
            ctx.request_paint();
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, env: &Env) -> Size {
        ctx.init();
        let size = if bc.is_width_bounded() && bc.is_height_bounded() {
            bc.max()
        } else {
            bc.constrain(Size::new(
                env.get(theme::BASIC_WIDGET_HEIGHT),
                env.get(theme::BASIC_WIDGET_HEIGHT),
            ))
        };

        trace!("Computed size: {}", size);
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx, env: &Env) {
        ctx.init();
        let t = self.t;
        let (width, height) = (ctx.size().width, ctx.size().height);
        let center = Point::new(width / 2.0, height / 2.0);
        let (r, g, b, original_alpha) = Color::as_rgba(&self.color.resolve(env));
        let scale_factor = width.min(height) / 40.0;

        for step in 1..=12 {
            let step = f64::from(step);
            let fade_t = (t * 12.0 + 1.0).trunc();
            let fade = ((fade_t + step).rem_euclid(12.0) / 12.0) + 1.0 / 12.0;
            let angle = Vec2::from_angle((step / 12.0) * -2.0 * PI);
            let ambit_start = center + (10.0 * scale_factor * angle);
            let ambit_end = center + (20.0 * scale_factor * angle);
            let color = Color::rgba(r, g, b, fade * original_alpha);

            ctx.stroke(
                Line::new(ambit_start, ambit_end),
                &color,
                3.0 * scale_factor,
            );
        }
    }

    fn children(&self) -> SmallVec<[WidgetRef<'_, dyn Widget>; 16]> {
        SmallVec::new()
    }
}

use crate::widget::StoreInWidgetMut;
impl StoreInWidgetMut for Spinner {
    type Mut<'a, 'b: 'a> = SpinnerMut<'a, 'b>;

    fn get_widget_and_ctx<'s: 'r, 'a: 'r, 'b: 'a, 'r>(
        widget_mut: &'s mut Self::Mut<'a, 'b>,
    ) -> (&'r mut Self, &'r mut WidgetCtx<'a, 'b>) {
        (widget_mut.1, &mut widget_mut.0)
    }

    fn from_widget_and_ctx<'a, 'b>(
        widget: &'a mut Self,
        ctx: WidgetCtx<'a, 'b>,
    ) -> Self::Mut<'a, 'b> {
        SpinnerMut(ctx, widget)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assert_render_snapshot;
    use crate::testing::Harness;
    //use instant::Duration;

    #[test]
    fn simple_spinner() {
        let spinner = Spinner::new();

        let mut harness = Harness::create(spinner);
        assert_render_snapshot!(harness, "spinner_init");

        // TODO
        //harness.move_timers_forward(Duration::from_millis(700));
        //assert_render_snapshot!(harness, "spinner_700ms");
    }

    #[test]
    fn edit_spinner() {
        let image_1 = {
            let spinner = Spinner::new().with_color(Color::PURPLE);

            let mut harness = Harness::create_with_size(spinner, Size::new(30.0, 30.0));
            harness.render()
        };

        let image_2 = {
            let spinner = Spinner::new();

            let mut harness = Harness::create_with_size(spinner, Size::new(30.0, 30.0));

            harness.edit_root_widget(|mut spinner, _| {
                let mut spinner = spinner.downcast::<Spinner>().unwrap();
                spinner.set_color(Color::PURPLE);
            });

            harness.render()
        };

        // We don't use assert_eq because we don't want rich assert
        assert!(image_1 == image_2);
    }
}
