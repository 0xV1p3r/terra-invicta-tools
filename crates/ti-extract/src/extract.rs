//! The `extract` pipeline (port of `cmd_extract` in `cli.py`).

use std::path::Path;
use std::sync::atomic::AtomicBool;

use crate::{ExtractOptions, ExtractReport, GameInstall, Progress, Result};

pub fn run(install: &GameInstall, opts: &ExtractOptions, progress: &mut dyn FnMut(Progress), cancel: &AtomicBool) -> Result<ExtractReport> {
    let _ = (install, opts, progress, cancel);
    todo!()
}

pub fn write_index(data_dir: &Path) -> Result<()> {
    todo!("{}", data_dir.display())
}
