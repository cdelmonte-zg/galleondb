use anyhow::bail;
use std::fmt::Write;

use crate::semantic::catalog::Catalog;

pub fn run(command: &str, catalog: &Catalog) -> anyhow::Result<Vec<String>> {
    let mut message = String::new();
    let mut result = vec![];
    match command {
        ".dbinfo" => {
            writeln!(&mut message, "database page size: {}", catalog.page_size())?;
            write!(
                &mut message,
                "number of tables: {}",
                catalog.table_names.len()
            )?;
            result.push(message);
        }
        ".tables" => {
            for name in catalog.table_names.keys() {
                if !name.starts_with("sqlite_") {
                    result.push(format!("{}", name));
                }
            }
        }
        _ => bail!("I don't know you {}!!", command),
    }
    
    Ok(result)
}
