use chrono::{Datelike, Local, NaiveDate};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Row, Table, Wrap},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::{
    app::{AddField, AddForm, App, CalendarState, DependencyForm, DetailsForm, Mode},
    model::Priority,
};

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(5),
    ])
    .split(frame.area());

    draw_header(frame, app, chunks[0]);
    draw_todos(frame, app, chunks[1]);
    draw_status(frame, app, chunks[2]);
    draw_footer(frame, chunks[3]);

    match &app.mode {
        Mode::Normal => {}
        Mode::Calendar(calendar) => draw_calendar(frame, app, *calendar),
        Mode::Adding(form) => draw_add_form(frame, form),
        Mode::Details(form) => draw_details(frame, app, form),
        Mode::Dependencies(form) => draw_dependencies(frame, app, form),
        Mode::Deleting(id) => draw_delete_confirmation(frame, app, *id),
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let today = Local::now().date_naive();
    let todos = app.todos();
    let completed = todos.iter().filter(|todo| todo.completed).count();
    let active = todos.len() - completed;
    let date_label = if app.selected_date == today {
        "Today"
    } else {
        "Selected date"
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(
                app.selected_date.format("%A, %B %d, %Y").to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  •  {date_label}"),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                format!("{active} active"),
                Style::default().fg(Color::Yellow),
            ),
            Span::styled("  •  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{completed} completed"),
                Style::default().fg(Color::Green),
            ),
        ]),
    ];
    let header = Paragraph::new(lines).block(
        Block::default()
            .title(" TS — Daily Todos ")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(header, area);
}

fn draw_todos(frame: &mut Frame, app: &App, area: Rect) {
    let todos = app.todos();
    let block = Block::default()
        .title(" Todos — priority order ")
        .title_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if todos.is_empty() {
        let empty = Paragraph::new(vec![
            Line::from(""),
            Line::from("No todos for this date."),
            Line::from(""),
            Line::from(Span::styled(
                "Press A to add your first todo.",
                Style::default().fg(Color::Yellow),
            )),
        ])
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray))
        .block(block);
        frame.render_widget(empty, area);
        return;
    }

    const PREFIX_WIDTH: usize = 18;
    let content_width = usize::from(area.width.saturating_sub(2));
    let selected_index = app.selected_index();
    let items: Vec<ListItem> = todos
        .into_iter()
        .enumerate()
        .map(|(index, todo)| {
            let selected = selected_index == Some(index);
            let checkbox = if todo.completed { "[✓] " } else { "[ ] " };
            let checkbox_style = if todo.completed {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Cyan)
            };
            let mut title_style = if todo.completed {
                Style::default()
                    .fg(Color::Gray)
                    .add_modifier(Modifier::CROSSED_OUT)
            } else {
                Style::default().fg(Color::White)
            };
            if selected {
                title_style = title_style.add_modifier(Modifier::BOLD);
            }

            let timestamp = todo
                .created_at
                .with_timezone(&Local)
                .format("%d %b %H:%M")
                .to_string();
            let show_timestamp = content_width >= PREFIX_WIDTH + timestamp.width() + 8;
            let title_width = if show_timestamp {
                content_width - PREFIX_WIDTH - timestamp.width() - 1
            } else {
                content_width.saturating_sub(PREFIX_WIDTH)
            };
            let title = truncate_to_width(&todo.title, title_width);
            let padding = title_width.saturating_sub(title.width());
            let has_prerequisites = !todo.prerequisites.is_empty();
            let prerequisite_style = if app.store.blocked_by(todo.id).is_empty() {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Yellow)
            };
            let mut spans = vec![
                Span::styled(
                    if selected { "›" } else { " " },
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(checkbox, checkbox_style),
                Span::styled(
                    format!(" {:^6} ", todo.priority.label()),
                    priority_style(todo.priority),
                ),
                Span::styled(
                    if has_prerequisites { " ↳ " } else { "    " },
                    prerequisite_style,
                ),
                Span::raw(" "),
                Span::styled(title, title_style),
            ];
            if show_timestamp {
                spans.push(Span::raw(" ".repeat(padding + 1)));
                spans.push(Span::styled(
                    timestamp,
                    Style::default().fg(if selected {
                        Color::Gray
                    } else {
                        Color::DarkGray
                    }),
                ));
            }

            ListItem::new(Line::from(spans)).style(if selected {
                Style::default().bg(Color::DarkGray)
            } else {
                Style::default()
            })
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default());
    let mut state = ListState::default().with_selected(selected_index);
    frame.render_stateful_widget(list, area, &mut state);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let (text, style) = match &app.status {
        Some(status) => (
            status.text.clone(),
            if status.is_error {
                Style::default().fg(Color::Red)
            } else {
                Style::default().fg(Color::Green)
            },
        ),
        None => (
            "Use U/E or ↑/↓ to select a todo.".to_owned(),
            Style::default().fg(Color::DarkGray),
        ),
    };
    frame.render_widget(Paragraph::new(text).style(style), area);
}

