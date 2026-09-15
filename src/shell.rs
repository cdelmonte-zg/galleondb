use anyhow::bail;
use std::fmt::Write;

use crate::semantic::catalog::Catalog;

pub fn run(command: &str, catalog: &Catalog) -> anyhow::Result<()> {
    let mut message = String::new();

    match command {
        ".dbinfo" => {
            writeln!(&mut message, "database page size: {}", catalog.page_size())?;
            writeln!(&mut message, "number of tables: {}", catalog.schema_cell_count())?;

            print!("{message}");
            Ok(())
        }
        ".tables" => {
            for name in catalog.tables.keys() {
                if !name.starts_with("sqlite_") {
                    writeln!(&mut message, "{}", name)?;
                }
            }

            print!("{message}");
            Ok(())
        }
        _ => bail!("I don't know you {}!!", command),
    }
}
