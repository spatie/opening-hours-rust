#![doc = include_str!("../README.md")]

mod day;
mod error;
mod schedule;
mod time;
mod timezone;

pub use day::{Day, OpeningHoursForDay};
pub use error::Error;
pub use schedule::{DateFilter, DateTimeRange, OpeningHours, SearchOptions, WeekGroup};
pub use time::{PreciseTime, Time, TimeRange};
pub use timezone::Timezone;

/// Date and time types used by this crate.
pub use chrono;
/// IANA timezone names and daylight-saving rules.
pub use chrono_tz;
/// JSON definitions and metadata.
pub use serde_json;

#[cfg(doctest)]
mod guide {
    include!(concat!(env!("OUT_DIR"), "/documentation.rs"));
}
