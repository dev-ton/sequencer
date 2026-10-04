use std::{io, time::Duration};

use crate::{
    engine::{Command, Sequencer},
    midi::Output,
};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

pub fn run(output: Output) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut sequencer = Sequencer::start(output);
    let mut bpm = 100u16;
    let mut seed = 666u64;
    let mut selected = 0usize;
    let mut muted = [false; 3];
    let mut status = "STOPPED".to_string();
    loop {
        while let Ok(next) = sequencer.rx.try_recv() {
            status = next;
        }
        terminal.draw(|frame| {
            let area = frame.area();
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Min(6),
                    Constraint::Length(3),
                ])
                .split(area);
            frame.render_widget(
                Paragraph::new(format!(
                    "TERMINAL SEQUENCER   BPM {bpm}   {status}   Seed {seed}"
                ))
                .block(Block::default().borders(Borders::ALL)),
                rows[0],
            );
            let tracks = [
                "PULSE      euclid(5,8)       ch 10     ● · ● · ● · · ·",
                "WALK       random-walk       ch 1      D Dorian",
                "SEQUENCE   [D3 F3 G3 C4]     ch 2      4 steps",
            ];
            let body = tracks
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    format!(
                        "{} {:02}  {}{}",
                        if selected == i { ">" } else { " " },
                        i + 1,
                        line,
                        if muted[i] { "  [MUTED]" } else { "" }
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            frame.render_widget(
                Paragraph::new(body).block(Block::default().title("Tracks").borders(Borders::ALL)),
                rows[1],
            );
            frame.render_widget(
                Paragraph::new(
                    "SPACE start/stop   ↑↓ track   M mute   +/- BPM   R reseed   Q quit",
                )
                .block(Block::default().borders(Borders::ALL)),
                rows[2],
            );
        })?;
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Char('Q') => break,
                    KeyCode::Char(' ') => {
                        let _ = sequencer.tx.send(Command::Toggle);
                    }
                    KeyCode::Up => selected = selected.saturating_sub(1),
                    KeyCode::Down => selected = (selected + 1).min(2),
                    KeyCode::Char('m') | KeyCode::Char('M') => {
                        muted[selected] = !muted[selected];
                        let _ = sequencer.tx.send(Command::ToggleMute(selected));
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        bpm = (bpm + 1).min(300);
                        let _ = sequencer.tx.send(Command::SetBpm(bpm));
                    }
                    KeyCode::Char('-') => {
                        bpm = bpm.saturating_sub(1).max(30);
                        let _ = sequencer.tx.send(Command::SetBpm(bpm));
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        seed = rand::random();
                        let _ = sequencer.tx.send(Command::Reseed(seed));
                    }
                    _ => {}
                }
            }
        }
    }
    sequencer.shutdown();
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
