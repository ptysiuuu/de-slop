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

use ratatui::widgets::{Clear, Gauge};

#[derive(Debug, PartialEq, Eq)]
pub enum InteractiveAction {
    Accept,
    AcceptAll,
    Skip,
    SkipRule,
    Quit,
    Edit,
    ToggleHelp,
    ScrollUp,
    ScrollDown,
    None,
}

pub fn process_key(key_code: KeyCode, is_flag_only: bool) -> InteractiveAction {
    match key_code {
        KeyCode::Char('y') if !is_flag_only => InteractiveAction::Accept,
        KeyCode::Char('n') => InteractiveAction::Skip,
        KeyCode::Char('f') if is_flag_only => InteractiveAction::Skip,
        KeyCode::Char('a') if !is_flag_only => InteractiveAction::AcceptAll,
        KeyCode::Char('s') => InteractiveAction::SkipRule,
        KeyCode::Char('e') if !is_flag_only => InteractiveAction::Edit,
        KeyCode::Char('q') | KeyCode::Esc => InteractiveAction::Quit,
        KeyCode::Char('?') => InteractiveAction::ToggleHelp,
        KeyCode::Up | KeyCode::Char('k') => InteractiveAction::ScrollUp,
        KeyCode::Down | KeyCode::Char('j') => InteractiveAction::ScrollDown,
        _ => InteractiveAction::None,
    }
}

