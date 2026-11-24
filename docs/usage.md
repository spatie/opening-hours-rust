---
title: Usage
weight: 4
---

The package should be used through the `OpeningHours` class. There are also value objects used throughout: `Time` represents a single time, `TimeRange` represents a period with a start and an end, and `OpeningHoursForDay` represents a set of ranges which can't overlap.

## Creating opening hours

Static factory method to fill the set of opening hours. Not all days are mandatory; if a day is missing, it will be set as closed.

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use spatie_opening_hours::{OpeningHours, serde_json::json};

let opening_hours = OpeningHours::create(json!({
    "monday": ["09:00-12:00", "13:00-18:00"],
    "exceptions": {
        "2016-12-25": [],
        "01-01": [],
        "12-25": ["09:00-12:00"]
    }
}))?;
# assert!(opening_hours.is_open_on("monday")?);
# assert!(opening_hours.is_closed_on("2016-12-25")?);
# Ok(())
# }
```

Definitions use JSON values, usually built with `json!`. Exceptions with a full date take precedence over recurring month/day exceptions. Filters take precedence over both.

### Timezones

If no timezone is specified, `OpeningHours` assumes you pass date/time objects in the timezone matching your schedule.

If you pass a `timezone`, input dates are converted to this timezone. Returned dates are converted back to the original input timezone. Alternatively you can specify both input and output timezone:

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# use spatie_opening_hours::{OpeningHours, chrono::DateTime, serde_json::json};
let opening_hours = OpeningHours::create(json!({
    "monday": ["09:00-12:00", "13:00-18:00"],
    "timezone": { "input": "America/New_York", "output": "Europe/Oslo" }
}))?;
# let date = DateTime::parse_from_rfc3339("2024-11-18T13:00:00+00:00")?;
# assert_eq!(opening_hours.next_open(&date)?.to_rfc3339(), "2024-11-18T15:00:00+01:00");
# Ok(())
# }
```

`set_timezone(Some("Europe/Brussels".parse()?))` and `set_output_timezone` can update these settings. IANA names carry daylight-saving rules; numeric offsets such as `-05:00` stay fixed. Without an explicit business timezone, pass a `DateTime<chrono_tz::Tz>` to retain daylight-saving rules when searching across dates. Returned values are `DateTime<FixedOffset>` and preserve the offset at the returned instant. Every search path applies the output timezone, including the current range's closing time.

### Merging overlapping ranges

For safety's sake, creating opening hours with overlapping daily ranges returns an error. You can explicitly merge them:

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# use spatie_opening_hours::{OpeningHours, Day, serde_json::json};
let opening_hours = OpeningHours::create_and_merge_overlapping_ranges(json!({
    "monday": ["08:00-11:00", "10:00-12:00"]
}), true)?;
# assert_eq!(opening_hours.for_day(Day::Monday).to_string(), "08:00-12:00");
# Ok(())
# }
```

`merge_overlapping_ranges(definition, ignore_data)` returns a merged definition. With `ignore_data = false`, only ranges with equal metadata are merged. Ranges left overlapping because their metadata differs still cannot be used to create a day.

### Overnight hours

On construction, you can set a flag for overflowing times across days. For example, a nightclub opens until 3am on Friday and Saturday:

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# use spatie_opening_hours::{OpeningHours, chrono::DateTime, serde_json::json};
let opening_hours = OpeningHours::create(json!({
    "overflow": true,
    "friday": ["20:00-03:00"],
    "saturday": ["20:00-03:00"]
}))?;
# let date = DateTime::parse_from_rfc3339("2024-11-23T02:00:00+00:00")?;
# assert!(opening_hours.is_open_at(&date));
# assert_eq!(opening_hours.current_open_range_end(&date)?.unwrap().to_rfc3339(), "2024-11-23T03:00:00+00:00");
# Ok(())
# }
```

This allows the API to look at the previous day's data. You can also split a night at midnight using `22:00-24:00` followed by `00:00-07:00` on the next day. A range ending at `24:00` closes at midnight on the following day.

### Data

You can add data in definitions then retrieve them. Data can be any JSON value, including nested objects and arrays.

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# use spatie_opening_hours::{OpeningHours, Day, serde_json::json};
let opening_hours = OpeningHours::create(json!({
    "monday": {
        "hours": ["09:00-12:00", "13:00-18:00"],
        "data": "Typical Monday"
    },
    "tuesday": [
        "09:00-12:00", "13:00-18:00",
        {"hours": "19:00-21:00", "data": "Extra on Tuesday evening"}
    ],
    "exceptions": {"2016-12-25": {"hours": [], "data": "Closed for Christmas"}}
}))?;

