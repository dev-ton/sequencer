use std::{io, time::Duration};

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

use crate::{
    engine::{Command, Sequencer, SequencerSnapshot, TrackControl},
    generators::GeneratorControl,
    midi::Output,
};

pub fn run(output: Output) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let mut sequencer = Sequencer::start(output);
    let mut snapshot = SequencerSnapshot::default();
    let mut selected_track = 0usize;
    let mut selected_control = 0usize;

    loop {
        while let Ok(next) = sequencer.rx.try_recv() {
            snapshot = next;
        }
        if !snapshot.tracks.is_empty() {
            selected_track = selected_track.min(snapshot.tracks.len() - 1);
            if let Some(track) = snapshot.tracks.get(selected_track) {
                selected_control = selected_control.min(track.controls.len().saturating_sub(1));
            }
        }

        terminal.draw(|frame| {
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),
                    Constraint::Length(6),
                    Constraint::Min(6),
                    Constraint::Length(4),
                ])
                .split(frame.area());
            let transport = if snapshot.running { "RUNNING" } else { "STOPPED" };
            frame.render_widget(
                Paragraph::new(format!(
                    "TERMINAL SEQUENCER   BPM {}   {transport}   Seed {}",
                    snapshot.bpm, snapshot.seed
                ))
                .block(Block::default().borders(Borders::ALL)),
                rows[0],
            );

            let track_lines = snapshot
                .tracks
                .iter()
                .enumerate()
                .map(|(index, track)| {
                    let marker = if index == selected_track { ">" } else { " " };
                    let state = if !track.enabled {
                        "OFF"
                    } else if track.muted {
                        "MUTED"
                    } else {
                        "     "
                    };
                    format!(
                        "{marker} {:02} {:<9} ch {:02}  step {:02}  {:<5}  {}",
                        index + 1,
                        track.name,
                        track.channel + 1,
                        track.step % 100 + 1,
                        state,
                        track.description
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            frame.render_widget(
                Paragraph::new(track_lines)
                    .block(Block::default().title("Tracks").borders(Borders::ALL)),
                rows[1],
            );

            if let Some(track) = snapshot.tracks.get(selected_track) {
                let controls = &track.controls;
                let control_lines = controls
                    .chunks(2)
                    .enumerate()
                    .map(|(row, pair)| {
                        let spans = pair.iter().enumerate().flat_map(|(column, control)| {
                            let index = row * 2 + column;
                            let selected = index == selected_control;
                            let style = if selected {
                                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default()
                            };
                            let value = format!(
                                "{} {}: {}",
                                if selected { ">" } else { " " },
                                control.label,
                                control.value
                            );
                            let spacer = if column == 0 { "     " } else { "" };
                            [Span::styled(value, style), Span::raw(spacer)]
                        });
                        Line::from(spans.collect::<Vec<_>>())
                    })
                    .collect::<Vec<_>>();
                frame.render_widget(
                    Paragraph::new(control_lines).block(
                        Block::default()
                            .title(format!("Edit {} · [ ] change · ←→ choose", track.name))
                            .borders(Borders::ALL),
                    ),
                    rows[2],
                );
            } else {
                frame.render_widget(
                    Paragraph::new("Loading tracks…")
                        .block(Block::default().title("Track controls").borders(Borders::ALL)),
                    rows[2],
                );
            }

            frame.render_widget(
                Paragraph::new(
                    "SPACE play/stop · ↑↓ track · ←→ field · [ ] adjust · X sequence rest · M mute\n+/- BPM · R new seeded arrangement · Q quit",
                )
                .block(Block::default().borders(Borders::ALL)),
                rows[3],
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
                    KeyCode::Up => {
                        selected_track = selected_track.saturating_sub(1);
                        selected_control = 0;
                    }
                    KeyCode::Down => {
                        selected_track =
                            (selected_track + 1).min(snapshot.tracks.len().saturating_sub(1));
                        selected_control = 0;
                    }
                    KeyCode::Left => selected_control = selected_control.saturating_sub(1),
                    KeyCode::Right => {
                        let control_count = snapshot
                            .tracks
                            .get(selected_track)
                            .map_or(0, |track| track.controls.len());
                        selected_control =
                            (selected_control + 1).min(control_count.saturating_sub(1));
                    }
                    KeyCode::Char('m') | KeyCode::Char('M') => {
                        let _ = sequencer.tx.send(Command::ToggleMute(selected_track));
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        let _ = sequencer
                            .tx
                            .send(Command::SetBpm(snapshot.bpm.saturating_add(1).min(300)));
                    }
                    KeyCode::Char('-') => {
                        let _ = sequencer
                            .tx
                            .send(Command::SetBpm(snapshot.bpm.saturating_sub(1).max(30)));
                    }
                    KeyCode::Char('[') => {
                        adjust_selected(&sequencer, &snapshot, selected_track, selected_control, -1)
                    }
                    KeyCode::Char(']') => {
                        adjust_selected(&sequencer, &snapshot, selected_track, selected_control, 1)
                    }
                    KeyCode::Char('x') | KeyCode::Char('X') => {
                        if let Some(control) =
                            selected_parameter(&snapshot, selected_track, selected_control).filter(
                                |control| {
                                    matches!(
                                        control,
                                        TrackControl::Generator(GeneratorControl::SequenceNote(_))
                                    )
                                },
                            )
                        {
                            let _ = sequencer.tx.send(Command::ToggleRest {
                                index: selected_track,
                                control,
                            });
                        }
                    }
                    KeyCode::Char('r') | KeyCode::Char('R') => {
                        let _ = sequencer.tx.send(Command::Reseed(rand::random()));
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

fn selected_parameter(
    snapshot: &SequencerSnapshot,
    track_index: usize,
    control_index: usize,
) -> Option<TrackControl> {
    snapshot
        .tracks
        .get(track_index)?
        .controls
        .get(control_index)
        .map(|control| control.key)
}

fn adjust_selected(
    sequencer: &Sequencer,
    snapshot: &SequencerSnapshot,
    track_index: usize,
    control_index: usize,
    direction: i8,
) {
    if let Some(control) = selected_parameter(snapshot, track_index, control_index) {
        let _ = sequencer.tx.send(Command::AdjustTrack {
            index: track_index,
            control,
            direction,
        });
    }
}
