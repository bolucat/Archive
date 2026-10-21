#![doc = include_str!("../README.md")]

extern crate self as nyanpasu_serde;

use std::{fmt, marker::PhantomData};

use serde::{
    Deserialize, Deserializer,
    de::{self, DeserializeSeed, MapAccess, Visitor},
};

pub use nyanpasu_serde_derive::{DeserializeFields, DeserializeMap, DeserializeUntagged};

mod scalar;
pub use scalar::{from_str, number_or_string};

/// Implementation details shared by generated code. Not a handwritten adapter API.
#[doc(hidden)]
pub mod __private {
    pub use crate::scalar::{StringEnum, deserialize_string_enum};
    pub use serde;

    use super::*;

    pub trait MapFields<'de>: Sized {
        type State: Default;

        fn field(name: &str) -> Option<usize>;
        fn read<M: MapAccess<'de>>(
            state: &mut Self::State,
            field: usize,
            map: &mut M,
        ) -> Result<(), M::Error>;
        fn unknown<M: MapAccess<'de>>(
            state: &mut Self::State,
            key: String,
            map: &mut M,
        ) -> Result<(), M::Error>;
        fn finish<E: de::Error>(state: Self::State) -> Result<Self, E>;
    }

    // Only groups without an extra sink can be flattened into another map.
    pub trait KnownFields<'de>: MapFields<'de> {}

    pub trait ExtraFields<'de>: Default {
        fn read<M: MapAccess<'de>>(&mut self, key: String, map: &mut M) -> Result<(), M::Error>;
    }

    #[cfg(feature = "indexmap")]
    impl<'de, V: Deserialize<'de>, S: std::hash::BuildHasher + Default> ExtraFields<'de>
        for indexmap::IndexMap<String, V, S>
    {
        #[inline(always)]
        fn read<M: MapAccess<'de>>(&mut self, key: String, map: &mut M) -> Result<(), M::Error> {
            self.insert(key, map.next_value()?);
            Ok(())
        }
    }

    impl<'de, V: Deserialize<'de>> ExtraFields<'de> for std::collections::BTreeMap<String, V> {
        #[inline(always)]
        fn read<M: MapAccess<'de>>(&mut self, key: String, map: &mut M) -> Result<(), M::Error> {
            self.insert(key, map.next_value()?);
            Ok(())
        }
    }

    enum Key {
        Known(usize),
        Unknown(String),
    }

    struct KeySeed<T>(PhantomData<T>);

    impl<'de, T: MapFields<'de>> DeserializeSeed<'de> for KeySeed<T> {
        type Value = Key;

        #[inline(always)]
        fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Key, D::Error> {
            deserializer.deserialize_identifier(self)
        }
    }

    impl<'de, T: MapFields<'de>> Visitor<'de> for KeySeed<T> {
        type Value = Key;

        #[inline(always)]
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a string field name")
        }

        #[inline(always)]
        fn visit_str<E: de::Error>(self, value: &str) -> Result<Key, E> {
            Ok(match T::field(value) {
                Some(field) => Key::Known(field),
                None => Key::Unknown(value.to_owned()),
            })
        }

        #[inline(always)]
        fn visit_string<E: de::Error>(self, value: String) -> Result<Key, E> {
            Ok(match T::field(&value) {
                Some(field) => Key::Known(field),
                None => Key::Unknown(value),
            })
        }
    }

    struct MapVisitor<T>(PhantomData<T>);

    impl<'de, T: MapFields<'de>> Visitor<'de> for MapVisitor<T> {
        type Value = T;

        #[inline(always)]
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a string-keyed object")
        }

        #[inline(always)]
        fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<T, M::Error> {
            let mut state = T::State::default();
            while let Some(key) = map.next_key_seed(KeySeed::<T>(PhantomData))? {
                match key {
                    Key::Known(field) => T::read(&mut state, field, &mut map)?,
                    Key::Unknown(key) => T::unknown(&mut state, key, &mut map)?,
                }
            }
            T::finish(state)
        }
    }

    #[inline(always)]
    pub fn deserialize_map<'de, T: MapFields<'de>, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<T, D::Error> {
        deserializer.deserialize_map(MapVisitor::<T>(PhantomData))
    }

    // Match Serde's missing-field behavior, including Option aliases, using public APIs.
    struct MissingField<E>(&'static str, PhantomData<E>);

    impl<'de, E: de::Error> Deserializer<'de> for MissingField<E> {
        type Error = E;

        #[inline(always)]
        fn deserialize_any<V: Visitor<'de>>(self, _: V) -> Result<V::Value, E> {
            Err(E::missing_field(self.0))
        }

        #[inline(always)]
        fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, E> {
            visitor.visit_none()
        }

        serde::forward_to_deserialize_any! {
            bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string bytes byte_buf
            unit unit_struct newtype_struct seq tuple tuple_struct map struct enum
            identifier ignored_any
        }
    }

    #[inline(always)]
    pub fn missing<'de, T: Deserialize<'de>, E: de::Error>(name: &'static str) -> Result<T, E> {
        T::deserialize(MissingField(name, PhantomData))
    }
}
