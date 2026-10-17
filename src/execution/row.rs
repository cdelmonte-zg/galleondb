use std::fmt::{Display, Formatter};

use crate::execution::ExecutionResult;
use crate::value::Value;

#[derive(Debug)]
pub(crate) struct Row {
    pub(crate) values: Vec<Value>,
}

impl Display for Row {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        for (index, value) in self.values.iter().enumerate() {
            if index > 0 {
                write!(f, "|")?;
            }

            write!(f, "{value}")?;
        }

        Ok(())
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        match self {
            Value::Null => write!(f, "NULL"),
            Value::Blob(blob) => write!(f, "{:?}", blob),
            Value::Integer(integer) => write!(f, "{integer}"),
            Value::Float(float) => write!(f, "{float}"),
            Value::Text(text) => write!(f, "{text}"),
            Value::Boolean(boolean) => write!(f, "{boolean}"),
        }
    }
}

impl Display for ExecutionResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionResult::Query(rows) => {
                for (index, row) in rows.iter().enumerate() {
                    if index > 0 {
                        writeln!(f)?;
                    }

                    write!(f, "{row}")?;
                }
            }

            ExecutionResult::Shell(lines) => {
                for (index, line) in lines.iter().enumerate() {
                    if index > 0 {
                        writeln!(f)?;
                    }

                    write!(f, "{line}")?;
                }
            }
        }

        Ok(())
    }
}