fn draw_footer(frame: &mut Frame, area: Rect) {
    let rows = vec![
        Row::new(vec![
            shortcut("A", "Add"),
            shortcut("Space/Enter", "Toggle"),
            shortcut("R", "Edit"),
            shortcut("S", "Details"),
        ]),
        Row::new(vec![
            shortcut("U/E or ↑/↓", "Select"),
            shortcut("N/I or ←/→", "Day"),
            shortcut("C", "Calendar"),
            shortcut("T", "Today"),
        ]),
        Row::new(vec![
            shortcut("1-4", "Priority"),
            shortcut("D", "Prereqs"),
            shortcut("X", "Delete"),
            shortcut("Q", "Quit"),
        ]),
    ];
    let footer = Table::new(rows, [Constraint::Ratio(1, 4); 4])
        .column_spacing(2)
        .style(Style::default().fg(Color::Gray))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(footer, area);
}

fn shortcut(key: &'static str, label: &'static str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{key:<11}"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" {label}")),
    ])
}

fn draw_calendar(frame: &mut Frame, app: &App, calendar: CalendarState) {
    let area = centered_rect(42, 14, frame.area());
    let highlighted = calendar.highlighted;
    let first = NaiveDate::from_ymd_opt(highlighted.year(), highlighted.month(), 1)
        .expect("the highlighted month is valid");
    let next_month = if highlighted.month() == 12 {
        NaiveDate::from_ymd_opt(highlighted.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(highlighted.year(), highlighted.month() + 1, 1)
    }
    .expect("the following month is valid");
    let days_in_month = (next_month - first).num_days();
    let leading_blanks = first.weekday().num_days_from_monday() as i64;
    let today = Local::now().date_naive();

    let mut lines = vec![
        Line::from(highlighted.format("%B %Y").to_string())
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
        Line::from(""),
        Line::from("Mon Tue Wed Thu Fri Sat Sun")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Yellow)),
    ];

    for row in 0..6 {
        let mut cells = Vec::new();
        for column in 0..7 {
            let index = (row * 7 + column) as i64;
            let day = index - leading_blanks + 1;
            if (1..=days_in_month).contains(&day) {
                let date =
                    NaiveDate::from_ymd_opt(highlighted.year(), highlighted.month(), day as u32)
                        .expect("the calendar day is valid");
                let has_todos = app
                    .store
                    .days
                    .get(&date)
                    .is_some_and(|todos| !todos.is_empty());
                let mut style = Style::default().fg(if has_todos {
                    Color::White
                } else {
                    Color::DarkGray
                });
                if date == app.selected_date {
                    style = style.add_modifier(Modifier::BOLD);
                }
                if date == highlighted {
                    style = style
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD);
                }
                if date == today {
                    style = style
                        .fg(Color::Black)
                        .bg(Color::Rgb(255, 165, 0))
                        .add_modifier(Modifier::BOLD);
                }
                cells.push(Span::styled(format!("{day:^4}"), style));
            } else {
                cells.push(Span::raw("    "));
            }
        }
        lines.push(Line::from(cells).alignment(Alignment::Center));
    }

    lines.push(Line::from(""));
    lines.push(
        Line::from("N/I: day  U/E: week  PgUp/PgDn: month")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray)),
    );
    lines.push(
        Line::from("T: today   Enter: select   Esc: cancel")
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray)),
    );

    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(" Select a date ")
            .title_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    frame.render_widget(Clear, area);
    frame.render_widget(popup, area);
}

