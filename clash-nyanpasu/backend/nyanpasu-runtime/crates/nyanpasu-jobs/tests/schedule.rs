use jiff::Timestamp;
use nyanpasu_jobs::Schedule;

#[test]
fn cron_timezone_validation_and_dst_are_explicit() {
    let spring = Schedule::Cron {
        expression: "30 2 * * *".into(),
        timezone: "America/New_York".into(),
    };
    let before: Timestamp = "2026-03-08T06:59:59Z".parse().unwrap();
    let next = spring.next_cron(before).unwrap();
    // Fixed-time expression in a spring-forward DST gap shifts to the first valid instant (03:00 EDT).
    assert_eq!(next, "2026-03-08T07:00:00Z".parse().unwrap());

    let fall = Schedule::Cron {
        expression: "30 1 * * *".into(),
        timezone: "America/New_York".into(),
    };
    let before: Timestamp = "2026-11-01T04:00:00Z".parse().unwrap();
    let first = fall.next_cron(before).unwrap();
    let second = fall.next_cron(first).unwrap();
    assert_eq!(first, "2026-11-01T05:30:00Z".parse().unwrap());
    assert_eq!(second, "2026-11-02T06:30:00Z".parse().unwrap());

    // FixedTimeRule::NoWildcard treats hour lists as fixed-time, so fall overlap runs once.
    let fall_list = Schedule::Cron {
        expression: "30 1,13 * * *".into(),
        timezone: "America/New_York".into(),
    };
    let first = fall_list.next_cron(before).unwrap();
    let second = fall_list.next_cron(first).unwrap();
    assert_eq!(first, "2026-11-01T05:30:00Z".parse().unwrap());
    assert_eq!(second, "2026-11-01T18:30:00Z".parse().unwrap());
    for (expression, timezone) in [
        ("0 0 30 2 *", "UTC"),
        ("0 0 0 * * * 2026", "UTC"),
        ("* * * * *", "Invalid/Zone"),
        ("*/0 * * * *", "UTC"),
    ] {
        assert!(
            Schedule::Cron {
                expression: expression.into(),
                timezone: timezone.into()
            }
            .validate(Timestamp::now())
            .is_err()
        );
    }
}

#[test]
fn cron_five_and_six_fields_have_exclusive_future_occurrences() {
    let after: Timestamp = "2026-09-07T00:00:00Z".parse().unwrap();
    for expression in ["* * * * *", "0 * * * * *"] {
        let schedule = Schedule::Cron {
            expression: expression.into(),
            timezone: "UTC".into(),
        };
        assert_eq!(
            schedule.next_cron(after).unwrap(),
            "2026-09-07T00:01:00Z".parse().unwrap()
        );
    }
    let seconds = Schedule::Cron {
        expression: "15 * * * * *".into(),
        timezone: "UTC".into(),
    };
    assert_eq!(
        seconds.next_cron(after).unwrap(),
        "2026-09-07T00:00:15Z".parse().unwrap()
    );
}

#[test]
fn cron_hourly_wildcard_visits_both_fall_overlap_hours() {
    let schedule = Schedule::Cron {
        expression: "0 * * * *".into(),
        timezone: "America/New_York".into(),
    };
    let first: Timestamp = "2026-11-01T05:00:00Z".parse().unwrap();
    assert_eq!(
        schedule.next_cron(first).unwrap(),
        "2026-11-01T06:00:00Z".parse().unwrap()
    );
}

#[test]
fn cron_subsecond_input_stays_exclusive() {
    let schedule = Schedule::Cron {
        expression: "* * * * *".into(),
        timezone: "UTC".into(),
    };
    let after: Timestamp = "2026-09-07T00:00:00.5Z".parse().unwrap();
    assert_eq!(
        schedule.next_cron(after).unwrap(),
        "2026-09-07T00:01:00Z".parse().unwrap()
    );
}

#[test]
fn cron_day_fields_use_posix_or_and_sunday_zero_or_seven() {
    let either = Schedule::Cron {
        expression: "0 0 1 * MON".into(),
        timezone: "UTC".into(),
    };
    let after: Timestamp = "2026-01-01T00:00:00Z".parse().unwrap();
    assert_eq!(
        either.next_cron(after).unwrap(),
        "2026-01-05T00:00:00Z".parse().unwrap()
    );
    let late_jan: Timestamp = "2026-01-26T00:00:00Z".parse().unwrap();
    assert_eq!(
        either.next_cron(late_jan).unwrap(),
        "2026-02-01T00:00:00Z".parse().unwrap()
    );

    let both = Schedule::Cron {
        expression: "0 0 1 * +MON".into(),
        timezone: "UTC".into(),
    };
    assert_eq!(
        both.next_cron(after).unwrap(),
        "2026-06-01T00:00:00Z".parse().unwrap()
    );

    let sunday_zero = Schedule::Cron {
        expression: "0 0 * * 0".into(),
        timezone: "UTC".into(),
    };
    let sunday_seven = Schedule::Cron {
        expression: "0 0 * * 7".into(),
        timezone: "UTC".into(),
    };
    let monday = Schedule::Cron {
        expression: "0 0 * * 1".into(),
        timezone: "UTC".into(),
    };
    let saturday = Schedule::Cron {
        expression: "0 0 * * 6".into(),
        timezone: "UTC".into(),
    };
    let friday: Timestamp = "2026-09-04T00:00:00Z".parse().unwrap();
    assert_eq!(
        saturday.next_cron(friday).unwrap(),
        "2026-09-05T00:00:00Z".parse().unwrap()
    );
    assert_eq!(
        sunday_zero.next_cron(friday).unwrap(),
        "2026-09-06T00:00:00Z".parse().unwrap()
    );
    assert_eq!(
        sunday_seven.next_cron(friday).unwrap(),
        "2026-09-06T00:00:00Z".parse().unwrap()
    );
    assert_eq!(
        monday.next_cron(friday).unwrap(),
        "2026-09-07T00:00:00Z".parse().unwrap()
    );
    assert!(
        Schedule::Cron {
            expression: "0 0 * * 8".into(),
            timezone: "UTC".into()
        }
        .validate(friday)
        .is_err()
    );
}

#[test]
fn cron_fractional_second_respects_weekday() {
    let monday = Schedule::Cron {
        expression: "* * * * * MON".into(),
        timezone: "UTC".into(),
    };
    let sunday: Timestamp = "2026-09-06T00:00:00.5Z".parse().unwrap();
    assert_eq!(
        monday.next_cron(sunday).unwrap(),
        "2026-09-07T00:00:00Z".parse().unwrap()
    );
}

#[test]
fn cron_library_shorthand_aliases_remain_available() {
    let after: Timestamp = "2026-09-07T00:00:00Z".parse().unwrap();
    for (alias, next) in [
        ("@hourly", "2026-09-07T01:00:00Z"),
        ("@daily", "2026-09-08T00:00:00Z"),
        ("@weekly", "2026-09-13T00:00:00Z"),
        ("@monthly", "2026-10-01T00:00:00Z"),
        ("@yearly", "2027-01-01T00:00:00Z"),
        ("@annually", "2027-01-01T00:00:00Z"),
    ] {
        let schedule = Schedule::Cron {
            expression: alias.into(),
            timezone: "UTC".into(),
        };
        assert_eq!(schedule.next_cron(after).unwrap(), next.parse().unwrap());
    }
    let invalid = Schedule::Cron {
        expression: "@quarterly".into(),
        timezone: "UTC".into(),
    };
    assert!(invalid.validate(after).is_err());
}
