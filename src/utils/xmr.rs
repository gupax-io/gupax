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

// This file is for handling actual XMR integers/floats using [AtomicUnit] & [PayoutOrd]
// AtomicUnit is just a wrapper around a [u64] implementing common XMR Atomic Unit functions.
// PayoutOrd is a wrapper around a [Vec] for sorting P2Pool payouts with this type signature:
//     "Vec<(String, AtomicUnit, HumanNumber)>"
// These represent:
//     "(DATE, ATOMIC_UNIT, MONERO_BLOCK)"

use crate::human::*;
use crate::regex::P2POOL_REGEX;

use log::*;

//---------------------------------------------------------------------------------------------------- XMR AtomicUnit
// After I initially wrote this struct, I forgot why I even needed it.
// I get the XMR received as a float, I display it as a float and it wouldn't be
// too bad if I wrote it to disk as a float, but then I realized [.cmp()] doesn't
// work on [f64] and also that Rust makes sorting floats a pain so instead of deleting
// this code and making some float sorter, I might as well use it.

// [u64] can hold max: 18_446_744_073_709_551_615 which equals to 18,446,744,073 XMR (18 billion).
// Given the constant XMR tail emission of (0.3 per minute|18 per hour|432 per day|157,680 per year)
// this would take: 116,976~ years to overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AtomicUnit(u64);

impl AtomicUnit {
    pub const fn new() -> Self {
        Self(0)
    }

    pub const fn from_u64(u: u64) -> Self {
        Self(u)
    }

    // pub const fn add_u64(self, u: u64) -> Self {
    //     Self(self.0 + u)
    // }

    pub const fn add_self(self, atomic_unit: Self) -> Self {
        Self(self.0 + atomic_unit.0)
    }

    pub const fn to_u64(self) -> u64 {
        self.0
    }

    #[allow(clippy::inherent_to_string_shadow_display)]
    // This is terrible but it formats it in a different way
    // than `Display`, but for backwards compat, changing it
    // requires touching other code, so...
    pub fn to_string(self) -> String {
        self.0.to_string()
    }

    // pub fn sum_vec(vec: &Vec<Self>) -> Self {
    //     let mut sum = 0;
    //     for int in vec {
    //         sum += int.0;
    //     }
    //     Self(sum)
    // }

    pub fn from_f64(f: f64) -> Self {
        Self((f * 1_000_000_000_000.0) as u64)
    }

    // pub fn f64(&self) -> f64 {
    //     self.0 as f64 / 1_000_000_000_000.0
    // }

    pub fn human_number_12_point(&self) -> HumanNumber {
        let f = self.0 as f64 / 1_000_000_000_000.0;
        HumanNumber::from_f64_12_point(f)
    }

    // pub fn human_number_no_fmt(&self) -> HumanNumber {
    //     let f = self.0 as f64 / 1_000_000_000_000.0;
    //     HumanNumber::from_f64_no_fmt(f)
    // }
}

// Displays AtomicUnit as a real XMR floating point.
impl std::fmt::Display for AtomicUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", Self::human_number_12_point(self))
    }
}

//---------------------------------------------------------------------------------------------------- [PayoutOrd]
// This is the struct for ordering P2Pool payout lines into a structured and ordered vector of elements.
// The structure goes as follows:
//     "Vec<(String, AtomicUnit, HumanNumber)>"
// Which displays as:
//     "2022-08-17 12:16:11.8662" | 0.002382256231 XMR | Block 2573821
//
// [0] = DATE
// [1] = XMR IN ATOMIC-UNITS
// [2] = MONERO BLOCK
#[derive(Debug, Clone)]
pub struct PayoutOrd(Vec<(String, AtomicUnit, HumanNumber)>);

