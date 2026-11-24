use crate::{Day, Error, OpeningHoursForDay, PreciseTime, Time, TimeRange, Timezone};
use chrono::{DateTime, Datelike, Duration, FixedOffset, Local, NaiveDate, TimeZone};
use serde_json::{Value, json};
use std::{fmt, sync::Arc};

/// A date-specific override. The first filter returning `Some` wins, before exceptions.
pub type DateFilter =
    Arc<dyn Fn(DateTime<FixedOffset>) -> Option<OpeningHoursForDay> + Send + Sync>;

/// Optional strict search limit and fallback cap. Dates are absolute instants.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchOptions {
    /// Return an error when no boundary is found before/after this instant.
    pub search_until: Option<DateTime<FixedOffset>>,
    /// Return this instant instead when no boundary is found before/after it.
    pub cap: Option<DateTime<FixedOffset>>,
}

/// A current opening range with calendar dates and timezone offsets attached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateTimeRange {
    start: DateTime<FixedOffset>,
    end: DateTime<FixedOffset>,
    data: Value,
}
impl DateTimeRange {
    /// Inclusive opening date/time.
    pub fn start(&self) -> DateTime<FixedOffset> {
        self.start
    }
    /// Exclusive closing date/time.
    pub fn end(&self) -> DateTime<FixedOffset> {
        self.end
    }
    /// Range metadata.
    pub fn data(&self) -> &Value {
        &self.data
    }
    /// Attaches a range to the date containing the queried instant.
    pub fn from_time_range<T: TimeZone>(
        date: &DateTime<T>,
        range: &TimeRange,
    ) -> Result<Self, Error> {
        let mut opening_date = date.date_naive();
        if range.overflows_next_day()
            && PreciseTime::from_date_time(date).nanos() < range.end().nanos()
        {
            opening_date = opening_date.pred_opt().ok_or(Error::DateOverflow)?;
        }
        interval(
            opening_date,
            range,
            |wall| crate::timezone::resolve_local(&date.timezone(), wall),
            true,
        )
    }
}

/// Days sharing the same hours. Metadata does not affect grouping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeekGroup<'a> {
    /// Weekdays in Monday-to-Sunday order.
    pub days: Vec<Day>,
    /// Hours of the first day in the group.
    pub opening_hours: &'a OpeningHoursForDay,
}

/// A regular weekly schedule, date exceptions, optional filters and timezones.
#[derive(Clone)]
pub struct OpeningHours {
    week: [OpeningHoursForDay; 7],
    exceptions: Vec<(String, OpeningHoursForDay)>,
    filters: Vec<DateFilter>,
    timezone: Option<Timezone>,
    output_timezone: Option<Timezone>,
    overflow: bool,
    day_limit: u32,
    data: Value,
}
impl fmt::Debug for OpeningHours {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OpeningHours")
            .field("week", &self.week)
            .field("exceptions", &self.exceptions)
            .field("filter_count", &self.filters.len())
            .field("timezone", &self.timezone)
            .field("output_timezone", &self.output_timezone)
            .field("overflow", &self.overflow)
            .field("day_limit", &self.day_limit)
            .field("data", &self.data)
            .finish()
    }
}
impl OpeningHours {
    /// Default search horizon when there are no exceptions.
    pub const DEFAULT_DAY_LIMIT: u32 = 8;

