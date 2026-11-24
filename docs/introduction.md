---
title: Introduction
weight: 1
---

With `spatie-opening-hours` you create an object that describes a business' opening hours, which you can query for open or closed on days or specific dates, or use to present the times per day.

A set of opening hours is created by passing in a regular schedule, and a list of exceptions.

```rust
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use spatie_opening_hours::{OpeningHours, serde_json::json};

let opening_hours = OpeningHours::create(json!({
    "monday": ["09:00-12:00", "13:00-18:00"],
    "exceptions": { "2016-12-25": [], "01-01": [] }
}))?;

assert!(opening_hours.is_open_on("monday")?);
assert!(opening_hours.is_closed_on("2016-12-25")?);
# Ok(())
# }
```

Continue with [installation](installation-and-setup.md) and [usage](usage.md).
