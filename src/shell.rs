use anyhow::bail;
use std::fs::File;
use std::fmt::Write;

use crate::database::{Value, get_cells, get_db_info};

pub fn run(command: &str, file: &mut File) -> anyhow::Result<()> {
    let mut message = String::new();
            
    match command {
        ".dbinfo" => {
            let (page_size, cells_nr) = match get_db_info(file) {
                Ok((page_size, cells_nr)) => (page_size, cells_nr),
                Err(e) => return Err(e),
            };

            writeln!(&mut message, "database page size: {}", page_size)?;
            writeln!(&mut message, "number of tables: {}", cells_nr)?;

                print!("{message}");
            Ok(())
        }
        ".tables" => {
            let (_, _, cells) = get_cells(file)?;

            for cell in &cells {
                if let [
                    Value::Text(object_type),
                    Value::Text(name),
                    Value::Text(_tbl_name),
                    Value::Integer(_rootpage),
                    Value::Text(_sql),
                ] = cell.record.body.as_slice()
                {
                    if object_type == "table" && !name.starts_with("sqlite_") {
                        writeln!(&mut message, "{}", name)?;
                    }
                }
            }

            print!("{message}");
            Ok(())
        }
        _ => bail!("I don't know you {}!!", command),
    }
}
