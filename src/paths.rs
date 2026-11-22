//! Every file tomat touches.

use std::io;
use std::os::unix::fs::DirBuilderExt;
use std::path::PathBuf;

use anyhow::{Context, Result};

/// Where things are kept.
#[derive(Debug, Clone)]
pub struct Paths {
    state: PathBuf,
    runtime: PathBuf,
    /// The desktop's config folder: read for the theme, never written.
    pub config: PathBuf,
    /// The home folder, written as `~` in logs.
    pub home: PathBuf,
}

impl Paths {
    /// Finds them from the environment.
    ///
    /// # Errors
    /// Without a home or runtime folder.
    pub fn find() -> Result<Self> {
        Ok(Self {
            state: dirs::state_dir().context("no state folder")?.join("tomat"),
            runtime: dirs::runtime_dir().context("no runtime folder")?.join("tomat"),
            config: dirs::config_dir().context("no config folder")?,
            home: dirs::home_dir().context("no home folder")?,
        })
    }

    /// Creates both of tomat's folders, owner-only.
    pub(crate) fn create(&self) -> io::Result<()> {
        for folder in [&self.state, &self.runtime] {
            std::fs::DirBuilder::new().recursive(true).mode(0o700).create(folder)?;
        }
        Ok(())
    }

    /// The daily logs.
    #[must_use]
    pub fn logs(&self) -> PathBuf {
        self.state.join("logs")
    }

    pub(crate) fn slots(&self) -> PathBuf {
        self.state.join("slots")
    }

    pub(crate) fn position(&self) -> PathBuf {
        self.state.join("glass-position")
    }

    /// In the runtime folder on purpose: it is erased at logout, so no history builds up.
    pub(crate) fn timer(&self) -> PathBuf {
        self.runtime.join("timer")
    }

    pub(crate) fn lock(&self) -> PathBuf {
        self.runtime.join("lock")
    }

    pub(crate) fn door(&self) -> PathBuf {
        self.runtime.join("door")
    }
}