impl PayoutOrd {
    pub fn new() -> Self {
        Self(vec![(
            String::from("????-??-?? ??:??:??.????"),
            AtomicUnit::new(),
            HumanNumber::unknown(),
        )])
    }
    #[cfg(test)]
    pub const fn from_vec(vec: Vec<(String, AtomicUnit, HumanNumber)>) -> Self {
        Self(vec)
    }
    #[cfg(test)]
    pub fn is_same(a: &Self, b: &Self) -> bool {
        if a.0.is_empty() && b.0.is_empty() {
            return true;
        }
        if a.0.len() != b.0.len() {
            return false;
        }
        for (n, (date, atomic_unit, block)) in a.0.iter().enumerate() {
            if *date != b.0[n].0 {
                return false;
            }
            if *atomic_unit != b.0[n].1 {
                return false;
            }
            if *block != b.0[n].2 {
                return false;
            }
        }
        true
    }
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    // Expected input: "NOTICE  2022-01-27 01:30:23.1377 P2Pool You received a payout of 0.000000000001 XMR in block 2642816"
    pub fn parse_raw_payout_line(line: &str) -> (String, AtomicUnit, HumanNumber) {
        // Date
        let date = match P2POOL_REGEX.date.find(line) {
            Some(date) => date.as_str().to_string(),
            None => {
                error!("P2Pool | Date parse error: [{line}]");
                "????-??-?? ??:??:??.????".to_string()
            }
        };
        // AtomicUnit
        let atomic_unit = if let Some(word) = P2POOL_REGEX.payout.find(line) {
            if let Some(word) = P2POOL_REGEX.payout_float.find(word.as_str()) {
                match word.as_str().parse::<f64>() {
                    Ok(au) => AtomicUnit::from_f64(au),
                    Err(e) => {
                        error!("P2Pool | AtomicUnit parse error: [{e}] on [{line}]");
                        AtomicUnit::new()
                    }
                }
            } else {
                error!("P2Pool | AtomicUnit parse error: [{line}]");
                AtomicUnit::new()
            }
        } else {
            error!("P2Pool | AtomicUnit parse error: [{line}]");
            AtomicUnit::new()
        };
        // Block
        let block = if let Some(word) = P2POOL_REGEX.block.find(line) {
            if let Some(word) = P2POOL_REGEX.block_int.find(word.as_str()) {
                match word.as_str().parse::<u64>() {
                    Ok(b) => HumanNumber::from_u64(b),
                    Err(e) => {
                        error!("P2Pool | Block parse error: [{e}] on [{line}]");
                        HumanNumber::unknown()
                    }
                }
            } else {
                error!("P2Pool | Block parse error: [{line}]");
                HumanNumber::unknown()
            }
        } else {
            error!("P2Pool | Block parse error: [{line}]");
            HumanNumber::unknown()
        };
        (date, atomic_unit, block)
    }

    // Expected input: "2022-01-27 01:30:23.1377 | 0.000000000001 XMR | Block 2,642,816"
    pub fn parse_formatted_payout_line(line: &str) -> (String, AtomicUnit, HumanNumber) {
        // Date
        let date = match P2POOL_REGEX.date.find(line) {
            Some(date) => date.as_str().to_string(),
            None => {
                error!("P2Pool | Date parse error: [{line}]");
                "????-??-?? ??:??:??.????".to_string()
            }
        };
        // AtomicUnit
        let atomic_unit = if let Some(word) = P2POOL_REGEX.payout_float.find(line) {
            match word.as_str().parse::<f64>() {
                Ok(au) => AtomicUnit::from_f64(au),
                Err(e) => {
                    error!("P2Pool | AtomicUnit parse error: [{e}] on [{line}]");
                    AtomicUnit::new()
                }
            }
        } else {
            error!("P2Pool | AtomicUnit parse error: [{line}]");
            AtomicUnit::new()
        };
        // Block
        let block = match P2POOL_REGEX.block_comma.find(line) {
            Some(b) => HumanNumber::from_str(b.as_str()),
            None => {
                error!("P2Pool | Block parse error: [{line}]");
                HumanNumber::unknown()
            }
        };
        (date, atomic_unit, block)
    }

    // Takes in input of ONLY P2Pool payout logs and converts it into a usable [PayoutOrd]
    // It expects formatted log lines like this: "2022-04-11 00:20:17.2571 | 0.001371623621 XMR | Block 2,562,511"
    // For efficiency reasons, I'd like to know the byte size
    // we should allocate for the vector so we aren't adding every loop.
    // Given a log [str], the equation for how many bytes the final vec will be is:
    // (BYTES_OF_DATE + BYTES OF XMR + BYTES OF BLOCK) + (SPACES, PIPES, MISC WORDS) * amount_of_lines
    // The first three are more or less constants (monero block 10m is in 10,379 years...): [23, 14, 7] (sum: 44)
    // Spaces, pipes, commas and words (XMR, Block): [19]
    // Add 7 more bytes for wrapper type overhead and it's an even [70] bytes per line.
    pub fn update_from_payout_log(&mut self, log: &str) {
        let amount_of_lines = log.lines().count();
        let mut vec: Vec<(String, AtomicUnit, HumanNumber)> =
            Vec::with_capacity(70 * amount_of_lines);
        for line in log.lines() {
            debug!("PayoutOrd | Parsing line: [{line}]");
            vec.push(Self::parse_formatted_payout_line(line));
        }
        *self = Self(vec);
    }

    // Takes the wrapper types, and pushes to existing [Self]
    pub fn push(&mut self, date: String, atomic_unit: AtomicUnit, block: HumanNumber) {
        self.0.push((date, atomic_unit, block));
    }

