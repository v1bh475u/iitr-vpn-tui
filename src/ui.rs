use crate::{
    app::{App, Field},
    vpn::ConnectionState,
};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};

pub fn draw(frame: &mut Frame, app: &App) {
    let area = centered(frame.area(), 88, 34);
    frame.render_widget(Clear, area);
    let outer = Block::default()
        .title(" IIT Roorkee VPN · AnyConnect via openconnect ")
        .title_alignment(Alignment::Center)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(15),
            Constraint::Min(7),
            Constraint::Length(2),
        ])
        .split(inner);

    draw_status(frame, app, rows[0]);
    draw_form(frame, app, rows[1]);
    draw_log(frame, app, rows[2]);
    draw_footer(frame, app, rows[3]);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let color = match app.state {
        ConnectionState::Connected => Color::Green,
        ConnectionState::Connecting | ConnectionState::Disconnecting => Color::Yellow,
        ConnectionState::Failed => Color::Red,
        ConnectionState::Disconnected => Color::DarkGray,
    };
    let line = Line::from(vec![
        Span::raw(" Status  "),
        Span::styled(
            app.state.label(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
        Span::styled(&app.message, Style::default().fg(Color::Gray)),
    ]);
    frame.render_widget(
        Paragraph::new(line).block(Block::default().borders(Borders::BOTTOM)),
        area,
    );
}

fn draw_form(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().title(" Connection ").borders(Borders::ALL);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let fields = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2); 5])
        .margin(1)
        .split(inner);

    let definitions = [
        (Field::Gateway, "Gateway", false),
        (Field::Username, "Username", false),
        (Field::Group, "Auth group (optional)", false),
        (Field::Password, "Password", true),
        (Field::SecondFactor, "OTP / 2nd factor (optional)", true),
    ];
    for ((field, label, secret), area) in definitions.into_iter().zip(fields.iter().copied()) {
        let focused = app.focus == field;
        let value = if secret {
            "•".repeat(app.field_text(field).chars().count())
        } else {
            app.field_text(field).to_owned()
        };
        let style = if focused {
            Style::default().fg(Color::Black).bg(Color::Cyan)
        } else {
            Style::default().fg(Color::White)
        };
        let line = Line::from(vec![
            Span::styled(
                format!(" {label:<29}"),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(format!(" {value}"), style),
        ]);
        frame.render_widget(Paragraph::new(line), area);
    }
}

fn draw_log(frame: &mut Frame, app: &App, area: Rect) {
    let height = area.height.saturating_sub(2) as usize;
    let skip = app.logs.len().saturating_sub(height);
    let items: Vec<ListItem> = app
        .logs
        .iter()
        .skip(skip)
        .map(|line| {
            let color = if line.contains("[!!]") || line.to_ascii_lowercase().contains("failed") {
                Color::Red
            } else if line.contains("[ok]") {
                Color::Green
            } else {
                Color::Gray
            };
            ListItem::new(line.as_str()).style(Style::default().fg(color))
        })
        .collect();
    frame.render_widget(
        List::new(items).block(
            Block::default()
                .title(" Session log ")
                .borders(Borders::ALL),
        ),
        area,
    );
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let hint = if matches!(app.focus, Field::Gateway | Field::Username | Field::Group) {
        "Tab next · Shift-Tab previous · Enter on secret field connects · Esc clears secrets"
    } else {
        "c connect · d disconnect · r check · s save · q quit · Tab next"
    };
    frame.render_widget(
        Paragraph::new(hint)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::DarkGray))
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn centered(area: Rect, max_width: u16, max_height: u16) -> Rect {
    let width = area.width.min(max_width);
    let height = area.height.min(max_height);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}