assert_eq!(opening_hours.for_day(Day::Monday).data(), "Typical Monday");
assert_eq!(opening_hours.for_day(Day::Tuesday)[2].data(), "Extra on Tuesday evening");
# Ok(())
# }
```

### Day and date ranges

You can use the separator `to` to specify multiple days at once, for the week or for exceptions:

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# use spatie_opening_hours::{OpeningHours, serde_json::json};
let opening_hours = OpeningHours::create(json!({
    "monday to friday": ["09:00-19:00"],
    "saturday to sunday": [],
    "exceptions": {
        "12-24 to 12-26": {"hours": [], "data": "Holidays"},
        "2024-06-25 to 2024-07-01": {"hours": [], "data": "Closed for works"}
    }
}))?;
# assert!(opening_hours.is_closed_on("2024-06-26")?);
# assert!(opening_hours.is_open_on("2024-07-02")?);
# Ok(())
# }
```

Dash-separated forms are also supported, such as `monday - wednesday` and `11-30-12-01`. Duplicate definitions, including overlapping day/date ranges, return an error. Recurring date ranges use a leap year so `02-29` is valid. Reversed calendar-date ranges are rejected; split year-end holidays into two ranges.

### Filters

A filter takes a date and returns settings for that date. The first filter returning `Some` wins. `None` means the filter does not apply. `Some(OpeningHoursForDay::default())` means closed all day.

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use spatie_opening_hours::{OpeningHours, OpeningHoursForDay, chrono::Datelike, serde_json::json};

let mut opening_hours = OpeningHours::create(json!({"monday": ["09:00-12:00"]}))?;
opening_hours.add_filter(|date| {
    if date.month() == 12 && date.day() == 25 {
        return Some(OpeningHoursForDay::default());
    }
    None
});
# assert!(opening_hours.is_closed_on("2023-12-25")?);
# Ok(())
# }
```

Filters are closures registered in Rust, not JSON values. Register filters from PHP's `filters` list first, followed by any callable exceptions, to retain their precedence. Filters run on each query and are not cached; slow filters can have a significant performance cost. `get_filters()` returns the installed filters.

## Querying the schedule

### For a week, day or date

`for_week()` returns weekday/hour pairs for a regular week. `for_week_combined()` groups days with the same hours. `for_week_consecutive_days()` groups adjacent days with the same hours, from Monday to Sunday without looping.

`for_day(Day::Monday)` returns regular hours. `for_day_name("monday")` accepts case-insensitive English names. `for_date(&date)` applies exceptions and filters. `exceptions()` returns all exception keys and their hours. `for_date_time(&date)` returns matching ranges, including the previous night's ranges.

`is_open_on("saturday")` checks the regular schedule. An ISO date such as `2020-09-03` checks exceptions too. `09-03` uses the current year. `is_closed_on` is its inverse.

### At a specific time

`is_open_at(&date)` checks a specific instant, with subsecond precision. `is_closed_at` is its inverse. `is_open()` and `is_closed()` check right now.

`is_always_open()` checks whether all regular and exceptional days are `00:00-24:00` and there are no filters. `is_always_closed()` checks whether all are empty and there are no filters. If an empty schedule is invalid in your domain, use this method to report an error.

`regular_closing_days()` returns typed weekdays; `regular_closing_days_iso()` returns ISO weekday numbers. `exceptional_closing_dates()` returns closed exception keys, including recurring dates.

### Next and previous openings and closings

Returns the next open or close date/time from a given date/time.

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use spatie_opening_hours::{OpeningHours, chrono::DateTime, serde_json::json};

let opening_hours = OpeningHours::create(json!({
    "monday to saturday": ["09:00-12:00", "13:00-18:00"],
    "exceptions": {"2016-12-25": []}
}))?;
let date = DateTime::parse_from_rfc3339("2016-12-25T10:00:00+00:00")?;

let next_open = opening_hours.next_open(&date)?;
assert_eq!(next_open.to_rfc3339(), "2016-12-26T09:00:00+00:00");
# assert_eq!(opening_hours.next_close(&date)?.to_rfc3339(), "2016-12-26T12:00:00+00:00");
# Ok(())
# }
```

`next_open` skips the current range. `next_close` includes its end. `previous_open` and `previous_close` search backwards. PHP's midnight and overnight placement rules are retained; a previous boundary at midnight can equal the input. Use `chrono::Local::now()` when you want to search from now.

The default search horizon is 8 days, or 366 if exceptions are present. `set_day_limit` increases it. When no result is found, these methods return `Error::MaximumLimitExceeded`.

The `_with` forms accept `SearchOptions`. `search_until` returns `Error::SearchLimitReached` when crossed; `cap` returns the cap instead. Unlike PHP, the limit also applies to a candidate found on the same day.

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# use spatie_opening_hours::{OpeningHours, SearchOptions, chrono::DateTime, serde_json::json};
# let opening_hours = OpeningHours::create(json!({"monday": ["09:00-12:00"]}))?;
# let date = DateTime::parse_from_rfc3339("2016-12-25T10:00:00+00:00")?;
let cap = DateTime::parse_from_rfc3339("2016-12-25T18:00:00+00:00")?;
let next_open = opening_hours.next_open_with(&date, SearchOptions {
    cap: Some(cap),
    search_until: None,
})?;
# assert_eq!(next_open, cap);
# Ok(())
# }
```

### Open and closed time differences

Returns the amount of open time between two date/times, as a signed floating-point number. Reversing the dates reverses the sign. Seconds retain subsecond precision.

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# use spatie_opening_hours::{OpeningHours, chrono::DateTime, serde_json::json};
# let opening_hours = OpeningHours::create(json!({"saturday": ["09:00-12:00", "13:00-16:00"]}))?;
let start = DateTime::parse_from_rfc3339("2016-12-24T11:00:00+00:00")?;
let end = DateTime::parse_from_rfc3339("2016-12-24T16:34:25+00:00")?;

assert_eq!(opening_hours.diff_in_open_hours(&start, &end)?, 4.0);
# assert_eq!(opening_hours.diff_in_closed_seconds(&start, &end)?, 5665.0);
# Ok(())
# }
```

