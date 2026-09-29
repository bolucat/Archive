//! `Connection`/`ConnectionMetadata` used to fail specta export entirely: their
//! `extra: IndexMap<String, serde_json::Value>` fields made `serde_json::Value`
//! recursively reference itself in the exported type graph. This asserts the
//! export now succeeds and that the serialize-phase shape carries a named,
//! typed `_extra` field instead of flattening unknown keys inline.

use std::borrow::Cow;

use specta::{
    Format, FormatError, Type, Types,
    datatype::{DataType, Primitive},
};

/// Mirrors the bigint-to-number remapping the app's own exporter applies on
/// top of `specta_serde::PhasesFormat` (tauri-specta's `dangerously_cast_bigints_to_number`).
/// clash-api itself does not depend on this; it only exists so this test can
/// exercise a realistic export instead of tripping specta's BigInt guard.
struct AppLikeFormat(specta_util::Remapper);

impl AppLikeFormat {
    fn new() -> Self {
        let number = <specta_typescript::Number as Type>::definition(&mut Types::default());
        let mut remapper = specta_util::Remapper::new();
        for primitive in [
            Primitive::usize,
            Primitive::isize,
            Primitive::u64,
            Primitive::i64,
            Primitive::u128,
            Primitive::i128,
        ] {
            remapper = remapper.rule(DataType::Primitive(primitive), number.clone());
        }
        Self(remapper)
    }
}

impl Format for AppLikeFormat {
    fn map_types(&'_ self, types: &Types) -> Result<Cow<'_, Types>, FormatError> {
        let types = specta_serde::PhasesFormat.map_types(types)?;
        Ok(Cow::Owned(self.0.remap_types(types.into_owned())))
    }
    fn map_type(&'_ self, types: &Types, dt: &DataType) -> Result<Cow<'_, DataType>, FormatError> {
        let dt = specta_serde::PhasesFormat.map_type(types, dt)?;
        Ok(Cow::Owned(self.0.remap_dt(dt.into_owned())))
    }
}

#[test]
fn connection_exports_with_named_typed_extra() {
    let types = Types::default().register::<clash_api::Connection>();
    let ts = specta_typescript::Typescript::default()
        .export(&types, AppLikeFormat::new())
        .expect("Connection and ConnectionMetadata must export despite their extra maps");

    assert!(
        ts.contains("_extra"),
        "expected the serialize-phase Connection/ConnectionMetadata shape to carry a named `_extra` field:\n{ts}"
    );
    assert!(
        ts.contains("JsonValue"),
        "expected the specta-only JsonValue description type to be exported:\n{ts}"
    );
}
