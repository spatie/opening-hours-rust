use crate::Error;
use chrono::{DateTime, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike, Utc};
use serde_json::Value;
use std::{fmt, str::FromStr};

/// A minute-precision wall time, including the special end-of-day time 24:00.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Time {
    minutes: u16,
    data: Value,
}

impl Time {
    pub(crate) fn from_minutes(minutes: u16) -> Self {
        Self {
            minutes,
            data: Value::Null,
        }
    }
    /// Parses HH:MM. Hours and minutes must have two digits.
    pub fn from_string(value: &str) -> Result<Self, Error> {
        value.parse()
    }
    /// Extracts the hour and minute from a date/time.
    pub fn from_date_time<T: TimeZone>(date: &DateTime<T>) -> Self {
        Self {
            minutes: (date.hour() * 60 + date.minute()) as u16,
            data: Value::Null,
        }
    }
    /// Attaches arbitrary JSON metadata.
    #[must_use]
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }
    /// Attached metadata.
    pub fn data(&self) -> &Value {
        &self.data
    }
    /// Hour, from 0 through 24.
    pub fn hours(&self) -> u32 {
        u32::from(self.minutes / 60)
    }
    /// Minute within the hour.
    pub fn minutes(&self) -> u32 {
        u32::from(self.minutes % 60)
    }
    /// Compares clock time, ignoring metadata.
    pub fn is_same(&self, other: &Self) -> bool {
        self.minutes == other.minutes
    }
    /// Whether this time is later.
    pub fn is_after(&self, other: &Self) -> bool {
        self.minutes > other.minutes
    }
    /// Whether this time is earlier.
    pub fn is_before(&self, other: &Self) -> bool {
        self.minutes < other.minutes
    }
    /// Whether this time is the same or later.
    pub fn is_same_or_after(&self, other: &Self) -> bool {
        self.minutes >= other.minutes
    }
    /// Whether this time is the same or earlier.
    pub fn is_same_or_before(&self, other: &Self) -> bool {
        self.minutes <= other.minutes
    }
    /// Signed difference from this time to another.
    pub fn diff(&self, other: &Self) -> Duration {
        Duration::minutes(i64::from(other.minutes) - i64::from(self.minutes))
    }
    /// Applies this wall time to a date; 24:00 becomes midnight on the following day.
    pub fn to_date_time(&self, date: NaiveDate) -> Result<NaiveDateTime, Error> {
        date.and_hms_opt(0, 0, 0)
            .and_then(|d| d.checked_add_signed(Duration::minutes(i64::from(self.minutes))))
            .ok_or(Error::DateOverflow)
    }
    /// Formats using chrono/strftime directives, preserving 24:00 with `%H:%M`.
    ///
    /// # Panics
    /// Panics on invalid format directives, like chrono's `DateTime::format`.
    pub fn format(&self, format: &str) -> String {
        let time = DateTime::<Utc>::UNIX_EPOCH + Duration::minutes(i64::from(self.minutes));
        if self.minutes == 1440 && format.starts_with("%H:%M") {
            format!("24:00{}", time.format(&format[5..]))
        } else {
            time.format(format).to_string()
        }
    }
    pub(crate) fn nanos(&self) -> u64 {
        u64::from(self.minutes) * 60_000_000_000
    }
}
impl FromStr for Time {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self, Error> {
        let bytes = value.as_bytes();
        if bytes.len() != 5
            || bytes[2] != b':'
            || ![bytes[0], bytes[1], bytes[3], bytes[4]]
                .iter()
                .all(u8::is_ascii_digit)
        {
            return Err(Error::InvalidTime(value.into()));
        }
        let hour = u16::from(bytes[0] - b'0') * 10 + u16::from(bytes[1] - b'0');
        let minute = u16::from(bytes[3] - b'0') * 10 + u16::from(bytes[4] - b'0');
        if hour > 24 || minute > 59 || (hour == 24 && minute != 0) {
            return Err(Error::InvalidTime(value.into()));
        }
        Ok(Self {
            minutes: hour * 60 + minute,
            data: Value::Null,
        })
    }
}
impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.hours(), self.minutes())
    }
}

