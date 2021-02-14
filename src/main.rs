use crossterm::{
    event::{self, Event as CEvent, KeyCode},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use regex::Regex;
use wikimedia_types::{HtmlPageResult, Search, SearchResponse};
use std::{convert::TryInto, fmt::Debug};
use std::sync::mpsc;
use std::thread;
use std::io;
use std::time::{Duration, Instant};
use thiserror::Error;
use tui::{Terminal, backend::CrosstermBackend, layout::{Alignment, Constraint, Direction, Layout}, style::{Color, Modifier, Style}, text::{Span, Spans}, widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap}};
mod wikimedia_types;


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
#[derive(Copy, Clone, Debug, )]
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

impl PartialEq for MenuItem {
    fn eq(&self, other: &MenuItem) -> bool {
        usize::from(*self) == usize::from(*other)
    }
}

impl Eq for MenuItem {}

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
    let mut is_selected = false;

    let mut scroll: u16 = 0;
    let mut current_content: Option<String> = None;

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

            let copyright = Paragraph::new("by Lucas Engleder")
                .style(Style::default().fg(Color::LightCyan))
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .style(Style::default().fg(Color::White))
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

            { 
                let tabs = Tabs::new(menu)
                    .select(active_menu_item.into())
                    .block(Block::default().title("Menu").borders(Borders::ALL))
                    .style(Style::default().fg(Color::White))
                    .highlight_style(Style::default().fg(Color::Yellow))
                    .divider(Span::raw("|"));

                let search_box = Block::default() 
                    .borders(Borders::ALL)
                    .style(Style::default().fg(Color::Yellow))
                    .border_type(BorderType::Plain);

                let search_text = Paragraph::new(format!("{}{}"," 🔍 ", search_string.clone()))
                    .block(search_box)
                    .style(Style::default()
                    .fg(Color::Yellow));

                let navbar = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(
                        [Constraint::Percentage(70), Constraint::Percentage(30)].as_ref(),
                    )
                    .split(chunks[0]);


                rect.render_widget(tabs, navbar[0]);

                rect.render_widget(search_text, navbar[1]);   
            }

            //Content Page, depends on which tab
            match active_menu_item {
                MenuItem::Home => rect.render_widget(render_home(), chunks[1]),
                MenuItem::Results => {
                    let results_chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints(
                            [Constraint::Percentage(20), Constraint::Percentage(80)].as_ref(),
                        )
                        .split(chunks[1]);


                    let list = render_search_list(current_search_results.clone());
                    rect.render_stateful_widget(list, results_chunks[0], &mut search_result_list_state);

                    if is_selected {
                        let selected_item = get_selected_search(current_search_results.clone(), &mut search_result_list_state);

                        let res  = render_page_content(selected_item.clone(), current_content.clone(), scroll,(size.width as f64 * 0.8).floor() as u16);
                        let page = res.0;
                        current_content = Some(res.1);
                        rect.render_widget(page, results_chunks[1]);
                    }
                }
            }

            //Footer
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

                            current_search_results = res.query.search;
                            search_mode = false;
                            active_menu_item = MenuItem::Results;

                            is_selected = false;
                            search_result_list_state.select(Some(0));
                        }
                        KeyCode::Esc => search_mode = false, 
                        _ => {}
                    }
                } 
                else if is_selected {
                    match event.code {
                        KeyCode::Esc => {
                            is_selected = false;
                        }
                        KeyCode::Down => {
                            scroll += 1;
                        }
                        KeyCode::Up => {
                            if scroll > 0 {
                                scroll -= 1;
                            }
                        }
                        _ => {}
                    }
                } 
                else if  active_menu_item == MenuItem::Results {
                    match event.code {
                        KeyCode::Enter => {
                            is_selected = true;
                            current_content = None;
                            scroll = 0;
                        },
                        KeyCode::Down => {
                            if let Some(selected) = search_result_list_state.selected() {
                                let amount_results = current_search_results.len();
                                if selected >= amount_results - 1 && amount_results != 0 {
                                    search_result_list_state.select(Some(0));
                                } else if amount_results != 0 {
                                    search_result_list_state.select(Some(selected + 1));
                                }
                            }
                        }
                        KeyCode::Up => {
                            if let Some(selected) = search_result_list_state.selected() {
                                let amount_results = current_search_results.len();
                            
                                if selected > 0 &&  amount_results != 0 {
                                    search_result_list_state.select(Some(selected - 1));
                                } else if  amount_results != 0  {
                                    search_result_list_state.select(Some(amount_results - 1));
                                }
                            }
                        }
                        _ => {}
                    }
                }

                if !search_mode {
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
                        _ => {}
                    }
                } 
            },
            Event::Tick => {}
        }
    }

    Ok(())
}

