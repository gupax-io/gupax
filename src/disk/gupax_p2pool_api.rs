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

use super::*;
use crate::helper::p2pool::P2poolNodes;
use crate::utils::regex::{P2POOL_REGEX, p2pool_no_payout_height};
//---------------------------------------------------------------------------------------------------- Gupax-P2Pool API
#[derive(Clone, Debug)]
pub struct GupaxP2poolApi {
    pub log: String,           // Log file only containing full payout lines
    pub log_rev: String,       // Same as above but reversed based off lines
    pub payout: HumanNumber,   // Human-friendly display of payout count
    pub payout_u64: u64,       // [u64] version of above
    pub payout_ord: PayoutOrd, // Ordered Vec of payouts, see [PayoutOrd]
    pub payout_low: String, // A pre-allocated/computed [String] of the above Vec from low payout to high
    pub payout_high: String, // Same as above but high -> low
    pub xmr: AtomicUnit,    // XMR stored as atomic units
    pub path_log: PathBuf,  // Path to [log]
    pub path_scan: PathBuf, // Path to [scan]
    pub sync: String,       // Result of the last payouts sync
    pub syncing: bool,
    pub stop_sync: bool,
    pub sync_failed: bool,     // Did the last payouts sync fail?
    pub found_blocks: PathBuf, // Blocks found by the pool, in the data API of the running P2Pool
    pub address: String,       // Address the running P2Pool pays
    pub nodes: P2poolNodes,    // Monero nodes of the running P2Pool
    pub observer: String,      // Observer used to sync the payouts
    pub view_key: String,      // Private view key used to sync the payouts, kept in memory only
}

impl Default for GupaxP2poolApi {
    fn default() -> Self {
        Self::new()
    }
}

impl GupaxP2poolApi {
    //---------------------------------------------------------------------------------------------------- Init, these pretty much only get called once
    pub fn new() -> Self {
        Self {
            log: String::new(),
            log_rev: String::new(),
            payout: HumanNumber::unknown(),
            payout_u64: 0,
            payout_ord: PayoutOrd::new(),
            payout_low: String::new(),
            payout_high: String::new(),
            xmr: AtomicUnit::new(),
            path_log: PathBuf::new(),
            path_scan: PathBuf::new(),
            sync: String::new(),
            syncing: false,
            stop_sync: false,
            sync_failed: false,
            found_blocks: PathBuf::new(),
            address: String::new(),
            nodes: P2poolNodes::default(),
            observer: String::new(),
            view_key: String::new(),
        }
    }

    #[cfg(test)]
    // Creates the files in the directory [dir].
    pub fn temporary(dir: &Path) -> Self {
        Self::create_all_files(dir).unwrap();
        let mut api = Self::new();
        api.fill_paths(dir);
        api.read_all_files_and_update().unwrap();
        api
    }

    pub fn fill_paths(&mut self, gupax_p2pool_dir: &Path) {
        let mut path_log = gupax_p2pool_dir.to_path_buf();
        let mut path_scan = gupax_p2pool_dir.to_path_buf();
        path_log.push(GUPAX_P2POOL_API_LOG);
        path_scan.push(GUPAX_P2POOL_API_SCAN);
        *self = Self {
            path_log,
            path_scan,
            ..std::mem::take(self)
        };
    }

    pub fn create_all_files(gupax_p2pool_dir: &Path) -> Result<(), TomlError> {
        for file in GUPAX_P2POOL_API_FILE_ARRAY {
            let mut path = gupax_p2pool_dir.to_path_buf();
            path.push(file);
            if path.exists() {
                info!(
                    "GupaxP2poolApi | [{}] already exists, skipping...",
                    path.display()
                );
                continue;
            }
            match std::fs::File::create(&path) {
                Ok(_) => {
                    info!("GupaxP2poolApi | [{}] create ... OK", path.display());
                }
                Err(e) => {
                    warn!(
                        "GupaxP2poolApi | [{}] create ... FAIL: {}",
                        path.display(),
                        e
                    );
                    return Err(TomlError::Io(e));
                }
            }
        }
        Ok(())
    }

    pub fn read_all_files_and_update(&mut self) -> Result<(), TomlError> {
        let log = read_to_string(File::Log, &self.path_log)?;
        self.update(log);
        Ok(())
    }

    // Replaces the history with [log], and the totals with its totals.
    fn update(&mut self, log: String) {
        self.payout_ord.update_from_payout_log(&log);
        self.update_payout_strings();
        (self.payout_u64, self.xmr) = self.payout_ord.total();
        self.payout = HumanNumber::from_u64(self.payout_u64);
        self.log = log;
        self.update_log_rev();
    }

