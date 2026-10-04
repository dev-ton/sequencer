mod engine;
mod generators;
mod midi;
mod music;
mod ui;

use anyhow::Result;
use midi::Output;

fn main() -> Result<()> {
    let output = Output::select()?;
    ui::run(output)
}
