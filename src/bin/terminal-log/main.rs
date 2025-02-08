use chrono::{DateTime, Datelike, Duration, Local, Timelike};
use crossterm::terminal::EnableLineWrap;
use ratatui::{
    buffer::Buffer,
    crossterm::event::{self, KeyCode, KeyEventKind},
    layout::{self, Constraint, Layout, Margin, Rect},
    style::{palette::tailwind, Color, Modifier, Stylize},
    symbols::{self, border},
    text::{Line, Text},
    widgets::{
        Block, Borders, List, ListItem, Padding, Paragraph, StatefulWidget, Table, Tabs, Widget,
        Wrap,
    },
    DefaultTerminal,
};
use std::{collections::LinkedList, time::SystemTime};
use strum::{Display, EnumIter, FromRepr, IntoEnumIterator};
use tokio::time::sleep;
use tor_rev_proxy::{
    controller::controller::controller_api::client::ControllerClient,
    logger::Event as ProxyInnerEvent, types::client_addr::ClientAddr,
};

pub type ProxyEvent = (DateTime<Local>, ProxyInnerEvent);
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let mut terminal = ratatui::init();
    let mut app = TerminalClientApp {
        events_list: EventsList {
            events: Vec::new(),
            scroll_pos: 0,
        },
        client: ControllerClient::new(),
        connections: Connections {
            conns: Vec::new(),
            scroll_pos: 0,
        },
    };
    loop {
        sleep(Duration::milliseconds(100).to_std().unwrap()).await;
        let new_events = app.client.get_events().await.unwrap();
        app.events_list.scroll_pos += new_events.len();
        app.events_list.events.extend(new_events);
        
        let conns = app.client.get_current_connections().await.unwrap();
        app.connections.conns = conns;

        terminal
            .draw(|f| {
                f.render_widget(&app, f.area());
            })
            .unwrap();

        ratatui::restore();
    }

    Ok(())
}

pub struct TerminalClientApp {
    client: ControllerClient,
    connections: Connections,
    events_list: EventsList,
}
impl TerminalClientApp{
    
}
impl TerminalClientApp {
}

pub struct EventsList {
    events: Vec<ProxyEvent>,
    scroll_pos: usize,
}

pub struct Connections {
    conns: Vec<ClientAddr >,
    scroll_pos: usize,
}


pub struct EventWidget<'a>(usize, &'a ProxyEvent);

impl<'a> Widget for EventWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let block = Block::new().borders(Borders::TOP).title(Line::raw(format!("[{:05}]", self.0)).left_aligned()).title(Line::raw(TimeRefWidget(&self.1 .0).get_text()).right_aligned());
        let area = block.render_and_inner(area, buf);

        Paragraph::new(format!("{:?}", &self.1 .1))
            .wrap(Wrap { trim: true })
            .render(area, buf);

    }
}

#[derive(Clone, Copy)]
pub struct TimeRefWidget<'a>(&'a DateTime<Local>);

impl<'a> TimeRefWidget<'a> {
    fn get_text(self) -> String {
        let time = self.0;
        format!(
            "{:02}/{:02}/{:04} {:02}:{:02}:{:02}",          
            time.day()  , time.month()  , time.year(),
            time.hour() , time.minute() , time.second() 
        )
    }
}

impl<'a> Widget for &'a EventsList {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let mut remaining = area;
        let mut event_widgets = 
            self
            .events[..self.scroll_pos]
            .iter()
            .rev()
            .enumerate()
            .map(|(i, e)| EventWidget(self.scroll_pos - i, e));

        'DRAW_EVENTS: while remaining.height >= 3 {
            let Some(event_widget) = event_widgets.next() else {
                break 'DRAW_EVENTS;
            };
            let [remaining_, widget_space] =
                Layout::vertical([Constraint::Fill(1), Constraint::Length(3)]).areas(remaining);
            remaining = remaining_;
            event_widget.render(widget_space, buf);
        }
    }
}
impl<'a> Widget for &'a Connections {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let mut connections = self.conns[self.scroll_pos..].iter();
        let mut remaining = area;
        'DRAW_CONNECTIONS: while remaining.height > 1 {
            let Some(current_conn) = connections.next() else {
                break 'DRAW_CONNECTIONS;
            };
            let [curr_conn_area, remaining_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(remaining);
            let curr_conn_area = curr_conn_area.inner(Margin::new(2, 0));
            Paragraph::new(format!("{}", current_conn.0))
                .left_aligned()
                .bold()
                .render(curr_conn_area, buf);

            remaining = remaining_area;
        }
    }
}
impl<'a> Widget for &'a TerminalClientApp {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        let [mut events_list_area, mut connections_area] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Length(32)]).areas(area);
        events_list_area = Block::bordered()
            .title("Events")
            .render_and_inner(events_list_area, buf);
        self.events_list.render(events_list_area, buf);

        connections_area = Block::bordered()
            .title("Connections")
            .render_and_inner(connections_area, buf);
        self.connections.render(connections_area, buf);
    }
}
pub trait BlockExt {
    fn render_and_inner(self, area: Rect, buf: &mut Buffer) -> Rect;
}

impl BlockExt for Block<'_> {
    fn render_and_inner(self, area: Rect, buf: &mut Buffer) -> Rect {
        let inner_area = self.inner(area);
        self.render(area, buf);
        inner_area
    }
}
