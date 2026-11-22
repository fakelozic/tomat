//! Two shared-memory buffers: one on screen, one to paint the next second into.

use std::fs::File;
use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;

use anyhow::{Context, Result};
use wayland_client::QueueHandle;
use wayland_client::protocol::{wl_buffer::WlBuffer, wl_shm};

use super::surface::State;

/// Says which buffer a release event is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Slot {
    generation: u32,
    index: usize,
}

pub(super) struct Buffers {
    file: File,
    pair: [WlBuffer; 2],
    busy: [bool; 2],
    size: (usize, usize),
    generation: u32,
}

impl Buffers {
    pub(super) fn new(
        shm: &wl_shm::WlShm,
        handle: &QueueHandle<State>,
        size: (usize, usize),
        generation: u32,
    ) -> Result<Self> {
        let width = i32::try_from(size.0)?;
        let height = i32::try_from(size.1)?;
        let bytes = width * height * 4;
        let fd =
            rustix::fs::memfd_create("tomat-glass", rustix::fs::MemfdFlags::CLOEXEC).context("no shared memory")?;
        let file = File::from(fd);
        file.set_len(u64::try_from(bytes * 2)?)?;
        let pool = shm.create_pool(file.as_fd(), bytes * 2, handle, ());
        let pair = [0, 1].map(|index: usize| {
            let offset = if index == 0 { 0 } else { bytes };
            let slot = Slot { generation, index };
            pool.create_buffer(offset, width, height, width * 4, wl_shm::Format::Argb8888, handle, slot)
        });
        pool.destroy();
        Ok(Self { file, pair, busy: [false; 2], size, generation })
    }

    pub(super) fn size(&self) -> (usize, usize) {
        self.size
    }

    /// A buffer the compositor isn't reading, if there is one.
    pub(super) fn free(&self) -> Option<usize> {
        self.busy.iter().position(|busy| !busy)
    }

    pub(super) fn fill(&mut self, index: usize, pixels: &[u8]) -> Result<&WlBuffer> {
        let offset = index * self.size.0 * self.size.1 * 4;
        self.file.write_all_at(pixels, u64::try_from(offset)?)?;
        self.busy[index] = true;
        Ok(&self.pair[index])
    }

    pub(super) fn released(&mut self, slot: Slot) {
        if slot.generation == self.generation {
            self.busy[slot.index] = false;
        }
    }
}

impl Drop for Buffers {
    fn drop(&mut self) {
        for buffer in &self.pair {
            buffer.destroy();
        }
    }
}
