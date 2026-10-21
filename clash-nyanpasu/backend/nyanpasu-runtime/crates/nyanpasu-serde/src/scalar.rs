use std::{fmt, marker::PhantomData, str::FromStr};

use serde::{
    Deserializer,
    de::{self, DeserializeSeed, MapAccess, Visitor},
};

/// Decode a number or a decimal string accepted by its `FromStr` implementation.
#[inline(always)]
pub fn number_or_string<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: serde::Deserialize<'de> + FromStr,
    T::Err: fmt::Display,
{
    #[derive(crate::DeserializeUntagged)]
    #[serde(untagged)]
    enum Number<T>
    where
        T: FromStr,
        T::Err: fmt::Display,
    {
        #[nyanpasu_serde(kind = "number")]
        Number(T),
        #[nyanpasu_serde(kind = "string")]
        Text(#[serde(deserialize_with = "crate::from_str")] T),
    }
    use serde::Deserialize;
    Ok(match Number::<T>::deserialize(deserializer)? {
        Number::Number(value) | Number::Text(value) => value,
    })
}

/// Decode a scalar string with `FromStr`, without an intermediate owned String.
#[inline(always)]
pub fn from_str<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    struct Parse<T>(PhantomData<T>);
    impl<'de, T: FromStr> Visitor<'de> for Parse<T>
    where
        T::Err: fmt::Display,
    {
        type Value = T;
        #[inline(always)]
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a string accepted by FromStr")
        }
        #[inline(always)]
        fn visit_str<E: de::Error>(self, value: &str) -> Result<T, E> {
            value.parse().map_err(E::custom)
        }
    }
    deserializer.deserialize_str(Parse(PhantomData))
}

pub trait StringEnum: Sized {
    const VARIANTS: &'static [&'static str];
    fn known(value: &str) -> Option<Self>;
    fn unknown(value: String) -> Self;
}

struct EnumVisitor<T>(PhantomData<T>);

impl<'de, T: StringEnum> Visitor<'de> for EnumVisitor<T> {
    type Value = T;

    #[inline(always)]
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a string or a single unit-variant object")
    }

    #[inline(always)]
    fn visit_str<E: de::Error>(self, value: &str) -> Result<T, E> {
        Ok(T::known(value).unwrap_or_else(|| T::unknown(value.to_owned())))
    }

    #[inline(always)]
    fn visit_string<E: de::Error>(self, value: String) -> Result<T, E> {
        Ok(T::known(&value).unwrap_or_else(|| T::unknown(value)))
    }

    #[inline(always)]
    fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<T, M::Error> {
        struct Key<T>(PhantomData<T>);
        impl<'de, T: StringEnum> DeserializeSeed<'de> for Key<T> {
            type Value = T;
            #[inline(always)]
            fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<T, D::Error> {
                deserializer.deserialize_identifier(self)
            }
        }
        impl<'de, T: StringEnum> Visitor<'de> for Key<T> {
            type Value = T;
            #[inline(always)]
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a known unit variant")
            }
            #[inline(always)]
            fn visit_str<E: de::Error>(self, value: &str) -> Result<T, E> {
                T::known(value).ok_or_else(|| E::unknown_variant(value, T::VARIANTS))
            }
        }
        let value = map
            .next_key_seed(Key::<T>(PhantomData))?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;
        map.next_value::<()>()?;
        if map.next_key::<de::IgnoredAny>()?.is_some() {
            return Err(de::Error::invalid_length(2, &self));
        }
        Ok(value)
    }
}

#[inline(always)]
pub fn deserialize_string_enum<'de, T: StringEnum, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    deserializer.deserialize_any(EnumVisitor::<T>(PhantomData))
}
