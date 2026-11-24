//! Public schedule, value-object, timezone and interchange contracts.
use spatie_opening_hours::{
    Day, Error, OpeningHours, OpeningHoursForDay, PreciseTime, SearchOptions, Time, TimeRange,
    chrono::{DateTime, Datelike, FixedOffset, TimeZone, Timelike},
    chrono_tz::Europe::Brussels,
    serde_json::json,
};

fn date(value: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(value).unwrap()
}

#[test]
fn time_validation_comparisons_and_end_of_day() {
    for invalid in [
        "9:00", "24:01", "25:00", "12:60", "10:00:00", "é:00", "", "-1:00",
    ] {
        assert!(
            matches!(Time::from_string(invalid), Err(Error::InvalidTime(_))),
            "{invalid}"
        );
    }
    let end = Time::from_string("24:00").unwrap();
    assert_eq!(end.format("%H:%M:%S"), "24:00:00");
    assert_eq!(
        end.to_date_time(date("2024-02-29T00:00:00Z").date_naive())
            .unwrap()
            .to_string(),
        "2024-03-01 00:00:00"
    );
    let start = Time::from_string("09:15").unwrap();
    assert_eq!(
        start.format("%Y-%m-%d %H:%M %:z"),
        "1970-01-01 09:15 +00:00"
    );
    assert_eq!(start.diff(&end).num_minutes(), 885);
    assert!(start.is_before(&end));
    assert!(end.is_same_or_after(&end));
    assert_eq!(
        start.clone().with_data(json!({"label": "start"})).data()["label"],
        "start"
    );
    let precise = PreciseTime::from_string("09:15:00.123456789").unwrap();
    assert!(!precise.is_same(&start));
    assert_eq!(
        PreciseTime::from(&start).diff(precise).num_nanoseconds(),
        Some(123_456_789)
    );
    assert_eq!(precise.format("%H:%M:%S%.9f"), "09:15:00.123456789");
}

#[test]
fn daily_ranges_are_sorted_half_open_and_read_only() {
    let day = OpeningHoursForDay::from_strings(["13:00-18:00", "09:00-12:00"]).unwrap();
    assert_eq!(day.to_string(), "09:00-12:00,13:00-18:00");
    assert!(day.is_open_at(PreciseTime::from_string("11:59:59.999999").unwrap()));
    assert!(!day.is_open_at(PreciseTime::from_string("12:00:00").unwrap()));
    assert_eq!(
        day.next_open(PreciseTime::from_string("09:00:00").unwrap())
            .unwrap()
            .to_string(),
        "13:00"
    );
    assert_eq!(
        day.previous_close(PreciseTime::from_string("12:00:00.000001").unwrap())
            .unwrap()
            .to_string(),
        "12:00"
    );
    assert!(OpeningHoursForDay::from_strings(["08:00-15:00", "10:00-11:00"]).is_err());
    assert!(OpeningHoursForDay::from_strings(["08:00-10:00", "10:00-11:00"]).is_ok());
    assert_eq!(day.iter().count(), 2);
}

#[test]
fn metadata_and_exception_precedence_survive_definitions() {
    let schedule = OpeningHours::create(json!({
        "data": {"shop": 42},
        "monday": {"hours": ["09:00-12:00", {"hours": "13:00-18:00", "data": ["afternoon", 3]}], "data": "typical"},
        "exceptions": {"12-25": ["10:00-11:00"], "2023-12-25": {"hours": [], "data": "Christmas"}}
    })).unwrap();
    assert_eq!(schedule.data()["shop"], 42);
    assert_eq!(schedule.for_day(Day::Monday).data(), "typical");
    assert_eq!(
        schedule.for_day(Day::Monday)[1].data(),
        &json!(["afternoon", 3])
    );
    assert_eq!(
        schedule.for_date(&date("2023-12-25T10:30:00Z")).data(),
        "Christmas"
    );
    assert!(!schedule.is_open_at(&date("2023-12-25T10:30:00Z")));
    assert!(schedule.is_open_at(&date("2024-12-25T10:30:00Z")));
}

