//! Custom serde deserializer for URL path parameters.
//!
//! Converts `PathParams` (a list of key-value string pairs) into
//! arbitrary Rust types via serde deserialization. Supports:
//!
//! - **Single value**: `Path<u32>` → first param value
//! - **Tuple**: `Path<(u32, String)>` → positional extraction
//! - **Struct**: `Path<UserParams>` → extraction by field name

use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::forward_to_deserialize_any;
use std::fmt;

/// A serde deserializer for path parameters.
pub(crate) struct PathDeserializer<'de> {
    params: &'de [(String, String)],
}

impl<'de> PathDeserializer<'de> {
    pub(crate) fn new(params: &'de [(String, String)]) -> Self {
        Self { params }
    }
}

macro_rules! delegate_single {
    ($($method:ident)*) => {
        $(
            fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
                if self.params.len() == 1 {
                    ValueDeserializer(self.params[0].1.clone()).$method(visitor)
                } else {
                    Err(PathDeserializeError::custom(concat!(
                        "expected single path parameter for ",
                        stringify!($method)
                    )))
                }
            }
        )*
    };
}

impl<'de> Deserializer<'de> for PathDeserializer<'de> {
    type Error = PathDeserializeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        if self.params.len() == 1 {
            visitor.visit_string(self.params[0].1.clone())
        } else {
            self.deserialize_map(visitor)
        }
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_map(PathMapAccess::new(self.params))
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_seq(PathSeqAccess::new(self.params))
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        if self.params.len() == 1 {
            visitor.visit_enum(self.params[0].1.clone().into_enum_deserializer())
        } else {
            Err(PathDeserializeError::custom(
                "enums can only be extracted from a single path parameter",
            ))
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        if self.params.len() == 1 {
            ValueDeserializer(self.params[0].1.clone()).deserialize_option(visitor)
        } else {
            Err(PathDeserializeError::custom(
                "expected single path parameter for option",
            ))
        }
    }

    delegate_single! {
        deserialize_bool deserialize_i8 deserialize_i16 deserialize_i32 deserialize_i64
        deserialize_u8 deserialize_u16 deserialize_u32 deserialize_u64 deserialize_f32
        deserialize_f64 deserialize_char deserialize_str deserialize_string deserialize_bytes
        deserialize_byte_buf
    }

    forward_to_deserialize_any! {
        unit unit_struct identifier ignored_any
    }
}

// ---------------------------------------------------------------------------
// Map access (for structs)
// ---------------------------------------------------------------------------

struct PathMapAccess<'de> {
    params: &'de [(String, String)],
    index: usize,
}

impl<'de> PathMapAccess<'de> {
    fn new(params: &'de [(String, String)]) -> Self {
        Self { params, index: 0 }
    }
}

impl<'de> MapAccess<'de> for PathMapAccess<'de> {
    type Error = PathDeserializeError;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Self::Error> {
        if self.index >= self.params.len() {
            return Ok(None);
        }
        let key = &self.params[self.index].0;
        seed.deserialize(key.as_str().into_deserializer()).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, Self::Error> {
        let value = &self.params[self.index].1;
        self.index += 1;
        seed.deserialize(ValueDeserializer(value.clone()))
    }
}

// ---------------------------------------------------------------------------
// Seq access (for tuples)
// ---------------------------------------------------------------------------

struct PathSeqAccess<'de> {
    params: &'de [(String, String)],
    index: usize,
}

impl<'de> PathSeqAccess<'de> {
    fn new(params: &'de [(String, String)]) -> Self {
        Self { params, index: 0 }
    }
}

impl<'de> SeqAccess<'de> for PathSeqAccess<'de> {
    type Error = PathDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, Self::Error> {
        if self.index >= self.params.len() {
            return Ok(None);
        }
        let value = &self.params[self.index].1;
        self.index += 1;
        seed.deserialize(ValueDeserializer(value.clone())).map(Some)
    }
}

// ---------------------------------------------------------------------------
// Value deserializer (parses a single string value into typed values)
// ---------------------------------------------------------------------------

struct ValueDeserializer(String);