fn draw_add_form(frame: &mut Frame, form: &AddForm) {
    let area = centered_rect(68, 12, frame.area());
    let window_title = if form.editing_id.is_some() {
        " Edit todo "
    } else {
        " New todo "
    };
    let block = Block::default()
        .title(window_title)
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(2),
        Constraint::Min(0),
    ])
    .split(inner);

    let title_focused = form.field == AddField::Title;
    let mut title_spans = vec![Span::raw(form.title.clone())];
    if title_focused {
        title_spans.push(Span::styled("█", Style::default().fg(Color::Cyan)));
    }
    let title = Paragraph::new(Line::from(title_spans)).block(
        Block::default()
            .title(" Title ")
            .borders(Borders::ALL)
            .border_style(if title_focused {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(Color::DarkGray)
            }),
    );
    frame.render_widget(title, chunks[0]);

    let mut priority_spans = Vec::new();
    for (index, priority) in Priority::ALL.into_iter().enumerate() {
        if index > 0 {
            priority_spans.push(Span::raw("   "));
        }
        let selected = priority == form.priority;
        priority_spans.push(Span::styled(
            format!(" {} ", priority.label()),
            if selected {
                priority_style(priority)
            } else {
                Style::default().fg(Color::DarkGray)
            },
        ));
    }
    let priority = Paragraph::new(Line::from(priority_spans))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title(" Priority ")
                .borders(Borders::ALL)
                .border_style(if form.field == AddField::Priority {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                }),
        );
    frame.render_widget(priority, chunks[1]);

    if let Some(error) = &form.error {
        frame.render_widget(
            Paragraph::new(error.as_str()).style(Style::default().fg(Color::Red)),
            chunks[3],
        );
    }

    let help = Paragraph::new(vec![
        Line::from("Tab: switch field   N/I or ←/→: change priority"),
        Line::from("Enter: save   Esc: cancel"),
    ])
    .alignment(Alignment::Center)
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[4]);
}

