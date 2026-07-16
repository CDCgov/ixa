use std::fmt::{self, Display};

use serde::de::{self, DeserializeOwned, IntoDeserializer, Visitor};
use serde::ser::{self, Impossible};
use serde::Serialize;

#[derive(Debug)]
pub(crate) struct ScalarCodecError {
    message: String,
}

impl ScalarCodecError {
    fn custom(message: impl Display) -> Self {
        Self {
            message: message.to_string(),
        }
    }

    fn unsupported(kind: &str) -> Self {
        Self {
            message: format!("{kind} values are not supported in population CSV cells"),
        }
    }
}

impl Display for ScalarCodecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ScalarCodecError {}

impl ser::Error for ScalarCodecError {
    fn custom<T: Display>(message: T) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

impl de::Error for ScalarCodecError {
    fn custom<T: Display>(message: T) -> Self {
        Self {
            message: message.to_string(),
        }
    }
}

pub(crate) fn to_cell<T: Serialize>(value: &T) -> Result<Option<String>, ScalarCodecError> {
    value.serialize(ScalarSerializer)
}

pub(crate) fn from_cell<T: DeserializeOwned>(cell: &str) -> Result<T, ScalarCodecError> {
    T::deserialize(ScalarDeserializer { cell })
}

struct ScalarSerializer;

impl ser::Serializer for ScalarSerializer {
    type Ok = Option<String>;
    type Error = ScalarCodecError;
    type SerializeSeq = Impossible<Self::Ok, Self::Error>;
    type SerializeTuple = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeTupleVariant = Impossible<Self::Ok, Self::Error>;
    type SerializeMap = Impossible<Self::Ok, Self::Error>;
    type SerializeStruct = Impossible<Self::Ok, Self::Error>;
    type SerializeStructVariant = Impossible<Self::Ok, Self::Error>;

    fn serialize_bool(self, value: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_i8(self, value: i8) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_i16(self, value: i16) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_i32(self, value: i32) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_i64(self, value: i64) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_i128(self, value: i128) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_u8(self, value: u8) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_u16(self, value: u16) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_u32(self, value: u32) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_u64(self, value: u64) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_u128(self, value: u128) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_f32(self, value: f32) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_f64(self, value: f64) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_char(self, value: char) -> Result<Self::Ok, Self::Error> {
        Ok(Some(value.to_string()))
    }

    fn serialize_str(self, value: &str) -> Result<Self::Ok, Self::Error> {
        if value.is_empty() {
            return Err(ScalarCodecError::custom(
                "empty string values are reserved for optional None values",
            ));
        }
        Ok(Some(value.to_owned()))
    }

    fn serialize_bytes(self, _value: &[u8]) -> Result<Self::Ok, Self::Error> {
        Err(ScalarCodecError::unsupported("byte sequence"))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(None)
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Err(ScalarCodecError::unsupported("unit"))
    }

    fn serialize_unit_struct(self, _name: &'static str) -> Result<Self::Ok, Self::Error> {
        Err(ScalarCodecError::unsupported("unit struct"))
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        self.serialize_str(variant)
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _value: &T,
    ) -> Result<Self::Ok, Self::Error> {
        Err(ScalarCodecError::unsupported("data-carrying enum"))
    }

    fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Err(ScalarCodecError::unsupported("sequence"))
    }

    fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        Err(ScalarCodecError::unsupported("tuple"))
    }

    fn serialize_tuple_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        Err(ScalarCodecError::unsupported("tuple struct"))
    }

    fn serialize_tuple_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(ScalarCodecError::unsupported("data-carrying enum"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(ScalarCodecError::unsupported("map"))
    }

    fn serialize_struct(
        self,
        _name: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Err(ScalarCodecError::unsupported("struct"))
    }

    fn serialize_struct_variant(
        self,
        _name: &'static str,
        _variant_index: u32,
        _variant: &'static str,
        _len: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(ScalarCodecError::unsupported("data-carrying enum"))
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}

#[derive(Clone, Copy)]
struct ScalarDeserializer<'de> {
    cell: &'de str,
}

impl<'de> ScalarDeserializer<'de> {
    fn parse<T>(self, type_name: &str) -> Result<T, ScalarCodecError>
    where
        T: std::str::FromStr,
        T::Err: Display,
    {
        self.cell
            .parse()
            .map_err(|error| ScalarCodecError::custom(format_args!("invalid {type_name}: {error}")))
    }
}

macro_rules! deserialize_number {
    ($method:ident, $visit:ident, $type:ty, $name:literal) => {
        fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
            visitor.$visit(self.parse::<$type>($name)?)
        }
    };
}

