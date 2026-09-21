use std::fmt;

use schemars::gen::SchemaGenerator;
use schemars::schema::{InstanceType, Schema, SchemaObject};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::ModelError;

/// Closed unit vocabulary. Unknown units are *unrepresentable*: parsing an
/// unrecognized token is an error, so a bare or misspelled unit can never
/// enter the system. `Other` is the explicit escape hatch and is namespaced
/// (`"other:<token>"`) so it can never collide with the closed vocabulary.
#[derive(Debug, Clone, PartialEq)]
pub enum Unit {
    Gram,
    Kilogram,
    Milligram,
    Milliliter,
    Liter,
    Microliter,
    Mol,
    Millimol,
    Celsius,
    Kelvin,
    Atm,
    Bar,
    Pascal,
    Millibar,
    Torr,
    Second,
    Minute,
    Hour,
    Day,
    Other(String),
}

const UNIT_TOKENS: [(&str, Unit); 19] = [
    ("g", Unit::Gram),
    ("kg", Unit::Kilogram),
    ("mg", Unit::Milligram),
    ("mL", Unit::Milliliter),
    ("L", Unit::Liter),
    ("µL", Unit::Microliter),
    ("mol", Unit::Mol),
    ("mmol", Unit::Millimol),
    ("°C", Unit::Celsius),
    ("K", Unit::Kelvin),
    ("atm", Unit::Atm),
    ("bar", Unit::Bar),
    ("Pa", Unit::Pascal),
    ("mbar", Unit::Millibar),
    ("Torr", Unit::Torr),
    ("s", Unit::Second),
    ("min", Unit::Minute),
    ("h", Unit::Hour),
    ("d", Unit::Day),
];

impl Unit {
    /// Canonical closed tokens (the `Other` escape hatch is intentionally
    /// absent — it is matched by prefix instead).
    pub const CLOSED_TOKENS: &'static [&'static str] = &[
        "g", "kg", "mg", "mL", "L", "µL", "mol", "mmol", "°C", "K", "atm", "bar", "Pa", "mbar",
        "Torr", "s", "min", "h", "d",
    ];

    pub fn as_token(&self) -> String {
        match self {
            Unit::Other(s) => format!("other:{s}"),
            closed => UNIT_TOKENS
                .iter()
                .find(|(_, u)| u == closed)
                .map(|(t, _)| (*t).to_string())
                .expect("every closed unit has a token"),
        }
    }

    pub fn from_token(token: &str) -> Result<Self, ModelError> {
        if let Some((_, u)) = UNIT_TOKENS.iter().find(|(t, _)| *t == token) {
            return Ok(u.clone());
        }
        if let Some(rest) = token.strip_prefix("other:") {
            return Ok(Unit::Other(rest.to_string()));
        }
        Err(ModelError::UnknownToken {
            type_name: "Unit",
            token: token.to_string(),
        })
    }
}

impl Serialize for Unit {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.as_token())
    }
}

