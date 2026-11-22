//! Wayland events, each handed to the state. Nothing is decided or drawn here.

use wayland_client::globals::GlobalListContents;
use wayland_client::protocol::{
    wl_buffer::{self, WlBuffer},
    wl_compositor::WlCompositor,
    wl_output::{self, WlOutput},
    wl_pointer::{self, WlPointer},
    wl_region::WlRegion,
    wl_registry::{self, WlRegistry},
    wl_seat::{self, WlSeat},
    wl_shm::WlShm,
    wl_shm_pool::WlShmPool,
    wl_surface::{self, WlSurface},
};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum, delegate_noop};
use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1::{self, ExtBackgroundEffectManagerV1},
    ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1},
    wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    wp_fractional_scale_v1::{self, WpFractionalScaleV1},
};
use wayland_protocols::wp::viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter};
use wayland_protocols::xdg::xdg_output::zv1::client::{
    zxdg_output_manager_v1::ZxdgOutputManagerV1,
    zxdg_output_v1::{self, ZxdgOutputV1},
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::ZwlrLayerShellV1,
    zwlr_layer_surface_v1::{self, ZwlrLayerSurfaceV1},
};

use super::buffers::Slot;
use super::surface::{Pointer, Role, State};

const LEFT_BUTTON: u32 = 0x110;
/// No real scale is outside a half to four. It bounds the bitmap's size.
const SCALES: std::ops::RangeInclusive<u32> = 60..=480;

impl Dispatch<WlRegistry, GlobalListContents> for State {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global { name, interface, version } if interface == WlOutput::interface().name => {
                state.add_monitor(registry, name, version);
            }
            wl_registry::Event::GlobalRemove { name } => state.remove_monitor(name),
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, ()> for State {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: wl_output::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event
            && let Some(monitor) = state.monitors.iter_mut().find(|one| one.output == *output)
        {
            monitor.name = name;
        }
    }
}

impl Dispatch<ZxdgOutputV1, WlOutput> for State {
    fn event(
        state: &mut Self,
        _: &ZxdgOutputV1,
        event: zxdg_output_v1::Event,
        output: &WlOutput,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zxdg_output_v1::Event::LogicalSize { width, height } = event
            && let Some(monitor) = state.monitors.iter_mut().find(|one| one.output == *output)
        {
            monitor.size = Some((width, height));
        }
    }
}

impl Dispatch<WlSurface, Role> for State {
    fn event(
        state: &mut Self,
        _: &WlSurface,
        event: wl_surface::Event,
        role: &Role,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let (Role::Pill, wl_surface::Event::Enter { output }) = (role, event)
            && state.on.is_none()
            && state.pill.is_some()
        {
            state.on = Some(output);
        }
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, Role> for State {
    fn event(
        state: &mut Self,
        layer: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        role: &Role,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match (event, role) {
            (zwlr_layer_surface_v1::Event::Configure { serial, width, height }, role) => {
                layer.ack_configure(serial);
                match role {
                    Role::Pill => {
                        if let Some(pill) = state.pill.as_mut() {
                            pill.ready = true;
                        }
                    }
                    Role::Catcher => {
                        if let (Some(catcher), Ok(width @ 1..), Ok(height @ 1..)) =
                            (state.catcher.as_mut(), i32::try_from(width), i32::try_from(height))
                        {
                            catcher.size = Some((width, height));
                        }
                    }
                }
            }
            (zwlr_layer_surface_v1::Event::Closed, _) => state.closed(),
            _ => {}
        }
    }
}

impl Dispatch<ExtBackgroundEffectManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ExtBackgroundEffectManagerV1,
        event: ext_background_effect_manager_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_background_effect_manager_v1::Event::Capabilities { flags } = event {
            let blur = matches!(
                flags,
                WEnum::Value(flags) if flags.contains(ext_background_effect_manager_v1::Capability::Blur)
            );
            // Only losing it is worth a line. Having it at the start would mark every timer's start.
            if state.blur && !blur {
                crate::log::warn("glass.blur_off", "blur was switched off: the glass is nearly solid");
            }
            state.blur = blur;
        }
    }
}

impl Dispatch<WpFractionalScaleV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &WpFractionalScaleV1,
        event: wp_fractional_scale_v1::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = event {
            state.scale = scale.clamp(*SCALES.start(), *SCALES.end());
        }
    }
}

impl Dispatch<WlBuffer, Slot> for State {
    fn event(
        state: &mut Self,
        _: &WlBuffer,
        event: wl_buffer::Event,
        slot: &Slot,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let (wl_buffer::Event::Release, Some(buffers)) = (event, state.buffers.as_mut()) {
            buffers.released(*slot);
        }
    }
}

impl Dispatch<WlSeat, ()> for State {
    fn event(
        state: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        (): &(),
        _: &Connection,
        handle: &QueueHandle<Self>,
    ) {
        let wl_seat::Event::Capabilities { capabilities: WEnum::Value(can) } = event else {
            return;
        };
        if !can.contains(wl_seat::Capability::Pointer) {
            state.release();
            state.pointer = None;
        } else if state.pointer.is_none() {
            let pointer = seat.get_pointer(handle, ());
            let cursor = state.shell.cursors.as_ref().map(|cursors| cursors.get_pointer(&pointer, handle, ()));
            state.pointer = Some(Pointer { device: pointer, cursor, serial: 0, at: (0.0, 0.0) });
        }
    }
}

impl Dispatch<WlPointer, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        (): &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            // Only the catcher takes input, so every position here is on the monitor.
            wl_pointer::Event::Enter { serial, surface_x, surface_y, .. } => {
                if let Some(pointer) = state.pointer.as_mut() {
                    pointer.serial = serial;
                    pointer.at = (surface_x, surface_y);
                }
                state.show_cursor(Shape::Grab);
            }
            wl_pointer::Event::Motion { surface_x, surface_y, .. } => {
                state.pointer_moved((surface_x, surface_y));
            }
            wl_pointer::Event::Button { button: LEFT_BUTTON, state: WEnum::Value(pressed), .. } => {
                if pressed == wl_pointer::ButtonState::Pressed {
                    state.press();
                } else {
                    state.release();
                }
            }
            wl_pointer::Event::Leave { .. } => state.release(),
            _ => {}
        }
    }
}

delegate_noop!(State: ignore WlCompositor);
delegate_noop!(State: ignore WlRegion);
delegate_noop!(State: ignore WlShm);
delegate_noop!(State: ignore WlShmPool);
// The catcher's one pixel: never reused, so its release doesn't matter.
delegate_noop!(State: ignore WlBuffer);
delegate_noop!(State: ignore ZwlrLayerShellV1);
delegate_noop!(State: ignore WpViewporter);
delegate_noop!(State: ignore WpViewport);
delegate_noop!(State: ignore WpFractionalScaleManagerV1);
delegate_noop!(State: ignore ExtBackgroundEffectSurfaceV1);
delegate_noop!(State: ignore ZxdgOutputManagerV1);
delegate_noop!(State: ignore WpCursorShapeManagerV1);
delegate_noop!(State: ignore WpCursorShapeDeviceV1);
