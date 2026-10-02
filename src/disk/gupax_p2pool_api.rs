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
    pub path_payout: PathBuf, // Path to [payout]
    pub path_xmr: PathBuf,  // Path to [xmr]
    pub path_scan: PathBuf, // Path to [scan]
    pub sync: String,       // Result of the last payouts sync
    pub syncing: bool,
    pub stop_sync: bool,
    pub found_blocks: PathBuf, // Blocks found by the pool, in the data API of the running P2Pool
    pub address: String,       // Address the running P2Pool pays
    pub observer: String,      // Observer set when P2Pool started, used by the automatic sync
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
            path_xmr: PathBuf::new(),
            path_payout: PathBuf::new(),
            path_log: PathBuf::new(),
            path_scan: PathBuf::new(),
            sync: String::new(),
            syncing: false,
            stop_sync: false,
            found_blocks: PathBuf::new(),
            address: String::new(),
            observer: String::new(),
            view_key: String::new(),
        }
    }

    #[cfg(test)]
    // Creates the files in a new temporary directory named after [name].
    pub fn temporary(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("gupax_test_{name}_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        Self::create_all_files(&dir).unwrap();
        let mut api = Self::new();
        api.fill_paths(&dir);
        api.read_all_files_and_update().unwrap();
        api
    }

    pub fn fill_paths(&mut self, gupax_p2pool_dir: &Path) {
        let mut path_log = gupax_p2pool_dir.to_path_buf();
        let mut path_payout = gupax_p2pool_dir.to_path_buf();
        let mut path_xmr = gupax_p2pool_dir.to_path_buf();
        let mut path_scan = gupax_p2pool_dir.to_path_buf();
        path_log.push(GUPAX_P2POOL_API_LOG);
        path_payout.push(GUPAX_P2POOL_API_PAYOUT);
        path_xmr.push(GUPAX_P2POOL_API_XMR);
        path_scan.push(GUPAX_P2POOL_API_SCAN);
        *self = Self {
            path_log,
            path_payout,
            path_xmr,
            path_scan,
            ..std::mem::take(self)
        };
    }

    pub fn create_all_files(gupax_p2pool_dir: &Path) -> Result<(), TomlError> {
        use std::io::Write;
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
                Ok(mut f) => {
                    match file {
                        GUPAX_P2POOL_API_PAYOUT | GUPAX_P2POOL_API_XMR => writeln!(f, "0")?,
                        _ => (),
                    }
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
        let payout_u64 = match read_to_string(File::Payout, &self.path_payout)?
            .trim()
            .parse::<u64>()
        {
            Ok(o) => o,
            Err(e) => {
                warn!("GupaxP2poolApi | [payout] parse error: {e}");
                return Err(TomlError::Parse("payout"));
            }
        };
        let xmr = match read_to_string(File::Xmr, &self.path_xmr)?
            .trim()
            .parse::<u64>()
        {
            Ok(o) => AtomicUnit::from_u64(o),
            Err(e) => {
                warn!("GupaxP2poolApi | [xmr] parse error: {e}");
                return Err(TomlError::Parse("xmr"));
            }
        };
        let payout = HumanNumber::from_u64(payout_u64);
        let log = read_to_string(File::Log, &self.path_log)?;
        self.payout_ord.update_from_payout_log(&log);
        self.update_payout_strings();
        *self = Self {
            log,
            payout,
            payout_u64,
            xmr,
            ..std::mem::take(self)
        };
        self.update_log_rev();
        Ok(())
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

    // Height of the block of a formatted payout line.
    pub fn payout_height(line: &str) -> Option<u64> {
        line.rsplit_once("Block ")?.1.replace(',', "").parse().ok()
    }

    pub fn has_payout(&self, height: u64) -> bool {
        self.log
            .lines()
            .any(|line| Self::payout_height(line) == Some(height))
    }

    // Removes the payouts received in the blocks at [heights] and updates [self] and the files.
    // Returns the number of payouts removed.
    pub fn remove_payouts(&mut self, heights: &[u64]) -> Result<u64, TomlError> {
        if heights.is_empty() {
            return Ok(0);
        }
        // Another Gupax may have changed the files.
        self.read_all_files_and_update()?;
        let mut lines = Vec::new();
        let mut removed = 0;
        for line in self.log.lines() {
            if Self::payout_height(line).is_some_and(|height| heights.contains(&height)) {
                removed += 1;
            } else {
                lines.push(line.to_string());
            }
        }
        if removed > 0 {
            self.rewrite(&lines)?;
        }
        Ok(removed)
    }

    // Adds the (date, atomic_unit, height) payouts of blocks missing from the history,
    // updates [self] and the files. Returns the number of payouts added.
    pub fn merge_payouts(
        &mut self,
        payouts: &[(String, AtomicUnit, u64)],
    ) -> Result<u64, TomlError> {
        if payouts.is_empty() {
            return Ok(0);
        }
        // Another Gupax may have changed the files.
        self.read_all_files_and_update()?;
        let mut heights: std::collections::HashSet<u64> =
            self.log.lines().filter_map(Self::payout_height).collect();
        let mut lines: Vec<String> = self.log.lines().map(String::from).collect();
        let mut added = 0;
        for (date, atomic_unit, height) in payouts {
            if heights.insert(*height) {
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

    // Replaces the history with [lines] and the totals with the totals of [lines].
    fn rewrite(&mut self, lines: &[String]) -> Result<(), TomlError> {
        let log: String = lines.iter().map(|line| format!("{line}\n")).collect();
        let xmr = lines.iter().fold(AtomicUnit::new(), |xmr, line| {
            xmr.add_self(PayoutOrd::parse_formatted_payout_line(line).1)
        });
        Self::disk_replace(&log, &self.path_log)?;
        Self::disk_overwrite(&lines.len().to_string(), &self.path_payout)?;
        Self::disk_overwrite(&xmr.to_string(), &self.path_xmr)?;
        self.read_all_files_and_update()
    }

    // Next block to scan for the payouts of [address].
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

    pub fn write_scan(&self, height: u64, address: &str) -> Result<(), TomlError> {
        let scan = read_to_string(File::Scan, &self.path_scan)?;
        let mut scan: String = scan
            .lines()
            .filter(|line| {
                line.split_once(' ')
                    .is_none_or(|(_, scanned)| scanned != address)
            })
            .map(|line| format!("{line}\n"))
            .collect();
        scan.push_str(&format!("{height} {address}\n"));
        Self::disk_replace(&scan, &self.path_scan)
    }

    // Writes the log first, so a payout counted in the totals is in the log.
    pub fn write_to_all_files(&self, formatted_log_line: &str) -> Result<(), TomlError> {
        Self::disk_append(formatted_log_line, &self.path_log)?;
        Self::disk_overwrite(&self.payout_u64.to_string(), &self.path_payout)?;
        Self::disk_overwrite(&self.xmr.to_string(), &self.path_xmr)?;
        Ok(())
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

    pub fn disk_overwrite(string: &str, path: &PathBuf) -> Result<(), TomlError> {
        use std::io::Write;
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(path)
        {
            Ok(f) => f,
            Err(e) => {
                error!(
                    "GupaxP2poolApi | Overwrite [{}] ... FAIL: {}",
                    path.display(),
                    e
                );
                return Err(TomlError::Io(e));
            }
        };
        match writeln!(file, "{string}") {
            Ok(_) => {
                debug!("GupaxP2poolApi | Overwrite [{}] ... OK", path.display());
                Ok(())
            }
            Err(e) => {
                error!(
                    "GupaxP2poolApi | Overwrite [{}] ... FAIL: {}",
                    path.display(),
                    e
                );
                Err(TomlError::Io(e))
            }
        }
    }
}