#[test]
fn filters_override_exceptions_in_registration_order() {
    let mut schedule = OpeningHours::create(json!({"exceptions": {"2023-12-25": []}})).unwrap();
    let special = OpeningHoursForDay::from_strings(["07:00-08:00"]).unwrap();
    schedule.add_filter(move |date| (date.month() == 12).then(|| special.clone()));
    schedule.add_filter(|_| Some(OpeningHoursForDay::default()));
    assert!(schedule.is_open_at(&date("2023-12-25T07:30:00Z")));
    assert!(!schedule.is_always_closed());
    assert_eq!(schedule.get_filters().len(), 2);

    let mut overnight = OpeningHours::create(json!({"overflow": true})).unwrap();
    let night = OpeningHoursForDay::from_strings(["22:00-03:00"]).unwrap();
    overnight.add_filter(move |date| (date.day() == 22 && date.hour() == 2).then(|| night.clone()));
    let query = date("2024-11-23T02:00:00Z");
    assert!(overnight.is_open_at(&query));
    assert_eq!(
        overnight.for_date_time(&query).unwrap()[0].to_string(),
        "22:00-03:00"
    );
}

#[test]
fn date_ranges_reject_duplicates_and_expand_leap_dates() {
    let schedule = OpeningHours::create(json!({
        "monday - wednesday": ["08:30-12:00"],
        "thursday to friday": ["14:30-18:00"],
        "exceptions": {"02-28 to 03-01": [], "2024-06-25 to 2024-07-01": [], "11-30-12-01": []}
    }))
    .unwrap();
    assert_eq!(schedule.for_day(Day::Tuesday).to_string(), "08:30-12:00");
    assert!(schedule.exceptional_closing_dates().contains(&"02-29"));
    for definition in [
        json!({"monday to wednesday": [], "tuesday": []}),
        json!({"exceptions": {"11-10 to 11-12": [], "11-11": []}}),
        json!({"exceptions": {"2024-02-30": []}}),
        json!({"exceptions": {"12-25 to 01-01": []}}),
        json!({"exceptions": {"€€€€€€€": []}}),
        json!({"funday": []}),
        json!({"timezone": 5}),
    ] {
        assert!(!OpeningHours::is_valid(definition));
    }
}

#[test]
fn weekday_grouping_ignores_metadata_and_does_not_wrap() {
    let schedule = OpeningHours::create(json!({
        "monday": {"hours": ["09:00-12:00"], "data": "first"},
        "tuesday": ["09:00-12:00"], "thursday": ["09:00-12:00"], "sunday": ["09:00-12:00"]
    }))
    .unwrap();
    assert_eq!(
        schedule.for_week_combined()[0].days,
        vec![Day::Monday, Day::Tuesday, Day::Thursday, Day::Sunday]
    );
    assert_eq!(
        schedule.for_week_consecutive_days()[0].days,
        vec![Day::Monday, Day::Tuesday]
    );
    assert_eq!(schedule.regular_closing_days_iso(), vec![3, 5, 6]);
    assert_eq!(Day::from_name("SUNDAY").unwrap().to_iso(), 7);
}

#[test]
fn overnight_current_ranges_have_calendar_endpoints() {
    let schedule =
        OpeningHours::create(json!({"overflow": true, "friday": ["20:00-03:00"]})).unwrap();
    let query = date("2024-11-23T02:59:59.999999Z");
    assert!(schedule.is_open_at(&query));
    let range = schedule.current_open_range(&query).unwrap().unwrap();
    assert_eq!(range.start(), date("2024-11-22T20:00:00Z"));
    assert_eq!(range.end(), date("2024-11-23T03:00:00Z"));
    assert!(schedule.is_closed_at(&range.end()));
    let midnight = OpeningHours::create(json!({"sunday": ["09:00-24:00"]})).unwrap();
    assert_eq!(
        midnight
            .current_open_range_end(&date("2026-03-22T18:00:00Z"))
            .unwrap(),
        Some(date("2026-03-23T00:00:00Z"))
    );
}

#[test]
fn business_timezone_and_explicit_output_apply_to_all_search_paths() {
    let schedule = OpeningHours::create(json!({"monday": ["09:00-17:00"], "timezone": {"input": "America/New_York", "output": "Europe/Oslo"}})).unwrap();
    let query = date("2024-11-18T15:00:00Z");
    assert_eq!(
        schedule.next_close(&query).unwrap().to_rfc3339(),
        "2024-11-18T23:00:00+01:00"
    );
    assert_eq!(
        schedule.previous_open(&query).unwrap().to_rfc3339(),
        "2024-11-18T15:00:00+01:00"
    );
    let mut schedule = OpeningHours::create(json!({"monday": ["09:00-17:00"]})).unwrap();
    schedule.set_timezone(Some("America/New_York".parse().unwrap()));
    assert_eq!(
        schedule.next_close(&query).unwrap().to_rfc3339(),
        "2024-11-18T22:00:00+00:00"
    );
}

