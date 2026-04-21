// SPLIT: Split from emRec.h — record types extracted
pub use crate::emRecParser::RecError;
use crate::emRecParser::RecStruct;

/// A configuration record that can be serialized to/from an emRec struct.
pub trait Record: Sized {
    fn from_rec(rec: &RecStruct) -> Result<Self, RecError>;
    fn to_rec(&self) -> RecStruct;
    fn SetToDefault(&mut self);
    fn IsSetToDefault(&self) -> bool;
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    #[derive(Debug, Default, PartialEq)]
    pub struct TestConfig {
        pub name: String,
        pub value: i64,
    }

    impl Record for TestConfig {
        fn from_rec(rec: &RecStruct) -> Result<Self, RecError> {
            let name = rec
                .get_str("name")
                .ok_or_else(|| RecError::MissingField("name".into()))?
                .to_string();
            let value =
                rec.get_int("value")
                    .ok_or_else(|| RecError::MissingField("value".into()))? as i64;
            Ok(Self { name, value })
        }

        fn to_rec(&self) -> RecStruct {
            let mut s = RecStruct::new();
            s.set_str("name", &self.name);
            s.set_int("value", self.value as i32);
            s
        }

        fn SetToDefault(&mut self) {
            *self = Self::default();
        }

        fn IsSetToDefault(&self) -> bool {
            *self == Self::default()
        }
    }

    #[test]
    fn rec_round_trip() {
        let original = TestConfig {
            name: "hello".into(),
            value: 42,
        };
        let rec = original.to_rec();
        let restored = TestConfig::from_rec(&rec).unwrap();
        assert_eq!(original, restored);
    }
}