impl<'de> Deserializer<'de> for ValueDeserializer {
    type Error = PathDeserializeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_string(self.0)
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        match self.0.as_str() {
            "true" | "1" => visitor.visit_bool(true),
            "false" | "0" => visitor.visit_bool(false),
            _ => Err(PathDeserializeError::custom(format!(
                "invalid boolean: `{}`",
                self.0
            ))),
        }
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let v: i8 = self
            .0
            .parse()
            .map_err(|_| PathDeserializeError::custom(format!("invalid i8: `{}`", self.0)))?;
        visitor.visit_i8(v)
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let v: i16 = self
            .0
            .parse()
            .map_err(|_| PathDeserializeError::custom(format!("invalid i16: `{}`", self.0)))?;
        visitor.visit_i16(v)
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let v: i32 = self
            .0
            .parse()
            .map_err(|_| PathDeserializeError::custom(format!("invalid i32: `{}`", self.0)))?;
        visitor.visit_i32(v)
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let v: i64 = self
            .0
            .parse()
            .map_err(|_| PathDeserializeError::custom(format!("invalid i64: `{}`", self.0)))?;
        visitor.visit_i64(v)
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let v: u8 = self
            .0
            .parse()
            .map_err(|_| PathDeserializeError::custom(format!("invalid u8: `{}`", self.0)))?;
        visitor.visit_u8(v)
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let v: u16 = self
            .0
            .parse()
            .map_err(|_| PathDeserializeError::custom(format!("invalid u16: `{}`", self.0)))?;
        visitor.visit_u16(v)
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let v: u32 = self
            .0
            .parse()
            .map_err(|_| PathDeserializeError::custom(format!("invalid u32: `{}`", self.0)))?;
        visitor.visit_u32(v)
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let v: u64 = self
            .0
            .parse()
            .map_err(|_| PathDeserializeError::custom(format!("invalid u64: `{}`", self.0)))?;
        visitor.visit_u64(v)
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let v: f32 = self
            .0
            .parse()
            .map_err(|_| PathDeserializeError::custom(format!("invalid f32: `{}`", self.0)))?;
        visitor.visit_f32(v)
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let v: f64 = self
            .0
            .parse()
            .map_err(|_| PathDeserializeError::custom(format!("invalid f64: `{}`", self.0)))?;
        visitor.visit_f64(v)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_string(self.0)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_string(self.0)
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let mut chars = self.0.chars();
        match (chars.next(), chars.next()) {
            (Some(c), None) => visitor.visit_char(c),
            _ => Err(PathDeserializeError::custom(format!(
                "invalid char: `{}`",
                self.0
            ))),
        }
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_some(self)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_enum(self.0.into_enum_deserializer())
    }

    forward_to_deserialize_any! {
        bytes byte_buf unit unit_struct seq tuple tuple_struct
        map struct identifier ignored_any
    }
}

// ---------------------------------------------------------------------------
// String-as-enum deserializer
// ---------------------------------------------------------------------------

struct StringEnumDeserializer(String);

impl<'de> de::EnumAccess<'de> for StringEnumDeserializer {
    type Error = PathDeserializeError;
    type Variant = UnitVariant;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Self::Error> {
        let value = seed.deserialize(ValueDeserializer(self.0))?;
        Ok((value, UnitVariant))
    }
}

struct UnitVariant;

impl<'de> de::VariantAccess<'de> for UnitVariant {
    type Error = PathDeserializeError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(
        self,
        _seed: T,
    ) -> Result<T::Value, Self::Error> {
        Err(PathDeserializeError::custom(
            "newtype variants not supported",
        ))
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        _len: usize,
        _visitor: V,
    ) -> Result<V::Value, Self::Error> {
        Err(PathDeserializeError::custom("tuple variants not supported"))
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error> {
        Err(PathDeserializeError::custom(
            "struct variants not supported",
        ))
    }
}

// ---------------------------------------------------------------------------
// IntoDeserializer helpers
// ---------------------------------------------------------------------------

trait IntoEnumDeserializer {
    fn into_enum_deserializer(self) -> StringEnumDeserializer;
}

impl IntoEnumDeserializer for String {
    fn into_enum_deserializer(self) -> StringEnumDeserializer {
        StringEnumDeserializer(self)
    }
}

// Simple string deserializer for map keys
struct StrDeserializer<'a>(&'a str);

impl<'de, 'a> Deserializer<'de> for StrDeserializer<'a> {
    type Error = PathDeserializeError;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_str(self.0)
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64
        char str string bytes byte_buf option unit unit_struct newtype_struct
        seq tuple tuple_struct map struct enum identifier ignored_any
    }
}

trait StrIntoDeserializer<'a> {
    fn into_deserializer(self) -> StrDeserializer<'a>;
}

impl<'a> StrIntoDeserializer<'a> for &'a str {
    fn into_deserializer(self) -> StrDeserializer<'a> {
        StrDeserializer(self)
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error type for path parameter deserialization.
#[derive(Debug)]
pub(crate) struct PathDeserializeError {
    message: String,
}

impl PathDeserializeError {
    pub(crate) fn custom(msg: impl fmt::Display) -> Self {
        Self {
            message: msg.to_string(),
        }
    }

    pub(crate) fn into_message(self) -> String {
        self.message
    }
}

impl fmt::Display for PathDeserializeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PathDeserializeError {}

impl de::Error for PathDeserializeError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self {
            message: msg.to_string(),
        }
    }
}
