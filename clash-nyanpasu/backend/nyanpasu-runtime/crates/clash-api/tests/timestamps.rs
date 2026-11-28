//! Pins the wire form of every controller timestamp. Expected strings are what
//! chrono's serde implementations wrote for the same input.

use clash_api::{
    Connection, DelayHistory, OffsetTimestamp, ProxyProvider, Rule, RuleProvider,
    StructuredLogEntry,
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

fn rewritten<T: DeserializeOwned + Serialize>(value: Value) -> Value {
    serde_json::to_value(serde_json::from_value::<T>(value).unwrap()).unwrap()
}

/// `(input, serialized)`.
const TIMESTAMPS: &[(&str, &str)] = &[
    // Already canonical: written back unchanged.
    ("2026-09-07T12:00:00Z", "2026-09-07T12:00:00Z"),
    ("2026-09-07T12:00:00+08:00", "2026-09-07T12:00:00+08:00"),
    ("2026-09-07T12:00:00-05:30", "2026-09-07T12:00:00-05:30"),
    ("2026-09-07T12:00:00.500Z", "2026-09-07T12:00:00.500Z"),
    (
        "2026-09-07T12:00:00.123456+08:00",
        "2026-09-07T12:00:00.123456+08:00",
    ),
    (
        "2026-07-29T00:17:26.518376100+08:00",
        "2026-07-29T00:17:26.518376100+08:00",
    ),
    (
        "2026-07-28T16:16:26.000000001Z",
        "2026-07-28T16:16:26.000000001Z",
    ),
    // Go's zero `time.Time`.
    ("0001-01-01T00:00:00Z", "0001-01-01T00:00:00Z"),
    (
        "1969-12-31T23:59:59.999+23:59",
        "1969-12-31T23:59:59.999+23:59",
    ),
    // Go trims trailing fraction zeros; the output pads to 3, 6 or 9 digits.
    (
        "2026-09-07T12:00:00.5+08:00",
        "2026-09-07T12:00:00.500+08:00",
    ),
    (
        "2026-07-29T00:17:26.5183761+08:00",
        "2026-07-29T00:17:26.518376100+08:00",
    ),
    ("2026-09-07T12:00:00.000Z", "2026-09-07T12:00:00Z"),
    ("2026-09-07T12:00:00.123000Z", "2026-09-07T12:00:00.123Z"),
    // Digits past nanoseconds are dropped.
    (
        "2026-09-07T12:00:00.1234567899999+08:00",
        "2026-09-07T12:00:00.123456789+08:00",
    ),
    // Every spelling of a zero offset is written as `Z`.
    ("2026-09-07T12:00:00+00:00", "2026-09-07T12:00:00Z"),
    ("2026-09-07T12:00:00-00:00", "2026-09-07T12:00:00Z"),
    ("2026-09-07t12:00:00z", "2026-09-07T12:00:00Z"),
    ("2026-09-07 12:00:00+0800", "2026-09-07T12:00:00+08:00"),
];

#[test]
fn offset_timestamps_keep_their_offset_and_fraction_width() {
    for (input, expected) in TIMESTAMPS {
        assert_eq!(
            rewritten::<OffsetTimestamp>(json!(input)),
            json!(expected),
            "{input}"
        );
        assert_eq!(
            rewritten::<OffsetTimestamp>(json!(expected)),
            json!(expected)
        );
    }
}

#[test]
fn offset_timestamps_require_a_date_a_time_and_an_offset() {
    for input in [
        "",
        "invalid",
        "2026-09-07",
        "2026-09-07T12:00:00",
        "2026-09-07+08:00",
        "2026-13-07T12:00:00Z",
    ] {
        assert!(input.parse::<OffsetTimestamp>().is_err(), "{input}");
    }
}

#[test]
fn to_rfc3339_writes_utc_as_a_numeric_offset() {
    for (input, expected) in [
        ("2026-09-07T12:00:00Z", "2026-09-07T12:00:00+00:00"),
        ("2026-09-07T12:00:00.5Z", "2026-09-07T12:00:00.500+00:00"),
        ("2026-09-07T12:00:00+08:00", "2026-09-07T12:00:00+08:00"),
    ] {
        let timestamp: OffsetTimestamp = input.parse().unwrap();
        assert_eq!(timestamp.to_rfc3339(), expected);
    }
}

#[test]
fn offset_timestamps_compare_by_instant() {
    let utc: OffsetTimestamp = "2026-09-07T04:00:00Z".parse().unwrap();
    let local: OffsetTimestamp = "2026-09-07T12:00:00+08:00".parse().unwrap();
    let later: OffsetTimestamp = "2026-09-07T12:00:01+08:00".parse().unwrap();
    assert_eq!(utc, local);
    assert!(utc < later);
}

/// chrono kept `:60`; jiff clamps it to `:59`. No supported core writes one.
#[test]
fn leap_seconds_are_clamped() {
    assert_eq!(
        rewritten::<OffsetTimestamp>(json!("2016-12-31T23:59:60.5Z")),
        json!("2016-12-31T23:59:59.500Z")
    );
    let log = json!({"time":"23:59:60","level":"info","message":"","fields":[]});
    assert_eq!(
        rewritten::<StructuredLogEntry>(log)["time"],
        json!("23:59:59")
    );
}

#[test]
fn structured_log_times_keep_their_fraction_width() {
    for (input, expected) in [
        ("12:34:56", "12:34:56"),
        ("00:00:00", "00:00:00"),
        ("12:34", "12:34:00"),
        ("12:34:56.5", "12:34:56.500"),
        ("12:34:56.000", "12:34:56"),
        ("12:34:56.123456", "12:34:56.123456"),
        ("12:34:56.1234567", "12:34:56.123456700"),
        ("12:34:56.000000001", "12:34:56.000000001"),
    ] {
        let log = json!({"time":input,"level":"warn","message":"x","fields":[]});
        assert_eq!(
            rewritten::<StructuredLogEntry>(log)["time"],
            json!(expected),
            "{input}"
        );
    }
}

#[test]
fn every_timestamp_field_round_trips() {
    const LOCAL: &str = "2026-09-07T10:00:00.123456789+08:00";
    const UTC: &str = "2026-09-07T02:00:00Z";

    let history = json!({"time":LOCAL,"delay":42});
    assert_eq!(rewritten::<DelayHistory>(history.clone()), history);

    let provider = rewritten::<ProxyProvider>(json!({
        "name":"p","type":"Proxy","vehicleType":"HTTP","updatedAt":LOCAL,
        "proxies":[{"name":"n","type":"Vless","udp":true,"history":[history],
            "extra":{"https://example.com":{"alive":true,"history":[{"time":UTC,"delay":1}]}}}]
    }));
    assert_eq!(provider["updatedAt"], json!(LOCAL));
    assert_eq!(provider["proxies"][0]["history"][0]["time"], json!(LOCAL));
    assert_eq!(
        provider["proxies"][0]["extra"]["https://example.com"]["history"][0]["time"],
        json!(UTC)
    );

    let extra = json!({"disabled":false,"hitCount":1,"hitAt":LOCAL,"missCount":0,"missAt":UTC});
    let rule =
        rewritten::<Rule>(json!({"type":"Match","payload":"","proxy":"DIRECT","extra":extra}));
    assert_eq!(rule["extra"], extra);

    let rule_provider = rewritten::<RuleProvider>(json!({"name":"r","updatedAt":LOCAL}));
    assert_eq!(rule_provider["updatedAt"], json!(LOCAL));

    let connection: Value = serde_json::from_str(include_str!("fixtures/connection.json")).unwrap();
    assert_eq!(
        rewritten::<Connection>(connection.clone())["start"],
        connection["start"]
    );
}
