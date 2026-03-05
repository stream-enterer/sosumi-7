use std::fmt;

/// Errors that can occur when loading configuration records from KDL.
#[derive(Debug)]
pub enum ConfigError {
    MissingField(String),
    InvalidValue { field: String, message: String },
    ParseError(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField(name) => write!(f, "missing field: {name}"),
            Self::InvalidValue { field, message } => {
                write!(f, "invalid value for '{field}': {message}")
            }
            Self::ParseError(msg) => write!(f, "parse error: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// A configuration record that can be serialized to/from a KDL node.
pub trait Record: Sized {
    fn from_kdl(node: &kdl::KdlNode) -> Result<Self, ConfigError>;
    fn to_kdl(&self) -> kdl::KdlNode;
    fn set_to_default(&mut self);
    fn is_default(&self) -> bool;
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
        fn from_kdl(node: &kdl::KdlNode) -> Result<Self, ConfigError> {
            let name = node
                .get("name")
                .and_then(|e| e.as_string())
                .ok_or_else(|| ConfigError::MissingField("name".into()))?
                .to_owned();

            let value = node
                .get("value")
                .and_then(|e| e.as_integer())
                .map(|v| v as i64)
                .ok_or_else(|| ConfigError::MissingField("value".into()))?;

            Ok(Self { name, value })
        }

        fn to_kdl(&self) -> kdl::KdlNode {
            let mut node = kdl::KdlNode::new("test-config");
            node.push(kdl::KdlEntry::new_prop("name", self.name.as_str()));
            node.push(kdl::KdlEntry::new_prop("value", self.value as i128));
            node
        }

        fn set_to_default(&mut self) {
            *self = Self::default();
        }

        fn is_default(&self) -> bool {
            *self == Self::default()
        }
    }

    #[test]
    fn kdl_round_trip() {
        let original = TestConfig {
            name: "hello".into(),
            value: 42,
        };
        let node = original.to_kdl();
        let restored = TestConfig::from_kdl(&node).unwrap();
        assert_eq!(original, restored);
    }
}
