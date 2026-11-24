---
title: PHP behavior mapping
weight: 6
---

This port follows [spatie/opening-hours at afc0eec](https://github.com/spatie/opening-hours/tree/afc0eecf115fc4e4f93c0ca6113cdd75e84dcbaf). The PHP README supplies the guide's wording and examples. Rust changes language mechanics, not the weekly schedule model.

## Feature mapping

All rows below have executed Rust coverage in `tests/opening_hours.rs`, `tests/parity.rs`, or the README and guide doctests. The differential fixture covers schedule queries, not every public value-object method.

| PHP behavior | Rust API | Verification |
| --- | --- | --- |
| Regular week, missing days closed, sorted non-overlapping hours | `OpeningHours::create`, `for_week`, `for_day`, `for_day_name`, `OpeningHoursForDay` | Daily range and validation tests; guide examples |
| Full and recurring exceptions, exact-date precedence | `exceptions`, `for_date` | Metadata/exception test; PHP fixture |
| Inclusive weekday and date ranges, duplicate detection | JSON day/date keys with `to` or `-` | Date-range test; guide example |
| Dynamic filters, first applicable filter wins | `add_filter`, `get_filters` | Filter-order test |
| Metadata on schedules, days, ranges and minute times | `data`, `with_data` | Metadata test |
| Merge overlaps and touching ranges, optionally respecting metadata | `merge_overlapping_ranges`, `create_and_merge_overlapping_ranges` | Merge test |
| Combined and consecutive-day grouping | `for_week_combined`, `for_week_consecutive_days` | Grouping test |
| Open/closed on weekdays, dates, instants and now | `is_open_on`, `is_closed_on`, `is_open_at`, `is_closed_at`, `is_open`, `is_closed` | PHP fixture and guide examples; the current wall clock is not a deterministic fixture |
| Always open/closed and closing-day lists | `is_always_open`, `is_always_closed`, `regular_closing_days`, `regular_closing_days_iso`, `exceptional_closing_dates` | All-day, grouping and exception tests |
| Overnight ranges and `24:00` endpoints | `overflow`, `for_date_time`, `current_open_range` and endpoint helpers | Overnight test; PHP fixture |
| Business and output timezones | `timezone` definition, setters, `Timezone` | Output-zone and DST tests; PHP fixture |
| Next/previous open/close, search horizon, limits and caps | Four search methods, `_with` forms, `SearchOptions`, `set_day_limit`, `get_day_limit` | PHP fixture; cap/limit tests |
| Signed open/closed hours, minutes and fractional seconds | `diff_in_open_*`, `diff_in_closed_*` | Duration and DST tests |
| Schema.org and Google-flavored specification input/output | `create_from_structured_data`, `_str`, `as_structured_data`, `_formatted` | Round-trip and malformed-specification tests; guide example |
| Time, precise time, wall ranges and dated ranges | `Time`, `PreciseTime`, `TimeRange`, `DateTimeRange` | Value-object and overnight tests |
| Array access and collection callbacks | Read-only indexing and iterators; `.map`, `.filter`, `.flat_map`, `.all` | Daily test; collection guide example |

## Differential fixture

`tests/fixtures/php-parity.json` records results obtained by executing the PHP reference. Its 1,176 cases cover seven schedules across weeks containing the Brussels spring DST change, autumn DST change, and Christmas. Queries include midnight, exact opening/closing boundaries, lunch breaks and late-night hours.

For every case, `tests/parity.rs` compares the open predicate, daily hours, and all four search results. Date/time results are compared as absolute instants. Searches without a result must return the corresponding search-horizon error. Output timezone representation is tested separately because PHP's current-range `nextClose` path can ignore the configured output timezone.

## Rust translations and deliberate differences

- Definitions and metadata use JSON values. Register callable exceptions and filters as Rust closures. Register the PHP `filters` entries before callable exceptions to preserve their order. JSON cannot contain executable callbacks or arbitrary PHP objects.
- Rust uses chrono date/time values instead of configurable PHP date classes, reflection, macros or inheritance. Parser factories are methods on `OpeningHours`, not a separate parser object. PHP aliases with identical behavior, such as `spillsOverToNextDay`, use the canonical Rust `overflows_next_day` method.
- Errors return `Result`; absent daily ranges return `Option`. Weekdays are an enum. Collection operations use iterators instead of a second callback API. `OpeningHoursForDay` cannot be mutated through its index.
- Formatting uses chrono/strftime directives instead of PHP date letters. `Time` is a clock time; apply it to an explicit date with `to_date_time`. Dated range endpoints retain dates and offsets. `PreciseTime` is a copyable precise clock value, not an inherited PHP object retaining metadata and an implicit reference date. Keep the source chrono value when that date or timezone is needed.
- Every search path converts to the requested output timezone. PHP can skip this conversion when returning the current range's closing time. This changes the offset representation, not the instant.
- A cap or search limit also applies to a candidate on the initial day. PHP checks these only on some day-advancing paths. Midnight and overnight boundary placement otherwise follows the PHP fixture, including a previous midnight boundary that can equal the input.
- Reversed calendar-date ranges return an error instead of silently expanding to no dates. Split year-end ranges into two definitions.
- Custom merge `excludedKeys` are unnecessary for the fixed Rust definition shape. Unknown keys are invalid weekdays rather than arbitrary configuration extensions. Use the explicit timezone and filter APIs.

## Dependency maintenance

The crate uses chrono and chrono-tz for calendar and IANA timezone rules. `cargo audit` currently exits successfully with no vulnerability findings, but reports unmaintained-dependency advisories [RUSTSEC-2026-0543](https://rustsec.org/advisories/RUSTSEC-2026-0543) and [RUSTSEC-2026-0555](https://rustsec.org/advisories/RUSTSEC-2026-0555). No fixed upgrade is available. These warnings are not suppressed. Dependency replacement would change the public date/time API and needs a separate compatibility decision.
