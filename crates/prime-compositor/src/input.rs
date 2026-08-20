use crate::Runtime;
use smithay::{
    backend::input::{
        Axis, AxisSource, Event, InputBackend, InputEvent, KeyboardKeyEvent, PointerAxisEvent,
        PointerButtonEvent, PointerMotionEvent,
    },
    desktop::{find_popup_root_surface, layer_map_for_output, WindowSurfaceType},
    input::{
        keyboard::FilterResult,
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
    },
    reexports::wayland_server::protocol::{wl_pointer, wl_surface::WlSurface},
    utils::{Logical, Point, SERIAL_COUNTER},
    wayland::compositor::get_parent,
};
use std::convert::TryInto;

pub(crate) fn process_input_event<B: InputBackend>(runtime: &mut Runtime, event: InputEvent<B>) {
    runtime.readiness.input_events_seen = runtime.readiness.input_events_seen.saturating_add(1);

    match event {
        InputEvent::Keyboard { event, .. } => keyboard_key::<B>(runtime, event),
        InputEvent::PointerMotion { event, .. } => pointer_motion::<B>(runtime, event),
        InputEvent::PointerButton { event, .. } => pointer_button::<B>(runtime, event),
        InputEvent::PointerAxis { event, .. } => pointer_axis::<B>(runtime, event),
        _ => {}
    }
}

fn keyboard_key<B: InputBackend>(runtime: &mut Runtime, event: B::KeyboardKeyEvent) {
    let keyboard = runtime.protocols.keyboard.clone();
    let _ = keyboard.input(
        runtime,
        event.key_code(),
        event.state(),
        SERIAL_COUNTER.next_serial(),
        event.time_msec(),
        |_, _, _| FilterResult::<()>::Forward,
    );
}

fn pointer_motion<B: InputBackend>(runtime: &mut Runtime, event: B::PointerMotionEvent) {
    let pointer = runtime.protocols.pointer.clone();
    let mut location = pointer.current_location();
    location += event.delta();
    location = clamp_to_output(runtime, location);
    let under = surface_under(runtime, location);

    pointer.motion(
        runtime,
        under,
        &MotionEvent {
            location,
            serial: SERIAL_COUNTER.next_serial(),
            time: event.time_msec(),
        },
    );
    pointer.frame(runtime);
}

fn pointer_button<B: InputBackend>(runtime: &mut Runtime, event: B::PointerButtonEvent) {
    let serial = SERIAL_COUNTER.next_serial();
    let pointer = runtime.protocols.pointer.clone();
    let state = wl_pointer::ButtonState::from(event.state());

    if state == wl_pointer::ButtonState::Pressed {
        let focus = keyboard_focus_under(runtime, pointer.current_location());
        runtime
            .protocols
            .keyboard
            .clone()
            .set_focus(runtime, focus, serial);
    }

    let Ok(state) = state.try_into() else {
        return;
    };
    pointer.button(
        runtime,
        &ButtonEvent {
            button: event.button_code(),
            state,
            serial,
            time: event.time_msec(),
        },
    );
    pointer.frame(runtime);
}

fn pointer_axis<B: InputBackend>(runtime: &mut Runtime, event: B::PointerAxisEvent) {
    let horizontal = event
        .amount(Axis::Horizontal)
        .unwrap_or_else(|| event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.0);
    let vertical = event
        .amount(Axis::Vertical)
        .unwrap_or_else(|| event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.0);

    let mut frame = AxisFrame::new(event.time_msec()).source(event.source());
    if horizontal != 0.0 {
        frame = frame
            .relative_direction(Axis::Horizontal, event.relative_direction(Axis::Horizontal))
            .value(Axis::Horizontal, horizontal);
        if let Some(v120) = event.amount_v120(Axis::Horizontal) {
            frame = frame.v120(Axis::Horizontal, v120 as i32);
        }
    }
    if vertical != 0.0 {
        frame = frame
            .relative_direction(Axis::Vertical, event.relative_direction(Axis::Vertical))
            .value(Axis::Vertical, vertical);
        if let Some(v120) = event.amount_v120(Axis::Vertical) {
            frame = frame.v120(Axis::Vertical, v120 as i32);
        }
    }
    if event.source() == AxisSource::Finger {
        if event.amount(Axis::Horizontal) == Some(0.0) {
            frame = frame.stop(Axis::Horizontal);
        }
        if event.amount(Axis::Vertical) == Some(0.0) {
            frame = frame.stop(Axis::Vertical);
        }
    }

    let pointer = runtime.protocols.pointer.clone();
    pointer.axis(runtime, frame);
    pointer.frame(runtime);
}

