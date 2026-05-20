use crate::rules::{Match, MatchKind};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::io;

pub fn run_interactive(file_path: &str, content: &str, matches: Vec<Match>) -> Result<Vec<Match>> {
    if matches.is_empty() {
        return Ok(Vec::new());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut accepted = Vec::new();
    let mut i = 0;
    let mut skip_rule = Vec::new();

    while i < matches.len() {
        let m = &matches[i];
        if skip_rule.contains(&m.rule_id) {
            i += 1;
            continue;
        }

        let is_flag_only = matches!(m.kind, MatchKind::FlagOnly);

        let mut done_with_match = false;
        while !done_with_match {
            terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(10),
                        Constraint::Length(3),
                    ])
                    .split(f.area());

                let header = Paragraph::new(format!(
                    " Match {} of {} in {} ",
                    i + 1,
                    matches.len(),
                    file_path
                ))
                .block(Block::default().borders(Borders::ALL).title(" Progress "));
                f.render_widget(header, chunks[0]);

                let lines: Vec<&str> = content.lines().collect();
                let start_line = m.line.saturating_sub(3).max(1);
                let end_line = (m.line + 3).min(lines.len());

                let mut context_lines = Vec::new();
                for l in start_line..=end_line {
                    let text = lines.get(l - 1).unwrap_or(&"");
                    if l == m.line {
                        context_lines.push(Line::from(vec![
                            Span::styled(format!("{:4} | ", l), Style::default().fg(Color::DarkGray)),
                            Span::styled(
                                text.to_string(),
                                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                            ),
                        ]));
                    } else {
                        context_lines.push(Line::from(vec![
                            Span::styled(format!("{:4} | ", l), Style::default().fg(Color::DarkGray)),
                            Span::raw(text.to_string()),
                        ]));
                    }
                }

                context_lines.push(Line::from(""));
                context_lines.push(Line::from(format!("Rule: {} [{}]", m.rule_id, m.category)));
                context_lines.push(Line::from(format!("Confidence: {:.2}", m.confidence)));

                if is_flag_only {
                    context_lines.push(Line::from(Span::styled(
                        "Action: FLAG ONLY",
                        Style::default().fg(Color::Yellow),
                    )));
                } else {
                    let action = match &m.kind {
                        MatchKind::Replace(s) => {
                            if s.is_empty() {
                                "Delete match".to_string()
                            } else {
                                format!("Replace with '{}'", s)
                            }
                        }
                        MatchKind::DeleteLine => "Delete entire line".to_string(),
                        MatchKind::DeleteBlock => "Delete entire block".to_string(),
                        _ => String::new(),
                    };
                    context_lines.push(Line::from(Span::styled(
                        format!("Action: {}", action),
                        Style::default().fg(Color::Green),
                    )));
                }

                let content_widget =
                    Paragraph::new(context_lines).block(Block::default().borders(Borders::ALL).title(" Context "));
                f.render_widget(content_widget, chunks[1]);

                let footer_text = if is_flag_only {
                    "(f) flag in report   (s) skip this rule   (esc) quit"
                } else {
                    "(y) apply   (n) skip   (a) apply all   (s) skip this rule   (e) edit   (esc) quit"
                };
                let footer = Paragraph::new(footer_text)
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(footer, chunks[2]);
            })?;

            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('y') if !is_flag_only => {
                        accepted.push(m.clone());
                        done_with_match = true;
                    }
                    KeyCode::Char('n') => {
                        done_with_match = true;
                    }
                    KeyCode::Char('f') if is_flag_only => {
                        done_with_match = true;
                    }
                    KeyCode::Char('a') if !is_flag_only => {
                        accepted.push(m.clone());
                        for j in (i + 1)..matches.len() {
                            let next_m = &matches[j];
                            if !matches!(next_m.kind, MatchKind::FlagOnly) && !skip_rule.contains(&next_m.rule_id) {
                                accepted.push(next_m.clone());
                            }
                        }
                        disable_raw_mode()?;
                        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                        return Ok(accepted);
                    }
                    KeyCode::Char('s') => {
                        skip_rule.push(m.rule_id.clone());
                        done_with_match = true;
                    }
                    KeyCode::Char('e') if !is_flag_only => {
                        disable_raw_mode()?;
                        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

                        println!("\nEnter replacement text (empty to delete span):");
                        let mut input = String::new();
                        std::io::stdin().read_line(&mut input)?;
                        let input = input.trim_end_matches('\n').trim_end_matches('\r').to_string();

                        let mut new_m = m.clone();
                        new_m.kind = MatchKind::Replace(input.clone());
                        new_m.suggestion = input;
                        accepted.push(new_m);

                        enable_raw_mode()?;
                        execute!(terminal.backend_mut(), EnterAlternateScreen)?;
                        terminal.clear()?;

                        done_with_match = true;
                    }
                    KeyCode::Char('q') | KeyCode::Esc => {
                        disable_raw_mode()?;
                        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                        return Ok(accepted);
                    }
                    _ => {}
                }
            }
        }
        i += 1;
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(accepted)
}
