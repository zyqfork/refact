use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use headless_chrome::Browser;
use headless_chrome::protocol::cdp::types::Event;
use headless_chrome::protocol::cdp::Page;
use serde_json;
use tokio::sync::{Mutex as AMutex, RwLock as ARwLock};
use tracing::{info, warn};
use uuid::Uuid;

use crate::chat::types::WindowBounds;
use crate::global_context::GlobalContext;
use crate::integrations::integr_chrome::ChromeTab;
use crate::integrations::browser_types::{
    RecorderEvent, ConsoleEntry, NetworkEntry, MutationSummaryEntry,
    MAX_BUFFER_SIZE, SCROLL_DEBOUNCE_MS,
    apply_password_masking, enforce_buffer_limit, flush_buffer_since,
};

const RECORDER_SCRIPT_TEMPLATE: &str = include_str!("browser_recorder.js");

pub fn build_recorder_script(mask_passwords: bool) -> String {
    RECORDER_SCRIPT_TEMPLATE.replace(
        "__REFACT_MASK_PASSWORDS__",
        if mask_passwords { "true" } else { "false" },
    )
}

pub struct BrowserRuntime {
    pub runtime_id: String,
    pub attached_chat_id: Option<String>,
    pub browser: Browser,
    pub tabs: HashMap<String, Arc<AMutex<ChromeTab>>>,
    pub profile_dir: PathBuf,
    pub window_bounds: Option<WindowBounds>,
    pub action_buffer: Vec<RecorderEvent>,
    pub console_buffer: Vec<ConsoleEntry>,
    pub network_buffer: Vec<NetworkEntry>,
    pub mutation_summary: Vec<MutationSummaryEntry>,
    pub last_send_action_cursor: usize,
    pub last_send_console_cursor: usize,
    pub last_send_network_cursor: usize,
    pub last_send_mutation_cursor: usize,
    pub last_frame_hash: Option<u64>,
    pub last_frame_data: Option<Vec<u8>>,
    pub idle_timeout: Duration,
    pub is_connected: bool,
    pub last_activity: Instant,
    pub mask_passwords: bool,
}

impl BrowserRuntime {
    pub fn launch(
        profile_dir: PathBuf,
        window_bounds: Option<WindowBounds>,
        chrome_path: Option<PathBuf>,
        idle_timeout: Option<Duration>,
        mask_passwords: bool,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(&profile_dir)
            .map_err(|e| format!("Failed to create profile dir {:?}: {}", profile_dir, e))?;

        let window_size = window_bounds.as_ref().map(|wb| (wb.width, wb.height));
        let idle_timeout = idle_timeout.unwrap_or(Duration::from_secs(600));

        let mut launch_options = headless_chrome::LaunchOptions {
            headless: false,
            window_size,
            idle_browser_timeout: idle_timeout,
            user_data_dir: Some(profile_dir.clone()),
            ..Default::default()
        };
        if let Some(ref path) = chrome_path {
            launch_options.path = Some(path.clone());
        }

        let browser = Browser::new(launch_options).map_err(|e| e.to_string())?;
        let runtime_id = Uuid::new_v4().to_string();

        info!("BrowserRuntime {} launched with profile {:?}", runtime_id, profile_dir);

        Ok(Self {
            runtime_id,
            attached_chat_id: None,
            browser,
            tabs: HashMap::new(),
            profile_dir,
            window_bounds,
            action_buffer: Vec::new(),
            console_buffer: Vec::new(),
            network_buffer: Vec::new(),
            mutation_summary: Vec::new(),
            last_send_action_cursor: 0,
            last_send_console_cursor: 0,
            last_send_network_cursor: 0,
            last_send_mutation_cursor: 0,
            last_frame_hash: None,
            last_frame_data: None,
            idle_timeout,
            is_connected: true,
            last_activity: Instant::now(),
            mask_passwords,
        })
    }

    pub fn reattach(&mut self, chat_id: &str) {
        info!(
            "BrowserRuntime {} reattached from {:?} to {}",
            self.runtime_id, self.attached_chat_id, chat_id
        );
        self.attached_chat_id = Some(chat_id.to_string());
        self.last_activity = Instant::now();
    }

    pub fn detach(&mut self) {
        info!(
            "BrowserRuntime {} detached from {:?}",
            self.runtime_id, self.attached_chat_id
        );
        self.attached_chat_id = None;
    }

