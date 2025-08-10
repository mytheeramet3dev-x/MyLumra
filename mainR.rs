use ammonia::clean;
use arcstr::ArcStr;
use bytes::Bytes;
use bumpalo::Bump;
use lru::LruCache;
use ratatui::{
backend::TermionBackend,
layout::{Constraint, Direction, Layout},
style::{Color, Style},
text::{Span, Spans},
widgets::{Block, Borders, List, ListItem, Paragraph},
Terminal,
};
use reqwest::{Client, Certificate};
use sled::Db;
use std::sync::{Arc, Mutex};
use sysinfo::{System, SystemExt};
use termion::{event::Key, input::TermRead, raw::IntoRawMode};
use tokio::sync::{Semaphore, mpsc};
use quick_xml::{events::Event, Reader};
use std::io::{stdout, Write};
use tokio_retry::{Retry, strategy::ExponentialBackoff};
use priority_queue::PriorityQueue;
use tracing::{error, info};
use url::Url;

lazy_static::lazy_static! {
static ref CLIENT: Client = {
let cert = Certificate::from_pem(include_bytes!("path_to_ca_cert.pem")).unwrap(); // แทนด้วย path จริง
Client::builder()
.user_agent("MyTextBrowser/1.0")
.https_only(true)
.http2_prior_knowledge()
.add_root_certificate(cert)
.build()
.unwrap()
};
static ref CACHE: Mutex<LruCache<String, Bytes>> = Mutex::new(LruCache::new(100));
static ref SEMAPHORE: Semaphore = Semaphore::new(10);
static ref DB: Db = sled::open("browser_cache").unwrap();
static ref HSTS_LIST: Vec<&'static str> = vec!["example.com"];
}

