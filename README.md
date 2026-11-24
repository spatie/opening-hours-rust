# A helper to query and format a set of opening hours

[![Latest version on crates.io](https://img.shields.io/crates/v/spatie-opening-hours.svg?style=flat-square)](https://crates.io/crates/spatie-opening-hours)
[![Tests](https://img.shields.io/github/actions/workflow/status/spatie/opening-hours-rust/ci.yml?branch=main&style=flat-square)](https://github.com/spatie/opening-hours-rust/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](https://github.com/spatie/opening-hours-rust/blob/main/LICENSE.md)
[![Total downloads](https://img.shields.io/crates/d/spatie-opening-hours.svg?style=flat-square)](https://crates.io/crates/spatie-opening-hours)

With `spatie-opening-hours` you create an object that describes a business' opening hours, which you can query for open or closed on days or specific dates, or use to present the times per day.

This is a Rust port of [spatie/opening-hours](https://github.com/spatie/opening-hours).

## Support us

[<img src="https://spatie.be/github-ad-image/opening-hours-rust.jpg" alt="Support Spatie's open source work" width="419px" />](https://spatie.be/github-ad-click/opening-hours-rust)

We invest a lot of resources into creating [best in class open source packages](https://spatie.be/open-source). You can support us by [buying one of our paid products](https://spatie.be/open-source/support-us).

We highly appreciate you sending us a postcard from your hometown, mentioning which of our package(s) you are using. You'll find our address on [our contact page](https://spatie.be/about-us). We publish all received postcards on [our virtual postcard wall](https://spatie.be/open-source/postcards).

## Installation

Requires Rust 1.89 or newer.

```sh
cargo add spatie-opening-hours
```

## Usage

A set of opening hours is created by passing in a regular schedule, and a list of exceptions.

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use spatie_opening_hours::{OpeningHours, serde_json::json};

let opening_hours = OpeningHours::create(json!({
    "monday":    ["09:00-12:00", "13:00-18:00"],
    "tuesday":   ["09:00-12:00", "13:00-18:00"],
    "wednesday": ["09:00-12:00"],
    "thursday":  ["09:00-12:00", "13:00-18:00"],
    "friday":    ["09:00-12:00", "13:00-20:00"],
    "saturday":  ["09:00-12:00", "13:00-16:00"],
    "sunday":    [],
    "exceptions": {
        "2016-11-11": ["09:00-12:00"],
        "2016-12-25": [],
        "01-01": [],
        "12-25": ["09:00-12:00"]
    }
}))?;

assert!(opening_hours.is_open_on("monday")?);
assert!(opening_hours.is_closed_on("2016-12-25")?);
# Ok(())
# }
```

The crate supports overnight hours, recurring exceptions, date filters, business and output timezones, next/previous opening and closing times, open/closed durations, and schema.org structured data.

See the [usage guide](https://github.com/spatie/opening-hours-rust/blob/main/docs/usage.md) and [PHP behavior mapping](https://github.com/spatie/opening-hours-rust/blob/main/docs/php-parity.md). Rust uses chrono date/time values, snake_case methods, `Result` for errors, `Option` for absence, and iterator operations for collection callbacks.

## Testing

```sh
cargo test --locked --all-targets --all-features
cargo test --locked --doc --all-features
```

See [all quality checks](https://github.com/spatie/opening-hours-rust/blob/main/docs/testing.md). The README and Markdown guides are tested by rustdoc, including setup on hidden `# ` lines.

## Changelog

See [CHANGELOG.md](https://github.com/spatie/opening-hours-rust/blob/main/CHANGELOG.md).

## Contributing

See [CONTRIBUTING.md](https://github.com/spatie/opening-hours-rust/blob/main/CONTRIBUTING.md) and the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/).

Using an AI coding agent? Load Apollo's [rust-best-practices skill](https://github.com/apollographql/skills/tree/main/skills/rust-best-practices) when writing or reviewing code in this package.

## Security Vulnerabilities

Report vulnerabilities privately using the instructions in [SECURITY.md](https://github.com/spatie/opening-hours-rust/blob/main/SECURITY.md).

## Credits

- [Sebastian De Deyne](https://github.com/sebastiandedeyne)
- [Kyle Katarn](https://github.com/kylekatarnls)
- [Freek Van der Herten](https://github.com/freekmurze)
- [All contributors](https://github.com/spatie/opening-hours-rust/graphs/contributors)

## License

The MIT License (MIT). See [LICENSE.md](https://github.com/spatie/opening-hours-rust/blob/main/LICENSE.md).