pub fn run_interactive(file_path: &str, content: &str, matches: Vec<Match>) -> Result<Vec<Match>> {
    if matches.is_empty() {
        return Ok(Vec::new());
    }

    // In non-TTY mode (e.g., pipes or tests), raw mode is unavailable — skip interactive.
    if enable_raw_mode().is_err() {
        return Ok(Vec::new());
    }
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
        let mut scroll_offset: i16 = 0;
        let mut show_help = false;

        while !done_with_match {
            terminal.draw(|f| {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(3), // Header & Progress
                        Constraint::Min(10),   // Main Content
                        Constraint::Length(5), // Diff / Info
                        Constraint::Length(3), // Footer
                    ])
                    .split(f.area());

                // Progress Bar
                let ratio = (i as f64) / (matches.len() as f64);
                let progress = Gauge::default()
                    .block(Block::default().borders(Borders::ALL).title(format!(" Match {} of {} in {} ", i + 1, matches.len(), file_path)))
                    .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
                    .ratio(ratio.clamp(0.0, 1.0));
                f.render_widget(progress, chunks[0]);

                // Context
                let lines: Vec<&str> = content.lines().collect();
                let center_line = (m.line as i16 + scroll_offset).max(1) as usize;
                let start_line = center_line.saturating_sub(5).max(1);
                let end_line = (center_line + 5).min(lines.len().max(1));

                let mut context_lines = Vec::new();
                for l in start_line..=end_line {
                    let text = lines.get(l.saturating_sub(1)).unwrap_or(&"");
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

                let content_widget = Paragraph::new(context_lines)
                    .block(Block::default().borders(Borders::ALL).title(" Context (↑/↓ to scroll) "));
                f.render_widget(content_widget, chunks[1]);

                // Diff / Info Panel
                let mut info_lines = Vec::new();
                info_lines.push(Line::from(format!("Rule: {} [{}]", m.rule_id, m.category)));
                info_lines.push(Line::from(format!("Confidence: {:.2}", m.confidence)));

                if is_flag_only {
                    info_lines.push(Line::from(Span::styled("Action: FLAG ONLY", Style::default().fg(Color::Yellow))));
                } else {
                    match &m.kind {
                        MatchKind::Replace(s) => {
                            info_lines.push(Line::from(Span::styled(format!("- {}", m.original.replace("\n", "\\n")), Style::default().fg(Color::Red))));
                            info_lines.push(Line::from(Span::styled(format!("+ {}", s.replace("\n", "\\n")), Style::default().fg(Color::Green))));
                        }
                        MatchKind::DeleteLine => {
                            info_lines.push(Line::from(Span::styled(format!("- {}", m.original.replace("\n", "\\n")), Style::default().fg(Color::Red))));
                            info_lines.push(Line::from(Span::styled("(entire line will be deleted)", Style::default().fg(Color::Green))));
                        }
                        MatchKind::DeleteBlock => {
                            info_lines.push(Line::from(Span::styled(format!("- {}...", m.original.chars().take(40).collect::<String>().replace("\n", "\\n")), Style::default().fg(Color::Red))));
                            info_lines.push(Line::from(Span::styled("(entire block will be deleted)", Style::default().fg(Color::Green))));
                        }
                        _ => {}
                    }
                }
                let info_widget = Paragraph::new(info_lines)
                    .block(Block::default().borders(Borders::ALL).title(" Info & Diff "));
                f.render_widget(info_widget, chunks[2]);

                // Footer
                let footer_text = if is_flag_only {
                    "(f) flag   (s) skip rule   (↑/↓) scroll   (?) help   (q) quit"
                } else {
                    "(y) apply   (n) skip   (a) apply all   (s) skip rule   (e) edit   (↑/↓) scroll   (?) help   (q) quit"
                };
                let footer = Paragraph::new(footer_text)
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL));
                f.render_widget(footer, chunks[3]);

                // Help Popup
                if show_help {
                    let area = centered_rect(60, 50, f.area());
                    let help_text = vec![
                        Line::from(Span::styled(" Interactive Mode Help ", Style::default().add_modifier(Modifier::BOLD))),
                        Line::from(""),
                        Line::from(" y / n  : Accept or skip the current match"),
                        Line::from(" a      : Accept this and all remaining matches"),
                        Line::from(" f      : Acknowledge flag-only match"),
                        Line::from(" s      : Skip all matches from this rule"),
                        Line::from(" e      : Manually edit the replacement text"),
                        Line::from(" ↑ / ↓  : Scroll context window"),
                        Line::from(" q / Esc: Save applied matches and exit"),
                        Line::from(" ?      : Toggle this help menu"),
                    ];
                    let help_block = Paragraph::new(help_text)
                        .block(Block::default().borders(Borders::ALL).title(" Help ").style(Style::default().bg(Color::Black)))
                        .alignment(Alignment::Left);
                    f.render_widget(Clear, area);
                    f.render_widget(help_block, area);
                }
            })?;

            if let Event::Key(key) = event::read()? {
                let action = process_key(key.code, is_flag_only);
                match action {
                    InteractiveAction::Accept => {
                        accepted.push(m.clone());
                        done_with_match = true;
                    }
                    InteractiveAction::Skip => {
                        done_with_match = true;
                    }
                    InteractiveAction::AcceptAll => {
                        accepted.push(m.clone());
                        for next_m in matches.iter().skip(i + 1) {
                            if !matches!(next_m.kind, MatchKind::FlagOnly) && !skip_rule.contains(&next_m.rule_id) {
                                accepted.push(next_m.clone());
                            }
                        }
                        disable_raw_mode()?;
                        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                        return Ok(accepted);
                    }
                    InteractiveAction::SkipRule => {
                        skip_rule.push(m.rule_id.clone());
                        done_with_match = true;
                    }
                    InteractiveAction::Edit => {
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
                    InteractiveAction::ToggleHelp => {
                        show_help = !show_help;
                    }
                    InteractiveAction::ScrollUp => {
                        scroll_offset = scroll_offset.saturating_sub(1);
                    }
                    InteractiveAction::ScrollDown => {
                        scroll_offset = scroll_offset.saturating_add(1);
                    }
                    InteractiveAction::Quit => {
                        disable_raw_mode()?;
                        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
                        return Ok(accepted);
                    }
                    InteractiveAction::None => {}
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

/// Helper function to create a centered rect using up certain percentage of the available rect `r`
fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_key_accept() {
        assert_eq!(process_key(KeyCode::Char('y'), false), InteractiveAction::Accept);
        assert_eq!(process_key(KeyCode::Char('y'), true), InteractiveAction::None);
    }

    #[test]
    fn test_process_key_skip() {
        assert_eq!(process_key(KeyCode::Char('n'), false), InteractiveAction::Skip);
        assert_eq!(process_key(KeyCode::Char('n'), true), InteractiveAction::Skip);
        
        assert_eq!(process_key(KeyCode::Char('f'), true), InteractiveAction::Skip);
        assert_eq!(process_key(KeyCode::Char('f'), false), InteractiveAction::None);
    }

    #[test]
    fn test_process_key_accept_all() {
        assert_eq!(process_key(KeyCode::Char('a'), false), InteractiveAction::AcceptAll);
        assert_eq!(process_key(KeyCode::Char('a'), true), InteractiveAction::None);
    }

    #[test]
    fn test_process_key_skip_rule() {
        assert_eq!(process_key(KeyCode::Char('s'), false), InteractiveAction::SkipRule);
        assert_eq!(process_key(KeyCode::Char('s'), true), InteractiveAction::SkipRule);
    }

    #[test]
    fn test_process_key_scroll() {
        assert_eq!(process_key(KeyCode::Up, false), InteractiveAction::ScrollUp);
        assert_eq!(process_key(KeyCode::Char('k'), false), InteractiveAction::ScrollUp);
        
        assert_eq!(process_key(KeyCode::Down, false), InteractiveAction::ScrollDown);
        assert_eq!(process_key(KeyCode::Char('j'), false), InteractiveAction::ScrollDown);
    }

    #[test]
    fn test_process_key_help_and_quit() {
        assert_eq!(process_key(KeyCode::Char('?'), false), InteractiveAction::ToggleHelp);
        assert_eq!(process_key(KeyCode::Char('q'), false), InteractiveAction::Quit);
        assert_eq!(process_key(KeyCode::Esc, false), InteractiveAction::Quit);
    }
}
