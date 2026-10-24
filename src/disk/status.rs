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

use derive_more::derive::Display;
use strum::{EnumCount, EnumIter};

use super::*;

//---------------------------------------------------------------------------------------------------- [PayoutView] enum for [Status/P2Pool] tab
// The enum buttons for selecting which "view" to sort the payout log in.
#[derive(
    Clone, Copy, Eq, PartialEq, Debug, Deserialize, Serialize, Display, EnumIter, EnumCount,
)]
pub enum PayoutView {
    Latest,   // Shows the most recent logs first
    Oldest,   // Shows the oldest logs first
    Biggest,  // Shows highest to lowest payouts
    Smallest, // Shows lowest to highest payouts
}

impl PayoutView {
    pub fn msg_help(&self) -> &str {
        match self {
            Self::Latest => status_submenu_latest(),
            Self::Oldest => status_submenu_oldest(),
            Self::Biggest => status_submenu_smallest(),
            Self::Smallest => status_submenu_biggest(),
        }
    }
}

impl PayoutView {
    fn new() -> Self {
        Self::Latest
    }
}

impl Default for PayoutView {
    fn default() -> Self {
        Self::new()
    }
}

//---------------------------------------------------------------------------------------------------- [Hash] enum for [Status/P2Pool]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Deserialize, Serialize)]
#[allow(clippy::enum_variant_names)]
#[derive(Default)]
pub enum Hash {
    #[default]
    Hash,
    Kilo,
    Mega,
    Giga,
}

impl Hash {
    pub fn convert_to_hash(f: f64, from: Self) -> f64 {
        match from {
            Self::Hash => f,
            Self::Kilo => f * 1_000.0,
            Self::Mega => f * 1_000_000.0,
            Self::Giga => f * 1_000_000_000.0,
        }
    }
    #[cfg(test)]
    pub fn convert(f: f64, og: Self, new: Self) -> f64 {
        match og {
            Self::Hash => match new {
                Self::Hash => f,
                Self::Kilo => f / 1_000.0,
                Self::Mega => f / 1_000_000.0,
                Self::Giga => f / 1_000_000_000.0,
            },
            Self::Kilo => match new {
                Self::Hash => f * 1_000.0,
                Self::Kilo => f,
                Self::Mega => f / 1_000.0,
                Self::Giga => f / 1_000_000.0,
            },
            Self::Mega => match new {
                Self::Hash => f * 1_000_000.0,
                Self::Kilo => f * 1_000.0,
                Self::Mega => f,
                Self::Giga => f / 1_000.0,
            },
            Self::Giga => match new {
                Self::Hash => f * 1_000_000_000.0,
                Self::Kilo => f * 1_000_000.0,
                Self::Mega => f * 1_000.0,
                Self::Giga => f,
            },
        }
    }
}

impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Hash::Hash => write!(f, "H/s"),
            Hash::Kilo => write!(f, "KH/s"),
            Hash::Mega => write!(f, "MH/s"),
            Hash::Giga => write!(f, "GH/s"),
        }
    }
}