    // Completely delete the [p2pool] folder and create defaults.
    pub fn create_new(path: &PathBuf) -> Result<(), TomlError> {
        info!(
            "GupaxP2poolApi | Deleting old folder at [{}]...",
            path.display()
        );
        std::fs::remove_dir_all(path)?;
        info!(
            "GupaxP2poolApi | Creating new default folder at [{}]...",
            path.display()
        );
        create_gupax_p2pool_dir(path)?;
        Self::create_all_files(path)?;
        Ok(())
    }

    //---------------------------------------------------------------------------------------------------- Live, functions that actually update/write live stats
    pub fn update_log_rev(&mut self) {
        let mut log_rev = String::with_capacity(self.log.len());
        for line in self.log.lines().rev() {
            log_rev.push_str(line);
            log_rev.push('\n');
        }
        self.log_rev = log_rev;
    }

    pub fn format_payout(date: &str, atomic_unit: &AtomicUnit, block: &HumanNumber) -> String {
        format!("{date} | {atomic_unit} XMR | Block {block}")
    }

    pub fn append_log(&mut self, formatted_log_line: &str) {
        self.log.push_str(formatted_log_line);
        self.log.push('\n');
    }

    pub fn append_head_log_rev(&mut self, formatted_log_line: &str) {
        self.log_rev = format!("{}\n{}", formatted_log_line, self.log_rev);
    }

    pub fn update_payout_low(&mut self) {
        self.payout_ord.sort_payout_low_to_high();
        self.payout_low = self.payout_ord.to_string();
    }

    pub fn update_payout_high(&mut self) {
        self.payout_ord.sort_payout_high_to_low();
        self.payout_high = self.payout_ord.to_string();
    }

    pub fn update_payout_strings(&mut self) {
        self.update_payout_low();
        self.update_payout_high();
    }

    // Takes the (date, atomic_unit, block) and updates [self] and the [PayoutOrd]
    pub fn add_payout(
        &mut self,
        formatted_log_line: &str,
        date: String,
        atomic_unit: AtomicUnit,
        block: HumanNumber,
    ) {
        self.append_log(formatted_log_line);
        self.append_head_log_rev(formatted_log_line);
        self.payout_u64 += 1;
        self.payout = HumanNumber::from_u64(self.payout_u64);
        self.xmr = self.xmr.add_self(atomic_unit);
        self.payout_ord.push(date, atomic_unit, block);
        self.update_payout_strings();
    }

    #[cfg(test)]
    pub fn has_payout(&self, height: u64) -> bool {
        self.log
            .lines()
            .any(|line| PayoutOrd::payout_height(line) == Some(height))
    }

    // Records the block of the pool that P2Pool announces in [line], with its payout if any,
    // in place of the payouts at its height: they came from a block it replaced.
    pub fn record_found_block(&mut self, line: &str) -> Result<(), TomlError> {
        let payout = P2POOL_REGEX
            .payout
            .is_match(line)
            .then(|| PayoutOrd::parse_raw_payout_line(line));
        let height = match &payout {
            Some((_, _, height)) => *height,
            None => p2pool_no_payout_height(line),
        };
        if let Some(height) = height
            && let Err(e) = self.remove_payouts(&[height], &[])
        {
            warn!("GupaxP2poolApi | Payouts of block {height} kept: {e}");
        }
        if let Some((date, atomic_unit, height)) = payout {
            let block = height.map_or_else(HumanNumber::unknown, HumanNumber::from_u64);
            let formatted_log_line = Self::format_payout(&date, &atomic_unit, &block);
            self.add_payout(&formatted_log_line, date, atomic_unit, block);
            Self::disk_append(&formatted_log_line, &self.path_log)?;
        }
        Ok(())
    }

    // Removes the payouts received in the blocks at [heights], except the (height, amount)
    // payouts [kept], and updates [self] and the files. Returns the number of payouts removed.
    pub fn remove_payouts(
        &mut self,
        heights: &[u64],
        kept: &[(u64, u64)],
    ) -> Result<u64, TomlError> {
        if heights.is_empty() {
            return Ok(0);
        }
        let mut lines = self.read_lines()?;
        let count = lines.len();
        lines.retain(|line| {
            let Some(height) =
                PayoutOrd::payout_height(line).filter(|height| heights.contains(height))
            else {
                return true;
            };
            let amount = PayoutOrd::parse_formatted_payout_line(line).1.to_u64();
            kept.iter().any(|&(kept_height, kept_amount)| {
                kept_height == height && Self::same_amount(kept_amount, amount)
            })
        });
        let removed = (count - lines.len()) as u64;
        self.rewrite(&lines)?;
        Ok(removed)
    }

