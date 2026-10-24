use std::fmt::Display;

use serde::{Deserialize, Serialize};
use strum::{EnumIter, IntoEnumIterator};

use crate::utils::constants::{
    gupax_advanced, gupax_simple, gupax_updates, p2pool_advanced, p2pool_crawler, p2pool_simple,
    status_submenu_hashrate, status_submenu_p2pool, status_submenu_processes,
};

/// A submenu
pub trait Submenu: Display + PartialEq + Default + IntoEnumIterator {
    fn hover_text(&self) -> &'static str;
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, Deserialize, Serialize, Default, EnumIter)]
pub enum SubmenuStatus {
    #[default]
    Processes,
    P2pool,
    Benchmarks,
}

impl Display for SubmenuStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            SubmenuStatus::Processes => crate::utils::i18n::tr("Processes"),
            SubmenuStatus::P2pool => "P2Pool",
            SubmenuStatus::Benchmarks => crate::utils::i18n::tr("Benchmarks"),
        };
        write!(f, "{s}")
    }
}
impl Submenu for SubmenuStatus {
    fn hover_text(&self) -> &'static str {
        match self {
            Self::Processes => status_submenu_processes(),
            Self::P2pool => status_submenu_p2pool(),
            Self::Benchmarks => status_submenu_hashrate(),
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq, Debug, Deserialize, Serialize, Default, EnumIter)]
pub enum SubmenuP2pool {
    #[default]
    Simple,
    Advanced,
    Crawler,
}

impl Display for SubmenuP2pool {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            SubmenuP2pool::Simple => crate::utils::i18n::tr("Simple"),
            SubmenuP2pool::Advanced => crate::utils::i18n::tr("Advanced"),
            SubmenuP2pool::Crawler => crate::utils::i18n::tr("Crawler"),
        };
        write!(f, "{s}")
    }
}

impl Submenu for SubmenuP2pool {
    fn hover_text(&self) -> &'static str {
        match self {
            Self::Simple => p2pool_simple(),
            Self::Advanced => p2pool_advanced(),
            Self::Crawler => p2pool_crawler(),
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq, Debug, Deserialize, Serialize, Default, EnumIter)]
pub enum SubmenuGupax {
    #[default]
    Simple,
    Advanced,
    Updates,
}

impl Display for SubmenuGupax {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            SubmenuGupax::Simple => crate::utils::i18n::tr("Simple"),
            SubmenuGupax::Advanced => crate::utils::i18n::tr("Advanced"),
            SubmenuGupax::Updates => crate::utils::i18n::tr("Updates"),
        };
        write!(f, "{s}")
    }
}
impl Submenu for SubmenuGupax {
    fn hover_text(&self) -> &'static str {
        match self {
            Self::Simple => gupax_simple(),
            Self::Advanced => gupax_advanced(),
            Self::Updates => gupax_updates(),
        }
    }
}