async fn fetch_html(pageid: usize, text_width: u16) -> Result<String, Box<dyn std::error::Error>> {

    let url = format!("https://en.wikipedia.org/w/api.php?action=parse&format=json&pageid={0}&prop=text&formatversion=2", pageid);

    let resp = reqwest::get(&url)
        .await?    
        .json::<serde_json::Value>()        
        .await?;

    let page_res: HtmlPageResult = serde_json::from_value(resp).unwrap();

    let html_regex = Regex::new(r#"<a href=\\#".*\\#">"#).unwrap();
    let html_cleaned = html_regex.replace_all(&page_res.parse.text, "");

    let text = html2text::from_read( String::from(html_cleaned).as_bytes(), text_width.into());

    let re = Regex::new(r"(\[)+\d*(\])|(edit)+|\[|\]|(https:)?(/.*/.*)+[\s\S]|#+\s\W").unwrap();
    //only Numbers (\[)+\d*(\])+
    let cleaned = re.replace_all(&text, "");
    let a = Regex::new(r"\d\s").unwrap();
    let removed_single_digit = a.replace_all(&cleaned, "");

    let mut removed_contents: String = String::from(removed_single_digit);

    let contents_start = removed_contents.find("## Contents");
    match contents_start {
        None => {}
        Some(i) => {
            let end_index = removed_contents[(i+11)..].find("## ").unwrap();

            removed_contents = format!("{}{}", removed_contents[..i].to_string(), removed_contents[(end_index+11+i)..].to_string());
        }
    }


    Ok(removed_contents)
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

fn get_selected_search(search_results: Vec<Search>, search_result_list_state: &ListState) -> Search {
    let selected_result = search_results
        .get(
            search_result_list_state
                .selected()
                .expect("there is always a selected result"),
        )
        .expect("no search results")
        .clone();

    selected_result
}


fn render_search_list<'a>(search_results: Vec<Search>) -> List<'a> {
    let results = Block::default() 
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title("Results")
        .border_type(BorderType::Plain);


    let items: Vec<_> = if search_results.len() > 0 {
        search_results
        .iter()
        .map(|s| {
            ListItem::new(Spans::from(vec![Span::styled(
                s.title.clone(),
                Style::default(),
            )]))
        })
        .collect()
    } else {
        vec![ListItem::new(Span::styled("No Results found", Style::default().fg(Color::LightRed)))]
    };
    

    let list = List::new(items).block(results).highlight_style(
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD),
    );

    list
}

fn render_page_content<'a>(selected_search: Search, content: Option<String>, scroll: u16, width: u16) -> (Paragraph<'a>,String) {
    let text_block = Block::default() 
        .borders(Borders::ALL)
        .style(Style::default().fg(Color::White))
        .title(Span::styled(selected_search.title, Style::default().fg(Color::Green)))
        .border_type(BorderType::Plain);


    let text: String = match content {
        None => {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(fetch_html(selected_search.pageid.try_into().unwrap(), width - 10)).unwrap()
        }
        Some(c) => c
    };

    let text_paragraph = Paragraph::new(text.clone())
        .block(text_block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));


    (text_paragraph, text)
}
