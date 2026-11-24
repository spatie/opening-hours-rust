use thiserror::Error;

/// Invalid definitions and unsuccessful date/time searches.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A time is not HH:MM or 24:00.
    #[error("Invalid time: {0}")]
    InvalidTime(String),
    /// A range is not HH:MM-HH:MM.
    #[error("Invalid time range: {0}")]
    InvalidTimeRange(String),
    /// Two daily ranges overlap.
    #[error("Overlapping time ranges: {0} and {1}")]
    OverlappingTimeRanges(String, String),
    /// An English weekday is invalid.
    #[error("Invalid day name: {0}")]
    InvalidDayName(String),
    /// A calendar date is invalid.
    #[error("Invalid date: {0}")]
    InvalidDate(String),
    /// A day or date is defined more than once, or a range is reversed.
    #[error("Invalid or duplicate date range: {0}")]
    InvalidDateRange(String),
    /// A definition has the wrong shape.
    #[error("Invalid opening hours definition: {0}")]
    InvalidDefinition(String),
    /// A timezone is neither an IANA name nor an offset.
    #[error("Invalid timezone: {0}")]
    InvalidTimezone(String),
    /// Schema.org input is invalid.
    #[error("Invalid OpeningHoursSpecification: {0}")]
    InvalidSpecification(String),
    /// No boundary was found within the configured day limit.
    #[error("No matching date/time found within {0} days; increase set_day_limit")]
    MaximumLimitExceeded(u32),
    /// A search reached its explicit limit.
    #[error("Search limit reached")]
    SearchLimitReached,
    /// Arithmetic exceeded the supported calendar.
    #[error("Date/time outside the supported calendar")]
    DateOverflow,
}
