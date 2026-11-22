//! The glass's surfaces.
//!
//! There are two. The **pill** is what you see, and no click ever lands on it.
//! While the glass is movable there is also a **catcher**: an invisible surface
//! over the whole monitor that takes clicks only where the pill is. Dragging
//! moves the pill under a catcher that stays put, so pointer positions never
//! shift mid-drag.

use std::fs::File;
use std::os::fd::AsFd;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use wayland_client::globals::GlobalList;
use wayland_client::protocol::{
    wl_buffer::WlBuffer, wl_compositor::WlCompositor, wl_output::WlOutput, wl_pointer::WlPointer,
    wl_registry::WlRegistry, wl_seat::WlSeat, wl_shm, wl_surface::WlSurface,
};
use wayland_client::{Proxy, QueueHandle};
use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1::ExtBackgroundEffectManagerV1,
    ext_background_effect_surface_v1::ExtBackgroundEffectSurfaceV1,
};
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1},
    wp_cursor_shape_manager_v1::WpCursorShapeManagerV1,
};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    wp_fractional_scale_v1::WpFractionalScaleV1,
};
use wayland_protocols::wp::viewporter::client::{
    wp_viewport::WpViewport, wp_viewporter::WpViewporter,
};
use wayland_protocols::xdg::xdg_output::zv1::client::{
    zxdg_output_manager_v1::ZxdgOutputManagerV1, zxdg_output_v1::ZxdgOutputV1,
};
use wayland_protocols_wlr::layer_shell::v1::client::{
    zwlr_layer_shell_v1::{Layer, ZwlrLayerShellV1},
    zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity, ZwlrLayerSurfaceV1},
};

use super::Showing;
use super::buffers::Buffers;
use super::paint::{self, Face, SCALE_ONE};
use super::palette::Look;
use super::place::{self, Pair, Spot};
use super::shape::{self, HEIGHT, WIDTH};

/// KDE keeps "dock" surfaces up during Show Desktop. Any other name gets hidden.
const SCOPE: &str = "dock";
/// Moves are sent at most this often, however fast the mouse reports.
const MOVE_EVERY: Duration = Duration::from_millis(8);
/// How long to wait before coming back after the compositor closed the glass.
const COME_BACK: Duration = Duration::from_secs(1);

/// Which surface an event is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    Pill,
    Catcher,
}

/// The shared objects, bound once.
pub(super) struct Shell {
    pub handle: QueueHandle<State>,
    compositor: WlCompositor,
    shm: wl_shm::WlShm,
    layers: ZwlrLayerShellV1,
    viewporter: WpViewporter,
    effects: Option<ExtBackgroundEffectManagerV1>,
    scales: Option<WpFractionalScaleManagerV1>,
    pub outputs: Option<ZxdgOutputManagerV1>,
    pub cursors: Option<WpCursorShapeManagerV1>,
}

pub(super) struct Monitor {
    pub global: u32,
    pub output: WlOutput,
    pub info: Option<ZxdgOutputV1>,
    pub name: String,
    pub size: Option<Pair>,
}

impl Drop for Monitor {
    fn drop(&mut self) {
        if let Some(info) = &self.info {
            info.destroy();
        }
        self.output.release();
    }
}

pub(super) struct Pill {
    surface: WlSurface,
    layer: ZwlrLayerSurfaceV1,
    viewport: WpViewport,
    effect: Option<ExtBackgroundEffectSurfaceV1>,
    fractional: Option<WpFractionalScaleV1>,
    pub ready: bool,
    drawn: Option<Face>,
    placed: Option<Pair>,
}

impl Drop for Pill {
    fn drop(&mut self) {
        if let Some(effect) = &self.effect {
            effect.destroy();
        }
        if let Some(fractional) = &self.fractional {
            fractional.destroy();
        }
        self.viewport.destroy();
        self.layer.destroy();
        self.surface.destroy();
    }
}

pub(super) struct Catcher {
    surface: WlSurface,
    layer: ZwlrLayerSurfaceV1,
    viewport: WpViewport,
    clear: Option<WlBuffer>,
    /// Its size, once the compositor gave it one.
    pub size: Option<Pair>,
    /// The size and pill position it was last set up for.
    set_for: Option<(Pair, Pair)>,
}

impl Drop for Catcher {
    fn drop(&mut self) {
        self.viewport.destroy();
        self.layer.destroy();
        self.surface.destroy();
        if let Some(clear) = &self.clear {
            clear.destroy();
        }
    }
}