#[test]
fn daylight_saving_changes_measure_elapsed_hours_not_wall_hours() {
    let schedule =
        OpeningHours::create(json!({"sunday": ["01:00-04:00"], "timezone": "Europe/Brussels"}))
            .unwrap();
    assert_eq!(
        schedule
            .diff_in_open_hours(&date("2024-03-31T00:00:00Z"), &date("2024-03-31T02:00:00Z"))
            .unwrap(),
        2.0
    );
    assert_eq!(
        schedule
            .diff_in_open_hours(&date("2024-10-26T23:00:00Z"), &date("2024-10-27T03:00:00Z"))
            .unwrap(),
        4.0
    );
    let gap =
        OpeningHours::create(json!({"sunday": ["02:30-04:00"], "timezone": "Europe/Brussels"}))
            .unwrap();
    assert_eq!(
        gap.next_open(&date("2024-03-31T00:00:00Z")).unwrap(),
        date("2024-03-31T01:30:00Z")
    );
    let fold =
        OpeningHours::create(json!({"sunday": ["02:30-04:00"], "timezone": "Europe/Brussels"}))
            .unwrap();
    assert_eq!(
        fold.next_open(&date("2024-10-26T23:00:00Z")).unwrap(),
        date("2024-10-27T00:30:00Z")
    );
    let start = Brussels
        .with_ymd_and_hms(2024, 3, 30, 23, 0, 0)
        .single()
        .unwrap();
    let end = Brussels
        .with_ymd_and_hms(2024, 3, 31, 5, 0, 0)
        .single()
        .unwrap();
    let implicit_zone = OpeningHours::create(json!({"sunday": ["01:00-04:00"]})).unwrap();
    assert_eq!(
        implicit_zone.next_close(&start).unwrap(),
        date("2024-03-31T02:00:00Z")
    );
    assert_eq!(implicit_zone.diff_in_open_hours(&start, &end).unwrap(), 2.0);
    assert_eq!(
        implicit_zone.diff_in_open_hours(&end, &start).unwrap(),
        -2.0
    );
    assert_eq!(
        implicit_zone.previous_open(&end).unwrap(),
        date("2024-03-31T00:00:00Z")
    );
}

#[test]
fn limits_caps_and_search_horizons_work_in_both_directions() {
    let mut schedule =
        OpeningHours::create(json!({"monday": ["09:00-12:00", "13:00-18:00"]})).unwrap();
    let query = date("2024-11-18T12:30:00Z");
    let cap = date("2024-11-18T12:45:00Z");
    assert_eq!(
        schedule
            .next_open_with(
                &query,
                SearchOptions {
                    cap: Some(cap),
                    search_until: None
                }
            )
            .unwrap(),
        cap
    );
    assert!(matches!(
        schedule.next_open_with(
            &query,
            SearchOptions {
                cap: None,
                search_until: Some(cap)
            }
        ),
        Err(Error::SearchLimitReached)
    ));
    let cap = date("2024-11-18T12:15:00Z");
    assert_eq!(
        schedule
            .previous_close_with(
                &query,
                SearchOptions {
                    cap: Some(cap),
                    search_until: None
                }
            )
            .unwrap(),
        cap
    );
    assert!(matches!(
        schedule.previous_close_with(
            &query,
            SearchOptions {
                cap: None,
                search_until: Some(cap)
            }
        ),
        Err(Error::SearchLimitReached)
    ));
    schedule.set_day_limit(0);
    assert!(matches!(
        schedule.next_open(&date("2024-11-18T20:00:00Z")),
        Err(Error::MaximumLimitExceeded(0))
    ));
    assert_eq!(
        OpeningHours::create(json!({"exceptions": {"12-25": []}}))
            .unwrap()
            .get_day_limit(),
        366
    );
}

#[test]
fn all_day_schedules_have_no_transitions_but_can_be_capped() {
    let schedule = OpeningHours::create(json!({"monday to sunday": ["00:00-24:00"]})).unwrap();
    assert!(schedule.is_always_open());
    assert!(matches!(
        schedule.next_open(&date("2024-11-18T10:00:00Z")),
        Err(Error::MaximumLimitExceeded(8))
    ));
    let closed = OpeningHours::create(json!({})).unwrap();
    assert!(closed.is_always_closed());
    assert_eq!(
        closed
            .diff_in_open_hours(&date("2024-11-18T10:00:00Z"), &date("2024-11-19T10:00:00Z"))
            .unwrap(),
        0.0
    );
}