impl<'de> de::Deserializer<'de> for ScalarDeserializer<'de> {
    type Error = ScalarCodecError;

    fn deserialize_any<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
        Err(ScalarCodecError::unsupported("self-describing or untagged"))
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        visitor.visit_bool(self.parse::<bool>("boolean")?)
    }

    deserialize_number!(deserialize_i8, visit_i8, i8, "i8");
    deserialize_number!(deserialize_i16, visit_i16, i16, "i16");
    deserialize_number!(deserialize_i32, visit_i32, i32, "i32");
    deserialize_number!(deserialize_i64, visit_i64, i64, "i64");
    deserialize_number!(deserialize_i128, visit_i128, i128, "i128");
    deserialize_number!(deserialize_u8, visit_u8, u8, "u8");
    deserialize_number!(deserialize_u16, visit_u16, u16, "u16");
    deserialize_number!(deserialize_u32, visit_u32, u32, "u32");
    deserialize_number!(deserialize_u64, visit_u64, u64, "u64");
    deserialize_number!(deserialize_u128, visit_u128, u128, "u128");
    deserialize_number!(deserialize_f32, visit_f32, f32, "f32");
    deserialize_number!(deserialize_f64, visit_f64, f64, "f64");

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let mut chars = self.cell.chars();
        let value = chars
            .next()
            .ok_or_else(|| ScalarCodecError::custom("invalid char: value is empty"))?;
        if chars.next().is_some() {
            return Err(ScalarCodecError::custom(
                "invalid char: value contains more than one character",
            ));
        }
        visitor.visit_char(value)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        if self.cell.is_empty() {
            return Err(ScalarCodecError::custom(
                "empty string values are reserved for optional None values",
            ));
        }
        visitor.visit_borrowed_str(self.cell)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        if self.cell.is_empty() {
            return Err(ScalarCodecError::custom(
                "empty string values are reserved for optional None values",
            ));
        }
        visitor.visit_string(self.cell.to_owned())
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
        Err(ScalarCodecError::unsupported("byte sequence"))
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
        Err(ScalarCodecError::unsupported("byte sequence"))
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        if self.cell.is_empty() {
            visitor.visit_none()
        } else {
            visitor.visit_some(self)
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
        Err(ScalarCodecError::unsupported("unit"))
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _visitor: V,
    ) -> Result<V::Value, Self::Error> {
        Err(ScalarCodecError::unsupported("unit struct"))
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
        Err(ScalarCodecError::unsupported("sequence"))
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _len: usize,
        _visitor: V,
    ) -> Result<V::Value, Self::Error> {
        Err(ScalarCodecError::unsupported("tuple"))
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _len: usize,
        _visitor: V,
    ) -> Result<V::Value, Self::Error> {
        Err(ScalarCodecError::unsupported("tuple struct"))
    }

    fn deserialize_map<V: Visitor<'de>>(self, _visitor: V) -> Result<V::Value, Self::Error> {
        Err(ScalarCodecError::unsupported("map"))
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error> {
        Err(ScalarCodecError::unsupported("struct"))
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        if self.cell.is_empty() {
            return Err(ScalarCodecError::custom("enum variant is empty"));
        }
        let deserializer = self.cell.into_deserializer();
        visitor.visit_enum(deserializer)
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(
        self,
        _visitor: V,
    ) -> Result<V::Value, Self::Error> {
        Err(ScalarCodecError::unsupported("ignored"))
    }

    fn is_human_readable(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct Count(u16);

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    enum Status {
        Susceptible,
        Infected,
    }

    #[derive(Serialize)]
    struct Structured {
        first: u8,
        second: u8,
    }

    #[test]
    fn scalar_values_round_trip() {
        assert_eq!(to_cell(&Count(42)).unwrap(), Some("42".to_owned()));
        assert_eq!(from_cell::<Count>("42").unwrap(), Count(42));
        assert_eq!(
            to_cell(&Status::Infected).unwrap(),
            Some("Infected".to_owned())
        );
        assert_eq!(
            from_cell::<Status>("Susceptible").unwrap(),
            Status::Susceptible
        );
        assert_eq!(to_cell(&Some(true)).unwrap(), Some("true".to_owned()));
        assert_eq!(from_cell::<Option<bool>>("true").unwrap(), Some(true));
        assert_eq!(to_cell(&Option::<u8>::None).unwrap(), None);
        assert_eq!(from_cell::<Option<u8>>("").unwrap(), None);
    }

    #[test]
    fn invalid_scalar_values_return_errors() {
        assert!(from_cell::<u8>("not-a-number").is_err());
        assert!(to_cell(&"").is_err());
        assert!(from_cell::<String>("").is_err());
        assert!(to_cell(&Structured {
            first: 1,
            second: 2,
        })
        .is_err());
    }
}