pub(super) struct Pointer {
    pub device: WlPointer,
    pub cursor: Option<WpCursorShapeDeviceV1>,
    pub serial: u32,
    pub at: (f64, f64),
}

impl Drop for Pointer {
    fn drop(&mut self) {
        if let Some(cursor) = &self.cursor {
            cursor.destroy();
        }
        self.device.release();
    }
}

struct Drag {
    from: (f64, f64),
    start: Pair,
    point: Pair,
    moved: bool,
    sent: Option<Instant>,
}

pub(super) struct State {
    pub shell: Shell,
    pub showing: Showing,
    pub look: Look,
    pub movable: bool,
    pub blur: bool,
    pub scale: u32,
    pub monitors: Vec<Monitor>,
    /// The monitor the pill is on, once known.
    pub on: Option<WlOutput>,
    pub pill: Option<Pill>,
    pub catcher: Option<Catcher>,
    pub pointer: Option<Pointer>,
    pub buffers: Option<Buffers>,
    generation: u32,
    spot: Option<Spot>,
    file: Option<PathBuf>,
    drag: Option<Drag>,
    reopen: Option<Instant>,
    // Kept alive for its events.
    _seat: Option<WlSeat>,
}

impl State {
    pub(super) fn new(
        globals: &GlobalList,
        handle: &QueueHandle<Self>,
        showing: Showing,
        look: Look,
        file: Option<PathBuf>,
    ) -> Result<Self> {
        let shell = Shell {
            compositor: globals.bind(handle, 4..=4, ()).context("no compositor")?,
            shm: globals
                .bind(handle, 1..=1, ())
                .context("no shared memory")?,
            layers: globals
                .bind(handle, 2..=4, ())
                .context("this desktop can't float a surface on top (no layer shell)")?,
            viewporter: globals.bind(handle, 1..=1, ()).context("no viewporter")?,
            effects: globals.bind(handle, 1..=1, ()).ok(),
            scales: globals.bind(handle, 1..=1, ()).ok(),
            outputs: globals.bind(handle, 2..=3, ()).ok(),
            cursors: globals.bind(handle, 1..=1, ()).ok(),
            handle: handle.clone(),
        };
        let spot = file.as_deref().and_then(Spot::load);
        let mut state = Self {
            shell,
            showing,
            look,
            movable: false,
            blur: false,
            scale: SCALE_ONE,
            monitors: Vec::new(),
            on: None,
            pill: None,
            catcher: None,
            pointer: None,
            buffers: None,
            generation: 0,
            spot,
            file,
            drag: None,
            reopen: None,
            _seat: globals.bind(handle, 5..=5, ()).ok(),
        };
        for global in globals.contents().clone_list() {
            if global.interface == WlOutput::interface().name {
                state.add_monitor(globals.registry(), global.name, global.version);
            }
        }
        Ok(state)
    }

    pub(super) fn add_monitor(&mut self, registry: &WlRegistry, global: u32, version: u32) {
        // Version 4 gives monitors names. Version 3 is the first that can be released.
        if version < 3 {
            return;
        }
        let handle = &self.shell.handle;
        let output: WlOutput = registry.bind(global, version.min(4), handle, ());
        let info = self
            .shell
            .outputs
            .as_ref()
            .map(|manager| manager.get_xdg_output(&output, handle, output.clone()));
        self.monitors.push(Monitor {
            global,
            output,
            info,
            name: String::new(),
            size: None,
        });
    }

    pub(super) fn remove_monitor(&mut self, global: u32) {
        let Some(index) = self
            .monitors
            .iter()
            .position(|monitor| monitor.global == global)
        else {
            return;
        };
        if self.on.as_ref() == Some(&self.monitors[index].output) {
            self.closed();
        }
        self.monitors.remove(index);
    }

    /// The compositor took the glass away. It comes back by itself.
    pub(super) fn closed(&mut self) {
        self.drag = None;
        self.catcher = None;
        self.pill = None;
        self.on = None;
        self.reopen = Some(Instant::now() + COME_BACK);
    }

    fn here(&self) -> Option<(&Monitor, Pair)> {
        let on = self.on.as_ref()?;
        let monitor = self.monitors.iter().find(|monitor| monitor.output == *on)?;
        Some((monitor, monitor.size?))
    }

    /// Where the pill belongs now. `None` until its monitor is known: then it's top centre.
    fn point(&self) -> Option<Pair> {
        let (_, screen) = self.here()?;
        if let Some(drag) = &self.drag {
            return Some(drag.point);
        }
        Some(
            self.spot
                .as_ref()
                .map_or_else(|| place::home(screen), |spot| spot.point(screen)),
        )
    }

