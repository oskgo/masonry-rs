// Copyright 2019 The Druid Authors.

//! An image widget loaded from a URL.

use crate::promise::PromiseToken;
use crate::widget::prelude::*;
use crate::widget::WidgetRef;
use crate::widget::{FillStrat, Image, SizedBox, Spinner, WidgetPod};
use crate::{ImageBuf, Point};

use smallvec::{smallvec, SmallVec};
use tracing::{error, trace_span, Span};

pub struct WebImage {
    url: String,
    inner: Option<WidgetPod<Image>>,
    image_promise: PromiseToken<ImageBuf>,
    placeholder: WidgetPod<SizedBox>,
}

impl WebImage {
    pub fn new(url: String) -> Self {
        Self {
            url,
            inner: None,
            image_promise: PromiseToken::empty(),
            placeholder: WidgetPod::new(SizedBox::new(Spinner::new())),
        }
    }
}

// --- TRAIT IMPLS ---

impl Widget for WebImage {
    fn on_event(&mut self, ctx: &mut EventCtx, event: &Event, env: &Env) {
        ctx.init();
        match event {
            Event::PromiseResult(result) => {
                if let Some(image_buf) = result.try_get(self.image_promise) {
                    self.inner = Some(WidgetPod::new(
                        Image::new(image_buf).fill_mode(FillStrat::Contain),
                    ));
                    ctx.children_changed();
                    ctx.skip_child(&mut self.placeholder);
                    return;
                }
            }
            _ => {}
        }
        if let Some(inner) = &mut self.inner {
            inner.on_event(ctx, event, env)
        } else {
            self.placeholder.on_event(ctx, event, env)
        }
    }

    fn on_status_change(&mut self, _ctx: &mut LifeCycleCtx, _event: &StatusChange, _env: &Env) {}

    fn lifecycle(&mut self, ctx: &mut LifeCycleCtx, event: &LifeCycle, env: &Env) {
        fn load_image(url: &str) -> Option<ImageBuf> {
            let response = match reqwest::blocking::get(url) {
                Ok(response) => response,
                Err(err) => {
                    error!("Cannot load image at '{}': {}", url, err);
                    return None;
                }
            };
            let body = match response.bytes() {
                Ok(body) => body,
                Err(err) => {
                    error!("Cannot load image at '{}': {}", url, err);
                    return None;
                }
            };
            let image_buf = match ImageBuf::from_data(&body) {
                Ok(image_buf) => image_buf,
                Err(err) => {
                    error!("Cannot parse image at '{}': {}", url, err);
                    return None;
                }
            };
            Some(image_buf)
        }

        ctx.init();
        match event {
            LifeCycle::WidgetAdded => {
                let url = self.url.clone();
                self.image_promise = ctx
                    .compute_in_background(move |_| load_image(&url).unwrap_or(ImageBuf::empty()));
            }
            _ => {}
        }

        if let Some(inner) = &mut self.inner {
            inner.lifecycle(ctx, event, env)
        } else {
            self.placeholder.lifecycle(ctx, event, env)
        }
    }

    fn layout(&mut self, ctx: &mut LayoutCtx, bc: &BoxConstraints, env: &Env) -> Size {
        ctx.init();

        if let Some(inner) = &mut self.inner {
            let layout = inner.layout(ctx, bc, env);
            ctx.place_child(inner, Point::ORIGIN, env);
            layout
        } else {
            let layout = self.placeholder.layout(ctx, bc, env);
            ctx.place_child(&mut self.placeholder, Point::ORIGIN, env);
            layout
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, env: &Env) {
        ctx.init();

        if let Some(inner) = &mut self.inner {
            inner.paint(ctx, env)
        } else {
            self.placeholder.paint(ctx, env)
        }
    }

    fn children(&self) -> SmallVec<[WidgetRef<'_, dyn Widget>; 16]> {
        if let Some(inner) = &self.inner {
            smallvec![inner.as_dyn()]
        } else {
            smallvec![self.placeholder.as_dyn()]
        }
    }

    fn make_trace_span(&self) -> Span {
        trace_span!("WebImage")
    }
}