The methods are `diff_in_open_hours`, `diff_in_open_minutes`, `diff_in_open_seconds`, and the corresponding `diff_in_closed_*` methods. They use the configured search horizon too.

### Current open range

`current_open_range(&date)` returns `Some(DateTimeRange)` if open and `None` if closed. Its `start()` and `end()` are chrono date/time values, so you can format both date and time. `current_open_range_start` and `current_open_range_end` return the endpoints directly.

The opening date can be yesterday for overnight ranges. The closing date can be tomorrow. A `24:00` endpoint is midnight on the next date. Like PHP, range lookup considers overnight ranges even when `overflow` is false; `is_open_at` only considers the previous night when the flag is true.

### Collection callbacks

The PHP `filter`, `map`, `flatMap`, `every` methods, and their exception equivalents, map to Rust iterators:

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# use spatie_opening_hours::{OpeningHours, serde_json::json};
# let opening_hours = OpeningHours::create(json!({"monday": ["09:00-12:00"]}))?;
let closing_days: Vec<_> = opening_hours.for_week()
    .filter(|(_, hours)| hours.is_empty())
    .map(|(day, _)| day)
    .collect();
# assert_eq!(closing_days.len(), 6);
# Ok(())
# }
```

Use `.all(...)` for PHP's `every`, `.flat_map(...)` for `flatMap`, and `exceptions()` as the source for exception callbacks.

## Structured data

`create_from_structured_data` fills a schedule from a schema.org `OpeningHoursSpecification` JSON value. `create_from_structured_data_str` accepts a JSON string. `dayOfWeek` supports names (Google-flavored) or schema.org URLs.

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
# use spatie_opening_hours::{OpeningHours, Day, serde_json::json};
let opening_hours = OpeningHours::create_from_structured_data(json!([
    {
        "@type": "OpeningHoursSpecification",
        "opens": "08:00", "closes": "12:00",
        "dayOfWeek": ["https://schema.org/Monday", "Tuesday"]
    },
    {
        "@type": "OpeningHoursSpecification",
        "opens": "00:00", "closes": "00:00",
        "validFrom": "2023-12-25", "validThrough": "2023-12-25"
    }
]))?;

let structured_data = opening_hours.as_structured_data();
# assert_eq!(opening_hours.for_day(Day::Monday).to_string(), "08:00-12:00");
# assert_eq!(structured_data.as_array().unwrap().len(), 3);
# Ok(())
# }
```

Structured data allows overnight ranges by default. `00:00-00:00` means closed all day; `23:59` as a closing time means the end of the day. Seconds in structured input are stripped. `PublicHolidays` is unsupported, as in PHP.

`as_structured_data_formatted("%H:%M:%S%:z", Some("America/Toronto".parse()?))` customizes formatting and offsets. Timezone offsets for this export are applied on 1970-01-01; they do not represent summer daylight-saving time. JSON export does not include metadata or executable filters.

## Value objects

`OpeningHoursForDay` is read-only. It supports indexing, `len`, `is_empty`, and iteration. `from_strings` parses ranges, `from_ranges` validates typed ranges, and `from_definition` parses JSON. The day-level query methods include `is_open_at`, `is_open_at_night`, `next_open`, `next_close`, `previous_open`, `previous_close`, their `_range` forms, `for_time`, and `for_night_time`.

`TimeRange` describes a period with a start and end, and displays as `HH:MM-HH:MM`. `from_string`, `from_definition`, `from_list`, and `from_midnight` construct ranges. `contains_time` checks the opening-day part; `contains_night_time` checks the next morning. `is_reversed` and `overflows_next_day` identify night ranges. `overlaps` retains PHP's directional comparison.

`Time` describes a single time and displays as `HH:MM`. `from_string`, `from_date_time`, `hours`, `minutes`, comparison methods, `diff`, `to_date_time`, and `format` support clock-time operations. `24:00` is valid but `24:01` is not. `PreciseTime` keeps subsecond precision for comparisons. `with_data` attaches JSON metadata to a `Time`, `TimeRange`, or day.

Formatting uses chrono/strftime directives, not PHP date format letters: `%H:%M` instead of `H:i`, `%a %H` instead of `D G`. `DateTimeRange` endpoints carry their calendar dates rather than embedding a date reference in `Time`. Recoverable errors use `Error`; missing ranges use `Option`.
