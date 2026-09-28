use jiff::Timestamp;
use nyanpasu_jobs::*;

#[test]
fn persisted_timestamp_reads_existing_rfc3339_and_keeps_subsecond_precision() {
    let raw = format!(
        r#"{{"id":"{}","job":"job","definition_version":1,"trigger":"Manual","scheduled_at":null,"admitted_at":"2026-09-07T12:34:56.123456789+08:00","admission_sequence":1,"finished_at":null,"state":"Admitted","last_log_sequence":0,"dropped_log_count":0}}"#,
        RunId::new_v4()
    );
    let record: RunRecord = serde_json::from_str(&raw).unwrap();
    let expected: Timestamp = "2026-09-07T04:34:56.123456789Z".parse().unwrap();
    assert_eq!(record.admitted_at, expected);
    let encoded = serde_json::to_string(&record).unwrap();
    let restored: RunRecord = serde_json::from_str(&encoded).unwrap();
    assert_eq!(restored, record);
}
