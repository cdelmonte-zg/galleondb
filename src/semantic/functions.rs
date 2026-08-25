use std::{collections::HashMap, sync::LazyLock};

use anyhow::bail;

use crate::{semantic::LogicalType};


#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FunctionId {
    AbsReal,
    AbsInteger,
    Count,
    Lower,
}


#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FunctionKind {
    Scalar,
    Aggregate,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReturnType {
    Fixed(LogicalType),
    SameAsArgument(usize),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ArgumentType {
    Type(LogicalType),
    Wildcard,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ParameterType {
    Type(LogicalType),
    Any,
    Wildcard,
}

pub struct FunctionDefinition {
    id: FunctionId,
    name: &'static str,
    parameters: &'static[ParameterType],
    kind: FunctionKind,
    return_type: ReturnType
}

impl FunctionDefinition {
    pub(super) fn id(&self) -> FunctionId {
        self.id
    }

    pub(super) fn name(&self) -> &'static str {
        self.name
    }

    pub(super) fn kind(&self) -> FunctionKind {
        self.kind
    }

    pub(super) fn return_type(&self) -> ReturnType {
        self.return_type
    }
 }

static FUNCTION_REGISTRY: LazyLock<HashMap<&'static str, Vec<FunctionDefinition>>> = LazyLock::new(|| {
    let mut registry = HashMap::new();

    registry.insert(
        "abs",
        vec![
            FunctionDefinition {
                id: FunctionId::AbsInteger,
                name: "abs",
                parameters: &[ParameterType::Type(LogicalType::Integer)],
                kind: FunctionKind::Scalar,
                return_type: ReturnType::Fixed(LogicalType::Integer)
            },
            FunctionDefinition {
                id: FunctionId::AbsReal,
                name: "abs",
                parameters: &[ParameterType::Type(LogicalType::Real)],
                kind: FunctionKind::Scalar,
                return_type: ReturnType::Fixed(LogicalType::Real)
            }
        ]
    );

    registry.insert("count", vec![
        FunctionDefinition {
            id: FunctionId::Count,
            name: "count",
            parameters: &[ParameterType::Wildcard],
            kind: FunctionKind::Aggregate,
            return_type: ReturnType::Fixed(LogicalType::Integer)
        },
        FunctionDefinition {
            id: FunctionId::Count,
            name: "count",
            parameters: &[ParameterType::Any],
            kind: FunctionKind::Aggregate,
            return_type: ReturnType::Fixed(LogicalType::Integer)
        }
    ]);

    registry
});


fn matches(parameter: &ParameterType, argument: &ArgumentType) -> bool {
    match (parameter, argument) {
        (ParameterType::Type(expected), ArgumentType::Type(actual)) => expected == actual,
        (ParameterType::Any, ArgumentType::Type(_)) => true,
        (ParameterType::Wildcard, ArgumentType::Wildcard) => true,
        _ => false
    }
}

fn compare_signature(parameters: &[ParameterType], arguments: &[ArgumentType]) -> bool {
    parameters.len() == arguments.len()
    && parameters.iter().zip(arguments).all(|(p, a)| matches(p, a))
}

pub fn resolve(name: &str, argument_types: &[ArgumentType]) -> anyhow::Result<&'static FunctionDefinition>{
    let name = name.to_ascii_lowercase();

    let Some(candidates) = FUNCTION_REGISTRY.get(name.as_str()) else {
        bail!("No function found with {name}.")
    };

    candidates
        .iter()
        .find(
            |candidate| 
            compare_signature(candidate.parameters,argument_types) )
        .ok_or_else(|| anyhow::anyhow!("Non matching signature found for function {name}."))
}

