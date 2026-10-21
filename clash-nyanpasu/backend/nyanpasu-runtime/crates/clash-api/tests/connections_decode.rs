use clash_api::{
    ConfigEnum, Connection, ConnectionMetadata, ConnectionNetwork, ConnectionsSnapshot,
};
use serde_json::{Value, json};

const CONNECTION: &str = include_str!("fixtures/connection.json");

#[test]
fn complete_connection_preserves_every_field_and_the_ipc_shape() {
    let mut expected: Value = serde_json::from_str(CONNECTION).unwrap();
    expected["_extra"] = json!({});
    expected["metadata"]["_extra"] = json!({});
    let connection: Connection = serde_json::from_str(CONNECTION).unwrap();
    assert_eq!(serde_json::to_value(&connection).unwrap(), expected);
    // Exercise owned-value input as well as borrowed JSON input.
    let owned: Connection =
        serde_json::from_value(serde_json::from_str(CONNECTION).unwrap()).unwrap();
    assert_eq!(owned, connection);
}

#[test]
fn missing_and_null_optional_fields_keep_their_defaults() {
    let empty: ConnectionMetadata = serde_json::from_str("{}").unwrap();
    let mut metadata: Value = serde_json::from_str(CONNECTION).unwrap();
    for value in metadata["metadata"].as_object_mut().unwrap().values_mut() {
        *value = Value::Null;
    }
    let nulls: ConnectionMetadata = serde_json::from_value(metadata["metadata"].clone()).unwrap();
    assert_eq!(nulls, empty);
    assert_eq!(serde_json::to_value(empty).unwrap(), json!({"_extra": {}}));

    let mut value: Value = serde_json::from_str(CONNECTION).unwrap();
    value.as_object_mut().unwrap().remove("metadata");
    value.as_object_mut().unwrap().remove("providerChains");
    let missing: Connection = serde_json::from_value(value.clone()).unwrap();
    value["metadata"] = Value::Null;
    value["providerChains"] = Value::Null;
    assert_eq!(
        serde_json::from_value::<Connection>(value).unwrap(),
        missing
    );
    assert!(missing.metadata.is_none());
    assert!(missing.provider_chains.is_none());

    let snapshot: ConnectionsSnapshot =
        serde_json::from_str(r#"{"downloadTotal":0,"uploadTotal":0,"connections":null}"#).unwrap();
    assert!(snapshot.connections.is_none());
    assert!(snapshot.memory.is_none());
}

#[test]
fn unknown_fields_preserve_nested_values_order_and_last_duplicate_value() {
    let metadata: ConnectionMetadata = serde_json::from_str(
        r#"{"z":1,"extension":{"nested":[null,true,-1,1.5,"a\nb",{"x":2}]},"extra":3,"_extra":4,"z":5}"#,
    ).unwrap();
    assert_eq!(
        metadata
            .extra
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["z", "extension", "extra", "_extra"]
    );
    assert_eq!(
        serde_json::to_value(metadata).unwrap(),
        json!({"_extra": {
            "z":5, "extension":{"nested":[null,true,-1,1.5,"a\nb",{"x":2}]}, "extra":3, "_extra":4
        }})
    );

    let input = CONNECTION.trim_end().strip_suffix('}').unwrap().to_owned()
        + r#", "z":1,"extension":{"a":[null,"text"]},"_extra":4,"z":5}"#;
    let connection: Connection = serde_json::from_str(&input).unwrap();
    assert_eq!(
        connection
            .extra
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["z", "extension", "_extra"]
    );
    assert_eq!(connection.extra["z"], 5);
    assert_eq!(connection.extra["_extra"], 4);
    assert_eq!(connection.extra["extension"], json!({"a":[null,"text"]}));
}

#[test]
fn duplicate_known_fields_are_rejected_even_when_the_first_is_null() {
    let value: Value = serde_json::from_str(CONNECTION).unwrap();
    for (key, field) in value.as_object().unwrap() {
        let input = format!(
            "{}, {key:?}:{field}}}",
            CONNECTION.trim_end().strip_suffix('}').unwrap()
        );
        assert!(serde_json::from_str::<Connection>(&input).is_err(), "{key}");
    }
    for (key, field) in value["metadata"].as_object().unwrap() {
        for first in [field.clone(), Value::Null] {
            let input = format!("{{{key:?}:{first},{key:?}:{field}}}");
            assert!(
                serde_json::from_str::<ConnectionMetadata>(&input).is_err(),
                "{input}"
            );
        }
    }
    for key in ["metadata", "providerChains"] {
        let mut value = value.clone();
        value[key] = Value::Null;
        let input = format!(
            "{}, {key:?}:null}}",
            value.to_string().strip_suffix('}').unwrap()
        );
        assert!(serde_json::from_str::<Connection>(&input).is_err(), "{key}");
    }
}

#[test]
fn required_fields_and_invalid_types_are_rejected() {
    for key in [
        "id",
        "upload",
        "download",
        "start",
        "chains",
        "rule",
        "rulePayload",
    ] {
        let mut value: Value = serde_json::from_str(CONNECTION).unwrap();
        value.as_object_mut().unwrap().remove(key);
        assert!(
            serde_json::from_value::<Connection>(value.clone()).is_err(),
            "{key}"
        );
        value[key] = Value::Null;
        assert!(
            serde_json::from_value::<Connection>(value).is_err(),
            "{key}"
        );
    }
    for input in [
        "null",
        "[]",
        "1",
        r#""text""#,
        r#"{"uid":-1}"#,
        r#"{"dscp":256}"#,
        r#"{"network":true}"#,
        r#"{"sourceGeoIP":"LAN"}"#,
        r#"{"host":12}"#,
    ] {
        assert!(
            serde_json::from_str::<ConnectionMetadata>(input).is_err(),
            "{input}"
        );
    }
}

#[test]
fn ports_accept_decimal_strings_and_integers_without_changing_serialization() {
    for (input, expected) in [
        ("0", 0),
        ("65535", 65535),
        (r#""0""#, 0),
        (r#""65535""#, 65535),
        (r#""+1""#, 1),
        (r#""0001""#, 1),
        (r#""\u0034\u0034\u0033""#, 443),
    ] {
        for key in ["sourcePort", "destinationPort", "inboundPort"] {
            let metadata: ConnectionMetadata =
                serde_json::from_str(&format!("{{{key:?}:{input}}}")).unwrap();
            assert_eq!(
                serde_json::to_value(metadata).unwrap()[key],
                expected.to_string()
            );
        }
    }
    for input in [
        "-1",
        "65536",
        "1.0",
        "true",
        "[]",
        "{}",
        r#""""#,
        r#""-1""#,
        r#""65536""#,
        r#"" 1""#,
        r#""1.0""#,
    ] {
        for key in ["sourcePort", "destinationPort", "inboundPort"] {
            assert!(
                serde_json::from_str::<ConnectionMetadata>(&format!("{{{key:?}:{input}}}"))
                    .is_err(),
                "{key}: {input}"
            );
        }
    }
}

#[test]
fn open_enums_keep_known_and_unknown_escaped_strings() {
    for input in [r#""tcp""#, r#""t\u0063p""#] {
        assert_eq!(
            serde_json::from_str::<ConfigEnum<ConnectionNetwork>>(input).unwrap(),
            ConfigEnum::Known(ConnectionNetwork::Tcp)
        );
    }
    let unknown = ConfigEnum::<ConnectionNetwork>::Unknown("future\n网络".to_owned());
    assert_eq!(
        serde_json::from_str::<ConfigEnum<ConnectionNetwork>>(r#""future\n网络""#).unwrap(),
        unknown
    );
    assert_eq!(
        serde_json::from_value::<ConfigEnum<ConnectionNetwork>>(json!("future\n网络")).unwrap(),
        unknown
    );
    for input in ["null", "1", "true", "[]", "{}"] {
        assert!(serde_json::from_str::<ConfigEnum<ConnectionNetwork>>(input).is_err());
    }
}
