use serde::{Deserialize, Serialize};

use crate::error::ModelError;
use crate::quantity::{Operator, Quantity};

macro_rules! closed_enum {
    ($(#[$m:meta])* $name:ident { $( $var:ident => $tok:literal ),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $( $var ),+ }

        impl $name {
            /// Closed DB token list, mirrored by the SQL CHECK constraint.
            pub const DB_TOKENS: &'static [&'static str] = &[$( $tok ),+];

            pub fn as_db_token(&self) -> &'static str {
                match self { $( Self::$var => $tok ),+ }
            }

            pub fn from_db_token(token: &str) -> Result<Self, ModelError> {
                match token {
                    $( $tok => Ok(Self::$var), )+
                    other => Err(ModelError::UnknownToken {
                        type_name: stringify!($name),
                        token: other.to_string(),
                    }),
                }
            }
        }
    };
}

macro_rules! open_enum {
    ($(#[$m:meta])* $name:ident { $( $var:ident => $tok:literal ),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
        #[serde(rename_all = "snake_case")]
        pub enum $name {
            $( $var, )+
            /// Explicit escape hatch; the note is mandatory so `other` is
            /// never a silent bucket.
            Other(String),
        }

        impl $name {
            /// Closed DB tokens; `Other` is stored as `"other:<note>"`.
            pub const DB_TOKENS: &'static [&'static str] = &[$( $tok ),+];

            pub fn as_db_token(&self) -> String {
                match self {
                    $( Self::$var => $tok.to_string(), )+
                    Self::Other(note) => format!("other:{note}"),
                }
            }

            pub fn from_db_token(token: &str) -> Result<Self, ModelError> {
                $( if token == $tok { return Ok(Self::$var); } )+
                if let Some(note) = token.strip_prefix("other:") {
                    return Ok(Self::Other(note.to_string()));
                }
                Err(ModelError::UnknownToken {
                    type_name: stringify!($name),
                    token: token.to_string(),
                })
            }
        }
    };
}

closed_enum!(
    /// Lifecycle of a recipe definition.
    RecipeStatus {
        Draft => "draft",
        Deployed => "deployed",
        Retired => "retired",
    }
);

closed_enum!(
    /// How a provenance row was produced.
    ExtractionMethod {
        Structured => "structured",
        Parsed => "parsed",
        LlmExtracted => "llm_extracted",
        Manual => "manual",
    }
);

closed_enum!(
    /// Kind of source a provenance row points at.
    ProvenanceKind {
        DatasetImport => "dataset_import",
        Doi => "doi",
        File => "file",
        Manual => "manual",
    }
);

open_enum!(
    /// Abstract identity kind of a material (two-level identity: the physical
    /// lot level is reserved for the inventory pillar).
    MaterialKind {
        Molecule => "molecule",
        Formula => "formula",
        Mixture => "mixture",
    }
);

open_enum!(
    /// Broad synthesis family; extensible by design.
    SynthesisType {
        SolidState => "solid_state",
        SolutionBased => "solution_based",
        Organic => "organic",
    }
);

open_enum!(
    /// Role a material plays in a step. Roles belong to usage, never to
    /// material identity.
    MaterialRole {
        Reactant => "reactant",
        Precursor => "precursor",
        Solvent => "solvent",
        Catalyst => "catalyst",
        Product => "product",
        Byproduct => "byproduct",
        Wash => "wash",
    }
);

open_enum!(
    Atmosphere {
        Air => "air",
        N2 => "n2",
        Ar => "ar",
        H2 => "h2",
        O2 => "o2",
        Vacuum => "vacuum",
    }
);

/// Typed step operation. `other` carries a mandatory note.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Heat,
    Mix,
    Grind,
    Dissolve,
    Precipitate,
    Filter,
    Wash,
    Dry,
    Calcine,
    Mill,
    Sonicate,
    Other { note: String },
}

impl Operation {
    pub const DB_TOKENS: &'static [&'static str] = &[
        "heat",
        "mix",
        "grind",
        "dissolve",
        "precipitate",
        "filter",
        "wash",
        "dry",
        "calcine",
        "mill",
        "sonicate",
    ];

    pub fn as_db_token(&self) -> String {
        match self {
            Operation::Heat => "heat".to_string(),
            Operation::Mix => "mix".to_string(),
            Operation::Grind => "grind".to_string(),
            Operation::Dissolve => "dissolve".to_string(),
            Operation::Precipitate => "precipitate".to_string(),
            Operation::Filter => "filter".to_string(),
            Operation::Wash => "wash".to_string(),
            Operation::Dry => "dry".to_string(),
            Operation::Calcine => "calcine".to_string(),
            Operation::Mill => "mill".to_string(),
            Operation::Sonicate => "sonicate".to_string(),
            Operation::Other { note } => format!("other:{note}"),
        }
    }

    pub fn from_db_token(token: &str) -> Result<Self, ModelError> {
        let op = match token {
            "heat" => Operation::Heat,
            "mix" => Operation::Mix,
            "grind" => Operation::Grind,
            "dissolve" => Operation::Dissolve,
            "precipitate" => Operation::Precipitate,
            "filter" => Operation::Filter,
            "wash" => Operation::Wash,
            "dry" => Operation::Dry,
            "calcine" => Operation::Calcine,
            "mill" => Operation::Mill,
            "sonicate" => Operation::Sonicate,
            other => {
                if let Some(note) = other.strip_prefix("other:") {
                    Operation::Other {
                        note: note.to_string(),
                    }
                } else {
                    return Err(ModelError::UnknownToken {
                        type_name: "Operation",
                        token: other.to_string(),
                    });
                }
            }
        };
        Ok(op)
    }
}

