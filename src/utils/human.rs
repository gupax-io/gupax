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

//---------------------------------------------------------------------------------------------------- Constants
// The locale numbers are formatting in is English, which looks like: [1,000]
pub const LOCALE: num_format::Locale = num_format::Locale::en;
pub const ZERO_SECONDS: std::time::Duration = std::time::Duration::from_secs(0);

//---------------------------------------------------------------------------------------------------- [HumanTime]
// This converts a [std::time::Duration] into something more readable.
// Used for uptime display purposes: [7 years, 8 months, 15 days, 23 hours, 35 minutes, 1 second]
// Code taken from [https://docs.rs/humantime/] and edited to remove sub-second time, change spacing and some words.
use std::time::Duration;

use readable::num::Float;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HumanTime(Duration);

impl Default for HumanTime {
    fn default() -> Self {
        Self::new()
    }
}

impl HumanTime {
    #[inline]
    pub const fn new() -> HumanTime {
        HumanTime(ZERO_SECONDS)
    }

    #[inline]
    pub const fn into_human(d: Duration) -> HumanTime {
        HumanTime(d)
    }

    #[inline]
    pub const fn from_u64(u: u64) -> HumanTime {
        HumanTime(Duration::from_secs(u))
    }

    fn plural(started: &mut bool, name: &str, value: u64, separator: &str) -> String {
        // do not show time if value is 0 unless it is for seconds.
        let mut string = String::new();
        if value > 0 {
            if *started {
                string.push_str(separator);
            }
            string.push_str(&value.to_string());
            string.push(' ');
            string.push_str(name);
            if value > 1 {
                string.push('s');
            }
            *started = true;
        }
        string
    }
    pub fn display(&self, wrap: bool) -> String {
        let secs = self.0.as_secs();
        if secs == 0 {
            return String::from("0 second");
        }
        let separator = if wrap { "\n" } else { ", " };
        let years = secs / 31_557_600; // 365.25d
        let ydays = secs % 31_557_600;
        let months = ydays / 2_630_016; // 30.44d
        let mdays = ydays % 2_630_016;
        let days = mdays / 86400;
        let day_secs = mdays % 86400;
        let hours = day_secs / 3600;
        let minutes = day_secs % 3600 / 60;
        let seconds = day_secs % 60;
        let mut started = false;
        let mut string = String::new();
        string.push_str(&Self::plural(&mut started, "year", years, separator));
        string.push_str(&Self::plural(&mut started, "month", months, separator));
        string.push_str(&Self::plural(&mut started, "day", days, separator));
        string.push_str(&Self::plural(&mut started, "hour", hours, separator));
        string.push_str(&Self::plural(&mut started, "minute", minutes, separator));
        string.push_str(&Self::plural(&mut started, "second", seconds, separator));
        string
    }
}

//---------------------------------------------------------------------------------------------------- [HumanNumber]
// Human readable numbers.
// Float    | [1234.57] -> [1,234]                    | Casts as u64/u128, adds comma
// Unsigned | [1234567] -> [1,234,567]                | Adds comma
// Percent  | [99.123] -> [99.12%]                    | Truncates to 2 after dot, adds percent
// Percent  | [0.001]  -> [0%]                        | Rounds down, removes redundant zeros
// Hashrate | [123.0, 311.2, null] -> [123, 311, ???] | Casts, replaces null with [???]
// CPU Load | [12.0, 11.4, null] -> [12.0, 11.4, ???] | No change, just into [String] form
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct HumanNumber(String);

