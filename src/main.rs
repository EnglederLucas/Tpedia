use chrono::prelude::*;
use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use html2text::from_read;
use rand::{distributions::Alphanumeric, prelude::*};
use serde::{Deserialize, Serialize};
use wikimedia_types::{HtmlPageResult, Search, SearchResponse};
use std::{collections::HashMap, convert::TryInto, fmt::Debug, fs};
use std::io;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;
use tui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{
        Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Table, Tabs,
    },
    Terminal,
};
mod wikimedia_types;

const DB_PATH: &str = "./data/db.json";

#[derive(Serialize, Deserialize, Clone)]
struct Pet {
    id: usize,
    name: String,
    category: String,
    age: usize,
    created_at: DateTime<Utc>,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("error reading the DB file: {0}")]
    ReadDBError(#[from] std::io::Error),
    #[error("error parsing the DB file: {0}")]
    ParseDBError(#[from] serde_json::Error),
}

//Every User Interaction
enum Event<I> {
    Input(I),
    Tick,
}


//Menu
#[derive(Copy, Clone, Debug)]
enum MenuItem{
    Home,Results
}

impl From<MenuItem> for usize {
    fn from(input: MenuItem) -> usize {
        match input {
            MenuItem::Home => 0,
            MenuItem::Results => 1
        }
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode().expect("can run in raw mode");


    let mut search_mode = false;

    let (tx, rx) = mpsc::channel();
    let tick_rate = Duration::from_millis(200);
    thread::spawn(move || {
        let mut last_tick = Instant::now();
        loop {
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if event::poll(timeout).expect("poll works") {

                if let CEvent::Key(key) = event::read().expect("can read events") {
                    tx.send(Event::Input(key)).expect("can send events");
                }
            }

            if last_tick.elapsed() >= tick_rate {
                if let Ok(_) = tx.send(Event::Tick) {
                    last_tick = Instant::now();
                }
            }
        }
    });

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let menu_titles = vec!["Home", "Results", "Quit"];
    let mut active_menu_item = MenuItem::Home;

    let mut search_string: String = String::new();
    let mut search_result_list_state = ListState::default();
    search_result_list_state.select(Some(0));

    // let mut current_search_response: SearchResponse;
    let mut current_search_results: Vec<Search> = Vec::new();

    loop {
        terminal.draw(|rect| {
            let size = rect.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints(
                    [
                        Constraint::Length(3),
                        Constraint::Min(2),
                        Constraint::Length(3),
                    ]
                    .as_ref(),
                )
                .split(size);

            let copyright = Paragraph::new("pet-CLI 2020 - all rights reserved")
                .style(Style::default().fg(Color::LightCyan))
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::White))
                        .title("Copyright")
                        .border_type(BorderType::Plain),
                );

            let menu = menu_titles
                .iter()
                .map(|t| {
                    let (first, rest) = t.split_at(1);
                    Spans::from(vec![
                        Span::styled(
                            first,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::UNDERLINED),
                        ),
                        Span::styled(rest, Style::default().fg(Color::White)),
                    ])
                })
                .collect();

            let tabs = Tabs::new(menu)
                .select(active_menu_item.into())
                .block(Block::default().title("Menu").borders(Borders::ALL))
                .style(Style::default().fg(Color::White))
                .highlight_style(Style::default().fg(Color::Yellow))
                .divider(Span::raw("|"));

            let search_block = Block::default().title(
                Span::styled(&search_string, Style::default().fg(Color::Green))     
            );

            rect.render_widget(tabs, chunks[0]);

            rect.render_widget(search_block, chunks[0]);

            match active_menu_item {
                MenuItem::Home => rect.render_widget(render_home(), chunks[1]),
                MenuItem::Results => {
                    let results_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints(
                            [Constraint::Percentage(20), Constraint::Percentage(80)].as_ref(),
                        )
                        .split(chunks[1]);

                    let (left, right) = render_search_results(current_search_results.clone(), &search_result_list_state);

                    rect.render_stateful_widget(left, results_chunks[0], &mut search_result_list_state);
                    rect.render_widget(right, results_chunks[1]);
                }
            }
            rect.render_widget(copyright, chunks[2]);
        })?;

        match rx.recv()? {
            Event::Input(event) => {
                if search_mode {
                    match event.code {
                        KeyCode::Char(c) => {
                            search_string.push(c);
                        }
                        KeyCode::Backspace => {
                            search_string.pop();
                        }
                        KeyCode::Enter => {
                            let rt = tokio::runtime::Runtime::new().unwrap();
                        
                            let res = rt.block_on(search(search_string.clone())).unwrap();
                            // println!("{:?}", res);

                            current_search_results = res.query.search;
                            search_mode = false;
                            active_menu_item = MenuItem::Results
                        }
                        KeyCode::Esc => search_mode = false, 
                        _ => {}
                    }
                } else {
                    match event.code {
                        KeyCode::Char('q') => {
                            disable_raw_mode()?;
                            terminal.show_cursor()?;
                            break;
                        }
                        KeyCode::Char('h') => active_menu_item = MenuItem::Home,
                        KeyCode::Char('r') => active_menu_item = MenuItem::Results,
                        KeyCode::Char('s') => {
                            search_mode = true;
                        },
                        KeyCode::Down => {
                            if let Some(selected) = search_result_list_state.selected() {
                                let amount_results = current_search_results.len();
                                if selected >= amount_results - 1 {
                                    search_result_list_state.select(Some(0));
                                } else {
                                    search_result_list_state.select(Some(selected + 1));
                                }
                            }
                        }
                        KeyCode::Up => {
                            if let Some(selected) = search_result_list_state.selected() {
                                let amount_results = current_search_results.len();
                                if selected > 0 {
                                    search_result_list_state.select(Some(selected - 1));
                                } else {
                                    search_result_list_state.select(Some(amount_results - 1));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            },
            Event::Tick => {}
        }
    }

    Ok(())
}

async fn fetch_html(pageid: usize) -> Result<String, Box<dyn std::error::Error>> {

    let url = format!("https://en.wikipedia.org/w/api.php?action=parse&format=json&pageid={0}&prop=text&formatversion=2", pageid);

    let resp = reqwest::get(&url)
        .await?    
        .json::<serde_json::Value>()        
        .await?;

    let page_res: HtmlPageResult = serde_json::from_value(resp).unwrap();

    let text = html2text::from_read( page_res.parse.text.as_bytes(), 100);

    Ok(text)
}

async fn search (search_term: String) -> Result<SearchResponse, Box<dyn std::error::Error>>  {

    let url = format!("https://en.wikipedia.org/w/api.php?action=query&format=json&list=search&srsearch={}", search_term);

    let resp = reqwest::get(&url)
        .await?    
        .json::<serde_json::Value>()        
        .await?;

    let search_resp: SearchResponse = serde_json::from_value(resp).unwrap();

    // println!("{:#?}", search_resp.query.search);

    Ok(search_resp)
}

fn render_home<'a>() -> Paragraph<'a> {
    let home = Paragraph::new(vec![
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Welcome")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("to")]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::styled(
            "xPedia",
            Style::default().fg(Color::LightBlue),
        )]),
        Spans::from(vec![Span::raw("")]),
        Spans::from(vec![Span::raw("Press 's' to search")]),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Home")
            .border_type(BorderType::Plain),
    );
    home
}
fn render_search_results<'a>(search_results: Vec<Search>, search_result_list_state: &ListState) -> (List<'a>, Table<'a>) {
    let results = Block::default() 
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Results")
        .border_type(BorderType::Plain);

    let items:Vec<_> = search_results
        .iter()
        .map(|s| {
            ListItem::new(Spans::from(vec![Span::styled(
                s.title.clone(),
                Style::default(),
            )]))
        })
        .collect();



    let selected_result = search_results
        .get(
            search_result_list_state
                .selected()
                .expect("there is always a selected pet"),
        )
        .expect("exists")
        .clone();

    let list = List::new(items).block(results).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );


    
    let formatted_snippet = html2text::from_read( selected_result.snippet.as_bytes(), 20);  

    let results_detail = Table::new(vec![Row::new(vec![
        Cell::from(Span::raw(selected_result.title)),
        Cell::from(Span::raw(formatted_snippet)),
        Cell::from(Span::raw(selected_result.wordcount.to_string())),
    ])])
    .header(Row::new(vec![
        Cell::from(Span::styled(
            "Title",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Description",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            "Wordcount",
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ])
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::White))
            .title("Detail")
            .border_type(BorderType::Plain),
    )
    .column_spacing(3)
    .widths(&[
        Constraint::Percentage(15),
        Constraint::Percentage(75),
        Constraint::Percentage(5),
    ]);

    (list, results_detail)
}
