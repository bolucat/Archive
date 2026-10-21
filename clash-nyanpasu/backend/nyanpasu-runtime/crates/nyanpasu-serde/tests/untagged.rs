use nyanpasu_serde::DeserializeUntagged;
use serde::{
    Deserialize, Serialize,
    de::{DeserializeOwned, value::StringDeserializer},
};

#[derive(Debug, PartialEq, Serialize, DeserializeUntagged)]
#[serde(untagged)]
enum Scalar {
    Small(u8),
    Large(u64),
    Signed(i64),
    Float(f64),
    Flag(bool),
    Text(String),
    Null,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
enum ScalarReference {
    Small(u8),
    Large(u64),
    Signed(i64),
    Float(f64),
    Flag(bool),
    Text(String),
    Null,
}

fn compare<A: DeserializeOwned + Serialize, B: DeserializeOwned + Serialize>(input: &str) {
    let actual = serde_json::from_str::<A>(input);
    let expected = serde_json::from_str::<B>(input);
    assert_eq!(actual.is_ok(), expected.is_ok(), "{input}");
    if let (Ok(actual), Ok(expected)) = (actual, expected) {
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected).unwrap(),
            "{input}"
        );
    }
}

#[test]
fn scalar_dispatch_keeps_overlapping_variant_order_and_rejections() {
    for input in [
        "0",
        "255",
        "256",
        "-1",
        "18446744073709551615",
        "1.5",
        "true",
        "false",
        "null",
        r#""text""#,
        r#""\u0061""#,
        "{}",
        "[]",
    ] {
        compare::<Scalar, ScalarReference>(input);
    }
    assert_eq!(
        serde_json::from_str::<Scalar>("1").unwrap(),
        Scalar::Small(1)
    );
    assert_eq!(
        serde_json::from_str::<Scalar>("256").unwrap(),
        Scalar::Large(256)
    );
    assert_eq!(
        serde_json::from_str::<Scalar>("-1").unwrap(),
        Scalar::Signed(-1)
    );
    // Integer inputs also match float variants when an earlier integer variant cannot fit.
    #[derive(DeserializeUntagged, Serialize)]
    #[serde(untagged)]
    enum Floating {
        Float(f64),
        Integer(u8),
    }
    assert!(
        matches!(serde_json::from_str::<Floating>("1").unwrap(), Floating::Float(value) if value == 1.0)
    );
}