    /// How long until this needs to run again without an event.
    pub(super) fn due(&self, now: Instant) -> Option<Duration> {
        if let Some(reopen) = self.reopen {
            return Some(reopen.saturating_duration_since(now));
        }
        let drag = self.drag.as_ref()?;
        let waiting = self.pill.as_ref()?.placed != Some(drag.point);
        let sent = drag.sent?;
        waiting.then(|| (sent + MOVE_EVERY).saturating_duration_since(now))
    }

    /// Brings what is on screen up to date with the state.
    pub(super) fn update(&mut self) -> Result<()> {
        let now = Instant::now();
        if self.reopen.is_some_and(|reopen| now < reopen) {
            return Ok(());
        }
        self.reopen = None;
        if self.pill.is_none() {
            self.open_pill();
            return Ok(());
        }
        self.update_catcher()?;

        let point = self.point();
        let face = Face::new(self.showing, self.look, self.blur, self.movable, self.scale);
        let Some(pill) = self.pill.as_mut() else {
            return Ok(());
        };
        if !pill.ready {
            return Ok(());
        }
        let mut commit = false;
        if let Some(point) = point
            && pill.placed != Some(point)
        {
            let too_soon = self
                .drag
                .as_ref()
                .and_then(|drag| drag.sent)
                .is_some_and(|sent| now < sent + MOVE_EVERY);
            if !too_soon {
                pill.layer.set_anchor(Anchor::Top | Anchor::Left);
                pill.layer.set_margin(point.1, 0, 0, point.0);
                pill.placed = Some(point);
                commit = true;
                if let Some(drag) = self.drag.as_mut() {
                    drag.sent = Some(now);
                }
            }
        }
        if pill.drawn.as_ref() != Some(&face) {
            let size = face.size();
            if self
                .buffers
                .as_ref()
                .is_none_or(|buffers| buffers.size() != size)
            {
                self.generation = self.generation.wrapping_add(1);
                self.buffers = Some(Buffers::new(
                    &self.shell.shm,
                    &self.shell.handle,
                    size,
                    self.generation,
                )?);
            }
            // With both buffers busy, the release event brings us back here.
            if let Some(buffers) = self.buffers.as_mut()
                && let Some(free) = buffers.free()
            {
                let buffer = buffers.fill(free, &paint::paint(&face)?)?;
                pill.surface.attach(Some(buffer), 0, 0);
                pill.surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
                pill.drawn = Some(face);
                commit = true;
            }
        }
        if commit {
            pill.surface.commit();
        }
        Ok(())
    }

    fn open_pill(&mut self) {
        let shell = &self.shell;
        let wanted = self.spot.as_ref().and_then(|spot| {
            self.monitors
                .iter()
                .find(|monitor| !monitor.name.is_empty() && monitor.name == spot.monitor)
        });
        self.on = wanted.map(|monitor| monitor.output.clone());

        let handle = &shell.handle;
        let surface = shell.compositor.create_surface(handle, Role::Pill);
        let layer = shell.layers.get_layer_surface(
            &surface,
            self.on.as_ref(),
            Layer::Overlay,
            SCOPE.to_owned(),
            handle,
            Role::Pill,
        );
        layer.set_size(WIDTH.unsigned_abs(), HEIGHT.unsigned_abs());
        layer.set_anchor(Anchor::Top);
        layer.set_margin(place::GAP, 0, 0, 0);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(KeyboardInteractivity::None);

        // An empty region: every click goes to what's underneath.
        let nothing = shell.compositor.create_region(handle, ());
        surface.set_input_region(Some(&nothing));
        nothing.destroy();

        let effect = shell.effects.as_ref().map(|effects| {
            let effect = effects.get_background_effect(&surface, handle, ());
            let region = shell.compositor.create_region(handle, ());
            for (x, y, width) in shape::blur_strips() {
                region.add(x, y, width, 1);
            }
            effect.set_blur_region(Some(&region));
            region.destroy();
            effect
        });
        let fractional = shell
            .scales
            .as_ref()
            .map(|scales| scales.get_fractional_scale(&surface, handle, ()));
        let viewport = shell.viewporter.get_viewport(&surface, handle, ());
        viewport.set_destination(WIDTH, HEIGHT);
        surface.commit();
        self.pill = Some(Pill {
            surface,
            layer,
            viewport,
            effect,
            fractional,
            ready: false,
            drawn: None,
            placed: None,
        });
    }

