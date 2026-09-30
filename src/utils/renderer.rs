// Gupax
//
// Copyright (c) 2024-2025 Cyrix126
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Renderer choice after a start that crashed the process.
//!
//! A graphics driver can abort the process, and wgpu panics on some device
//! creation failures, before `eframe::run_native` returns an error.
//! [`RendererAttempts`] records each renderer before it is tried and is
//! deleted once a window exists, so a renderer still recorded at the next
//! start crashed the process.

use std::path::{Path, PathBuf};

use eframe::Renderer;
use log::warn;

/// The `renderer.attempt` file in the data directory, one renderer per line.
pub struct RendererAttempts {
    path: PathBuf,
}

impl RendererAttempts {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            path: data_dir.join("renderer.attempt"),
        }
    }

    fn read(&self) -> String {
        std::fs::read_to_string(&self.path).unwrap_or_default()
    }

    fn lists(content: &str, renderer: Renderer) -> bool {
        let name = renderer.to_string();
        content.lines().any(|line| line == name)
    }

    /// Whether a previous start crashed with this renderer.
    pub fn crashed(&self, renderer: Renderer) -> bool {
        Self::lists(&self.read(), renderer)
    }

    pub fn both_crashed(&self) -> bool {
        self.crashed(Renderer::Glow) && self.crashed(Renderer::Wgpu)
    }

    pub fn record(&self, renderer: Renderer) {
        let mut content = self.read();
        if Self::lists(&content, renderer) {
            return;
        }
        content.push_str(&renderer.to_string());
        content.push('\n');
        if let Err(e) = std::fs::write(&self.path, content) {
            warn!("Renderer | could not record the attempt: {e}");
        }
    }

    pub fn clear(&self) {
        let _ = std::fs::remove_file(&self.path);
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod test {
    use super::RendererAttempts;
    use eframe::Renderer;

    fn data_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("gupax-renderer-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn recorded_renderers_count_as_crashed_until_cleared() {
        let dir = data_dir("record");
        let attempts = RendererAttempts::new(&dir);
        attempts.record(Renderer::Wgpu);
        assert!(attempts.crashed(Renderer::Wgpu));
        assert!(!attempts.crashed(Renderer::Glow));
        assert!(!attempts.both_crashed());
        attempts.record(Renderer::Glow);
        assert!(attempts.both_crashed());
        attempts.clear();
        assert!(!attempts.crashed(Renderer::Wgpu));
        assert!(!attempts.crashed(Renderer::Glow));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_renderer_is_recorded_once() {
        let dir = data_dir("once");
        let attempts = RendererAttempts::new(&dir);
        attempts.record(Renderer::Glow);
        attempts.record(Renderer::Glow);
        assert_eq!(std::fs::read_to_string(attempts.path()).unwrap(), "glow\n");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