#[derive(Debug, PartialEq, Serialize, DeserializeUntagged)]
#[serde(rename_all = "kebab-case")]
pub enum Open {
    FirstValue,
    #[serde(rename = "HTTP", alias = "web")]
    Http,
    #[serde(skip_deserializing)]
    Disabled,
    #[serde(untagged)]
    Other(String),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum OpenReference {
    FirstValue,
    #[serde(rename = "HTTP", alias = "web")]
    Http,
    #[serde(skip_deserializing)]
    Disabled,
    #[serde(untagged)]
    Other(String),
}

#[test]
fn open_enums_keep_renames_aliases_skips_and_external_unit_maps() {
    assert_eq!(
        serde_json::to_value(Open::Disabled).unwrap(),
        serde_json::to_value(OpenReference::Disabled).unwrap()
    );
    for input in [
        r#""first-value""#,
        r#""HTTP""#,
        r#""web""#,
        r#""H\u0054TP""#,
        r#""disabled""#,
        r#""unknown""#,
        r#"{"web":null}"#,
        r#"{"HTTP":null}"#,
        r#"{"unknown":null}"#,
        r#"{"disabled":null}"#,
        r#"{"HTTP":1}"#,
        r#"{"HTTP":null,"HTTP":null}"#,
        "{}",
        "[]",
        "null",
        "1",
        "true",
    ] {
        compare::<Open, OpenReference>(input);
    }
    assert_eq!(
        serde_json::from_str::<Open>(r#""web""#).unwrap(),
        Open::Http
    );
    assert_eq!(
        serde_json::from_str::<Open>(r#""disabled""#).unwrap(),
        Open::Other("disabled".into())
    );
}

#[test]
fn owned_unknown_strings_are_moved_for_both_supported_enum_forms() {
    let value = String::from("unknown");
    let pointer = value.as_ptr();
    let parsed =
        Open::deserialize(StringDeserializer::<serde::de::value::Error>::new(value)).unwrap();
    let Open::Other(value) = parsed else {
        panic!("expected unknown")
    };
    assert_eq!(value.as_ptr(), pointer);

    let value = String::from("scalar");
    let pointer = value.as_ptr();
    let parsed =
        Scalar::deserialize(StringDeserializer::<serde::de::value::Error>::new(value)).unwrap();
    let Scalar::Text(value) = parsed else {
        panic!("expected text")
    };
    assert_eq!(value.as_ptr(), pointer);
}

#[derive(Debug, PartialEq)]
struct Parsed(u16);

impl std::str::FromStr for Parsed {
    type Err = std::num::ParseIntError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.parse().map(Self)
    }
}

#[derive(Debug, PartialEq, DeserializeUntagged)]
#[serde(untagged)]
enum ParsedOrText {
    #[nyanpasu_serde(kind = "string")]
    Port(#[serde(deserialize_with = "nyanpasu_serde::from_str")] Parsed),
    Text(String),
}

#[test]
fn custom_string_decoders_keep_first_success_order_without_deserialize_bounds() {
    assert_eq!(
        serde_json::from_str::<ParsedOrText>(r#""443""#).unwrap(),
        ParsedOrText::Port(Parsed(443))
    );
    let value = String::from("not-a-number");
    let pointer = value.as_ptr();
    let parsed =
        ParsedOrText::deserialize(StringDeserializer::<serde::de::value::Error>::new(value))
            .unwrap();
    let ParsedOrText::Text(value) = parsed else {
        panic!("expected fallback")
    };
    assert_eq!(value.as_ptr(), pointer);
}

#[derive(Debug, PartialEq, DeserializeUntagged)]
#[serde(untagged)]
pub enum Borrowed<'a, T = u32>
where
    T: PartialEq,
{
    #[nyanpasu_serde(kind = "string")]
    Text(&'a str),
    #[nyanpasu_serde(kind = "number")]
    Number(T),
}

#[test]
fn generic_lifetimes_preserve_borrowed_strings_and_reject_transient_borrows() {
    let input = String::from(r#""borrowed""#);
    let parsed: Borrowed<'_> = serde_json::from_str(&input).unwrap();
    let Borrowed::Text(value) = parsed else {
        panic!("expected text")
    };
    assert_eq!(value.as_ptr(), input[1..].as_ptr());
    assert_eq!(
        serde_json::from_str::<Borrowed<'_>>("42").unwrap(),
        Borrowed::Number(42)
    );
    assert!(serde_json::from_str::<Borrowed<'_>>(r#""\u0061""#).is_err());
}

#[test]
fn number_or_string_covers_signed_unsigned_bounds_and_null_rejection() {
    #[derive(Deserialize)]
    struct Model {
        #[serde(deserialize_with = "nyanpasu_serde::number_or_string")]
        signed: i64,
        #[serde(default, deserialize_with = "nyanpasu_serde::number_or_string")]
        unsigned: u64,
    }
    for input in [
        r#"{"signed":-9223372036854775808,"unsigned":18446744073709551615}"#,
        r#"{"signed":"-9223372036854775808","unsigned":"18446744073709551615"}"#,
    ] {
        let parsed: Model = serde_json::from_str(input).unwrap();
        assert_eq!(parsed.signed, i64::MIN);
        assert_eq!(parsed.unsigned, u64::MAX);
    }
    assert_eq!(
        serde_json::from_str::<Model>(r#"{"signed":0}"#)
            .unwrap()
            .unsigned,
        0
    );
    for input in [
        r#"{"signed":null}"#,
        r#"{"signed":1.0}"#,
        r#"{"signed":0,"unsigned":-1}"#,
        r#"{"signed":0,"unsigned":"18446744073709551616"}"#,
        r#"{"signed":0,"unsigned":null}"#,
    ] {
        assert!(serde_json::from_str::<Model>(input).is_err(), "{input}");
    }
}