    // Takes the raw components (no wrapper types), convert them and pushes to existing [Self]
    #[cfg(test)]
    pub fn push_raw(&mut self, date: &str, atomic_unit: u64, block: u64) {
        let atomic_unit = AtomicUnit(atomic_unit);
        let block = HumanNumber::from_u64(block);
        self.0.push((date.to_string(), atomic_unit, block));
    }
    #[cfg(test)]
    pub fn atomic_unit_sum(&self) -> AtomicUnit {
        let mut sum: u64 = 0;
        for (_, atomic_unit, _) in &self.0 {
            sum += atomic_unit.to_u64();
        }
        AtomicUnit::from_u64(sum)
    }

    // Sort [Self] from highest payout to lowest
    pub fn sort_payout_high_to_low(&mut self) {
        // This is a little confusing because wrapper types are basically 1 element tuples so:
        // self.0 = The [Vec] within [PayoutOrd]
        // a.1.0  = [a] is [(String, AtomicUnit, HumanNumber)], [.1] is the [AtomicUnit] inside it, [.0] is the [u64] inside that
        self.0.sort_by_key(|a| std::cmp::Reverse(a.1.0));
    }

    // These sorting functions take around [0.0035~] seconds on a Ryzen 5950x
    // given a Vec filled with 1_000_000 elements, not bad.
    pub fn sort_payout_low_to_high(&mut self) {
        self.0.sort_by_key(|a| a.1.0);
    }

    // Returns a reversed [Iter] of the [PayoutOrd]
    // This is obviously faster than actually reordering the Vec.
    #[cfg(test)]
    pub fn rev_iter(
        &self,
    ) -> std::iter::Rev<std::slice::Iter<'_, (String, AtomicUnit, HumanNumber)>> {
        self.0.iter().rev()
    }

    // Recent <-> Oldest relies on the line order.
    // The raw log lines will be shown instead of this struct.
}

impl Default for PayoutOrd {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for PayoutOrd {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for i in &self.0 {
            writeln!(f, "{} | {} XMR | Block {}", i.0, i.1, i.2)?;
        }
        Ok(())
    }
}

//---------------------------------------------------------------------------------------------------- TESTS
#[cfg(test)]
mod test {
    #[test]
    fn update_p2pool_payout_log() {
        use crate::xmr::PayoutOrd;
        let log = r#"2021-12-21 01:01:01.1111 | 0.001000000000 XMR | Block 1,234,567
2021-12-21 02:01:01.1111 | 0.002000000000 XMR | Block 2,345,678
2021-12-21 03:01:01.1111 | 0.003000000000 XMR | Block 3,456,789
"#;
        let mut payout_ord = PayoutOrd::new();
        println!("BEFORE: {}", payout_ord);
        PayoutOrd::update_from_payout_log(&mut payout_ord, log);
        println!("AFTER: {}", payout_ord);
        assert_eq!(payout_ord.to_string(), log);
    }

    #[test]
    fn push_to_payout_ord() {
        use crate::xmr::PayoutOrd;
        let mut payout_ord = PayoutOrd::from_vec(vec![]);
        let should_be = "2022-09-08 18:42:55.4636 | 0.000000000001 XMR | Block 2,654,321\n";
        println!("BEFORE: {:#?}", payout_ord);
        payout_ord.push_raw("2022-09-08 18:42:55.4636", 1, 2654321);
        println!("AFTER: {}", payout_ord);
        println!("SHOULD_BE: {}", should_be);
        assert_eq!(payout_ord.to_string(), should_be);
    }

    #[test]
    fn sum_payout_ord_atomic_unit() {
        use crate::human::HumanNumber;
        use crate::xmr::AtomicUnit;
        use crate::xmr::PayoutOrd;
        let payout_ord = PayoutOrd::from_vec(vec![
            (
                "2022-09-08 18:42:55.4636".to_string(),
                AtomicUnit::from_u64(1),
                HumanNumber::from_u64(2654321),
            ),
            (
                "2022-09-09 16:18:26.7582".to_string(),
                AtomicUnit::from_u64(1),
                HumanNumber::from_u64(2654322),
            ),
            (
                "2022-09-10 11:15:21.1272".to_string(),
                AtomicUnit::from_u64(1),
                HumanNumber::from_u64(2654323),
            ),
        ]);
        println!("OG: {:#?}", payout_ord);
        let sum = PayoutOrd::atomic_unit_sum(&payout_ord);
        println!("SUM: {}", sum.to_u64());
        assert_eq!(sum.to_u64(), 3);
    }