fn draw_details(frame: &mut Frame, app: &App, form: &DetailsForm) {
    let Some(todo) = app.todo_by_id(form.todo_id) else {
        return;
    };
    let area = centered_rect(78, 21, frame.area());
    let block = Block::default()
        .title(" Todo details ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(4),
        Constraint::Length(2),
    ])
    .split(inner);

    let title = Paragraph::new(todo.title.clone())
        .style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(title, chunks[0]);

    let created = todo
        .created_at
        .with_timezone(&Local)
        .format("%Y-%m-%d %H:%M:%S %:z")
        .to_string();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Created: ", Style::default().fg(Color::DarkGray)),
            Span::styled(created, Style::default().fg(Color::Gray)),
        ])),
        chunks[1],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Priority: ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!(" {} ", todo.priority.label()),
                priority_style(todo.priority),
            ),
        ])),
        chunks[2],
    );
    let prerequisites = if todo.prerequisites.is_empty() {
        "none".to_owned()
    } else {
        todo.prerequisites
            .iter()
            .map(|id| match app.store.todo(*id) {
                Some(prerequisite) => format!(
                    "#{} {}{}",
                    id,
                    prerequisite.title,
                    if prerequisite.completed { " ✓" } else { "" }
                ),
                None => format!("#{id} (missing)"),
            })
            .collect::<Vec<_>>()
            .join(", ")
    };
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Prerequisites: ", Style::default().fg(Color::DarkGray)),
            Span::styled(prerequisites, Style::default().fg(Color::Gray)),
        ])),
        chunks[3],
    );

    let details = Paragraph::new(format!("{}█", form.details))
        .style(Style::default().fg(Color::White))
        .block(
            Block::default()
                .title(" Details — editing ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: false })
        .scroll((form.scroll, 0));
    frame.render_widget(details, chunks[4]);

    let help = Paragraph::new(vec![
        Line::from("Enter: new line   Tab: indent   ↑/↓: scroll"),
        Line::from("Ctrl+S: save & close   Esc: cancel"),
    ])
    .alignment(Alignment::Center)
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(help, chunks[5]);
}

fn draw_dependencies(frame: &mut Frame, app: &App, form: &DependencyForm) {
    let Some(todo) = app.todo_by_id(form.todo_id) else {
        return;
    };
    let area = centered_rect(84, 22, frame.area());
    let block = Block::default()
        .title(" Prerequisites ")
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);

    let chunks = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(3),
        Constraint::Length(1),
        Constraint::Length(2),
    ])
    .split(inner);
    frame.render_widget(
        Paragraph::new(todo.title.clone())
            .style(
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .wrap(Wrap { trim: false }),
        chunks[0],
    );

    let candidates = app.dependency_candidates(form.todo_id);
    if candidates.is_empty() {
        frame.render_widget(
            Paragraph::new("No other todos available.")
                .alignment(Alignment::Center)
                .style(Style::default().fg(Color::DarkGray)),
            chunks[1],
        );
    } else {
        let selected_index = form
            .selected_id
            .and_then(|id| {
                candidates
                    .iter()
                    .position(|(_, candidate)| candidate.id == id)
            })
            .or(Some(0));
        let items: Vec<ListItem> = candidates
            .iter()
            .enumerate()
            .map(|(index, (date, candidate))| {
                let selected = selected_index == Some(index);
                let linked = todo.prerequisites.contains(&candidate.id);
                let relation_style = if linked {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                let completion_style = if candidate.completed {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::Yellow)
                };
                ListItem::new(Line::from(vec![
                    Span::styled(
                        if selected { "› " } else { "  " },
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(if linked { "[x] " } else { "[ ] " }, relation_style),
                    Span::styled(
                        if candidate.completed {
                            "[✓] "
                        } else {
                            "[ ] "
                        },
                        completion_style,
                    ),
                    Span::styled(
                        format!("{date} #{:<4} ", candidate.id),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        candidate.title.clone(),
                        if candidate.completed {
                            Style::default()
                                .fg(Color::Gray)
                                .add_modifier(Modifier::CROSSED_OUT)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                ]))
                .style(if selected {
                    Style::default().bg(Color::DarkGray)
                } else {
                    Style::default()
                })
            })
            .collect();
        let list = List::new(items).highlight_style(Style::default());
        let mut state = ListState::default().with_selected(selected_index);
        frame.render_stateful_widget(list, chunks[1], &mut state);
    }

    let message = form
        .error
        .as_deref()
        .unwrap_or("[x] prerequisite   [✓] completed");
    frame.render_widget(
        Paragraph::new(message).style(Style::default().fg(if form.error.is_some() {
            Color::Red
        } else {
            Color::DarkGray
        })),
        chunks[2],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from("U/E or ↑/↓: select   Space/Enter: add/remove"),
            Line::from("Esc: close"),
        ])
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray)),
        chunks[3],
    );
}

fn draw_delete_confirmation(frame: &mut Frame, app: &App, id: u64) {
    let Some(todo) = app.todo_by_id(id) else {
        return;
    };
    let area = centered_rect(62, 8, frame.area());
    let popup = Paragraph::new(vec![
        Line::from("Delete this todo?"),
        Line::from(""),
        Line::from(todo.title.clone()).style(Style::default().add_modifier(Modifier::BOLD)),
        Line::from(""),
        Line::from("Y: delete   N/Esc: cancel").style(Style::default().fg(Color::Yellow)),
    ])
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: true })
    .block(
        Block::default()
            .title(" Confirm deletion ")
            .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red)),
    );
    frame.render_widget(Clear, area);
    frame.render_widget(popup, area);
}

fn priority_style(priority: Priority) -> Style {
    let style = Style::default()
        .fg(Color::Black)
        .add_modifier(Modifier::BOLD);
    match priority {
        Priority::High => style.bg(Color::Red),
        Priority::Medium => style.bg(Color::Yellow),
        Priority::Normal => style.bg(Color::Green),
        Priority::Low => style.bg(Color::Cyan),
    }
}

fn truncate_to_width(text: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if text.width() <= max_width {
        return text.to_owned();
    }

    let target_width = max_width - 1;
    let mut result = String::new();
    let mut width = 0;
    for character in text.chars() {
        let character_width = UnicodeWidthChar::width(character).unwrap_or(0);
        if width + character_width > target_width {
            break;
        }
        result.push(character);
        width += character_width;
    }
    result.push('…');
    result
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}
