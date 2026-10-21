use std::collections::BTreeMap;

use nyanpasu_serde::{DeserializeFields, DeserializeMap};
use serde::Deserialize;
use serde_json::{Value, json};

type Optional<T> = Option<T>;

fn default_count() -> usize {
    7
}

#[derive(Debug, PartialEq, Deserialize, DeserializeFields)]
#[serde(rename_all = "camelCase")]
pub struct Known<T> {
    #[serde(rename = "type", alias = "kind")]
    pub category: T,
    pub optional_value: Optional<u16>,
    #[serde(default = "default_count")]
    pub count: usize,
}

#[derive(Debug, PartialEq, DeserializeMap)]
pub struct Envelope<T> {
    pub id: u32,
    #[serde(flatten)]
    pub known: Known<T>,
    #[serde(flatten)]
    #[nyanpasu_serde(extra)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, PartialEq, Deserialize)]
struct Reference<T> {
    id: u32,
    #[serde(flatten)]
    known: Known<T>,
    #[serde(flatten)]
    extra: BTreeMap<String, Value>,
}

#[test]
fn flattened_generic_fields_match_serde_and_keep_nested_extras() {
    for input in [
        r#"{"id":1,"kind":"tcp","z":{"a":[null,true]},"a":1}"#,
        r#"{"type":"tcp","optionalValue":null,"id":1,"count":3,"z":1,"z":2}"#,
        r#"{"id":1,"t\u0079pe":"tcp","optional\u0056alue":80,"extra":42}"#,
    ] {
        let actual: Envelope<String> = serde_json::from_str(input).unwrap();
        let expected: Reference<String> = serde_json::from_str(input).unwrap();
        assert_eq!(actual.id, expected.id);
        assert_eq!(actual.known, expected.known);
        assert_eq!(actual.extra, expected.extra);
        let reader: Envelope<String> = serde_json::from_reader(input.as_bytes()).unwrap();
        let owned: Envelope<String> =
            serde_json::from_value(serde_json::from_str(input).unwrap()).unwrap();
        assert_eq!(reader, actual);
        assert_eq!(owned, actual);
    }
}

#[test]
fn duplicates_aliases_missing_fields_and_bad_types_are_rejected() {
    for input in [
        r#"{"id":1,"type":"tcp","kind":"udp"}"#,
        r#"{"id":1,"type":"tcp","optionalValue":null,"optionalValue":80}"#,
        r#"{"id":1,"type":"tcp","id":2}"#,
        r#"{"type":"tcp"}"#,
        r#"{"id":1}"#,
        r#"{"id":1,"type":"tcp","count":null}"#,
        "[]",
        "null",
    ] {
        assert!(
            serde_json::from_str::<Envelope<String>>(input).is_err(),
            "{input}"
        );
    }
}

#[derive(Debug, PartialEq)]
struct Custom(u16);

fn decode_custom<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Custom, D::Error> {
    String::deserialize(deserializer)?
        .parse()
        .map(Custom)
        .map_err(serde::de::Error::custom)
}

#[derive(Debug, PartialEq, DeserializeMap)]
struct CustomModel<'a, T> {
    text: &'a str,
    value: T,
    #[serde(deserialize_with = "decode_custom")]
    custom: Custom,
    #[serde(skip_deserializing, default = "default_count")]
    skipped: usize,
}

#[test]
fn borrowing_and_custom_decoding_do_not_require_deserialize_on_custom_type() {
    let input = r#"{"text":"borrowed","value":true,"custom":"443","skipped":100}"#;
    let model: CustomModel<'_, bool> = serde_json::from_str(input).unwrap();
    assert_eq!(model.custom, Custom(443));
    assert_eq!(model.skipped, 7);
    let offset = input.find("borrowed").unwrap();
    assert_eq!(model.text.as_ptr(), input[offset..].as_ptr());
    for input in [
        r#"{"text":"a","value":true}"#,
        r#"{"text":"a","value":true,"custom":443}"#,
        r#"{"text":"a","value":true,"custom":"1","custom":"2"}"#,
    ] {
        assert!(serde_json::from_str::<CustomModel<'_, bool>>(input).is_err());
    }
}

#[test]
fn generic_defaults_where_clauses_and_internal_field_names_remain_usable() {
    #[derive(DeserializeMap)]
    struct Model<T = String>
    where
        T: Eq,
    {
        optional: Option<T>,
        __marker: u8,
        #[serde(deserialize_with = "decode_custom")]
        custom: Custom,
    }
    let model: Model = serde_json::from_str(r#"{"__marker":1,"custom":"80"}"#).unwrap();
    assert_eq!(model.optional, None);
    assert_eq!(model.__marker, 1);
    assert_eq!(model.custom, Custom(80));
}

#[cfg(feature = "indexmap")]
#[test]
fn indexmap_preserves_first_position_and_last_unknown_value() {
    #[derive(DeserializeMap)]
    struct Model {
        #[serde(flatten)]
        #[nyanpasu_serde(extra)]
        extra: indexmap::IndexMap<String, Value>,
    }
    let model: Model = serde_json::from_str(r#"{"z":null,"a":{},"z":[1]}"#).unwrap();
    assert_eq!(
        model.extra.keys().map(String::as_str).collect::<Vec<_>>(),
        ["z", "a"]
    );
    assert_eq!(model.extra["z"], json!([1]));
}

#[test]
fn direct_models_can_be_nested_in_options_and_newtypes() {
    #[derive(Debug, PartialEq, DeserializeMap)]
    struct Model {
        optional: Optional<u32>,
    }
    #[derive(Deserialize)]
    struct Wrapped(Model);
    let model: Option<Model> = serde_json::from_value(json!({})).unwrap();
    assert_eq!(model, Some(Model { optional: None }));
    assert!(
        serde_json::from_str::<Option<Model>>("null")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        serde_json::from_str::<Wrapped>("{}").unwrap().0,
        model.unwrap()
    );
}
