# nyanpasu-serde

Scoped derives for decoding maps and scalar enums without generic buffering.
Map decoders read typed fields and an optional unknown-field map directly.
Known fields never pass through Serde's `flatten`
`Content` buffer. Field definitions and Serde attributes remain the source of
truth; no separate key list or handwritten map visitor is needed.

```rust
use std::collections::BTreeMap;
use nyanpasu_serde::{DeserializeFields, DeserializeMap};

#[derive(DeserializeFields)]
struct Metadata {
    #[serde(rename = "sourcePort", default)]
    source_port: Option<u16>,
}

#[derive(DeserializeMap)]
struct Connection {
    id: String,
    #[serde(flatten)]
    metadata: Metadata,
    #[serde(flatten)]
    #[nyanpasu_serde(extra)]
    extra: BTreeMap<String, serde_json::Value>,
}

let connection: Connection = serde_json::from_str(
    r#"{"id":"one","sourcePort":443,"extension":{"nested":[null,true]}}"#,
).unwrap();
assert_eq!(connection.metadata.source_port, Some(443));
assert!(connection.extra.contains_key("extension"));
```

`DeserializeMap` implements `serde::Deserialize`. `DeserializeFields` supplies a
flattenable group; it can coexist with Serde's regular `Deserialize` derive when
the group also needs independent decoding. A group cannot contain an extra sink,
and a `DeserializeMap` model cannot be used as a flattened group. These rules
prevent an inner extra sink from silently losing unknown fields.

Supported inputs are named structs with ordinary fields, at most one flattened
group, and at most one explicit extra sink. Groups can themselves flatten one
other group. Extra sinks support `BTreeMap<String, V>` and, with the `indexmap`
feature, `IndexMap<String, V, S>`, where `V: Deserialize`. Unknown duplicate keys
replace the value; IndexMap retains their first position. The outer model takes
precedence if a name overlaps its flattened group.

The derives use Serde's attribute parser (`serde_derive_internals`, pinned because
its API is unstable). Supported decoding attributes are container `rename` and
`rename_all`, and field `rename`, `alias`, `default` (including a function path),
`with`, `deserialize_with`, `skip_deserializing`, and `flatten`. Serialization-only
field attributes `serialize_with`, `skip_serializing`, and `skip_serializing_if`
are accepted for coexistence with `Serialize`. Flattened fields cannot have
decoding modifiers. Other Serde attributes and unnamed structs/enums produce
compile errors for the map derives. This is intentionally a limited decoder, not a replacement
for all of Serde derive. Sequence-form struct decoding is unsupported.
Recursive models (including recursion hidden behind a type alias) are outside
this derive's scope; generic-bound inference for recursive models is not provided.
Direct recursive field types and overlapping names/aliases produce compile errors.

Missing ordinary `Option` fields, including type aliases, become `None`. Defaults
apply only to missing fields, not invalid values. Custom decoders without an
explicit default require the field to be present. Duplicate known fields are
rejected, including aliases and a first value of `null`. Borrowed field types
can borrow from the input when the underlying deserializer permits it; `borrow`
attributes for containers such as `Cow` are not supported yet.

All generated methods carry `#[inline(always)]`. There is no intermediate map or
known-key `String` allocation, including transient escaped keys and reader input.
Owned keys from owned input are moved into extras. Field values and retained
unknown data still allocate as their types require; removing the flatten buffer
does not make an owned model zero-copy. The hidden runtime module is exclusively
for matching generated code and is not a stable handwritten adapter API.

Run the behavior tests with `cargo test -p nyanpasu-serde --all-features` and the
attribute/inline checks with `cargo test -p nyanpasu-serde-derive`. Connection-level
regression and allocation probes live in `clash-api/tests` and `clash-api/examples`.

## Scalar untagged enums

`DeserializeUntagged` supports two explicit shapes. For `#[serde(untagged)]`
enums, scalar newtype variants dispatch directly by string, number or boolean
input, and unit variants accept null. Alternatives of the same input kind are
tried in their original declaration order, including numeric range overlaps and
custom decoder failures. Maps, arrays and struct/tuple alternatives are rejected;
they cannot be replayed after a failed attempt without buffering.

```rust
use nyanpasu_serde::DeserializeUntagged;

#[derive(Debug, PartialEq, DeserializeUntagged)]
#[serde(untagged)]
enum Scalar {
    Number(u16),
    #[nyanpasu_serde(kind = "string")]
    Decimal(#[serde(deserialize_with = "nyanpasu_serde::from_str")] u16),
    Text(String),
    Null,
}

assert_eq!(serde_json::from_str::<Scalar>("443").unwrap(), Scalar::Number(443));
assert_eq!(serde_json::from_str::<Scalar>(r#""443""#).unwrap(), Scalar::Decimal(443));
assert_eq!(serde_json::from_str::<Scalar>("null").unwrap(), Scalar::Null);
```

Built-in integer/float types, bool and String (unqualified or through their
standard paths) supply their input kind automatically. Aliases, generic fields,
borrowed fields and custom decoders require a variant annotation:
`#[nyanpasu_serde(kind = "string")]`, `"number"`, or `"bool"`. This annotation is
an explicit restriction on the accepted input family; it must cover the intended
behavior of the field's decoder. The macro does not guess what a custom
`Deserialize` implementation accepts. Supported field adapters are Serde `with`,
`deserialize_with` and `serialize_with`; container `rename`/`rename_all` and
variant `skip_deserializing`/`skip_serializing` are also accepted. Other attributes
and misplaced helper markers are compile errors. Borrowed strings are possible
only when the underlying deserializer provides a borrowed string; escaped or
owned strings do not become borrowed references.

The second shape is an externally tagged enum with unit variants followed by
one `#[serde(untagged)]` String fallback. Known strings use generated matches
without speculative errors; unknown strings are allocated only when needed, and
owned unknown strings are moved. Serde-compatible single-entry objects for known
unit variants remain accepted, including aliases, while unknown object tags,
non-unit values and additional entries remain errors.

```rust
use nyanpasu_serde::DeserializeUntagged;

#[derive(Debug, PartialEq, DeserializeUntagged)]
#[serde(rename_all = "lowercase")]
enum Kind {
    #[serde(alias = "web")]
    Http,
    #[serde(untagged)]
    Unknown(String),
}

assert_eq!(serde_json::from_str::<Kind>(r#""web""#).unwrap(), Kind::Http);
assert_eq!(serde_json::from_str::<Kind>(r#"{"http":null}"#).unwrap(), Kind::Http);
assert_eq!(serde_json::from_str::<Kind>(r#""future""#).unwrap(), Kind::Unknown("future".into()));
```

Open string enums support Serde container `rename`/`rename_all` and variant
`rename`, `alias`, `skip_deserializing` and `skip_serializing`. The fallback must
be a plain String without decoding adapters. General tagged enums and arbitrary
untagged payloads are outside this derive's scope.

`from_str` parses a scalar string without a temporary owned String.
`number_or_string` combines numeric deserialization and `FromStr` string parsing,
for use with `#[serde(deserialize_with = "nyanpasu_serde::number_or_string")]`.
The helpers do not trim strings, change range checks or turn invalid/null inputs
into defaults. Their errors, and the derive's errors, need not have the same
messages as Serde's buffered implementation. All generated methods remain
`#[inline(always)]`; matching generated code uses only public Serde runtime APIs.