impl<'de> Deserialize<'de> for Unit {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Unit::from_token(&s).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for Unit {
    fn schema_name() -> String {
        "Unit".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            enum_values: Some(
                Unit::CLOSED_TOKENS
                    .iter()
                    .map(|t| serde_json::Value::String((*t).to_string()))
                    .collect(),
            ),
            metadata: Some(Box::new(schemars::schema::Metadata {
                description: Some(
                    "Closed unit token; the escape hatch is the string \"other:<unit>\"."
                        .to_string(),
                ),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

/// Comparison operator on a measured value. `Range` carries its own bounds;
/// `Quantity::value` then holds the nominal value and must lie within
/// `[min, max]`.
#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    Eq,
    Approx,
    Lt,
    Gt,
    Lte,
    Gte,
    Range { min: f64, max: f64 },
}

const OPERATOR_TOKENS: [&str; 6] = ["eq", "approx", "lt", "gt", "lte", "gte"];

impl Operator {
    pub const CLOSED_TOKENS: &'static [&'static str] = &OPERATOR_TOKENS;

    /// Token used in the DB `operator` column (`Range` collapses to `"range"`;
    /// the bounds live in their own columns).
    pub fn db_token(&self) -> &'static str {
        match self {
            Operator::Eq => "eq",
            Operator::Approx => "approx",
            Operator::Lt => "lt",
            Operator::Gt => "gt",
            Operator::Lte => "lte",
            Operator::Gte => "gte",
            Operator::Range { .. } => "range",
        }
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        if let Operator::Range { min, max } = self {
            if min > max {
                return Err(ModelError::InvertedRange {
                    min: *min,
                    max: *max,
                });
            }
        }
        Ok(())
    }
}

impl Serialize for Operator {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Operator::Range { min, max } => {
                serde_json::json!({ "range": { "min": min, "max": max } }).serialize(s)
            }
            simple => s.serialize_str(simple.db_token()),
        }
    }
}

impl<'de> Deserialize<'de> for Operator {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match &v {
            serde_json::Value::String(s) if OPERATOR_TOKENS.contains(&s.as_str()) => {
                Ok(match s.as_str() {
                    "eq" => Operator::Eq,
                    "approx" => Operator::Approx,
                    "lt" => Operator::Lt,
                    "gt" => Operator::Gt,
                    "lte" => Operator::Lte,
                    _ => Operator::Gte,
                })
            }
            serde_json::Value::Object(_) => {
                #[derive(Deserialize)]
                struct R {
                    range: Bounds,
                }
                #[derive(Deserialize)]
                struct Bounds {
                    min: f64,
                    max: f64,
                }
                let r: R = serde_json::from_value(v).map_err(serde::de::Error::custom)?;
                let op = Operator::Range {
                    min: r.range.min,
                    max: r.range.max,
                };
                op.validate().map_err(serde::de::Error::custom)?;
                Ok(op)
            }
            _ => Err(serde::de::Error::custom(format!("unknown operator: {v}"))),
        }
    }
}

impl schemars::JsonSchema for Operator {
    fn schema_name() -> String {
        "Operator".to_string()
    }

    fn json_schema(_gen: &mut SchemaGenerator) -> Schema {
        let token_schema = SchemaObject {
            instance_type: Some(InstanceType::String.into()),
            enum_values: Some(
                OPERATOR_TOKENS
                    .iter()
                    .map(|t| serde_json::Value::String((*t).to_string()))
                    .collect(),
            ),
            ..Default::default()
        };
        let range_schema = serde_json::from_value(serde_json::json!({
            "type": "object",
            "required": ["range"],
            "properties": {
                "range": {
                    "type": "object",
                    "required": ["min", "max"],
                    "properties": {"min": {"type": "number"}, "max": {"type": "number"}},
                    "additionalProperties": false
                }
            },
            "additionalProperties": false
        }))
        .expect("static schema is valid");
        SchemaObject {
            subschemas: Some(Box::new(schemars::schema::SubschemaValidation {
                any_of: Some(vec![token_schema.into(), range_schema]),
                ..Default::default()
            })),
            metadata: Some(Box::new(schemars::schema::Metadata {
                description: Some(
                    "One of \"eq\" | \"approx\" | \"lt\" | \"gt\" | \"lte\" | \"gte\" or \
                     {\"range\": {\"min\": number, \"max\": number}}."
                        .to_string(),
                ),
                ..Default::default()
            })),
            ..Default::default()
        }
        .into()
    }
}

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.db_token())
    }
}

/// A unit-tagged measured value. No bare floats anywhere in the model.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Quantity {
    pub value: f64,
    pub unit: Unit,
    #[serde(default = "default_operator")]
    pub operator: Operator,
}

fn default_operator() -> Operator {
    Operator::Eq
}

impl Quantity {
    pub fn new(value: f64, unit: Unit, operator: Operator) -> Result<Self, ModelError> {
        operator.validate()?;
        Ok(Quantity {
            value,
            unit,
            operator,
        })
    }

    pub fn exact(value: f64, unit: Unit) -> Self {
        Quantity {
            value,
            unit,
            operator: Operator::Eq,
        }
    }
}
