use nyanpasu_jobs::{dto::*, *};
#[test]
fn wire_cursor_preserves_large_integer_and_rejects_invalid_ids() {
    let cursor = RunCursor {
        sequence: u64::MAX,
        id: RunId::new_v4(),
    };
    let dto = RunCursorDto::from(cursor.clone());
    let json = serde_json::to_string(&dto).unwrap();
    assert!(json.contains("\"18446744073709551615\""));
    assert_eq!(
        RunCursor::try_from(serde_json::from_str::<RunCursorDto>(&json).unwrap()).unwrap(),
        cursor
    );
    assert!(
        RunCursor::try_from(RunCursorDto {
            sequence: "-1".into(),
            id: cursor.id.to_string()
        })
        .is_err()
    );
}
#[cfg(feature = "specta")]
#[test]
fn finite_wire_types_export_without_recursive_json_any_or_bigint() {
    let types = specta::Types::default()
        .register::<RunPageDto>()
        .register::<LogPageDto>();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("jobs.ts");
    specta_typescript::Typescript::default()
        .export_to(&path, &types, specta_serde::Format)
        .unwrap();
    let output = std::fs::read_to_string(path).unwrap();
    assert!(output.contains("admission_sequence: string"));
    assert!(output.contains("sequence: string"));
    assert!(!output.contains("bigint"));
    assert!(!output.contains("any"));
    assert!(!output.contains("JsonValue"));
}