impl std::fmt::Display for HumanNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl HumanNumber {
    #[inline]
    pub fn unknown() -> Self {
        Self("???".to_string())
    }
    #[inline]
    pub fn from_str(s: &str) -> Self {
        Self(s.to_string())
    }
    #[inline]
    pub fn to_percent(f: f32) -> Self {
        if f < 0.01 {
            Self("0%".to_string())
        } else {
            Self(format!("{f:.2}%"))
        }
    }
    #[inline]
    #[cfg(test)]
    pub fn to_percent_3_point(f: f32) -> Self {
        Self(format!("{:.3}%", f))
    }
    #[inline]
    pub fn from_f64_to_percent_6_point(f: f64) -> Self {
        Self(format!("{f:.6}%"))
    }
    #[inline]
    #[cfg(test)]
    pub fn from_f32(f: f32) -> Self {
        let mut buf = num_format::Buffer::new();
        buf.write_formatted(&(f as u64), &LOCALE);
        Self(buf.as_str().to_string())
    }
    #[inline]
    #[cfg(test)]
    pub fn from_f64(f: f64) -> Self {
        let mut buf = num_format::Buffer::new();
        buf.write_formatted(&(f as u128), &LOCALE);
        Self(buf.as_str().to_string())
    }
    #[inline]
    #[cfg(test)]
    pub fn from_u16(u: u16) -> Self {
        let mut buf = num_format::Buffer::new();
        buf.write_formatted(&u, &LOCALE);
        Self(buf.as_str().to_string())
    }
    #[inline]
    pub fn from_u32(u: u32) -> Self {
        let mut buf = num_format::Buffer::new();
        buf.write_formatted(&u, &LOCALE);
        Self(buf.as_str().to_string())
    }
    #[inline]
    pub fn from_u64(u: u64) -> Self {
        let mut buf = num_format::Buffer::new();
        buf.write_formatted(&u, &LOCALE);
        Self(buf.as_str().to_string())
    }
    #[inline]
    #[cfg(test)]
    pub fn from_u128(u: u128) -> Self {
        let mut buf = num_format::Buffer::new();
        buf.write_formatted(&u, &LOCALE);
        Self(buf.as_str().to_string())
    }
    #[inline]
    pub fn from_hashrate(array: &[Option<u64>]) -> Self {
        let mut string = String::new();
        // let mut buf = num_format::Buffer::new();

        let mut n = 1;
        for i in array {
            match i {
                Some(f) => {
                    if *f == 0 {
                        string.push_str("[??? H/s]");
                    } else {
                        let f = *f as f64;
                        let (value, metric) = match f {
                            x if x >= 1000.0 => (Float::from_3(f / 1000.0), " K"),
                            x if x >= 1000000.0 => (Float::from_3(f / (1000.0 * 1000.0)), " M"),
                            _ => (Float::from_0(f), " "),
                        };
                        string.push('[');
                        // buf.write_formatted(&value, &LOCALE);
                        string.push_str(value.as_str());
                        string.push_str(metric);
                        string.push_str("H/s]");
                    }
                }
                None => string.push_str("[??? H/s]"),
            }
            if n != array.len() {
                string.push('\n');
                n += 1;
            } else {
                break;
            }
        }

        Self(string)
    }
    #[inline]
    pub fn from_load(array: [Option<f32>; 3]) -> Self {
        let mut string = "[".to_string();
        let mut n = 0;
        for i in array {
            match i {
                Some(f) => string.push_str(format!("{f:.2}").as_str()),
                None => string.push_str("???"),
            }
            if n != 2 {
                string.push_str(", ");
                n += 1;
            } else {
                string.push(']');
                break;
            }
        }
        Self(string)
    }
    // [1_000_000] -> [1.000 MH/s]
    #[inline]
    pub fn from_u64_to_megahash_3_point(hash: u64) -> Self {
        let hash = (hash as f64) / 1_000_000.0;
        let hash = format!("{hash:.3} MH/s");
        Self(hash)
    }
    // [1_000_000_000] -> [1.000 GH/s]
    #[inline]
    pub fn from_u64_to_gigahash_3_point(hash: u64) -> Self {
        let hash = (hash as f64) / 1_000_000_000.0;
        let hash = format!("{hash:.3} GH/s");
        Self(hash)
    }
    #[inline]
    pub fn from_f64_12_point(f: f64) -> Self {
        let f = format!("{f:.12}");
        Self(f)
    }
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

//---------------------------------------------------------------------------------------------------- TESTS
#[cfg(test)]
mod test {
    #[test]
    fn human_number() {
        use crate::human::HumanNumber;
        assert!(HumanNumber::to_percent(0.001).to_string() == "0%");
        assert!(HumanNumber::to_percent(12.123_123).to_string() == "12.12%");
        assert!(HumanNumber::to_percent_3_point(0.001).to_string() == "0.001%");
        assert!(
            HumanNumber::from_hashrate(&[Some(123), Some(11111), None]).to_string()
                == "[123 H/s]\n[11.111 KH/s]\n[??? H/s]"
        );
        assert!(
            HumanNumber::from_hashrate(&[None, Some(1), Some(123_123)]).to_string()
                == "[??? H/s]\n[1 H/s]\n[123.123 KH/s]"
        );
        assert!(
            HumanNumber::from_load([Some(123.1234), Some(321.321), None]).to_string()
                == "[123.12, 321.32, ???]"
        );
        assert!(
            HumanNumber::from_load([None, Some(4321.43), Some(1234.1)]).to_string()
                == "[???, 4321.43, 1234.10]"
        );
        assert!(HumanNumber::from_f32(123_123.125).to_string() == "123,123");
        assert!(HumanNumber::from_f64(123_123_123.123_123_12).to_string() == "123,123,123");
        assert!(HumanNumber::from_u16(1_000).to_string() == "1,000");
        assert!(HumanNumber::from_u16(65_535).to_string() == "65,535");
        assert!(HumanNumber::from_u32(65_536).to_string() == "65,536");
        assert!(HumanNumber::from_u32(100_000).to_string() == "100,000");
        assert!(HumanNumber::from_u32(1_000_000).to_string() == "1,000,000");
        assert!(HumanNumber::from_u32(10_000_000).to_string() == "10,000,000");
        assert!(HumanNumber::from_u32(100_000_000).to_string() == "100,000,000");
        assert!(HumanNumber::from_u32(1_000_000_000).to_string() == "1,000,000,000");
        assert!(HumanNumber::from_u32(4_294_967_295).to_string() == "4,294,967,295");
        assert!(HumanNumber::from_u64(4_294_967_296).to_string() == "4,294,967,296");
        assert!(HumanNumber::from_u64(10_000_000_000).to_string() == "10,000,000,000");
        assert!(HumanNumber::from_u64(100_000_000_000).to_string() == "100,000,000,000");
        assert!(HumanNumber::from_u64(1_000_000_000_000).to_string() == "1,000,000,000,000");
        assert!(HumanNumber::from_u64(10_000_000_000_000).to_string() == "10,000,000,000,000");
        assert!(HumanNumber::from_u64(100_000_000_000_000).to_string() == "100,000,000,000,000");
        assert!(
            HumanNumber::from_u64(1_000_000_000_000_000).to_string() == "1,000,000,000,000,000"
        );
        assert!(
            HumanNumber::from_u64(10_000_000_000_000_000).to_string() == "10,000,000,000,000,000"
        );
        assert!(
            HumanNumber::from_u64(18_446_744_073_709_551_615).to_string()
                == "18,446,744,073,709,551,615"
        );
        assert!(
            HumanNumber::from_u128(18_446_744_073_709_551_616).to_string()
                == "18,446,744,073,709,551,616"
        );
        assert!(
            HumanNumber::from_u128(100_000_000_000_000_000_000).to_string()
                == "100,000,000,000,000,000,000"
        );
        assert_eq!(
            HumanNumber::from_u128(340_282_366_920_938_463_463_374_607_431_768_211_455).to_string(),
            "340,282,366,920,938,463,463,374,607,431,768,211,455",
        );
        assert!(
            HumanNumber::from_u64_to_gigahash_3_point(1_000_000_000).to_string() == "1.000 GH/s"
        );
    }