    fn update_catcher(&mut self) -> Result<()> {
        let on = self.on.clone().filter(|_| self.movable);
        let (Some(on), Some(point)) = (on, self.point()) else {
            if self.catcher.take().is_some() {
                self.release();
            }
            return Ok(());
        };
        let dragging = self.drag.is_some();
        let shell = &self.shell;
        let handle = &shell.handle;
        let catcher = self.catcher.get_or_insert_with(|| {
            let surface = shell.compositor.create_surface(handle, Role::Catcher);
            let layer = shell.layers.get_layer_surface(
                &surface,
                Some(&on),
                Layer::Overlay,
                SCOPE.to_owned(),
                handle,
                Role::Catcher,
            );
            layer.set_anchor(Anchor::Top | Anchor::Bottom | Anchor::Left | Anchor::Right);
            layer.set_exclusive_zone(-1);
            layer.set_keyboard_interactivity(KeyboardInteractivity::None);
            // Until it knows where the pill is, it catches nothing.
            let nothing = shell.compositor.create_region(handle, ());
            surface.set_input_region(Some(&nothing));
            nothing.destroy();
            let viewport = shell.viewporter.get_viewport(&surface, handle, ());
            surface.commit();
            Catcher {
                surface,
                layer,
                viewport,
                clear: None,
                size: None,
                set_for: None,
            }
        });
        let Some(size) = catcher.size else {
            return Ok(());
        };
        // The press holds the pointer, so the region only needs to catch up after the drag.
        if dragging || catcher.set_for == Some((size, point)) {
            return Ok(());
        }
        if catcher.clear.is_none() {
            catcher.clear = Some(clear_pixel(shell)?);
        }
        catcher.surface.attach(catcher.clear.as_ref(), 0, 0);
        catcher.surface.damage_buffer(0, 0, 1, 1);
        catcher.viewport.set_destination(size.0, size.1);
        let pill_only = shell.compositor.create_region(handle, ());
        pill_only.add(point.0, point.1, WIDTH, HEIGHT);
        catcher.surface.set_input_region(Some(&pill_only));
        pill_only.destroy();
        catcher.surface.commit();
        catcher.set_for = Some((size, point));
        Ok(())
    }

    pub(super) fn show_cursor(&self, shape: Shape) {
        if let Some(pointer) = &self.pointer
            && let Some(cursor) = &pointer.cursor
        {
            cursor.set_shape(pointer.serial, shape);
        }
    }

    pub(super) fn press(&mut self) {
        let (Some(pointer), Some(start), Some(_)) = (&self.pointer, self.point(), &self.catcher)
        else {
            return;
        };
        self.drag = Some(Drag {
            from: pointer.at,
            start,
            point: start,
            moved: false,
            sent: None,
        });
        self.show_cursor(Shape::Grabbing);
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "pointer positions fit a monitor"
    )]
    pub(super) fn pointer_moved(&mut self, at: (f64, f64)) {
        if let Some(pointer) = self.pointer.as_mut() {
            pointer.at = at;
        }
        let Some((_, screen)) = self.here() else {
            return;
        };
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        let by = (
            (at.0 - drag.from.0).round() as i32,
            (at.1 - drag.from.1).round() as i32,
        );
        if by != (0, 0) || drag.moved {
            drag.moved = true;
            drag.point = place::settle((drag.start.0 + by.0, drag.start.1 + by.1), screen);
        }
    }

    /// The drag is over: remember where it ended.
    pub(super) fn release(&mut self) {
        let Some(drag) = self.drag.take() else { return };
        self.show_cursor(Shape::Grab);
        if !drag.moved {
            return;
        }
        let Some(spot) = self
            .here()
            .map(|(monitor, screen)| Spot::new(drag.point, screen, &monitor.name))
        else {
            return;
        };
        if let Some(file) = &self.file
            && let Err(error) = spot.save(file)
        {
            eprintln!("tomat: couldn't save the glass's place: {error}");
        }
        self.spot = Some(spot);
    }
}

/// One see-through pixel, stretched over the monitor. A full-size buffer would cost 8 MB at 1080p.
fn clear_pixel(shell: &Shell) -> Result<WlBuffer> {
    let file = File::from(
        rustix::fs::memfd_create("tomat-clear", rustix::fs::MemfdFlags::CLOEXEC)
            .context("no shared memory")?,
    );
    file.set_len(4)?;
    let pool = shell.shm.create_pool(file.as_fd(), 4, &shell.handle, ());
    let buffer = pool.create_buffer(0, 1, 1, 4, wl_shm::Format::Argb8888, &shell.handle, ());
    pool.destroy();
    Ok(buffer)
}