    pub fn check_connection(&mut self) -> bool {
        let connected = self.browser.get_version().is_ok();
        if self.is_connected && !connected {
            warn!("BrowserRuntime {} detected browser disconnect", self.runtime_id);
        }
        self.is_connected = connected;
        connected
    }

    pub fn is_idle_expired(&self) -> bool {
        self.last_activity.elapsed() > self.idle_timeout
    }

    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn handle_recorder_event(&mut self, json_str: &str) {
        match serde_json::from_str::<RecorderEvent>(json_str) {
            Ok(event) => {
                let event = if self.mask_passwords {
                    apply_password_masking(&event)
                } else {
                    event
                };

                if event.is_scroll() {
                    if let Some(last) = self.action_buffer.last() {
                        if last.is_scroll() {
                            let last_ts = last.timestamp();
                            let new_ts = event.timestamp();
                            if (new_ts - last_ts) < SCROLL_DEBOUNCE_MS {
                                self.action_buffer.pop();
                            }
                        }
                    }
                }

                match &event {
                    RecorderEvent::MutationSummary { added, removed, changed, timestamp } => {
                        self.mutation_summary.push(MutationSummaryEntry {
                            timestamp: *timestamp,
                            added: *added,
                            removed: *removed,
                            changed: *changed,
                            descriptions: Vec::new(),
                        });
                        enforce_buffer_limit(&mut self.mutation_summary, &mut self.last_send_mutation_cursor);
                    }
                    _ => {
                        self.action_buffer.push(event);
                        enforce_buffer_limit(&mut self.action_buffer, &mut self.last_send_action_cursor);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to parse recorder event: {}: {}", e, json_str);
            }
        }
    }

    pub fn handle_console_event(&mut self, timestamp: f64, level: String, text: String) {
        self.console_buffer.push(ConsoleEntry { timestamp, level, text });
        enforce_buffer_limit(&mut self.console_buffer, &mut self.last_send_console_cursor);
    }

    pub fn handle_network_request(&mut self, timestamp: f64, method: String, url: String, resource_type: String) {
        let allowed = matches!(resource_type.as_str(), "Document" | "XHR" | "Fetch");
        if !allowed {
            return;
        }
        self.network_buffer.push(NetworkEntry {
            timestamp,
            method,
            url,
            resource_type,
            status: None,
        });
        enforce_buffer_limit(&mut self.network_buffer, &mut self.last_send_network_cursor);
    }

    pub fn handle_network_response(&mut self, url: &str, status: u16) {
        for entry in self.network_buffer.iter_mut().rev() {
            if entry.url == url && entry.status.is_none() {
                entry.status = Some(status);
                break;
            }
        }
    }

    pub fn flush_action_buffer(&mut self) -> Vec<RecorderEvent> {
        flush_buffer_since(&self.action_buffer, &mut self.last_send_action_cursor)
    }

    pub fn flush_console_buffer(&mut self) -> Vec<ConsoleEntry> {
        flush_buffer_since(&self.console_buffer, &mut self.last_send_console_cursor)
    }

    pub fn flush_network_buffer(&mut self) -> Vec<NetworkEntry> {
        flush_buffer_since(&self.network_buffer, &mut self.last_send_network_cursor)
    }

    pub fn flush_mutation_summary(&mut self) -> Vec<MutationSummaryEntry> {
        flush_buffer_since(&self.mutation_summary, &mut self.last_send_mutation_cursor)
    }
}

pub fn inject_recorder_into_tab(
    tab: &headless_chrome::Tab,
    mask_passwords: bool,
    action_buffer: Arc<Mutex<Vec<String>>>,
) -> Result<(), String> {
    let script = build_recorder_script(mask_passwords);

    tab.call_method(Page::AddScriptToEvaluateOnNewDocument {
        source: script,
        world_name: None,
        include_command_line_api: None,
        run_immediately: None,
    }).map_err(|e| format!("Failed to add recorder script: {}", e))?;

    let binding_buffer = action_buffer.clone();
    tab.expose_function(
        "__refact_event",
        Arc::new(move |payload: serde_json::Value| {
            if let Some(json_str) = payload.as_str() {
                if let Some(inner) = json_str.strip_prefix('{') {
                    let rebuilt = format!("{{{}", inner);
                    if let Ok(mut buf) = binding_buffer.lock() {
                        buf.push(rebuilt);
                    }
                } else if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Ok(s) = serde_json::to_string(&parsed) {
                        if let Ok(mut buf) = binding_buffer.lock() {
                            buf.push(s);
                        }
                    }
                }
            }
        }),
    ).map_err(|e| format!("Failed to expose __refact_event binding: {}", e))?;

    Ok(())
}

pub fn setup_console_capture(
    tab: &headless_chrome::Tab,
    console_buffer: Arc<Mutex<Vec<ConsoleEntry>>>,
) -> Result<(), String> {
    tab.enable_log().map_err(|e| format!("Failed to enable log: {}", e))?;

    tab.add_event_listener(Arc::new(move |event: &Event| {
        if let Event::LogEntryAdded(e) = event {
            let entry = ConsoleEntry {
                timestamp: e.params.entry.timestamp,
                level: format!("{:?}", e.params.entry.level),
                text: e.params.entry.text.clone(),
            };
            if let Ok(mut buf) = console_buffer.lock() {
                buf.push(entry);
                if buf.len() > MAX_BUFFER_SIZE {
                    let excess = buf.len() - MAX_BUFFER_SIZE;
                    buf.drain(..excess);
                }
            }
        }
    })).map_err(|e| format!("Failed to add console listener: {}", e))?;

    Ok(())
}

pub fn setup_network_capture(
    tab: &headless_chrome::Tab,
    network_buffer: Arc<Mutex<Vec<NetworkEntry>>>,
) -> Result<(), String> {
    let buf = network_buffer.clone();
    tab.register_response_handling(
        "__refact_network",
        Box::new(move |params, _fetch_body| {
            let url = params.response.url.clone();
            let status = params.response.status;
            let resource_type = format!("{:?}", params.Type);
            let allowed = matches!(
                resource_type.as_str(),
                "Document" | "Xhr" | "Fetch" | "XHR" | "Other"
            );
            if allowed {
                if let Ok(mut buf) = buf.lock() {
                    buf.push(NetworkEntry {
                        timestamp: params.timestamp as f64,
                        method: String::new(),
                        url,
                        resource_type,
                        status: Some(status as u16),
                    });
                    if buf.len() > MAX_BUFFER_SIZE {
                        let excess = buf.len() - MAX_BUFFER_SIZE;
                        buf.drain(..excess);
                    }
                }
            }
        }),
    ).map_err(|e| format!("Failed to setup network capture: {}", e))?;

    Ok(())
}

pub fn get_browser_profile_dir(
    gcx_cache_dir: &PathBuf,
    thread_id: &str,
) -> PathBuf {
    gcx_cache_dir
        .join("browser_profiles")
        .join(thread_id)
}

pub async fn get_or_create_browser_runtime(
    gcx: Arc<ARwLock<GlobalContext>>,
    runtime_id: &str,
) -> Option<Arc<AMutex<BrowserRuntime>>> {
    let gcx_locked = gcx.read().await;
    gcx_locked.browser_runtimes.get(runtime_id).cloned()
}

pub async fn register_browser_runtime(
    gcx: Arc<ARwLock<GlobalContext>>,
    runtime: BrowserRuntime,
) -> String {
    let runtime_id = runtime.runtime_id.clone();
    let arc = Arc::new(AMutex::new(runtime));
    gcx.write().await.browser_runtimes.insert(runtime_id.clone(), arc);
    runtime_id
}

pub async fn remove_browser_runtime(
    gcx: Arc<ARwLock<GlobalContext>>,
    runtime_id: &str,
) -> Option<Arc<AMutex<BrowserRuntime>>> {
    gcx.write().await.browser_runtimes.remove(runtime_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_get_browser_profile_dir() {
        let cache_dir = PathBuf::from("/tmp/refact-cache");
        let profile = get_browser_profile_dir(&cache_dir, "thread-abc-123");
        assert_eq!(
            profile,
            PathBuf::from("/tmp/refact-cache/browser_profiles/thread-abc-123")
        );
    }

    #[test]
    fn test_get_browser_profile_dir_different_threads() {
        let cache_dir = PathBuf::from("/home/user/.cache/refact");
        let p1 = get_browser_profile_dir(&cache_dir, "thread-1");
        let p2 = get_browser_profile_dir(&cache_dir, "thread-2");
        assert_ne!(p1, p2);
        assert!(p1.to_str().unwrap().contains("thread-1"));
        assert!(p2.to_str().unwrap().contains("thread-2"));
    }

    #[test]
    fn test_build_recorder_script_mask_true() {
        let script = build_recorder_script(true);
        assert!(script.contains("var MASK_PASSWORDS = true;"));
        assert!(!script.contains("__REFACT_MASK_PASSWORDS__"));
    }

    #[test]
    fn test_build_recorder_script_mask_false() {
        let script = build_recorder_script(false);
        assert!(script.contains("var MASK_PASSWORDS = false;"));
    }

    #[test]
    fn test_handle_recorder_event_click() {
        let mut rt = make_test_runtime();
        let json = r##"{"type":"click","selector":"#btn","text":"OK","x":10.0,"y":20.0,"timestamp":1000.0}"##;
        rt.handle_recorder_event(json);
        assert_eq!(rt.action_buffer.len(), 1);
        assert!(matches!(&rt.action_buffer[0], RecorderEvent::Click { .. }));
    }

    #[test]
    fn test_handle_recorder_event_scroll_debounce() {
        let mut rt = make_test_runtime();
        rt.handle_recorder_event(r#"{"type":"scroll","scroll_x":0,"scroll_y":100,"timestamp":1000.0}"#);
        rt.handle_recorder_event(r#"{"type":"scroll","scroll_x":0,"scroll_y":200,"timestamp":1100.0}"#);
        rt.handle_recorder_event(r#"{"type":"scroll","scroll_x":0,"scroll_y":300,"timestamp":1150.0}"#);
        assert_eq!(rt.action_buffer.len(), 1);
        match &rt.action_buffer[0] {
            RecorderEvent::Scroll { scroll_y, .. } => assert_eq!(*scroll_y, 300.0),
            _ => panic!("Expected scroll"),
        }
    }

    #[test]
    fn test_handle_recorder_event_scroll_no_debounce_when_separated() {
        let mut rt = make_test_runtime();
        rt.handle_recorder_event(r#"{"type":"scroll","scroll_x":0,"scroll_y":100,"timestamp":1000.0}"#);
        rt.handle_recorder_event(r#"{"type":"scroll","scroll_x":0,"scroll_y":200,"timestamp":1500.0}"#);
        assert_eq!(rt.action_buffer.len(), 2);
    }

    #[test]
    fn test_handle_recorder_event_password_masking() {
        let mut rt = make_test_runtime();
        rt.mask_passwords = true;
        rt.handle_recorder_event(r##"{"type":"input","selector":"#pass","value":"secret","masked":true,"timestamp":1000.0}"##);
        assert_eq!(rt.action_buffer.len(), 1);
        match &rt.action_buffer[0] {
            RecorderEvent::Input { value, masked, .. } => {
                assert_eq!(value, "******");
                assert!(*masked);
            }
            _ => panic!("Expected input"),
        }
    }

    #[test]
    fn test_handle_recorder_event_no_masking_when_disabled() {
        let mut rt = make_test_runtime();
        rt.mask_passwords = false;
        rt.handle_recorder_event(r##"{"type":"input","selector":"#pass","value":"secret","masked":true,"timestamp":1000.0}"##);
        assert_eq!(rt.action_buffer.len(), 1);
        match &rt.action_buffer[0] {
            RecorderEvent::Input { value, .. } => assert_eq!(value, "secret"),
            _ => panic!("Expected input"),
        }
    }

    #[test]
    fn test_handle_recorder_event_mutation_goes_to_mutation_buffer() {
        let mut rt = make_test_runtime();
        rt.handle_recorder_event(r#"{"type":"mutation_summary","added":3,"removed":1,"changed":2,"timestamp":1000.0}"#);
        assert!(rt.action_buffer.is_empty());
        assert_eq!(rt.mutation_summary.len(), 1);
        assert_eq!(rt.mutation_summary[0].added, 3);
    }

    #[test]
    fn test_handle_recorder_event_invalid_json() {
        let mut rt = make_test_runtime();
        rt.handle_recorder_event("not valid json");
        assert!(rt.action_buffer.is_empty());
    }

    #[test]
    fn test_handle_console_event() {
        let mut rt = make_test_runtime();
        rt.handle_console_event(1000.0, "error".to_string(), "Uncaught TypeError".to_string());
        assert_eq!(rt.console_buffer.len(), 1);
        assert_eq!(rt.console_buffer[0].level, "error");
    }

    #[test]
    fn test_handle_network_request_filters() {
        let mut rt = make_test_runtime();
        rt.handle_network_request(1.0, "GET".to_string(), "https://example.com".to_string(), "Document".to_string());
        rt.handle_network_request(2.0, "GET".to_string(), "https://example.com/img.png".to_string(), "Image".to_string());
        rt.handle_network_request(3.0, "POST".to_string(), "https://api.example.com".to_string(), "XHR".to_string());
        rt.handle_network_request(4.0, "POST".to_string(), "https://api.example.com/v2".to_string(), "Fetch".to_string());
        assert_eq!(rt.network_buffer.len(), 3);
    }

    #[test]
    fn test_handle_network_response_updates_status() {
        let mut rt = make_test_runtime();
        rt.handle_network_request(1.0, "GET".to_string(), "https://example.com".to_string(), "Document".to_string());
        assert!(rt.network_buffer[0].status.is_none());
        rt.handle_network_response("https://example.com", 200);
        assert_eq!(rt.network_buffer[0].status, Some(200));
    }

    #[test]
    fn test_buffer_enforcement_on_action() {
        let mut rt = make_test_runtime();
        for i in 0..MAX_BUFFER_SIZE + 500 {
            rt.handle_recorder_event(&format!(
                r##"{{"type":"click","selector":"#btn","text":"OK","x":{},"y":0,"timestamp":{}}}"##,
                i, i
            ));
        }
        assert_eq!(rt.action_buffer.len(), MAX_BUFFER_SIZE);
    }

    #[test]
    fn test_flush_action_buffer() {
        let mut rt = make_test_runtime();
        rt.handle_recorder_event(r##"{"type":"click","selector":"#a","text":"A","x":0,"y":0,"timestamp":1.0}"##);
        rt.handle_recorder_event(r##"{"type":"click","selector":"#b","text":"B","x":0,"y":0,"timestamp":2.0}"##);
        let flushed = rt.flush_action_buffer();
        assert_eq!(flushed.len(), 2);
        let flushed2 = rt.flush_action_buffer();
        assert_eq!(flushed2.len(), 0);
    }

    #[test]
    fn test_flush_console_buffer() {
        let mut rt = make_test_runtime();
        rt.handle_console_event(1.0, "log".to_string(), "hello".to_string());
        let flushed = rt.flush_console_buffer();
        assert_eq!(flushed.len(), 1);
        let flushed2 = rt.flush_console_buffer();
        assert_eq!(flushed2.len(), 0);
    }

    #[test]
    fn test_flush_network_buffer() {
        let mut rt = make_test_runtime();
        rt.handle_network_request(1.0, "GET".to_string(), "https://example.com".to_string(), "Document".to_string());
        let flushed = rt.flush_network_buffer();
        assert_eq!(flushed.len(), 1);
        let flushed2 = rt.flush_network_buffer();
        assert_eq!(flushed2.len(), 0);
    }

    #[test]
    fn test_flush_mutation_summary() {
        let mut rt = make_test_runtime();
        rt.handle_recorder_event(r#"{"type":"mutation_summary","added":1,"removed":0,"changed":0,"timestamp":1.0}"#);
        let flushed = rt.flush_mutation_summary();
        assert_eq!(flushed.len(), 1);
        let flushed2 = rt.flush_mutation_summary();
        assert_eq!(flushed2.len(), 0);
    }

    #[tokio::test]
    async fn test_register_and_get_browser_runtime() {
        let gcx = crate::global_context::tests::make_test_gcx().await;
        let result = get_or_create_browser_runtime(gcx.clone(), "nonexistent").await;
        assert!(result.is_none());
    }

    fn make_test_runtime() -> BrowserRuntime {
        BrowserRuntime {
            runtime_id: "test-runtime".to_string(),
            attached_chat_id: None,
            browser: unsafe { std::mem::zeroed() },
            tabs: HashMap::new(),
            profile_dir: PathBuf::from("/tmp/test"),
            window_bounds: None,
            action_buffer: Vec::new(),
            console_buffer: Vec::new(),
            network_buffer: Vec::new(),
            mutation_summary: Vec::new(),
            last_send_action_cursor: 0,
            last_send_console_cursor: 0,
            last_send_network_cursor: 0,
            last_send_mutation_cursor: 0,
            last_frame_hash: None,
            last_frame_data: None,
            idle_timeout: Duration::from_secs(600),
            is_connected: true,
            last_activity: Instant::now(),
            mask_passwords: true,
        }
    }
}