    #[test]
    fn sort_p2pool_payout_ord() {
        use crate::human::HumanNumber;
        use crate::xmr::AtomicUnit;
        use crate::xmr::PayoutOrd;
        let mut payout_ord = PayoutOrd::from_vec(vec![
            (
                "2022-09-08 18:42:55.4636".to_string(),
                AtomicUnit::from_u64(1000000000),
                HumanNumber::from_u64(2654321),
            ),
            (
                "2022-09-09 16:18:26.7582".to_string(),
                AtomicUnit::from_u64(2000000000),
                HumanNumber::from_u64(2654322),
            ),
            (
                "2022-09-10 11:15:21.1272".to_string(),
                AtomicUnit::from_u64(3000000000),
                HumanNumber::from_u64(2654323),
            ),
        ]);
        println!("OG: {:#?}", payout_ord);

        // High to Low
        PayoutOrd::sort_payout_high_to_low(&mut payout_ord);
        println!("AFTER PAYOUT HIGH TO LOW: {:#?}", payout_ord);
        let should_be = r#"2022-09-10 11:15:21.1272 | 0.003000000000 XMR | Block 2,654,323
2022-09-09 16:18:26.7582 | 0.002000000000 XMR | Block 2,654,322
2022-09-08 18:42:55.4636 | 0.001000000000 XMR | Block 2,654,321
"#;
        println!("SHOULD_BE:\n{}", should_be);
        println!("IS:\n{}", payout_ord);
        assert_eq!(payout_ord.to_string(), should_be);

        // Low to High
        PayoutOrd::sort_payout_low_to_high(&mut payout_ord);
        println!("AFTER PAYOUT LOW TO HIGH: {:#?}", payout_ord);
        let should_be = r#"2022-09-08 18:42:55.4636 | 0.001000000000 XMR | Block 2,654,321
2022-09-09 16:18:26.7582 | 0.002000000000 XMR | Block 2,654,322
2022-09-10 11:15:21.1272 | 0.003000000000 XMR | Block 2,654,323
"#;
        println!("SHOULD_BE:\n{}", should_be);
        println!("IS:\n{}", payout_ord);
        assert_eq!(payout_ord.to_string(), should_be);
    }

    #[test]
    fn payout_ord_is_same() {
        use crate::human::HumanNumber;
        use crate::xmr::AtomicUnit;
        use crate::xmr::PayoutOrd;
        let mut payout_ord = PayoutOrd::from_vec(vec![
            (
                "2022-09-08 18:42:55.4636".to_string(),
                AtomicUnit::from_u64(1000000000),
                HumanNumber::from_u64(2654321),
            ),
            (
                "2022-09-09 16:18:26.7582".to_string(),
                AtomicUnit::from_u64(2000000000),
                HumanNumber::from_u64(2654322),
            ),
            (
                "2022-09-10 11:15:21.1272".to_string(),
                AtomicUnit::from_u64(3000000000),
                HumanNumber::from_u64(2654323),
            ),
        ]);
        let payout_ord_2 = payout_ord.clone();
        println!("1: {:#?}", payout_ord);
        println!("2: {:#?}", payout_ord);

        assert!(PayoutOrd::is_same(&payout_ord, &payout_ord_2));
        payout_ord.push_raw("2022-09-08 18:42:55.4636", 1000000000, 2654321);
        println!("1: {:#?}", payout_ord);
        println!("2: {:#?}", payout_ord);
        assert!(!PayoutOrd::is_same(&payout_ord, &payout_ord_2));
    }

    #[test]
    fn view_reverse_payout_ord() {
        use crate::human::HumanNumber;
        use crate::xmr::AtomicUnit;
        use crate::xmr::PayoutOrd;
        let payout_ord = PayoutOrd::from_vec(vec![
            (
                "2022-09-08 18:42:55.4636".to_string(),
                AtomicUnit::from_u64(1000000000),
                HumanNumber::from_u64(2654321),
            ),
            (
                "2022-09-09 16:18:26.7582".to_string(),
                AtomicUnit::from_u64(2000000000),
                HumanNumber::from_u64(2654322),
            ),
            (
                "2022-09-10 11:15:21.1272".to_string(),
                AtomicUnit::from_u64(3000000000),
                HumanNumber::from_u64(2654323),
            ),
        ]);
        println!("OG: {:#?}", payout_ord);

        #[allow(clippy::never_loop)]
        for (_, atomic_unit, _) in payout_ord.rev_iter() {
            if atomic_unit.to_u64() == 3000000000 {
                break;
            } else {
                println!("expected: 3000000000, found: {}", atomic_unit);
                panic!("not reversed");
            }
        }
    }
}
