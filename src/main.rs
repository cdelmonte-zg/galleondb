use anyhow::bail;
use std::fs::File;

mod database;
mod scanner;
mod shell;
mod parser;

fn main() -> anyhow::Result<()> {
    // Parse arguments
    let args = std::env::args().collect::<Vec<_>>();
    match args.len() {
        0 | 1 => bail!("Missing <database path> and <command>"),
        2 => bail!("Missing <command>"),
        _ => {}
    }

    let mut file = File::open(&args[1])?;

    let command = args[2].trim();

    let _ = match command {
        ".dbinfo" | ".tables" => shell::run(&command, &mut file),
        _ => parser::interpret(&command),
    };

    Ok(())

    // match command.as_str() {
    //     "select" => {
    //         let table_name = match expression.last() {
    //             Some(s) => s,
    //             None => return Ok(()),
    //         };

    //         let rootpage = get_table_rootpage(&mut file, table_name)?.unwrap();

    //         let (page_size, _) = get_db_info(&mut file)?;

    //         let offset = (rootpage - 1) * page_size as u64;
    //         let mut page_header = [0u8; 2];

    //         file.read_exact_at(&mut page_header, offset + 3)?;

    //         let num_rows = u16::from_be_bytes([page_header[0], page_header[1]]);

    //         println!("{num_rows}");
    //     }
    // }

}
