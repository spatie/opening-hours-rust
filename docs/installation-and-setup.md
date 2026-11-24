---
title: Installation
weight: 3
---

Requires Rust 1.89 or newer. Install Rust using [rustup](https://rustup.rs/).

Add it to your application:

```sh
cargo add spatie-opening-hours
```

The date/time types, IANA timezone names, and JSON definition macro are re-exported as `chrono`, `chrono_tz`, and `serde_json`. No PHP runtime or external service is required.
