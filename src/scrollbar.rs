use std::{cell::RefCell, rc::Rc};

use gpui::{
    Bounds, Context, CursorStyle, DispatchPhase, Entity, MouseButton, MouseMoveEvent, MouseUpEvent,
    Pixels, Render, ScrollHandle, canvas, div, fill, point, prelude::*, px, rgb, size,
};

const TRACK_WIDTH: Pixels = px(12.0);
const THUMB_WIDTH: Pixels = px(6.0);
const TRACK_PADDING: f32 = 3.0;
const MIN_THUMB_HEIGHT: f32 = 24.0;

#[derive(Clone)]
pub struct VerticalScroll {
    handle: ScrollHandle,
    drag: Rc<RefCell<Option<ScrollbarDrag>>>,
}

#[derive(Clone, Copy)]
struct ScrollbarDrag {
    grab_offset: Pixels,
}

#[derive(Clone, Copy)]
struct ThumbMetrics {
    top: f32,
    height: f32,
    travel: f32,
}

impl VerticalScroll {
    pub fn new() -> Self {
        Self {
            handle: ScrollHandle::new(),
            drag: Rc::new(RefCell::new(None)),
        }
    }

    pub fn handle(&self) -> &ScrollHandle {
        &self.handle
    }

    pub fn bar<V: Render>(&self, id: &'static str, cx: &mut Context<V>) -> impl IntoElement {
        let handle = self.handle.clone();
        let drag = self.drag.clone();
        let view = cx.entity();

        div()
            .id(id)
            .absolute()
            .top_0()
            .right_0()
            .w(TRACK_WIDTH)
            .h_full()
            .block_mouse_except_scroll()
            .cursor(CursorStyle::Arrow)
            .on_mouse_down(MouseButton::Left, {
                let handle = handle.clone();
                let drag = drag.clone();
                let view = view.clone();
                move |event, _, cx| {
                    cx.stop_propagation();
                    let Some(metrics) = metrics_for(&handle) else {
                        return;
                    };
                    let thumb_top = handle.bounds().top() + px(metrics.top);
                    let grab_offset = if event.position.y >= thumb_top
                        && event.position.y <= thumb_top + px(metrics.height)
                    {
                        event.position.y - thumb_top
                    } else {
                        px(metrics.height / 2.0)
                    };
                    drag.borrow_mut().replace(ScrollbarDrag { grab_offset });
                    scroll_to_thumb_top(&handle, event.position.y - grab_offset, &metrics);
                    cx.notify(view.entity_id());
                }
            })
            .child(scrollbar_canvas(handle, drag, view))
    }
}

fn scrollbar_canvas<V: Render>(
    handle: ScrollHandle,
    drag: Rc<RefCell<Option<ScrollbarDrag>>>,
    view: Entity<V>,
) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _cx| {
            let Some(metrics) = thumb_metrics(
                f32::from(bounds.size.height),
                f32::from(handle.bounds().size.height),
                f32::from(handle.max_offset().y),
                f32::from(handle.offset().y),
            ) else {
                return;
            };

            let thumb = Bounds::new(
                point(bounds.left() + px(3.0), bounds.top() + px(metrics.top)),
                size(THUMB_WIDTH, px(metrics.height)),
            );
            window.paint_quad(fill(thumb, rgb(0x5c6370)));

            window.on_mouse_event({
                let handle = handle.clone();
                let drag = drag.clone();
                let view = view.clone();
                move |event: &MouseMoveEvent, phase, _, cx| {
                    if phase != DispatchPhase::Bubble || !event.dragging() {
                        return;
                    }
                    let Some(current_drag) = *drag.borrow() else {
                        return;
                    };
                    let Some(metrics) = metrics_for(&handle) else {
                        return;
                    };
                    scroll_to_thumb_top(
                        &handle,
                        event.position.y - current_drag.grab_offset,
                        &metrics,
                    );
                    cx.notify(view.entity_id());
                }
            });

            window.on_mouse_event({
                let drag = drag.clone();
                move |_: &MouseUpEvent, phase, _, _| {
                    if phase == DispatchPhase::Bubble {
                        drag.borrow_mut().take();
                    }
                }
            });
        },
    )
    .size_full()
}

fn metrics_for(handle: &ScrollHandle) -> Option<ThumbMetrics> {
    thumb_metrics(
        f32::from(handle.bounds().size.height),
        f32::from(handle.bounds().size.height),
        f32::from(handle.max_offset().y),
        f32::from(handle.offset().y),
    )
}

fn scroll_to_thumb_top(handle: &ScrollHandle, thumb_top: Pixels, metrics: &ThumbMetrics) {
    let min_top = handle.bounds().top() + px(TRACK_PADDING);
    let travel = px(metrics.travel);
    let clamped = thumb_top.clamp(min_top, min_top + travel);
    let progress = if metrics.travel > 0.0 {
        (clamped - min_top) / travel
    } else {
        0.0
    };
    let mut offset = handle.offset();
    offset.y = -handle.max_offset().y * progress;
    handle.set_offset(offset);
}

fn thumb_metrics(
    track_height: f32,
    viewport_height: f32,
    max_offset: f32,
    offset_y: f32,
) -> Option<ThumbMetrics> {
    if track_height <= 0.0 || viewport_height <= 0.0 || max_offset <= 0.0 {
        return None;
    }

    let inner_height = (track_height - TRACK_PADDING * 2.0).max(1.0);
    let content_height = viewport_height + max_offset;
    let height = (inner_height * (viewport_height / content_height))
        .clamp(MIN_THUMB_HEIGHT.min(inner_height), inner_height);
    let travel = (inner_height - height).max(0.0);
    let progress = (-offset_y / max_offset).clamp(0.0, 1.0);
    Some(ThumbMetrics {
        top: TRACK_PADDING + travel * progress,
        height,
        travel,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_when_content_fits() {
        assert!(thumb_metrics(200.0, 200.0, 0.0, 0.0).is_none());
    }

    #[test]
    fn sizes_thumb_from_visible_ratio() {
        let metrics = thumb_metrics(100.0, 100.0, 100.0, 0.0).unwrap();
        assert!((metrics.height - 47.0).abs() < 0.01);
        assert!((metrics.top - TRACK_PADDING).abs() < 0.01);
    }

    #[test]
    fn moves_thumb_with_scroll_offset() {
        let start = thumb_metrics(100.0, 100.0, 100.0, 0.0).unwrap();
        let end = thumb_metrics(100.0, 100.0, 100.0, -100.0).unwrap();
        assert!(end.top > start.top);
        assert!((end.top - (TRACK_PADDING + end.travel)).abs() < 0.01);
    }
}
