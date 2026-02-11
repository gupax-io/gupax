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

use derive_more::Display;
use serde::Deserialize;

use crate::{helper::xvb::priv_stats::XvbPrivStats, utils::constants::XVB_SIDE_MARGIN_1H};

use strum::EnumIter;

#[derive(Debug, Clone, Default, Display, Deserialize, PartialEq, EnumIter)]
pub enum XvbRound {
    #[default]
    #[display("VIP")]
    #[serde(alias = "vip")]
    Vip,
    #[serde(alias = "mvp")]
    #[display("MVP")]
    Mvp,
    #[serde(alias = "donor")]
    Donor,
    #[display("VIP Donor")]
    #[serde(alias = "donor_vip")]
    DonorVip,
    #[display("Whale Donor")]
    #[serde(alias = "donor_whale")]
    DonorWhale,
    #[display("Mega Donor")]
    #[serde(alias = "donor_mega")]
    DonorMega,
}

impl XvbRound {
    pub fn get_hashrate(&self) -> f32 {
        match &self {
            Self::Donor => Self::XVB_ROUND_DONOR_MIN_HR as f32,
            Self::DonorVip => Self::XVB_ROUND_DONOR_VIP_MIN_HR as f32,
            Self::DonorWhale => Self::XVB_ROUND_DONOR_WHALE_MIN_HR as f32,
            Self::DonorMega => Self::XVB_ROUND_DONOR_MEGA_MIN_HR as f32,
            Self::Vip | Self::Mvp => 0.0,
        }
    }
    pub const XVB_ROUND_DONOR_MIN_HR: u32 = 1_000;
    pub const XVB_ROUND_DONOR_VIP_MIN_HR: u32 = 10_000;
    pub const XVB_ROUND_DONOR_WHALE_MIN_HR: u32 = 100_000;
    pub const XVB_ROUND_DONOR_MEGA_MIN_HR: u32 = 1_000_000;
}

impl XvbPrivStats {
    /// The round type that the algorithm detects we are in.
    /// The 1h average required is multiplied by 0.8 to reflect the 20% margin accepted by XvB
    /// So if the private stats are giving 800H average per hour and 1kH/day, the doner will be in the Donor round.
    pub(crate) fn round_type(&self, has_share: bool) -> Option<XvbRound> {
        if has_share {
            match (self.donor_1hr_avg as u32, self.donor_24hr_avg as u32) {
                x if x.0
                    >= (XvbRound::XVB_ROUND_DONOR_MEGA_MIN_HR as f32 * (1.0 - XVB_SIDE_MARGIN_1H))
                        as u32
                    && x.1 >= XvbRound::XVB_ROUND_DONOR_MEGA_MIN_HR =>
                {
                    Some(XvbRound::DonorMega)
                }
                x if x.0
                    >= (XvbRound::XVB_ROUND_DONOR_WHALE_MIN_HR as f32 * (1.0 - XVB_SIDE_MARGIN_1H))
                        as u32
                    && x.1 >= XvbRound::XVB_ROUND_DONOR_WHALE_MIN_HR =>
                {
                    Some(XvbRound::DonorWhale)
                }
                x if x.0
                    >= (XvbRound::XVB_ROUND_DONOR_VIP_MIN_HR as f32 * (1.0 - XVB_SIDE_MARGIN_1H))
                        as u32
                    && x.1 >= XvbRound::XVB_ROUND_DONOR_VIP_MIN_HR =>
                {
                    Some(XvbRound::DonorVip)
                }
                x if x.0
                    >= (XvbRound::XVB_ROUND_DONOR_MIN_HR as f32 * (1.0 - XVB_SIDE_MARGIN_1H))
                        as u32
                    && x.1 >= XvbRound::XVB_ROUND_DONOR_MIN_HR =>
                {
                    Some(XvbRound::Donor)
                }
                (_, _) => Some(XvbRound::Vip),
            }
        } else {
            None
        }
    }
}