    #[test]
    fn human_time() {
        use crate::human::HumanTime;
        use std::time::Duration;
        assert!(HumanTime::into_human(Duration::from_secs(0)).display(true) == "0 second");
        assert!(HumanTime::into_human(Duration::from_secs(1)).display(true) == "1 second");
        assert!(HumanTime::into_human(Duration::from_secs(2)).display(true) == "2 seconds");
        assert!(HumanTime::into_human(Duration::from_secs(59)).display(true) == "59 seconds");
        assert!(HumanTime::into_human(Duration::from_secs(60)).display(true) == "1 minute");
        assert!(
            HumanTime::into_human(Duration::from_secs(61)).display(true) == "1 minute\n1 second"
        );
        assert!(
            HumanTime::into_human(Duration::from_secs(62)).display(true) == "1 minute\n2 seconds"
        );
        assert!(HumanTime::into_human(Duration::from_secs(120)).display(true) == "2 minutes");
        assert!(
            HumanTime::into_human(Duration::from_secs(121)).display(true) == "2 minutes\n1 second"
        );
        assert!(
            HumanTime::into_human(Duration::from_secs(122)).display(true) == "2 minutes\n2 seconds"
        );
        assert!(
            HumanTime::into_human(Duration::from_secs(179)).display(true)
                == "2 minutes\n59 seconds"
        );
        assert!(
            HumanTime::into_human(Duration::from_secs(3599)).display(true)
                == "59 minutes\n59 seconds"
        );
        assert!(HumanTime::into_human(Duration::from_secs(3600)).display(true) == "1 hour");
        assert!(
            HumanTime::into_human(Duration::from_secs(3601)).display(true) == "1 hour\n1 second"
        );
        assert!(
            HumanTime::into_human(Duration::from_secs(3602)).display(true) == "1 hour\n2 seconds"
        );
        assert!(
            HumanTime::into_human(Duration::from_secs(3660)).display(true) == "1 hour\n1 minute"
        );
        assert!(
            HumanTime::into_human(Duration::from_secs(3720)).display(true) == "1 hour\n2 minutes"
        );
        assert!(
            HumanTime::into_human(Duration::from_secs(86399)).display(true)
                == "23 hours\n59 minutes\n59 seconds"
        );
        assert!(HumanTime::into_human(Duration::from_secs(86400)).display(true) == "1 day");
        assert!(
            HumanTime::into_human(Duration::from_secs(86401)).display(true) == "1 day\n1 second"
        );
        assert!(
            HumanTime::into_human(Duration::from_secs(86402)).display(true) == "1 day\n2 seconds"
        );
        assert!(
            HumanTime::into_human(Duration::from_secs(86460)).display(true) == "1 day\n1 minute"
        );
        assert!(
            HumanTime::into_human(Duration::from_secs(86520)).display(true) == "1 day\n2 minutes"
        );
        assert!(HumanTime::into_human(Duration::from_secs(90000)).display(true) == "1 day\n1 hour");
        assert!(
            HumanTime::into_human(Duration::from_secs(93600)).display(true) == "1 day\n2 hours"
        );
        assert!(
            HumanTime::into_human(Duration::from_secs(604799)).display(true)
                == "6 days\n23 hours\n59 minutes\n59 seconds"
        );
        assert!(HumanTime::into_human(Duration::from_secs(604800)).display(true) == "7 days");
        assert!(HumanTime::into_human(Duration::from_secs(2630016)).display(true) == "1 month");
        assert!(
            HumanTime::into_human(Duration::from_secs(3234815)).display(true)
                == "1 month\n6 days\n23 hours\n59 minutes\n59 seconds"
        );
        assert!(HumanTime::into_human(Duration::from_secs(5260032)).display(true) == "2 months");
        assert!(HumanTime::into_human(Duration::from_secs(31557600)).display(true) == "1 year");
        assert!(HumanTime::into_human(Duration::from_secs(63115200)).display(true) == "2 years");
        assert_eq!(
            HumanTime::into_human(Duration::from_secs(18446744073709551615)).display(true),
            "584542046090 years\n7 months\n15 days\n17 hours\n5 minutes\n3 seconds"
        );
    }
}
