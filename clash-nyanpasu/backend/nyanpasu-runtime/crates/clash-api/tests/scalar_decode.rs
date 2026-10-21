use clash_api::{BytesPerSecond, ProviderType, RuleFormat, RuleProviderBehavior, VehicleType};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Deserialize, Serialize)]
enum ProviderReference {
    Proxy,
    Rule,
    #[serde(untagged)]
    Unknown(String),
}

#[derive(Deserialize, Serialize)]
enum VehicleReference {
    File,
    #[serde(rename = "HTTP")]
    Http,
    Compatible,
    Inline,
    #[serde(untagged)]
    Unknown(String),
}

#[derive(Deserialize, Serialize)]
enum BehaviorReference {
    Domain,
    #[serde(rename = "IPCIDR")]
    IpCidr,
    Classical,
    #[serde(untagged)]
    Unknown(String),
}

#[derive(Deserialize, Serialize)]
enum FormatReference {
    YamlRule,
    TextRule,
    MrsRule,
    #[serde(untagged)]
    Unknown(String),
}

fn compare<A: DeserializeOwned + Serialize, B: DeserializeOwned + Serialize>(input: &str) {
    fn output<T: Serialize>(result: Result<T, serde_json::Error>) -> Result<serde_json::Value, ()> {
        result
            .map(|value| serde_json::to_value(value).unwrap())
            .map_err(|_| ())
    }
    assert_eq!(
        output(serde_json::from_str::<A>(input)),
        output(serde_json::from_str::<B>(input)),
        "slice: {input}"
    );
    assert_eq!(
        output(serde_json::from_reader::<_, A>(input.as_bytes())),
        output(serde_json::from_reader::<_, B>(input.as_bytes())),
        "reader: {input}"
    );
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(input) {
        assert_eq!(
            output(serde_json::from_value::<A>(value.clone())),
            output(serde_json::from_value::<B>(value)),
            "owned: {input}"
        );
    }
}

#[test]
fn open_string_enums_keep_strings_external_maps_and_rejected_inputs() {
    for name in [
        "Proxy",
        "Rule",
        "File",
        "HTTP",
        "Compatible",
        "Inline",
        "Domain",
        "IPCIDR",
        "Classical",
        "YamlRule",
        "TextRule",
        "MrsRule",
        "Unknown",
        "future\n网络",
        "",
    ] {
        let key = serde_json::to_string(name).unwrap();
        for input in [
            key.clone(),
            format!("{{{key}:null}}"),
            format!("{{{key}:0}}"),
            format!("{{{key}:null,{key}:null}}"),
            format!("{{{key}:null,\"other\":null}}"),
        ] {
            compare::<ProviderType, ProviderReference>(&input);
            compare::<VehicleType, VehicleReference>(&input);
            compare::<RuleProviderBehavior, BehaviorReference>(&input);
            compare::<RuleFormat, FormatReference>(&input);
        }
    }
    for input in [
        "null",
        "true",
        "0",
        "[]",
        "{}",
        r#""H\u0054TP""#,
        r#"{"H\u0054TP":null}"#,
    ] {
        compare::<ProviderType, ProviderReference>(input);
        compare::<VehicleType, VehicleReference>(input);
        compare::<RuleProviderBehavior, BehaviorReference>(input);
        compare::<RuleFormat, FormatReference>(input);
    }
}

#[derive(Deserialize, Serialize)]
#[serde(transparent)]
struct RateReference(#[serde(deserialize_with = "number_reference")] i64);

fn number_reference<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Number {
        Number(i64),
        Text(String),
    }
    match Number::deserialize(deserializer)? {
        Number::Number(value) => Ok(value),
        Number::Text(value) => value.parse().map_err(serde::de::Error::custom),
    }
}

#[test]
fn numeric_samples_preserve_bounds_string_parsing_and_rejections() {
    for input in [
        "0",
        "-1",
        "9223372036854775807",
        "-9223372036854775808",
        "9223372036854775808",
        "-9223372036854775809",
        "1.0",
        "1e2",
        "null",
        "true",
        "[]",
        "{}",
        r#""0""#,
        r#""-1""#,
        r#""+1""#,
        r#""0001""#,
        r#""9223372036854775807""#,
        r#""9223372036854775808""#,
        r#""\u0034\u0034\u0033""#,
        r#""""#,
        r#"" 1""#,
        r#""1.0""#,
    ] {
        compare::<BytesPerSecond, RateReference>(input);
    }
}
