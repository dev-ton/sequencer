use anyhow::{anyhow, Context, Result};
use midir::{MidiOutput, MidiOutputConnection};
use std::io::{self, Write};

pub struct Output {
    connection: MidiOutputConnection,
}

impl Output {
    pub fn select() -> Result<Self> {
        let midi_out =
            MidiOutput::new("terminal-sequencer").context("create MIDI output client")?;
        let ports = midi_out.ports();
        if ports.is_empty() {
            return Err(anyhow!(
                "No MIDI output ports found. Connect a MIDI device and retry."
            ));
        }
        println!("Available MIDI outputs:");
        for (index, port) in ports.iter().enumerate() {
            let name = midi_out
                .port_name(port)
                .unwrap_or_else(|_| "Unknown port".into());
            println!("  {index}: {name}");
        }
        print!("Select output [0]: ");
        io::stdout().flush().context("flush prompt")?;
        let mut choice = String::new();
        io::stdin()
            .read_line(&mut choice)
            .context("read MIDI port choice")?;
        let index = if choice.trim().is_empty() {
            0
        } else {
            choice
                .trim()
                .parse::<usize>()
                .context("port choice must be a number")?
        };
        let port = ports
            .get(index)
            .ok_or_else(|| anyhow!("MIDI output index {index} is out of range"))?;
        let connection = midi_out
            .connect(port, "terminal-sequencer-output")
            .map_err(|e| anyhow!("connect MIDI output: {e}"))?;
        Ok(Self { connection })
    }

    pub fn note_on(&mut self, channel: u8, note: u8, velocity: u8) -> Result<()> {
        self.connection
            .send(&[0x90 | channel, note, velocity])
            .context("send MIDI note on")
    }

    pub fn note_off(&mut self, channel: u8, note: u8) -> Result<()> {
        self.connection
            .send(&[0x80 | channel, note, 0])
            .context("send MIDI note off")
    }

    pub fn all_notes_off(&mut self) {
        for channel in 0..16 {
            let _ = self.connection.send(&[0xB0 | channel, 123, 0]);
        }
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        self.all_notes_off();
    }
}