/// pH condition with an explicit comparison operator (Chemotion's
/// `ph_operator`/`ph_value` split, generalized).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PhCondition {
    pub operator: Operator,
    pub value: f64,
}

/// One point of a programmed temperature series.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TemperaturePoint {
    pub time: Quantity,
    pub temperature: Quantity,
}

/// Temperature condition: a single setpoint, a min/max band, or a
/// time/temperature program.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Temperature {
    Scalar(Quantity),
    MinMax { min: Quantity, max: Quantity },
    Series { points: Vec<TemperaturePoint> },
}

/// Typed step conditions. Every field is optional; absence is data, not
/// error. Free-text condition strings are the anti-pattern this replaces.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Conditions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<Temperature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pressure: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atmosphere: Option<Atmosphere>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ph: Option<PhCondition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stirring: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub atmosphere_note: Option<String>,
}

/// Abstract material identity — no stock, no role hints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Material {
    #[serde(default)]
    pub id: i64,
    pub kind: MaterialKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inchikey: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_smiles: Option<String>,
    /// Normalized Hill formula (formula-kind materials).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub formula: Option<String>,
    /// Free-form composition for mixtures.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composition: Option<serde_json::Value>,
    /// Synonyms; the first name is the preferred one.
    pub names: Vec<String>,
    /// CAS number; nullable and deliberately non-unique.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cas: Option<String>,
    /// Dedup key: prefixed identity string (`inchikey:…`, `smiles:…`,
    /// `formula:…`, `composition:…`). Backed by a partial unique index on
    /// `(kind, identity)`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
}

/// One row per source. Confidence is a column, not a comment: mandatory in
/// 0..=1, expected to be 1.0 for structured/manual extraction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Provenance {
    #[serde(default)]
    pub id: i64,
    pub kind: ProvenanceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub doi: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_dataset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// Page/section locator within the source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extractor_version: Option<String>,
    pub extraction_method: ExtractionMethod,
    pub confidence: f64,
}

/// A recipe: the immutable-ish *definition* (see `Run` for executions).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Recipe {
    #[serde(default)]
    pub id: i64,
    pub name: String,
    pub version: i64,
    pub status: RecipeStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_material_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_quantity: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synthesis_type: Option<SynthesisType>,
    /// Human prose; allowed but never load-bearing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub narrative: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_from_recipe_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance_id: Option<i64>,
    /// ISO-8601 timestamp, supplied by the caller.
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<i64>,
    /// Deterministic external key (`<source_dataset>:<source id>`) for
    /// imported records; backed by a partial unique index. This is the
    /// idempotency fence for re-run imports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_key: Option<String>,
}

/// An ordered, typed step. Linear `ordering` is first-class; DAG wire edges
/// (`wire_from_step_id`, `wire_from_output`, `wire_to_input`) are reserved
/// columns in the schema but carry no logic until plans arrive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RecipeStep {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub recipe_id: i64,
    pub ordering: i64,
    pub operation: Operation,
    /// Typed parameter map per operation, validated against the Rust-side
    /// schema registry (not freeform text).
    #[serde(default)]
    pub parameters: serde_json::Value,
    #[serde(default)]
    pub conditions: Conditions,
}

/// The join that matters: a material's role-tagged, quantified attachment to
/// a step. `quantity` is nullable — unquantified steps exist in the
/// literature and absence is data, not error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StepMaterial {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub step_id: i64,
    pub material_id: i64,
    pub role: MaterialRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quantity: Option<Quantity>,
    /// Stoichiometric equivalents relative to the reference material.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub equivalents: Option<f64>,
    #[serde(default)]
    pub is_reference: bool,
    #[serde(default)]
    pub optional: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

/// One execution of a recipe version. Definitions and instances never share
/// a row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Run {
    #[serde(default)]
    pub id: i64,
    pub recipe_id: i64,
    pub recipe_version: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator: Option<String>,
    /// Actual yield as a unit-tagged quantity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yield_quantity: Option<Quantity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversion: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub purity: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
}

/// Mirrors a `RecipeStep` for one run, with actuals instead of targets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct RunStep {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub run_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recipe_step_id: Option<i64>,
    pub ordering: i64,
    pub operation: Operation,
    #[serde(default)]
    pub actual_parameters: serde_json::Value,
    #[serde(default)]
    pub actual_conditions: Conditions,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deviation_notes: Option<String>,
}

/// Append-only history entry. The log is never rewritten.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ChangeLogEntry {
    #[serde(default)]
    pub id: i64,
    pub entity_kind: String,
    pub entity_id: i64,
    pub at: String,
    pub by: String,
    pub patch: serde_json::Value,
}