fn validate_url(url: &str) -> Result<String, reqwest::Error> {
let parsed = Url::parse(url).map_err(|e| reqwest::Error::from(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;
if parsed.scheme() != "https" {
return Err(reqwest::Error::from(std::io::Error::new(std::io::ErrorKind::InvalidInput, "Only HTTPS URLs allowed")));
}
Ok(url.to_string())
}

async fn fetch_page(url: &str) -> Result<Bytes, reqwest::Error> {
let url = validate_url(url)?;
if HSTS_LIST.contains(&Url::parse(&url).unwrap().host_str().unwrap_or("")) {
info!("HSTS enforced for {}", url);
}
{
let cache = CACHE.lock().unwrap();
if let Some(cached) = cache.get(&url) {
return Ok(cached.clone());
}
}
if let Ok(cached) = DB.get(&url) {
if let Some(data) = cached {
return Ok(Bytes::from(data.to_vec()));
}
}
let _permit = SEMAPHORE.acquire().await.unwrap();
let retry_strategy = ExponentialBackoff::from_millis(100).max_delay(std::time::Duration::from_secs(2)).take(3);
let html = Retry::spawn(retry_strategy, || async {
match CLIENT.get(&url).send().await {
Ok(res) => res.text().await,
Err(e) => {
error!("Failed to fetch {}: {}", url, e);
Err(e)
}
}
}).await?;
let sanitized = clean(&html); // Sanitize HTML
let compressed = zstd::stream::encode_all(sanitized.as_bytes(), 3).map_err(|e| reqwest::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
let bytes = Bytes::from(compressed.clone());
{
CACHE.lock().unwrap().put(url.clone(), bytes.clone());
DB.insert(url, compressed).unwrap();
}
Ok(bytes)
}

fn render_text(html: &[u8], arena: &Bump) -> (String, Vec<ArcStr>) {
let html_str = String::from_utf8_lossy(html);
let mut reader = Reader::from_str(&html_str);
reader.trim_text(true);
let mut text = String::new();
let mut links = Vec::new();
let mut link_index = 0;
let mut in_script_or_style = false;

loop {  
    match reader.read_event() {  
        Ok(Event::Start(ref e)) => {  
            let tag = e.name().as_ref();  
            if tag == b"script" || tag == b"style" {  
                in_script_or_style = true;  
            } else if tag == b"a" {  
                if let Some(href) = e.attributes().find(|a| a.unwrap().key.as_ref() == b"href").and_then(|a| a.ok()) {  
                    if let Ok(valid_url) = validate_url(std::str::from_utf8(href.value.as_ref()).unwrap_or("")) {  
                        links.push(ArcStr::from(valid_url));  
                    }  
                }  
            }  
        }  
        Ok(Event::End(ref e)) => {  
            if e.name().as_ref() == b"script" || e.name().as_ref() == b"style" {  
                in_script_or_style = false;  
            }  
        }  
        Ok(Event::Text(e)) if !in_script_or_style => {  
            if let Ok(content) = e.unescape() {  
                let trimmed = content.trim();  
                if !trimmed.is_empty() {  
                    text.push_str(&format!("[{}] {}\n", link_index, trimmed));  
                    link_index += 1;  
                }  
            }  
        }  
        Ok(Event::Eof) => break,  
        Err(e) => {  
            error!("Parsing error: {}", e);  
            break;  
        }  
    }  
}  
(arena.alloc_str(&text).to_string(), links)

}

async fn prefetch_links(links: &[ArcStr], tx: mpsc::Sender<()>) {
let mut queue = PriorityQueue::new();
for (i, link) in links.iter().enumerate().take(5) {
queue.push(link.clone(), i);
}
while let Some((link, _)) = queue.pop() {
if let Err(e) = fetch_page(&link).await {
error!("Prefetch failed for {}: {}", link, e);
}
let _ = tx.send(()).await;
}
}

struct Tab {
url: ArcStr,
text: Option<String>,
links: Option<Vec<ArcStr>>,
}

struct Browser {
tabs: Vec<Tab>,
active_tab: usize,
system: System,
}

impl Browser {
fn new() -> Self {
Browser {
tabs: Vec::new(),
active_tab: 0,
system: System::new_all(),
}
}

async fn open_tab(&mut self, url: &str) {  
    if let Ok(valid_url) = validate_url(url) {  
        self.tabs.push(Tab {  
            url: ArcStr::from(valid_url),  
            text: None,  
            links: None,  
        });  
    }  
}  

async fn render_active_tab(&mut self, arena: &Bump) -> Result<(String, Vec<ArcStr>), reqwest::Error> {  
    let tab = &mut self.tabs[self.active_tab];  
    if tab.text.is_none() {  
        let html = fetch_page(&tab.url).await?;  
        let (text, links) = render_text(&html, arena);  
        tab.text = Some(text);  
        tab.links = Some(links.clone());  
        let (tx, mut rx) = mpsc::channel(1);  
        tokio::spawn(prefetch_links(&links, tx));  
        tokio::spawn(async move {  
            while rx.recv().await.is_some() {}  
        });  
    }  
    Ok((tab.text.as_ref().unwrap().clone(), tab.links.as_ref().unwrap().clone()))  
}  

fn switch_tab(&mut self, index: usize) {  
    if index < self.tabs.len() {  
        self.active_tab = index;  
        for (i, tab) in self.tabs.iter_mut().enumerate() {  
            if i != self.active_tab {  
                tab.text = None;  
                tab.links = None;  
            }  
        }  
    }  
}  

fn get_system_info(&mut self) -> String {  
    self.system.refresh_all();  
    format!(  
        "RAM: {} MB used / {} MB total\nCPU: {:.1}%",  
        self.system.used_memory() / 1024 / 1024,  
        self.system.total_memory() / 1024 / 1024,  
        self.system.global_cpu_info().cpu_usage()  
    )  
}

}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
// Initialize logging
tracing_subscriber::fmt()
.with_max_level(tracing::Level::INFO)
.init();

// Sandboxing with seccomp (Linux only)  
#[cfg(target_os = "linux")]  
{  
    use seccomp::{SeccompAction, SeccompFilter, SyscallRuleSet};  
    let filter = SeccompFilter::new(  
        vec![  
            SyscallRuleSet::allow_syscall("read"),  
            SyscallRuleSet::allow_syscall("write"),  
            SyscallRuleSet::allow_syscall("open"),  
            SyscallRuleSet::allow_syscall("close"),  
            SyscallRuleSet::allow_syscall("fstat"),  
            SyscallRuleSet::allow_syscall("mmap"),  
            SyscallRuleSet::allow_syscall("munmap"),  
            SyscallRuleSet::allow_syscall("exit"),  
        ],  
        SeccompAction::Trap,  
    )?;  
    filter.apply()?;  
}  

let mut browser = Browser::new();  
let urls = vec!["https://example.com"; 100];  
for url in urls {  
    browser.open_tab(url).await;  
}  

let stdout = stdout().into_raw_mode()?;  
let backend = TermionBackend::new(stdout);  
let mut terminal = Terminal::new(backend)?;  
terminal.clear()?;  

let mut stdin = termion::async_stdin().keys();  
let arena = Bump::new();  

loop {  
    let (text, links) = browser.render_active_tab(&arena).await.unwrap_or((  
        "Error loading page".to_string(),  
        vec![]  
    ));  

    terminal.draw(|f| {  
        let chunks = Layout::default()  
            .direction(Direction::Horizontal)  
            .constraints([  
                Constraint::Percentage(20),  
                Constraint::Percentage(60),  
                Constraint::Percentage(20),  
            ])  
            .split(f.size());  

        let tab_items: Vec<ListItem> = browser  
            .tabs  
            .iter()  
            .enumerate()  
            .map(|(i, tab)| {  
                let style = if i == browser.active_tab {  
                    Style::default().fg(Color::Yellow)  
                } else {  
                    Style::default()  
                };  
                ListItem::new(Span::styled(tab.url.as_str(), style))  
            })  
            .collect();  
        let tabs = List::new(tab_items)  
            .block(Block::default().borders(Borders::ALL).title("Tabs"))  
            .highlight_style(Style::default().fg(Color::Green));  
        f.render_widget(tabs, chunks[0]);  

        let content = Paragraph::new(text.as_str())  
            .block(Block::default().borders(Borders::ALL).title("Content"))  
            .scroll((0, 0));  
        f.render_widget(content, chunks[1]);  

        let sys_info = Paragraph::new(browser.get_system_info())  
            .block(Block::default().borders(Borders::ALL).title("System"));  
        f.render_widget(sys_info, chunks[2]);  
    })?;  

    if let Some(Ok(key)) = stdin.next() {  
        match key {  
            Key::Char('q') => break,  
            Key::Char('n') => {  
                let next_tab = (browser.active_tab + 1) % browser.tabs.len();  
                browser.switch_tab(next_tab);  
            }  
            Key::Char('p') => {  
                let prev_tab = if browser.active_tab == 0 {  
                    browser.tabs.len() - 1  
                } else {  
                    browser.active_tab - 1  
                };  
                browser.switch_tab(prev_tab);  
            }  
            Key::Char(c) if c.is_digit(10) => {  
                if let Some(index) = c.to_digit(10) {  
                    if (index as usize) < links.len() {  
                        let new_url = links[index as usize].clone();  
                        browser.open_tab(&new_url).await;  
                        browser.switch_tab(browser.tabs.len() - 1);  
                    }  
                }  
            }  
            _ => {}  
        }  
    }  
}  

Ok(())

}