    /// Creates a schedule from a PHP-shaped JSON definition. Missing days are closed.
    pub fn create(definition: Value) -> Result<Self, Error> {
        if definition.as_array().is_some_and(Vec::is_empty) {
            return Self::create(json!({}));
        }
        let object = definition
            .as_object()
            .ok_or_else(|| Error::InvalidDefinition("expected an object".into()))?;
        let mut schedule = Self {
            week: std::array::from_fn(|_| OpeningHoursForDay::default()),
            exceptions: Vec::new(),
            filters: Vec::new(),
            timezone: None,
            output_timezone: None,
            overflow: false,
            day_limit: Self::DEFAULT_DAY_LIMIT,
            data: object.get("data").cloned().unwrap_or_default(),
        };
        if let Some(value) = object.get("overflow") {
            schedule.overflow = value
                .as_bool()
                .ok_or_else(|| Error::InvalidDefinition("overflow must be a boolean".into()))?;
        }
        if let Some(value) = object.get("timezone") {
            if let Some(zones) = value.as_object() {
                schedule.timezone = parse_zone(zones.get("input"))?;
                schedule.output_timezone = parse_zone(zones.get("output"))?;
            } else {
                schedule.timezone = parse_zone(Some(value))?;
            }
        }
        let mut seen = [false; 7];
        for (key, value) in object {
            match key.as_str() {
                "data" | "overflow" | "timezone" => continue,
                "exceptions" => {
                    let exceptions = value.as_object().ok_or_else(|| {
                        Error::InvalidDefinition("exceptions must be an object".into())
                    })?;
                    for (key, value) in exceptions {
                        let hours = OpeningHoursForDay::from_definition(value)?;
                        for date in expand_dates(key)? {
                            if schedule.exceptions.iter().any(|(key, _)| *key == date) {
                                return Err(Error::InvalidDateRange(key.clone()));
                            }
                            schedule.exceptions.push((date, hours.clone()));
                        }
                    }
                    if !schedule.exceptions.is_empty() {
                        schedule.day_limit = 366;
                    }
                    continue;
                }
                "filters" | "dateTimeClass" => {
                    return Err(Error::InvalidDefinition(format!(
                        "{key} is not a JSON option; use add_filter and chrono date/time values"
                    )));
                }
                _ => {}
            }
            let hours = OpeningHoursForDay::from_definition(value)?;
            for day in expand_days(key)? {
                if seen[day as usize] {
                    return Err(Error::InvalidDateRange(key.clone()));
                }
                seen[day as usize] = true;
                schedule.week[day as usize] = hours.clone();
            }
        }
        Ok(schedule)
    }
    /// Whether a JSON definition is valid.
    pub fn is_valid(definition: Value) -> bool {
        Self::create(definition).is_ok()
    }
    /// Creates a schedule, first merging overlapping and touching ranges.
    pub fn create_and_merge_overlapping_ranges(
        definition: Value,
        ignore_data: bool,
    ) -> Result<Self, Error> {
        Self::create(Self::merge_overlapping_ranges(definition, ignore_data)?)
    }
    /// Merges ranges recursively while retaining day and schedule metadata.
    pub fn merge_overlapping_ranges(
        mut definition: Value,
        ignore_data: bool,
    ) -> Result<Value, Error> {
        let object = definition
            .as_object_mut()
            .ok_or_else(|| Error::InvalidDefinition("expected an object".into()))?;
        for (key, value) in object {
            match key.as_str() {
                "data" | "timezone" | "overflow" | "filters" | "dateTimeClass" => {}
                "exceptions" => {
                    *value = Self::merge_overlapping_ranges(value.clone(), ignore_data)?
                }
                _ => merge_day(value, ignore_data)?,
            }
        }
        Ok(definition)
    }
    /// Schedule metadata.
    pub fn data(&self) -> &Value {
        &self.data
    }
    /// Sets the schedule timezone; `None` uses each input's own timezone.
    pub fn set_timezone(&mut self, timezone: Option<Timezone>) {
        self.timezone = timezone;
    }
    /// Sets an explicit output timezone; otherwise results use the input timezone.
    pub fn set_output_timezone(&mut self, timezone: Option<Timezone>) {
        self.output_timezone = timezone;
    }
    /// Sets the maximum number of calendar days searched in either direction.
    pub fn set_day_limit(&mut self, days: u32) -> &mut Self {
        self.day_limit = days;
        self
    }
    /// Configured search horizon, initially 8 days or 366 when exceptions exist.
    pub fn get_day_limit(&self) -> u32 {
        self.day_limit
    }
    /// Adds a date filter; first non-None override wins. Filters are not cached.
    pub fn add_filter(
        &mut self,
        filter: impl Fn(DateTime<FixedOffset>) -> Option<OpeningHoursForDay> + Send + Sync + 'static,
    ) -> &mut Self {
        self.filters.push(Arc::new(filter));
        self
    }
    /// Installed filters, in precedence order.
    pub fn get_filters(&self) -> &[DateFilter] {
        &self.filters
    }
    /// Iterates Monday through Sunday with weekday keys.
    pub fn for_week(&self) -> impl DoubleEndedIterator<Item = (Day, &OpeningHoursForDay)> {
        Day::ALL.into_iter().zip(self.week.iter())
    }
    /// Regular hours for a typed weekday.
    pub fn for_day(&self, day: Day) -> &OpeningHoursForDay {
        &self.week[day as usize]
    }
    /// Regular hours for a case-insensitive English weekday name.
    pub fn for_day_name(&self, day: &str) -> Result<&OpeningHoursForDay, Error> {
        Ok(self.for_day(day.parse()?))
    }
    /// Groups every weekday sharing the same hours.
    pub fn for_week_combined(&self) -> Vec<WeekGroup<'_>> {
        self.group_week(false)
    }
    /// Groups adjacent days sharing the same hours, without wrapping Sunday to Monday.
    pub fn for_week_consecutive_days(&self) -> Vec<WeekGroup<'_>> {
        self.group_week(true)
    }
    fn group_week(&self, consecutive: bool) -> Vec<WeekGroup<'_>> {
        let mut groups: Vec<WeekGroup<'_>> = Vec::new();
        for (day, hours) in self.for_week() {
            let count = groups.len();
            let existing = groups.iter_mut().enumerate().find(|(i, group)| {
                (!consecutive || i + 1 == count)
                    && group.opening_hours.to_string() == hours.to_string()
            });
            if let Some((_, group)) = existing {
                group.days.push(day);
            } else {
                groups.push(WeekGroup {
                    days: vec![day],
                    opening_hours: hours,
                });
            }
        }
        groups
    }
    /// All exceptions in definition order, preserving recurring month/day keys.
    pub fn exceptions(&self) -> impl DoubleEndedIterator<Item = (&str, &OpeningHoursForDay)> {
        self.exceptions
            .iter()
            .map(|(key, hours)| (key.as_str(), hours))
    }
    /// Hours for a date/time: filters, then exact-date exceptions, recurring exceptions, regular week.
    pub fn for_date<T: TimeZone>(&self, date: &DateTime<T>) -> OpeningHoursForDay {
        self.hours_at(self.local(date))
    }
    fn hours_at(&self, date: DateTime<FixedOffset>) -> OpeningHoursForDay {
        for filter in &self.filters {
            if let Some(hours) = filter(date) {
                return hours;
            }
        }
        for key in [
            date.format("%Y-%m-%d").to_string(),
            date.format("%m-%d").to_string(),
        ] {
            if let Some((_, hours)) = self
                .exceptions
                .iter()
                .find(|(candidate, _)| *candidate == key)
            {
                return hours.clone();
            }
        }
        self.for_day(Day::on_date_time(&date)).clone()
    }
    /// Ranges matching this time, including overnight ranges from the previous date.
    pub fn for_date_time<T: TimeZone>(&self, date: &DateTime<T>) -> Result<Vec<TimeRange>, Error> {
        let input = date;
        let date = self.local(input);
        let previous = self.resolve_date(
            date.date_naive()
                .pred_opt()
                .ok_or(Error::DateOverflow)?
                .and_time(date.time()),
            input,
        )?;
        let time = PreciseTime::from_date_time(&date);
        let mut ranges: Vec<_> = self
            .hours_at(previous)
            .for_night_time(time)
            .cloned()
            .collect();
        ranges.extend(self.hours_at(date).for_time(time).cloned());
        Ok(ranges)
    }
    /// Whether a regular weekday or an ISO date has at least one range.
    /// A month/day without a year uses the current local year.
    pub fn is_open_on(&self, day: &str) -> Result<bool, Error> {
        if let Ok(day) = day.parse::<Day>() {
            return Ok(!self.for_day(day).is_empty());
        }
        let key = if day.len() == 5 {
            format!("{}-{day}", Local::now().year())
        } else {
            day.to_owned()
        };
        let date = parse_date(&key)?;
        let zone = self
            .timezone
            .unwrap_or(Timezone::Fixed(*Local::now().fixed_offset().offset()));
        Ok(!self
            .hours_at(zone.resolve(date.and_hms_opt(0, 0, 0).ok_or(Error::DateOverflow)?)?)
            .is_empty())
    }
    /// The inverse of `is_open_on`.
    pub fn is_closed_on(&self, day: &str) -> Result<bool, Error> {
        Ok(!self.is_open_on(day)?)
    }
    /// Whether the business is open at the given instant. Closing boundaries are exclusive.
    pub fn is_open_at<T: TimeZone>(&self, date: &DateTime<T>) -> bool {
        let input = date;
        let date = self.local(input);
        let time = PreciseTime::from_date_time(&date);
        if self.overflow
            && let Some(previous) = date
                .date_naive()
                .pred_opt()
                .and_then(|d| self.resolve_date(d.and_time(date.time()), input).ok())
            && self.hours_at(previous).is_open_at_night(time)
        {
            return true;
        }
        self.hours_at(date).is_open_at(time)
    }
    /// Whether the business is closed at the given instant.
    pub fn is_closed_at<T: TimeZone>(&self, date: &DateTime<T>) -> bool {
        !self.is_open_at(date)
    }
    /// Whether the business is open now.
    pub fn is_open(&self) -> bool {
        self.is_open_at(&Local::now())
    }
    /// Whether the business is closed now.
    pub fn is_closed(&self) -> bool {
        !self.is_open()
    }
    /// Whether all regular and exceptional days are empty and there are no filters.
    pub fn is_always_closed(&self) -> bool {
        self.filters.is_empty()
            && self
                .week
                .iter()
                .chain(self.exceptions.iter().map(|(_, h)| h))
                .all(OpeningHoursForDay::is_empty)
    }
    /// Whether all regular and exceptional days are exactly 00:00-24:00, with no filters.
    pub fn is_always_open(&self) -> bool {
        self.filters.is_empty()
            && self
                .week
                .iter()
                .chain(self.exceptions.iter().map(|(_, h)| h))
                .all(|h| h.to_string() == "00:00-24:00")
    }
    /// Regular closing weekdays in Monday-to-Sunday order.
    pub fn regular_closing_days(&self) -> Vec<Day> {
        self.for_week()
            .filter_map(|(d, h)| h.is_empty().then_some(d))
            .collect()
    }
    /// ISO weekday numbers of regular closing days.
    pub fn regular_closing_days_iso(&self) -> Vec<u8> {
        self.regular_closing_days()
            .into_iter()
            .map(Day::to_iso)
            .collect()
    }
    /// Closed exception keys, including recurring month/day keys.
    pub fn exceptional_closing_dates(&self) -> Vec<&str> {
        self.exceptions()
            .filter_map(|(d, h)| h.is_empty().then_some(d))
            .collect()
    }
    /// Current range, or None while closed. Like PHP, the last matching range wins.
    pub fn current_open_range<T: TimeZone>(
        &self,
        date: &DateTime<T>,
    ) -> Result<Option<DateTimeRange>, Error> {
        let local = self.local(date);
        let ranges = self.for_date_time(date)?;
        let Some(range) = ranges.last() else {
            return Ok(None);
        };
        let mut opening_date = local.date_naive();
        if range.overflows_next_day()
            && PreciseTime::from_date_time(&local).nanos() < range.end().nanos()
        {
            opening_date = opening_date.pred_opt().ok_or(Error::DateOverflow)?;
        }
        let mut result = interval(
            opening_date,
            range,
            |wall| self.resolve_date(wall, date),
            true,
        )?;
        result.start = self.output(result.start, date);
        result.end = self.output(result.end, date);
        Ok(Some(result))
    }
    /// Inclusive opening date of the current range, if open.
    pub fn current_open_range_start<T: TimeZone>(
        &self,
        date: &DateTime<T>,
    ) -> Result<Option<DateTime<FixedOffset>>, Error> {
        Ok(self.current_open_range(date)?.map(|r| r.start))
    }
    /// Exclusive closing date of the current range, if open.
    pub fn current_open_range_end<T: TimeZone>(
        &self,
        date: &DateTime<T>,
    ) -> Result<Option<DateTime<FixedOffset>>, Error> {
        Ok(self.current_open_range(date)?.map(|r| r.end))
    }
    /// Next opening strictly after the input, skipping the current opening range.
    pub fn next_open<T: TimeZone>(
        &self,
        date: &DateTime<T>,
    ) -> Result<DateTime<FixedOffset>, Error> {
        self.next_open_with(date, SearchOptions::default())
    }
    /// Next closing strictly after the input, including the current opening range.
    pub fn next_close<T: TimeZone>(
        &self,
        date: &DateTime<T>,
    ) -> Result<DateTime<FixedOffset>, Error> {
        self.next_close_with(date, SearchOptions::default())
    }
    /// Previous opening strictly before the input.
    pub fn previous_open<T: TimeZone>(
        &self,
        date: &DateTime<T>,
    ) -> Result<DateTime<FixedOffset>, Error> {
        self.previous_open_with(date, SearchOptions::default())
    }
    /// Previous closing strictly before the input.
    pub fn previous_close<T: TimeZone>(
        &self,
        date: &DateTime<T>,
    ) -> Result<DateTime<FixedOffset>, Error> {
        self.previous_close_with(date, SearchOptions::default())
    }
    /// Next opening with an explicit limit and/or cap.
    pub fn next_open_with<T: TimeZone>(
        &self,
        date: &DateTime<T>,
        options: SearchOptions,
    ) -> Result<DateTime<FixedOffset>, Error> {
        self.search(date, true, true, options)
    }
    /// Next closing with an explicit limit and/or cap.
    pub fn next_close_with<T: TimeZone>(
        &self,
        date: &DateTime<T>,
        options: SearchOptions,
    ) -> Result<DateTime<FixedOffset>, Error> {
        self.search(date, false, true, options)
    }
    /// Previous opening with an explicit limit and/or cap.
    pub fn previous_open_with<T: TimeZone>(
        &self,
        date: &DateTime<T>,
        options: SearchOptions,
    ) -> Result<DateTime<FixedOffset>, Error> {
        self.search(date, true, false, options)
    }
    /// Previous closing with an explicit limit and/or cap.
    pub fn previous_close_with<T: TimeZone>(
        &self,
        date: &DateTime<T>,
        options: SearchOptions,
    ) -> Result<DateTime<FixedOffset>, Error> {
        self.search(date, false, false, options)
    }
    fn search<T: TimeZone>(
        &self,
        input: &DateTime<T>,
        opening: bool,
        forward: bool,
        options: SearchOptions,
    ) -> Result<DateTime<FixedOffset>, Error> {
        let mut local = self.local(input);
        if forward
            && !opening
            && let Some(range) = self.current_open_range(input)?
            && range.end.time() != chrono::NaiveTime::MIN
        {
            return self.search_result(range.end, input, forward, options);
        }
        let mut hours = self.hours_at(local);
        let mut boundary = None;
        if !opening && self.overflow {
            let previous_date = local.date_naive().pred_opt().ok_or(Error::DateOverflow)?;
            let previous =
                self.hours_at(self.resolve_date(previous_date.and_time(local.time()), input)?);
            if previous.is_open_at_night(PreciseTime::from_date_time(&local)) {
                boundary = if forward {
                    previous.next_close(PreciseTime::from_date_time(&local))
                } else {
                    previous.previous_close(PreciseTime::from_date_time(&local))
                }
                .cloned();
            }
        }
        if boundary.is_none() {
            boundary = daily_boundary(
                &hours,
                PreciseTime::from_date_time(&local),
                opening,
                forward,
            );
        }
        if forward
            && !opening
            && let Some(close) = &boundary
            && close.hours() < 24
            && (close.nanos() < Time::from_date_time(&local).nanos()
                || (self.is_closed_at(&local)
                    && Time::from_date_time(
                        &self.next_open(&local.with_timezone(&input.timezone()))?,
                    )
                    .nanos()
                        > close.nanos()))
        {
            local = self.resolve_date(
                local
                    .date_naive()
                    .succ_opt()
                    .ok_or(Error::DateOverflow)?
                    .and_time(local.time()),
                input,
            )?;
        }
        let mut tries = 0;
        while boundary.as_ref().is_none_or(|time| {
            if forward {
                time.hours() >= 24
            } else {
                time.hours() == 0 && time.minutes() == 0
            }
        }) {
            if tries >= self.day_limit {
                return Err(Error::MaximumLimitExceeded(self.day_limit));
            }
            tries += 1;
            if forward {
                local = self.wall_date(
                    local.date_naive().succ_opt().ok_or(Error::DateOverflow)?,
                    input,
                )?;
                if self.is_open_at(&local) == opening
                    && hours.is_open_at_the_end_of_the_day() != opening
                {
                    return self.search_result(local, input, forward, options);
                }
            } else {
                let midnight = self.wall_date(local.date_naive(), input)?;
                local = midnight
                    .checked_sub_signed(Duration::seconds(1))
                    .ok_or(Error::DateOverflow)?;
                local = self.local(&local.with_timezone(&input.timezone()));
                hours = self.hours_at(local);
                if self.is_open_at(&midnight) == opening
                    && hours.is_open_at_the_end_of_the_day() != opening
                {
                    return self.search_result(midnight, input, forward, options);
                }
            }
            if let Some(cap) = options.cap
                && if forward { local > cap } else { local < cap }
            {
                return Ok(cap);
            }
            if let Some(limit) = options.search_until
                && if forward {
                    local > limit
                } else {
                    local < limit
                }
            {
                return Err(Error::SearchLimitReached);
            }
            hours = self.hours_at(local);
            boundary = daily_boundary(
                &hours,
                PreciseTime::from_date_time(&local),
                opening,
                forward,
            );
        }
        if forward
            && opening
            && local.time() == chrono::NaiveTime::MIN
            && self.is_open_at(
                &local
                    .checked_sub_signed(Duration::seconds(1))
                    .ok_or(Error::DateOverflow)?,
            )
        {
            return self
                .search(
                    &local
                        .checked_add_signed(Duration::seconds(1))
                        .ok_or(Error::DateOverflow)?
                        .with_timezone(&input.timezone()),
                    opening,
                    forward,
                    options,
                )
                .map(|date| self.output(date, input));
        }
        let boundary = boundary.ok_or(Error::MaximumLimitExceeded(self.day_limit))?;
        // PHP applies the selected clock time to the search date. Overnight placement above
        // intentionally follows its asymmetric next/previous boundary rules.
        let result = self.resolve_date(boundary.to_date_time(local.date_naive())?, input)?;
        self.search_result(result, input, forward, options)
    }
    fn search_result<T: TimeZone>(
        &self,
        result: DateTime<FixedOffset>,
        input: &DateTime<T>,
        forward: bool,
        options: SearchOptions,
    ) -> Result<DateTime<FixedOffset>, Error> {
        if let Some(cap) = options.cap
            && if forward { result > cap } else { result < cap }
        {
            return Ok(cap);
        }
        if let Some(limit) = options.search_until
            && if forward {
                result > limit
            } else {
                result < limit
            }
        {
            return Err(Error::SearchLimitReached);
        }
        Ok(self.output(result, input))
    }
    /// Signed open seconds between instants, retaining fractional seconds.
    pub fn diff_in_open_seconds<T: TimeZone, U: TimeZone>(
        &self,
        start: &DateTime<T>,
        end: &DateTime<U>,
    ) -> Result<f64, Error> {
        let input_zone = start.timezone();
        let start = start.fixed_offset();
        let end = end.fixed_offset();
        if end < start {
            return Ok(-self.diff_in_open_seconds(&end.with_timezone(&input_zone), &start)?);
        }
        let mut current = start;
        let mut seconds = 0.0;
        while current < end {
            let options = SearchOptions {
                cap: Some(end),
                search_until: None,
            };
            if self.is_open_at(&current.with_timezone(&input_zone)) {
                let next = self.next_close_with(&current.with_timezone(&input_zone), options)?;
                seconds += elapsed(current, next);
                current = next;
            } else {
                current = self.next_open_with(&current.with_timezone(&input_zone), options)?;
            }
        }
        Ok(seconds)
    }
    /// Signed open minutes between instants.
    pub fn diff_in_open_minutes<T: TimeZone, U: TimeZone>(
        &self,
        start: &DateTime<T>,
        end: &DateTime<U>,
    ) -> Result<f64, Error> {
        Ok(self.diff_in_open_seconds(start, end)? / 60.0)
    }
    /// Signed open hours between instants.
    pub fn diff_in_open_hours<T: TimeZone, U: TimeZone>(
        &self,
        start: &DateTime<T>,
        end: &DateTime<U>,
    ) -> Result<f64, Error> {
        Ok(self.diff_in_open_seconds(start, end)? / 3600.0)
    }
    /// Signed closed seconds between instants.
    pub fn diff_in_closed_seconds<T: TimeZone, U: TimeZone>(
        &self,
        start: &DateTime<T>,
        end: &DateTime<U>,
    ) -> Result<f64, Error> {
        Ok(elapsed(start.fixed_offset(), end.fixed_offset())
            - self.diff_in_open_seconds(start, end)?)
    }
    /// Signed closed minutes between instants.
    pub fn diff_in_closed_minutes<T: TimeZone, U: TimeZone>(
        &self,
        start: &DateTime<T>,
        end: &DateTime<U>,
    ) -> Result<f64, Error> {
        Ok(self.diff_in_closed_seconds(start, end)? / 60.0)
    }
    /// Signed closed hours between instants.
    pub fn diff_in_closed_hours<T: TimeZone, U: TimeZone>(
        &self,
        start: &DateTime<T>,
        end: &DateTime<U>,
    ) -> Result<f64, Error> {
        Ok(self.diff_in_closed_seconds(start, end)? / 3600.0)
    }
    /// Exports schema.org OpeningHoursSpecification JSON values.
    pub fn as_structured_data(&self) -> Value {
        self.as_structured_data_formatted("%H:%M", None)
    }
    /// Exports with chrono formatting. Offsets use the timezone on 1970-01-01, like PHP.
    pub fn as_structured_data_formatted(&self, format: &str, timezone: Option<Timezone>) -> Value {
        let render = |time: &Time| -> String {
            if let Some(zone) = timezone
                && let Some(date) = NaiveDate::from_ymd_opt(1970, 1, 1)
                && let Ok(date) = time.to_date_time(date).and_then(|d| zone.resolve(d))
            {
                if time.hours() == 24 && format.starts_with("%H:%M") {
                    return format!("24:00{}", date.format(&format[5..]));
                }
                return date.format(format).to_string();
            }
            time.format(format)
        };
        let mut result = Vec::new();
        for (day, hours) in self.for_week() {
            for range in hours {
                let name = day.name();
                let title = format!("{}{}", name[..1].to_uppercase(), &name[1..]);
                result.push(json!({"@type": "OpeningHoursSpecification", "dayOfWeek": title, "opens": render(range.start()), "closes": render(range.end())}));
            }
        }
        for (date, hours) in self.exceptions() {
            if hours.is_empty() {
                let zero = render(&Time::from_minutes(0));
                result.push(json!({"@type": "OpeningHoursSpecification", "opens": zero, "closes": zero, "validFrom": date, "validThrough": date}));
            }
            for range in hours {
                result.push(json!({"@type": "OpeningHoursSpecification", "opens": render(range.start()), "closes": render(range.end()), "validFrom": date, "validThrough": date}));
            }
        }
        Value::Array(result)
    }
    /// Parses schema.org specifications, allowing overnight ranges by default.
    pub fn create_from_structured_data(value: Value) -> Result<Self, Error> {
        let items = value
            .as_array()
            .ok_or_else(|| Error::InvalidSpecification("expected an array".into()))?;
        let mut definition = json!({"overflow": true, "exceptions": {}});
        for (index, item) in items.iter().enumerate() {
            parse_specification(&mut definition, item).map_err(|error| {
                Error::InvalidSpecification(format!("item at index {index}: {error}"))
            })?;
        }
        Self::create(definition)
    }
    /// Parses a schema.org JSON string.
    pub fn create_from_structured_data_str(value: &str) -> Result<Self, Error> {
        Self::create_from_structured_data(
            serde_json::from_str(value).map_err(|e| Error::InvalidSpecification(e.to_string()))?,
        )
    }
    fn local<T: TimeZone>(&self, date: &DateTime<T>) -> DateTime<FixedOffset> {
        self.timezone
            .map_or_else(|| date.fixed_offset(), |zone| zone.at(date.fixed_offset()))
    }
    fn resolve_date<T: TimeZone>(
        &self,
        wall: chrono::NaiveDateTime,
        input: &DateTime<T>,
    ) -> Result<DateTime<FixedOffset>, Error> {
        match self.timezone {
            Some(zone) => zone.resolve(wall),
            None => crate::timezone::resolve_local(&input.timezone(), wall),
        }
    }
    fn wall_date<T: TimeZone>(
        &self,
        date: NaiveDate,
        reference: &DateTime<T>,
    ) -> Result<DateTime<FixedOffset>, Error> {
        self.resolve_date(
            date.and_hms_opt(0, 0, 0).ok_or(Error::DateOverflow)?,
            reference,
        )
    }
    fn output<T: TimeZone>(
        &self,
        date: DateTime<FixedOffset>,
        input: &DateTime<T>,
    ) -> DateTime<FixedOffset> {
        self.output_timezone.map_or_else(
            || date.with_timezone(&input.timezone()).fixed_offset(),
            |zone| zone.at(date),
        )
    }
}

