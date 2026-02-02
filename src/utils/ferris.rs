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

// Some images of ferris in byte form for error messages, etc

pub const FERRIS_HAPPY: &[u8] = include_bytes!("../../assets/images/ferris/happy.png");
pub const FERRIS_CUTE: &[u8] = include_bytes!("../../assets/images/ferris/cute.png");
pub const FERRIS_OOPS: &[u8] = include_bytes!("../../assets/images/ferris/oops.png");
pub const FERRIS_ERROR: &[u8] = include_bytes!("../../assets/images/ferris/error.png");
pub const FERRIS_PANIC: &[u8] = include_bytes!("../../assets/images/ferris/panic.png"); // This isnt technically ferris but its ok since its spooky
#[cfg(target_os = "windows")]
pub const FERRIS_ADMIN: &[u8] = include_bytes!("../../assets/images/ferris/sudo.png");