fn surface_under(
    runtime: &Runtime,
    location: Point<f64, Logical>,
) -> Option<(WlSurface, Point<f64, Logical>)> {
    let output_geometry = runtime.protocols.space.output_geometry(&runtime._output)?;
    if !output_geometry.contains(location.to_i32_round()) {
        return None;
    }
    let output_local = location - output_geometry.loc.to_f64();
    let map = layer_map_for_output(&runtime._output);

    if let Some(hit) = layer_hit(
        &map,
        smithay::wayland::shell::wlr_layer::Layer::Overlay,
        output_local,
        output_geometry.loc,
    )
    .or_else(|| {
        layer_hit(
            &map,
            smithay::wayland::shell::wlr_layer::Layer::Top,
            output_local,
            output_geometry.loc,
        )
    }) {
        return Some(hit);
    }

    if let Some(hit) =
        runtime
            .protocols
            .space
            .element_under(location)
            .and_then(|(window, window_location)| {
                window
                    .surface_under(location - window_location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(surface, surface_location)| {
                        (surface, (surface_location + window_location).to_f64())
                    })
            })
    {
        return Some(hit);
    }

    layer_hit(
        &map,
        smithay::wayland::shell::wlr_layer::Layer::Bottom,
        output_local,
        output_geometry.loc,
    )
    .or_else(|| {
        layer_hit(
            &map,
            smithay::wayland::shell::wlr_layer::Layer::Background,
            output_local,
            output_geometry.loc,
        )
    })
}

fn layer_hit(
    map: &smithay::desktop::LayerMap,
    layer_kind: smithay::wayland::shell::wlr_layer::Layer,
    output_local: Point<f64, Logical>,
    output_location: Point<i32, Logical>,
) -> Option<(WlSurface, Point<f64, Logical>)> {
    let layer = map.layer_under(layer_kind, output_local)?;
    let layer_location = map.layer_geometry(layer)?.loc;
    layer
        .surface_under(
            output_local - layer_location.to_f64(),
            WindowSurfaceType::ALL,
        )
        .map(|(surface, surface_location)| {
            (
                surface,
                (surface_location + layer_location + output_location).to_f64(),
            )
        })
}

fn keyboard_focus_under(runtime: &Runtime, location: Point<f64, Logical>) -> Option<WlSurface> {
    let (surface, _) = surface_under(runtime, location)?;
    let map = layer_map_for_output(&runtime._output);
    if let Some(layer) = map.layer_for_surface(&surface, WindowSurfaceType::ALL) {
        return layer
            .can_receive_keyboard_focus()
            .then(|| layer.wl_surface().clone());
    }

    let mut root = surface;
    while let Some(parent) = get_parent(&root) {
        root = parent;
    }
    if let Some(popup) = runtime.protocols.popups.find_popup(&root) {
        return find_popup_root_surface(&popup).ok();
    }
    Some(root)
}

fn clamp_to_output(runtime: &Runtime, mut location: Point<f64, Logical>) -> Point<f64, Logical> {
    if let Some(mode) = runtime._output.current_mode() {
        let max_x = f64::from(mode.size.w.saturating_sub(1));
        let max_y = f64::from(mode.size.h.saturating_sub(1));
        location.x = location.x.clamp(0.0, max_x);
        location.y = location.y.clamp(0.0, max_y);
    }
    location
}