fn parse_zone(value: Option<&Value>) -> Result<Option<Timezone>, Error> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(name)) => Ok(Some(name.parse()?)),
        _ => Err(Error::InvalidTimezone(
            value.map_or_else(String::new, Value::to_string),
        )),
    }
}
fn daily_boundary(
    hours: &OpeningHoursForDay,
    time: PreciseTime,
    opening: bool,
    forward: bool,
) -> Option<Time> {
    match (opening, forward) {
        (true, true) => hours.next_open(time),
        (false, true) => hours.next_close(time),
        (true, false) => hours.previous_open(time),
        (false, false) => hours.previous_close(time),
    }
    .cloned()
}
fn parse_date(value: &str) -> Result<NaiveDate, Error> {
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| Error::InvalidDate(value.into()))?;
    if date.format("%Y-%m-%d").to_string() != value {
        return Err(Error::InvalidDate(value.into()));
    }
    Ok(date)
}
fn expand_days(key: &str) -> Result<Vec<Day>, Error> {
    let range = key.split_once(" to ").or_else(|| key.split_once('-'));
    let Some((start, end)) = range else {
        return Ok(vec![key.parse()?]);
    };
    let start: Day = start.trim().parse()?;
    let end: Day = end.trim().parse()?;
    let count = (end as usize + 7 - start as usize) % 7;
    Ok((0..=count)
        .map(|n| Day::ALL[(start as usize + n) % 7])
        .collect())
}
fn expand_dates(key: &str) -> Result<Vec<String>, Error> {
    if !key.is_ascii() {
        return Err(Error::InvalidDate(key.into()));
    }
    let key = key.trim();
    let spaced = key.split_once(" - ");
    let range = key
        .split_once(" to ")
        .or(spaced)
        .or_else(|| match key.len() {
            11 => Some((&key[..5], &key[6..])),
            21 => Some((&key[..10], &key[11..])),
            _ => None,
        });
    let (start, end) = range.unwrap_or((key, key));
    let start = start.trim();
    let end = end.trim();
    let recurring = start.len() == 5;
    if recurring != (end.len() == 5) {
        return Err(Error::InvalidDateRange(key.into()));
    }
    let start = parse_date(&if recurring {
        format!("2024-{start}")
    } else {
        start.into()
    })?;
    let end = parse_date(&if recurring {
        format!("2024-{end}")
    } else {
        end.into()
    })?;
    if end < start {
        return Err(Error::InvalidDateRange(key.into()));
    }
    let mut result = Vec::new();
    let mut date = start;
    loop {
        result.push(
            date.format(if recurring { "%m-%d" } else { "%Y-%m-%d" })
                .to_string(),
        );
        if date == end {
            break;
        }
        date = date.succ_opt().ok_or(Error::DateOverflow)?;
    }
    Ok(result)
}
fn interval(
    date: NaiveDate,
    range: &TimeRange,
    resolve: impl Fn(chrono::NaiveDateTime) -> Result<DateTime<FixedOffset>, Error>,
    overflow: bool,
) -> Result<DateTimeRange, Error> {
    let start = resolve(range.start().to_date_time(date)?)?;
    let end_date = if range.overflows_next_day() {
        if overflow {
            date.succ_opt().ok_or(Error::DateOverflow)?
        } else {
            date
        }
    } else {
        date
    };
    let end_time = if range.overflows_next_day() && !overflow {
        Time::from_minutes(1440).to_date_time(end_date)?
    } else {
        range.end().to_date_time(end_date)?
    };
    Ok(DateTimeRange {
        start,
        end: resolve(end_time)?,
        data: range.data().clone(),
    })
}
fn elapsed(start: DateTime<FixedOffset>, end: DateTime<FixedOffset>) -> f64 {
    (end.timestamp() - start.timestamp()) as f64
        + (f64::from(end.timestamp_subsec_nanos()) - f64::from(start.timestamp_subsec_nanos()))
            / 1e9
}

