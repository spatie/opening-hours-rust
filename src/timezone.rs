use crate::Error;
use chrono::{DateTime, Duration, FixedOffset, LocalResult, NaiveDateTime, Offset, TimeZone, Utc};
use std::str::FromStr;

/// An IANA timezone or a fixed UTC offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timezone {
    /// A timezone with historical daylight-saving rules.
    Named(chrono_tz::Tz),
    /// A constant offset from UTC.
    Fixed(FixedOffset),
}

impl From<chrono_tz::Tz> for Timezone {
    fn from(value: chrono_tz::Tz) -> Self {
        Self::Named(value)
    }
}
impl From<FixedOffset> for Timezone {
    fn from(value: FixedOffset) -> Self {
        Self::Fixed(value)
    }
}
impl FromStr for Timezone {
    type Err = Error;
    fn from_str(value: &str) -> Result<Self, Error> {
        if let Ok(zone) = value.parse() {
            return Ok(Self::Named(zone));
        }
        if let Ok(zone) = value.parse() {
            return Ok(Self::Fixed(zone));
        }
        Err(Error::InvalidTimezone(value.into()))
    }
}
impl Timezone {
    pub(crate) fn at(self, instant: DateTime<FixedOffset>) -> DateTime<FixedOffset> {
        match self {
            Self::Named(zone) => instant.with_timezone(&zone).fixed_offset(),
            Self::Fixed(zone) => instant.with_timezone(&zone),
        }
    }

    pub(crate) fn resolve(self, local: NaiveDateTime) -> Result<DateTime<FixedOffset>, Error> {
        match self {
            Self::Fixed(zone) => resolve_local(&zone, local),
            Self::Named(zone) => resolve_local(&zone, local),
        }
    }
}

pub(crate) fn resolve_local<T: TimeZone>(
    zone: &T,
    local: NaiveDateTime,
) -> Result<DateTime<FixedOffset>, Error> {
    match zone.from_local_datetime(&local) {
        LocalResult::Single(date) => Ok(date.fixed_offset()),
        LocalResult::Ambiguous(earlier, _) => Ok(earlier.fixed_offset()),
        LocalResult::None => {
            // PHP normalizes a missing wall time forward by the size of the DST gap.
            for minutes in 1..=1440 {
                let before = local
                    .checked_sub_signed(Duration::minutes(minutes))
                    .ok_or(Error::DateOverflow)?;
                if let Some(previous) = zone.from_local_datetime(&before).earliest() {
                    let utc = local
                        .checked_sub_signed(Duration::seconds(i64::from(
                            previous.offset().fix().local_minus_utc(),
                        )))
                        .ok_or(Error::DateOverflow)?;
                    return Ok(DateTime::<Utc>::from_naive_utc_and_offset(utc, Utc)
                        .with_timezone(zone)
                        .fixed_offset());
                }
            }
            Err(Error::DateOverflow)
        }
    }
}