    // Adds the (date, atomic_unit, height) payouts missing from the history,
    // updates [self] and the files. Returns the number of payouts added.
    pub fn merge_payouts(
        &mut self,
        payouts: &[(String, AtomicUnit, u64)],
    ) -> Result<u64, TomlError> {
        if payouts.is_empty() {
            return Ok(0);
        }
        let mut lines = self.read_lines()?;
        // A block may pay several addresses of the user: a payout is told apart by block and amount.
        let mut paid: std::collections::HashMap<u64, Vec<u64>> = std::collections::HashMap::new();
        for line in &lines {
            if let Some(height) = PayoutOrd::payout_height(line) {
                let amount = PayoutOrd::parse_formatted_payout_line(line).1.to_u64();
                paid.entry(height).or_default().push(amount);
            }
        }
        let mut added = 0;
        for (date, atomic_unit, height) in payouts {
            let amounts = paid.entry(*height).or_default();
            let amount = atomic_unit.to_u64();
            if !amounts.iter().any(|&paid| Self::same_amount(paid, amount)) {
                amounts.push(amount);
                let block = HumanNumber::from_u64(*height);
                lines.push(Self::format_payout(date, atomic_unit, &block));
                added += 1;
            }
        }
        if added > 0 {
            // Each line starts with its date.
            lines.sort();
            self.rewrite(&lines)?;
        }
        Ok(added)
    }

    // Whether the amounts [a] and [b] of a payout are the same: an amount of the history may be
    // truncated, 1 atomic unit lower.
    fn same_amount(a: u64, b: u64) -> bool {
        a.abs_diff(b) <= 1
    }

    // Lines of the history, read again since another Gupax may have changed it.
    fn read_lines(&mut self) -> Result<Vec<String>, TomlError> {
        self.read_all_files_and_update()?;
        Ok(self.log.lines().map(String::from).collect())
    }

    // Replaces the history with [lines].
    fn rewrite(&mut self, lines: &[String]) -> Result<(), TomlError> {
        let log: String = lines.iter().map(|line| format!("{line}\n")).collect();
        if log != self.log {
            Self::disk_replace(&log, &self.path_log)?;
            self.update(log);
        }
        Ok(())
    }

    // Block from which to sync the payouts of [address].
    pub fn read_scan(&self, address: &str) -> Option<u64> {
        let scan = read_to_string(File::Scan, &self.path_scan).ok()?;
        scan.lines().find_map(|line| {
            let (height, scanned_address) = line.split_once(' ')?;
            if scanned_address == address {
                height.parse().ok()
            } else {
                None
            }
        })
    }

    // Writes [height] as the block from which to sync the payouts of [address], in the file [path].
    pub fn write_scan(path: &Path, height: u64, address: &str) -> Result<(), TomlError> {
        // The file may have been deleted since Gupax started.
        let scan = match fs::read_to_string(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            scan => scan?,
        };
        let mut scan: String = scan
            .lines()
            .filter(|line| {
                line.split_once(' ')
                    .is_none_or(|(_, scanned)| scanned != address)
            })
            .map(|line| format!("{line}\n"))
            .collect();
        scan.push_str(&format!("{height} {address}\n"));
        Self::disk_replace(&scan, path)
    }

    pub fn disk_append(formatted_log_line: &str, path: &PathBuf) -> Result<(), TomlError> {
        use std::io::Write;
        let mut file = match fs::OpenOptions::new().append(true).create(true).open(path) {
            Ok(f) => f,
            Err(e) => {
                error!(
                    "GupaxP2poolApi | Append [{}] ... FAIL: {}",
                    path.display(),
                    e
                );
                return Err(TomlError::Io(e));
            }
        };
        match writeln!(file, "{formatted_log_line}") {
            Ok(_) => {
                debug!("GupaxP2poolApi | Append [{}] ... OK", path.display());
                Ok(())
            }
            Err(e) => {
                error!(
                    "GupaxP2poolApi | Append [{}] ... FAIL: {}",
                    path.display(),
                    e
                );
                Err(TomlError::Io(e))
            }
        }
    }

    // Replaces the file at [path] atomically.
    fn disk_replace(string: &str, path: &Path) -> Result<(), TomlError> {
        use std::io::Write;
        let tmp = path.with_extension("tmp");
        let mut file = fs::File::create(&tmp)?;
        file.write_all(string.as_bytes())?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}