/// A wall time retaining seconds and nanoseconds for precise boundary comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PreciseTime(u64);
impl PreciseTime {
    /// Extracts the exact local wall time.
    pub fn from_date_time<T: TimeZone>(date: &DateTime<T>) -> Self {
        Self(
            u64::from(date.num_seconds_from_midnight()) * 1_000_000_000
                + u64::from(date.nanosecond()),
        )
    }
    /// Parses a time using chrono's NaiveTime parser.
    pub fn from_string(value: &str) -> Result<Self, Error> {
        let time: NaiveTime = value
            .parse()
            .map_err(|_| Error::InvalidTime(value.into()))?;
        Ok(Self(
            u64::from(time.num_seconds_from_midnight()) * 1_000_000_000
                + u64::from(time.nanosecond()),
        ))
    }
    /// Hour within the day.
    pub fn hours(self) -> u32 {
        (self.0 / 3_600_000_000_000) as u32
    }
    /// Minute within the hour.
    pub fn minutes(self) -> u32 {
        (self.0 / 60_000_000_000 % 60) as u32
    }
    /// Whether clock times are equal, including subsecond precision.
    pub fn is_same(self, other: impl Into<Self>) -> bool {
        self == other.into()
    }
    /// Whether this time is strictly later.
    pub fn is_after(self, other: impl Into<Self>) -> bool {
        self > other.into()
    }
    /// Whether this time is strictly earlier.
    pub fn is_before(self, other: impl Into<Self>) -> bool {
        self < other.into()
    }
    /// Whether this time is the same or later.
    pub fn is_same_or_after(self, other: impl Into<Self>) -> bool {
        self >= other.into()
    }
    /// Whether this time is the same or earlier.
    pub fn is_same_or_before(self, other: impl Into<Self>) -> bool {
        self <= other.into()
    }
    /// Signed clock-time difference, including nanoseconds.
    pub fn diff(self, other: impl Into<Self>) -> Duration {
        Duration::nanoseconds(other.into().0 as i64 - self.0 as i64)
    }
    /// Applies this precise wall time to a calendar date.
    pub fn to_date_time(self, date: NaiveDate) -> Result<NaiveDateTime, Error> {
        date.and_hms_opt(0, 0, 0)
            .and_then(|date| date.checked_add_signed(Duration::nanoseconds(self.0 as i64)))
            .ok_or(Error::DateOverflow)
    }
    /// Formats the time using chrono/strftime directives on the UTC epoch date.
    ///
    /// # Panics
    /// Panics on invalid format directives, like chrono's `DateTime::format`.
    pub fn format(self, format: &str) -> String {
        (DateTime::<Utc>::UNIX_EPOCH + Duration::nanoseconds(self.0 as i64))
            .format(format)
            .to_string()
    }
    pub(crate) fn nanos(self) -> u64 {
        self.0
    }
}
impl From<&Time> for PreciseTime {
    fn from(time: &Time) -> Self {
        Self(time.nanos())
    }
}

/// A half-open wall-time interval. Reversed ranges continue after midnight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeRange {
    start: Time,
    end: Time,
    data: Value,
}
impl TimeRange {
    /// Parses HH:MM-HH:MM.
    pub fn from_string(value: &str) -> Result<Self, Error> {
        value.parse()
    }
    /// Creates a range from an hours string or an object with `hours` and `data`.
    pub fn from_definition(value: &Value) -> Result<Self, Error> {
        if let Some(value) = value.as_str() {
            return value.parse();
        }
        let (hours, data) = if let Some(object) = value.as_object() {
            (object.get("hours"), object.get("data"))
        } else if let Some(array) = value.as_array() {
            (array.first(), array.get(1))
        } else {
            return Err(Error::InvalidDefinition(value.to_string()));
        };
        Ok(Self::from_string(
            hours
                .and_then(Value::as_str)
                .ok_or_else(|| Error::InvalidDefinition(value.to_string()))?,
        )?
        .with_data(data.cloned().unwrap_or_default()))
    }
    /// Smallest range spanning a nonempty list of ranges.
    pub fn from_list(ranges: &[Self]) -> Result<Self, Error> {
        let first = ranges
            .first()
            .ok_or_else(|| Error::InvalidTimeRange("empty list".into()))?;
        let mut result = first.clone();
        for range in &ranges[1..] {
            if range.start.is_before(&result.start) {
                result.start = range.start.clone();
            }
            if range.end.is_after(&result.end) {
                result.end = range.end.clone();
            }
        }
        Ok(result)
    }
    /// A range from midnight until the given end.
    pub fn from_midnight(end: Time) -> Self {
        Self {
            start: Time {
                minutes: 0,
                data: Value::Null,
            },
            end,
            data: Value::Null,
        }
    }
    /// Attaches metadata.
    #[must_use]
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = data;
        self
    }
    /// Attached metadata.
    pub fn data(&self) -> &Value {
        &self.data
    }
    /// Inclusive opening time.
    pub fn start(&self) -> &Time {
        &self.start
    }
    /// Exclusive closing time.
    pub fn end(&self) -> &Time {
        &self.end
    }
    /// Whether the end is earlier than the start.
    pub fn is_reversed(&self) -> bool {
        self.start.is_after(&self.end)
    }
    /// Whether this range continues into the following day.
    pub fn overflows_next_day(&self) -> bool {
        self.is_reversed()
    }
    /// Whether the time belongs to the opening day portion of the range.
    pub fn contains_time(&self, time: impl Into<PreciseTime>) -> bool {
        let time = time.into().nanos();
        time >= self.start.nanos() && (self.is_reversed() || time < self.end.nanos())
    }
    /// Whether the time belongs to the following morning portion.
    pub fn contains_night_time(&self, time: impl Into<PreciseTime>) -> bool {
        self.is_reversed() && time.into().nanos() < self.end.nanos()
    }
    /// PHP's directional overlap check (includes the other end, excludes this end).
    pub fn overlaps(&self, other: &Self) -> bool {
        self.contains_time(&other.start) || self.contains_time(&other.end)
    }
    /// Formats both endpoints, joined by a separator.
    pub fn format(&self, time_format: &str, separator: &str) -> String {
        format!(
            "{}{}{}",
            self.start.format(time_format),
            separator,
            self.end.format(time_format)
        )
    }
}
impl FromStr for TimeRange {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self, Error> {
        let (start, end) = value
            .split_once('-')
            .ok_or_else(|| Error::InvalidTimeRange(value.into()))?;
        Ok(Self {
            start: start.parse()?,
            end: end.parse()?,
            data: Value::Null,
        })
    }
}
impl fmt::Display for TimeRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}
