//! Differential fixtures from spatie/opening-hours at afc0eecf115fc4e4f93c0ca6113cdd75e84dcbaf.
use spatie_opening_hours::{
    Error, OpeningHours,
    chrono::DateTime,
    serde_json::{self, Value},
};

#[test]
fn schedule_queries_match_php_fixtures() {
    let fixture: Value = serde_json::from_str(include_str!("fixtures/php-parity.json")).unwrap();
    for (group_index, group) in fixture["groups"].as_array().unwrap().iter().enumerate() {
        let hours = OpeningHours::create(group["definition"].clone()).unwrap();
        for (index, case) in group["cases"].as_array().unwrap().iter().enumerate() {
            let date = DateTime::parse_from_rfc3339(case[0].as_str().unwrap()).unwrap();
            assert_eq!(
                hours.is_open_at(&date),
                case[1].as_bool().unwrap(),
                "group {group_index}, case {index}"
            );
            assert_eq!(
                hours.for_date(&date).to_string(),
                case[2].as_str().unwrap(),
                "group {group_index}, case {index}"
            );
            for (offset, result) in [
                hours.next_open(&date),
                hours.next_close(&date),
                hours.previous_open(&date),
                hours.previous_close(&date),
            ]
            .into_iter()
            .enumerate()
            {
                let expected = case[offset + 3]
                    .as_str()
                    .map(|value| DateTime::parse_from_rfc3339(value).unwrap());
                match (result, expected) {
                    (Ok(actual), Some(expected)) => assert_eq!(
                        actual, expected,
                        "group {group_index}, case {index}, boundary {offset}"
                    ),
                    (Err(Error::MaximumLimitExceeded(_)), None) => {}
                    (actual, expected) => panic!(
                        "group {group_index}, case {index}, boundary {offset}: {actual:?} != {expected:?}"
                    ),
                }
            }
        }
    }
}
