use anyhow::bail;
use codecrafters_sqlite::engine::Engine;
use std::fs::File;

fn main() -> anyhow::Result<()> {
    // Parse arguments
    let args = std::env::args().collect::<Vec<_>>();
    match args.len() {
        0 | 1 => bail!("Missing <database path> and <command>"),
        2 => bail!("Missing <command>"),
        _ => {}
    }

    let file = File::open(&args[1])?;
    let command = args[2].trim();

    let mut engine = Engine::open(file)?;
    let result = engine.execute(command)?;

    println!("{result}");

    Ok(())
}
