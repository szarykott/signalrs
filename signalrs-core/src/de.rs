use crate::protocol::Object;
use serde::{
    de::{
        self, value::StrDeserializer, DeserializeSeed, EnumAccess, Error, IntoDeserializer,
        MapAccess, SeqAccess, Unexpected, VariantAccess, Visitor,
    },
    forward_to_deserialize_any,
};
use signalrs_error::SignalRError;
use std::{
    collections::hash_map::{Keys, Values},
    convert::TryInto,
    iter::{Enumerate, Peekable},
    slice::Iter,
};

impl<'de> de::Deserializer<'de> for &'de Object {
    type Error = SignalRError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self {
            Object::Nil => visitor.visit_none(),
            Object::String(v) => visitor.visit_str(v.as_str()),
            Object::Int(v) => visitor.visit_i64(*v),
            Object::Float(v) => visitor.visit_f64(*v),
            Object::Bool(v) => visitor.visit_bool(*v),
            Object::Array(a) => visitor.visit_seq(SeqAccessor(a.iter().enumerate())),
            Object::Obj(m) => visitor.visit_map(MapAccessor(m.keys().peekable(), m.values())),
        }
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_bool(TryInto::try_into(self)?)
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(TryInto::try_into(self)?)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(TryInto::try_into(self)?)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(TryInto::try_into(self)?)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(TryInto::try_into(self)?)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i8(TryInto::try_into(self)?)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i16(TryInto::try_into(self)?)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i32(TryInto::try_into(self)?)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_i64(TryInto::try_into(self)?)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f32(TryInto::try_into(self)?)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_f64(TryInto::try_into(self)?)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        let self_string: String = TryInto::try_into(self)?;
        let characters: Vec<char> = self_string.chars().collect();
        if characters.len() == 1 {
            visitor.visit_char(characters[0])
        } else {
            Err(Error::invalid_length(
                characters.len(),
                &"string of length 1",
            ))
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_borrowed_str(TryInto::try_into(self)?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(TryInto::try_into(self)?)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self {
            ConfigurationTree::Value(Some(_)) => visitor.visit_some(self),
            ConfigurationTree::Value(None) => visitor.visit_none(),
            cr => Err(Error::invalid_type(
                Unexpected::Other(&cr.node_type().to_string()),
                &"value",
            )),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self {
            ConfigurationTree::Value(Some(Value::String(s))) => {
                if s.trim().is_empty() {
                    visitor.visit_unit()
                } else {
                    Err(Error::custom(
                        "value should be null or empty to deserialize unit",
                    ))
                }
            }
            ConfigurationTree::Value(None) => visitor.visit_unit(),
            ConfigurationTree::Value(_) => Err(Error::invalid_type(
                Unexpected::Other("non empty value"),
                &"null or empty value",
            )),
            cr => Err(Error::invalid_type(
                Unexpected::Other(&cr.node_type().to_string()),
                &"null or empty value",
            )),
        }
    }

    fn deserialize_unit_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(EnumAccessor { root: self })
    }

    forward_to_deserialize_any!(bytes byte_buf seq map tuple tuple_struct struct identifier ignored_any);
}

struct MapAccessor<'conf>(
    Peekable<Keys<'conf, String, ConfigurationTree>>,
    Values<'conf, String, ConfigurationTree>,
);

impl<'de> MapAccess<'de> for MapAccessor<'de> {
    type Error = ConfigurationError;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        match self.0.peek() {
            Some(v) => {
                let deserializer: StrDeserializer<ConfigurationError> =
                    v.as_str().into_deserializer();
                let key = seed
                    .deserialize(deserializer)
                    .map_err(|e| e.enrich_with_key(Key::Map((*v).to_owned())))?;
                Ok(Some(key))
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        let key = self.0.next();
        if key.is_none() {
            return Err(Error::custom(
                "missing key corresponding to value being deserialized in a map",
            ));
        }

        let key = key.unwrap();

        match self.1.next() {
            Some(v) => Ok(seed
                .deserialize(v)
                .map_err(|e| e.enrich_with_key(Key::Map(key.to_owned())))?),
            None => Err(Error::custom(format!(
                "missing value corresponding to a key {} in a map",
                key
            ))),
        }
    }
}

struct SeqAccessor<'conf>(Enumerate<Iter<'conf, ConfigurationTree>>);

impl<'de> SeqAccess<'de> for SeqAccessor<'de> {
    type Error = ConfigurationError;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.0.next() {
            Some((index, v)) => Ok(Some(
                seed.deserialize(v)
                    .map_err(|e| e.enrich_with_key(Key::Array(index)))?,
            )),
            None => Ok(None),
        }
    }
}

struct EnumAccessor<'conf> {
    root: &'conf ConfigurationTree,
}

impl<'de> EnumAccess<'de> for EnumAccessor<'de> {
    type Error = ConfigurationError;
    type Variant = Self;

    fn variant_seed<V>(mut self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        match self.root {
            ConfigurationTree::Value(Some(Value::String(v))) => {
                let deserializer: StrDeserializer<ConfigurationError> =
                    v.as_str().into_deserializer();
                let value = seed.deserialize(deserializer)?;

                Ok((value, self))
            }
            ConfigurationTree::Value(_) => Err(Error::custom(
                "expected string or single key map, got other value type",
            )),
            ConfigurationTree::Map(m) => {
                if m.len() != 1 {
                    return Err(Error::invalid_length(m.len(), &"expected map of length 1"));
                }

                let key = m.keys().next().unwrap().as_str();
                let deserializer: StrDeserializer<ConfigurationError> = key.into_deserializer();
                let value = seed.deserialize(deserializer)?;

                self.root = m.get(key).unwrap(); // safe due to previous check;

                Ok((value, self))
            }
            ConfigurationTree::Array(_) => Err(Error::invalid_type(
                Unexpected::Seq,
                &"expected string or single key map",
            )),
        }
    }
}

impl<'de> VariantAccess<'de> for EnumAccessor<'de> {
    type Error = ConfigurationError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(()) // TODO: Is it correct?
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.root {
            ConfigurationTree::Value(Some(tv)) => seed.deserialize(tv),
            cr => Err(Error::custom(format!(
                "expected value, got {}",
                cr.node_type(),
            ))),
        }
    }

    fn tuple_variant<V>(self, _len: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.root {
            ConfigurationTree::Array(a) => visitor.visit_seq(SeqAccessor(a.iter().enumerate())),
            cr => Err(Error::custom(format!(
                "expected array, got {}",
                cr.node_type(),
            ))),
        }
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self.root {
            ConfigurationTree::Map(m) => {
                visitor.visit_map(MapAccessor(m.keys().peekable(), m.values()))
            }
            cr => Err(Error::custom(format!(
                "expected map, got {}",
                cr.node_type(),
            ))),
        }
    }
}

impl<'de> de::Deserializer<'de> for &Value {
    type Error = ConfigurationError;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match self {
            Value::String(s) => visitor.visit_str(s.as_str()),
            Value::Bool(b) => visitor.visit_bool(*b),
            Value::Float(f) => visitor.visit_f64(*f),
            Value::SignedInteger(i) => visitor.visit_i64(*i),
        }
    }

    forward_to_deserialize_any!(
        ignored_any identifier enum struct map tuple_struct tuple
        seq newtype_struct unit_struct byte_buf bytes unit option
        string str char f32 f64 i8 i16 i32 i64 u8 u16 u32 u64 bool
    );
}