fn merge_day(value: &mut Value, ignore_data: bool) -> Result<(), Error> {
    let (list, data) = if let Some(object) = value.as_object() {
        (
            object.get("hours").cloned().unwrap_or_else(|| json!([])),
            object.get("data").cloned().unwrap_or_default(),
        )
    } else {
        (value.clone(), Value::Null)
    };
    let list = list
        .as_array()
        .ok_or_else(|| Error::InvalidDefinition(value.to_string()))?;
    let mut ranges = list
        .iter()
        .map(TimeRange::from_definition)
        .collect::<Result<Vec<_>, _>>()?;
    ranges.sort_by_key(|r| r.start().nanos());
    let mut merged: Vec<TimeRange> = Vec::new();
    for mut range in ranges {
        let mut i = 0;
        while i < merged.len() {
            let previous = &merged[i];
            if (ignore_data || previous.data() == range.data())
                && (previous.overlaps(&range) || range.overlaps(previous))
            {
                let data = range.data().clone();
                range = TimeRange::from_list(&[merged.remove(i), range])?.with_data(data);
                i = 0;
            } else {
                i += 1;
            }
        }
        merged.push(range);
    }
    *value = json!({"hours": merged.iter().map(|range| json!({"hours": range.to_string(), "data": range.data()})).collect::<Vec<_>>(), "data": data});
    Ok(())
}
fn parse_specification(definition: &mut Value, item: &Value) -> Result<(), Error> {
    let object = item
        .as_object()
        .ok_or_else(|| Error::InvalidSpecification("expected an object".into()))?;
    let time = |key: &str| -> Result<Option<Time>, Error> {
        match object.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(time)) => {
                let minute = if time.len() == 8
                    && time.as_bytes()[5] == b':'
                    && time.as_bytes()[6..].iter().all(u8::is_ascii_digit)
                {
                    &time[..5]
                } else {
                    time
                };
                Ok(Some(minute.parse()?))
            }
            _ => Err(Error::InvalidSpecification(format!("invalid {key} time"))),
        }
    };
    let opens = time("opens")?;
    let closes = time("closes")?;
    let hours = match (opens, closes) {
        (None, None) => None,
        (Some(opens), Some(closes))
            if opens.hours() == 0
                && opens.minutes() == 0
                && closes.hours() == 0
                && closes.minutes() == 0 =>
        {
            None
        }
        (Some(opens), Some(closes)) => Some(format!(
            "{opens}-{}",
            if closes.to_string() == "23:59" {
                "24:00".into()
            } else {
                closes.to_string()
            }
        )),
        _ => {
            return Err(Error::InvalidSpecification(
                "opens and closes must both be null or strings".into(),
            ));
        }
    };
    let mut append = |key: &str, exception: bool| -> Result<(), Error> {
        let target = if exception {
            definition
                .get_mut("exceptions")
                .and_then(Value::as_object_mut)
        } else {
            definition.as_object_mut()
        }
        .ok_or_else(|| Error::InvalidSpecification("invalid definition".into()))?;
        let entry = target.entry(key.to_owned()).or_insert_with(|| json!([]));
        if let Some(hours) = &hours {
            entry
                .as_array_mut()
                .ok_or_else(|| Error::InvalidSpecification("invalid ranges".into()))?
                .push(json!(hours));
        }
        Ok(())
    };
    if let Some(days) = object.get("dayOfWeek").filter(|v| !v.is_null()) {
        let days = if let Some(days) = days.as_array() {
            days.clone()
        } else {
            vec![days.clone()]
        };
        for day in days {
            let name = day.as_str().ok_or_else(|| {
                Error::InvalidSpecification("dayOfWeek must contain strings".into())
            })?;
            let name = name.strip_prefix("https://schema.org/").unwrap_or(name);
            let day: Day = name.parse()?;
            append(day.name(), false)?;
        }
    } else {
        let from = object
            .get("validFrom")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::InvalidSpecification("missing validFrom".into()))?;
        let through = object
            .get("validThrough")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::InvalidSpecification("missing validThrough".into()))?;
        for key in expand_dates(&format!("{from} to {through}"))? {
            append(&key, true)?;
        }
    }
    Ok(())
}
