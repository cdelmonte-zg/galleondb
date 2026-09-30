use anyhow::bail;

use crate::semantic::catalog::Catalog;

pub fn run(command: &str, catalog: &Catalog) -> anyhow::Result<Vec<String>> {
    match command {
        ".dbinfo" => Ok(vec![
            format!("database page size: {}", catalog.page_size()),
            format!("number of tables: {}", catalog.table_names.len()),
        ]),

        ".tables" => {
            let mut names: Vec<_> = catalog
                .table_names
                .keys()
                .filter(|name| !name.starts_with("sqlite_"))
                .cloned()
                .collect();

            names.sort_unstable();

            Ok(vec![names.join(" ")])
        }

        _ => bail!("I don't know you {}!!", command),
    }
}
