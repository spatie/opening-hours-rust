use crate::{Error, PreciseTime, Time, TimeRange};
use chrono::{DateTime, Datelike, TimeZone};
use serde_json::Value;
use std::{fmt, ops::Index, str::FromStr};

/// An English weekday, ordered Monday through Sunday.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum Day {
    /// Monday.
    Monday,
    /// Tuesday.
    Tuesday,
    /// Wednesday.
    Wednesday,
    /// Thursday.
    Thursday,
    /// Friday.
    Friday,
    /// Saturday.
    Saturday,
    /// Sunday.
    Sunday,
}
impl Day {
    /// All weekdays in ISO order.
    pub const ALL: [Self; 7] = [
        Self::Monday,
        Self::Tuesday,
        Self::Wednesday,
        Self::Thursday,
        Self::Friday,
        Self::Saturday,
        Self::Sunday,
    ];
    /// Parses a case-insensitive English name.
    pub fn from_name(name: &str) -> Result<Self, Error> {
        name.parse()
    }
    /// The weekday of a local date/time.
    pub fn on_date_time<T: TimeZone>(date: &DateTime<T>) -> Self {
        Self::ALL[date.weekday().num_days_from_monday() as usize]
    }
    /// ISO weekday number (Monday = 1).
    pub fn to_iso(self) -> u8 {
        self as u8 + 1
    }
    /// Lowercase English weekday name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Monday => "monday",
            Self::Tuesday => "tuesday",
            Self::Wednesday => "wednesday",
            Self::Thursday => "thursday",
            Self::Friday => "friday",
            Self::Saturday => "saturday",
            Self::Sunday => "sunday",
        }
    }
}
impl FromStr for Day {
    type Err = Error;
    fn from_str(name: &str) -> Result<Self, Error> {
        Self::ALL
            .into_iter()
            .find(|d| d.name().eq_ignore_ascii_case(name))
            .ok_or_else(|| Error::InvalidDayName(name.into()))
    }
}
impl fmt::Display for Day {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A read-only, sorted collection of non-overlapping daily ranges.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpeningHoursForDay {
    ranges: Vec<TimeRange>,
    data: Value,
}
impl OpeningHoursForDay {
    /// Parses a list of HH:MM-HH:MM strings.
    pub fn from_strings<I, S>(strings: I) -> Result<Self, Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Self::from_ranges(
            strings
                .into_iter()
                .map(|s| TimeRange::from_string(s.as_ref()))
                .collect::<Result<Vec<_>, _>>()?,
        )
    }
    /// Sorts and validates ranges. Touching ranges are allowed, overlapping ranges are not.
    pub fn from_ranges(mut ranges: Vec<TimeRange>) -> Result<Self, Error> {
        ranges.sort_by_key(|range| range.start().nanos());
        for (i, range) in ranges.iter().enumerate() {
            for other in &ranges[i + 1..] {
                if range.overlaps(other) {
                    return Err(Error::OverlappingTimeRanges(
                        range.to_string(),
                        other.to_string(),
                    ));
                }
            }
        }
        Ok(Self {
            ranges,
            data: Value::Null,
        })
    }
    /// Parses a JSON array or an object with `hours` and optional `data`.
    pub fn from_definition(value: &Value) -> Result<Self, Error> {
        let (hours, data) = if let Some(object) = value.as_object() {
            (
                object.get("hours"),
                object.get("data").cloned().unwrap_or_default(),
            )
        } else {
            (Some(value), Value::Null)
        };
        let ranges = match hours {
            None => Vec::new(),
            Some(Value::Array(values)) => values
                .iter()
                .map(TimeRange::from_definition)
                .collect::<Result<_, _>>()?,
            _ => return Err(Error::InvalidDefinition(value.to_string())),
        };
        Ok(Self::from_ranges(ranges)?.with_data(data))
    }
    /// Attaches day-level metadata.
    #[must_use]
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }
    /// Day-level metadata.
    pub fn data(&self) -> &Value {
        &self.data
    }
    /// Number of ranges.
    pub fn len(&self) -> usize {
        self.ranges.len()
    }
    /// Whether the day is closed all day.
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }
    /// Iterates in opening-time order.
    pub fn iter(&self) -> std::slice::Iter<'_, TimeRange> {
        self.ranges.iter()
    }
    /// Whether the opening-day portion contains a time.
    pub fn is_open_at(&self, time: impl Into<PreciseTime>) -> bool {
        let time = time.into();
        self.ranges.iter().any(|r| r.contains_time(time))
    }
    /// Whether the following-morning portion contains a time.
    pub fn is_open_at_night(&self, time: impl Into<PreciseTime>) -> bool {
        let time = time.into();
        self.ranges.iter().any(|r| r.contains_night_time(time))
    }
    /// Whether the day is open at 23:59.
    pub fn is_open_at_the_end_of_the_day(&self) -> bool {
        self.is_open_at(PreciseTime::from(&Time::from_minutes(1439)))
    }
    /// All ranges containing a time on their opening day.
    pub fn for_time(&self, time: impl Into<PreciseTime>) -> impl Iterator<Item = &TimeRange> {
        let time = time.into();
        self.iter().filter(move |r| r.contains_time(time))
    }
    /// All overnight ranges containing a time on the following day.
    pub fn for_night_time(&self, time: impl Into<PreciseTime>) -> impl Iterator<Item = &TimeRange> {
        let time = time.into();
        self.iter().filter(move |r| r.contains_night_time(time))
    }
    /// First opening strictly after the time.
    pub fn next_open(&self, time: impl Into<PreciseTime>) -> Option<&Time> {
        self.next_open_range(time).map(TimeRange::start)
    }
    /// First range opening strictly after the time.
    pub fn next_open_range(&self, time: impl Into<PreciseTime>) -> Option<&TimeRange> {
        let time = time.into().nanos();
        self.iter().find(|r| r.start().nanos() > time)
    }
    /// Closing time of the current or next range.
    pub fn next_close(&self, time: impl Into<PreciseTime>) -> Option<&Time> {
        self.next_close_range(time).map(TimeRange::end)
    }
    /// Current or next range.
    pub fn next_close_range(&self, time: impl Into<PreciseTime>) -> Option<&TimeRange> {
        let time = time.into();
        self.iter()
            .find(|r| r.contains_time(time) || r.start().nanos() > time.nanos())
    }
    /// Last opening strictly before the time.
    pub fn previous_open(&self, time: impl Into<PreciseTime>) -> Option<&Time> {
        let time = time.into().nanos();
        self.iter()
            .rev()
            .find(|r| r.start().nanos() < time)
            .map(TimeRange::start)
    }
    /// PHP's reverse range search: the last range starting strictly after the time.
    pub fn previous_open_range(&self, time: impl Into<PreciseTime>) -> Option<&TimeRange> {
        let time = time.into().nanos();
        self.iter().rev().find(|r| r.start().nanos() > time)
    }
    /// Last closing strictly before the time.
    pub fn previous_close(&self, time: impl Into<PreciseTime>) -> Option<&Time> {
        let time = time.into().nanos();
        self.iter()
            .rev()
            .find(|r| r.end().nanos() < time)
            .map(TimeRange::end)
    }
    /// Last range with both endpoints strictly before the time.
    pub fn previous_close_range(&self, time: impl Into<PreciseTime>) -> Option<&TimeRange> {
        let time = time.into().nanos();
        self.iter()
            .rev()
            .find(|r| r.start().nanos() < time && r.end().nanos() < time)
    }
    /// Transforms ranges using the standard iterator API.
    pub fn map<T>(&self, callback: impl FnMut(&TimeRange) -> T) -> Vec<T> {
        self.iter().map(callback).collect()
    }
    /// Finds the first callback result, optionally visiting ranges in reverse.
    pub fn opening_hours_filter<T>(
        &self,
        mut callback: impl FnMut(&TimeRange) -> Option<T>,
        reverse: bool,
    ) -> Option<T> {
        if reverse {
            self.iter().rev().find_map(callback)
        } else {
            self.iter().find_map(&mut callback)
        }
    }
}
impl Index<usize> for OpeningHoursForDay {
    type Output = TimeRange;
    fn index(&self, i: usize) -> &TimeRange {
        &self.ranges[i]
    }
}
impl<'a> IntoIterator for &'a OpeningHoursForDay {
    type Item = &'a TimeRange;
    type IntoIter = std::slice::Iter<'a, TimeRange>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl fmt::Display for OpeningHoursForDay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, range) in self.ranges.iter().enumerate() {
            if i > 0 {
                f.write_str(",")?;
            }
            write!(f, "{range}")?;
        }
        Ok(())
    }
}