#[test]
fn time_differences_are_signed_and_retain_fractional_seconds() {
    let schedule = OpeningHours::create(json!({"monday": ["10:00-16:00", "19:30-20:30"]})).unwrap();
    let start = date("2024-11-18T09:59:58.5Z");
    let end = date("2024-11-18T10:59:58.5Z");
    assert_eq!(schedule.diff_in_open_seconds(&start, &end).unwrap(), 3598.5);
    assert_eq!(schedule.diff_in_closed_seconds(&start, &end).unwrap(), 1.5);
    assert_eq!(
        schedule.diff_in_open_seconds(&end, &start).unwrap(),
        -3598.5
    );
    assert_eq!(
        schedule
            .diff_in_open_minutes(&date("2024-11-18T15:00:00Z"), &date("2024-11-18T20:00:00Z"))
            .unwrap(),
        90.0
    );
}

#[test]
fn merges_handle_nested_ranges_bridges_and_metadata_groups() {
    let schedule = OpeningHours::create_and_merge_overlapping_ranges(
        json!({
            "monday": ["08:00-10:00", "12:00-14:00", "09:00-13:00", "14:00-16:00"],
            "exceptions": {"12-25": {"hours": ["08:00-10:00", "09:00-11:00"], "data": "Christmas"}}
        }),
        true,
    )
    .unwrap();
    assert_eq!(schedule.for_day(Day::Monday).to_string(), "08:00-16:00");
    assert_eq!(
        schedule.for_date(&date("2024-12-25T10:00:00Z")).data(),
        "Christmas"
    );
    let definition = json!({"monday": [{"hours": "08:00-12:00", "data": "a"}, {"hours": "10:00-14:00", "data": "b"}]});
    let preserved = OpeningHours::merge_overlapping_ranges(definition.clone(), false).unwrap();
    assert_eq!(preserved["monday"]["hours"].as_array().unwrap().len(), 2);
    assert!(OpeningHours::create(preserved).is_err());
    assert_eq!(
        OpeningHours::create_and_merge_overlapping_ranges(definition, true)
            .unwrap()
            .for_day(Day::Monday)
            .to_string(),
        "08:00-14:00"
    );
}

#[test]
fn structured_data_round_trips_closed_exceptions_and_overnight_hours() {
    let schedule = OpeningHours::create_from_structured_data(json!([
        {"dayOfWeek": ["Monday", "https://schema.org/Tuesday"], "opens": "08:00:30", "closes": "12:00:45"},
        {"dayOfWeek": "Friday", "opens": "18:00", "closes": "02:00"},
        {"dayOfWeek": "Saturday", "opens": "00:00", "closes": "23:59"},
        {"validFrom": "12-24", "validThrough": "12-26", "opens": "00:00", "closes": "00:00"}
    ])).unwrap();
    assert_eq!(schedule.for_day(Day::Monday).to_string(), "08:00-12:00");
    assert_eq!(schedule.for_day(Day::Saturday).to_string(), "00:00-24:00");
    assert!(schedule.is_open_at(&date("2024-11-23T01:30:00Z")));
    let exported = schedule.as_structured_data();
    assert_eq!(
        OpeningHours::create_from_structured_data_str(&exported.to_string())
            .unwrap()
            .as_structured_data(),
        exported
    );
    assert_eq!(
        schedule.as_structured_data_formatted("%H:%M:%S%:z", Some("Europe/Paris".parse().unwrap()))
            [0]["opens"],
        "08:00:00+01:00"
    );
}

#[test]
fn structured_data_rejects_invalid_shapes_and_unsupported_holidays() {
    for input in [
        json!({}),
        json!([{"dayOfWeek": true}]),
        json!([{"dayOfWeek": [true]}]),
        json!([{"dayOfWeek": "PublicHolidays"}]),
        json!([{"dayOfWeek": "Monday", "closes": "12:00"}]),
        json!([{"validFrom": "2024-12-25"}]),
        json!([{"dayOfWeek": "Monday", "opens": "noon", "closes": "14:00"}]),
    ] {
        assert!(OpeningHours::create_from_structured_data(input).is_err());
    }
    assert!(OpeningHours::create_from_structured_data_str("{").is_err());
    let range = TimeRange::from_string("20:00-03:00").unwrap();
    assert!(range.contains_night_time(&Time::from_string("02:59").unwrap()));
    assert!(!range.contains_time(&Time::from_string("02:59").unwrap()));
}
