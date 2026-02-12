use gtk::prelude::*;
use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

use crate::config::types::{TaskbarConfig, TaskbarDisplay};

/// Info about a toplevel window
#[derive(Debug, Clone)]
pub struct ToplevelInfo {
    pub id: u32,
    pub app_id: String,
    pub title: String,
    pub focused: bool,
}

/// Events from the Wayland thread to GTK
#[derive(Debug)]
enum ToplevelEvent {
    New(ToplevelInfo),
    Update(ToplevelInfo),
    Remove(u32),
}

/// Requests from GTK to the Wayland thread
#[derive(Debug)]
enum ToplevelRequest {
    Activate(u32),
    Close(u32),
}

/// Cache of app_id -> icon name mappings from .desktop files
fn desktop_icon_cache() -> &'static std::sync::Mutex<HashMap<String, String>> {
    static CACHE: OnceLock<std::sync::Mutex<HashMap<String, String>>> = OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Resolve an app_id to an icon name, checking .desktop files
fn resolve_icon_name(app_id: &str) -> String {
    // Check cache first
    {
        let cache = desktop_icon_cache().lock().unwrap();
        if let Some(icon) = cache.get(app_id) {
            return icon.clone();
        }
    }

    // Search .desktop files for this app_id
    let icon = find_desktop_icon(app_id).unwrap_or_else(|| app_id.to_string());

    // Cache the result
    {
        let mut cache = desktop_icon_cache().lock().unwrap();
        cache.insert(app_id.to_string(), icon.clone());
    }

    icon
}

/// Search XDG data directories for a .desktop file matching app_id
fn find_desktop_icon(app_id: &str) -> Option<String> {
    let data_dirs = get_xdg_data_dirs();
    let app_id_lower = app_id.to_lowercase();

    // Candidates to search for: exact match, lowercased, with dots (flatpak-style)
    let candidates: Vec<String> = vec![
        format!("{app_id}.desktop"),
        format!("{app_id_lower}.desktop"),
    ];

    for dir in &data_dirs {
        let apps_dir = std::path::Path::new(dir).join("applications");
        if !apps_dir.is_dir() {
            continue;
        }

        // Try exact filename matches first
        for candidate in &candidates {
            let path = apps_dir.join(candidate);
            if let Some(icon) = read_desktop_icon(&path) {
                return Some(icon);
            }
        }

        // Scan directory for partial matches (handles org.foo.AppName.desktop)
        if let Ok(entries) = std::fs::read_dir(&apps_dir) {
            for entry in entries.flatten() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if !fname.ends_with(".desktop") {
                    continue;
                }
                let stem = fname.trim_end_matches(".desktop").to_lowercase();
                // Match if stem ends with the app_id (e.g. org.zen_browser.zen-beta ends with zen-beta)
                if stem == app_id_lower || stem.ends_with(&format!(".{app_id_lower}")) {
                    if let Some(icon) = read_desktop_icon(&entry.path()) {
                        return Some(icon);
                    }
                }
            }
        }
    }

    None
}

/// Read the Icon= field from a .desktop file
fn read_desktop_icon(path: &std::path::Path) -> Option<String> {
    let contents = std::fs::read_to_string(path).ok()?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if let Some(icon) = trimmed.strip_prefix("Icon=") {
            let icon = icon.trim();
            if !icon.is_empty() {
                return Some(icon.to_string());
            }
        }
    }
    None
}

fn get_xdg_data_dirs() -> Vec<String> {
    let mut dirs = Vec::new();

    // User data dir
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(format!("{home}/.local/share"));
    }
    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        dirs.push(data_home);
    }

    // System data dirs
    if let Ok(data_dirs) = std::env::var("XDG_DATA_DIRS") {
        dirs.extend(data_dirs.split(':').map(String::from));
    } else {
        dirs.push("/usr/local/share".to_string());
        dirs.push("/usr/share".to_string());
    }

    // Nix profile paths
    if let Ok(home) = std::env::var("HOME") {
        dirs.push(format!("{home}/.nix-profile/share"));
    }
    dirs.push("/run/current-system/sw/share".to_string());
    if let Ok(user) = std::env::var("USER") {
        dirs.push(format!("/etc/profiles/per-user/{user}/share"));
    }

    dirs
}

/// Build the button content (icon, title, or both) for a taskbar entry
fn make_button_content(
    app_id: &str,
    title: &str,
    display: &TaskbarDisplay,
    icon_size: i32,
    max_title: usize,
) -> gtk::Widget {
    let icon_name = resolve_icon_name(app_id);
    match display {
        TaskbarDisplay::Icon => {
            let image = gtk::Image::from_icon_name(&icon_name);
            image.set_pixel_size(icon_size);
            image.upcast()
        }
        TaskbarDisplay::Title => {
            let label = gtk::Label::new(Some(&truncate_title(title, max_title)));
            label.upcast()
        }
        TaskbarDisplay::Both => {
            let hbox = gtk::Box::new(gtk::Orientation::Horizontal, 4);
            let image = gtk::Image::from_icon_name(&icon_name);
            image.set_pixel_size(icon_size);
            let label = gtk::Label::new(Some(&truncate_title(title, max_title)));
            hbox.append(&image);
            hbox.append(&label);
            hbox.upcast()
        }
    }
}

/// Update button content on toplevel change
fn update_button_content(
    button: &gtk::Button,
    app_id: &str,
    title: &str,
    display: &TaskbarDisplay,
    icon_size: i32,
    max_title: usize,
) {
    let content = make_button_content(app_id, title, display, icon_size, max_title);
    button.set_child(Some(&content));
}

pub fn build(config: &TaskbarConfig) -> gtk::Widget {
    let (event_tx, event_rx) = mpsc::channel::<ToplevelEvent>(32);
    let (request_tx, request_rx) = mpsc::channel::<ToplevelRequest>(16);

    let max_title = config.max_title_length;
    let icon_size = config.icon_size;
    let display = config.display.clone();

    // Spawn the Wayland toplevel watcher on a blocking thread
    crate::spawn(async move {
        // Run on a blocking thread since Wayland needs its own event loop
        let result = tokio::task::spawn_blocking(move || {
            if let Err(e) = run_toplevel_watcher(event_tx, request_rx) {
                error!("Toplevel watcher failed: {e}");
            }
        })
        .await;

        if let Err(e) = result {
            error!("Toplevel watcher task panicked: {e}");
        }
    });

    // Build widget
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    container.add_css_class("module");
    container.add_css_class("taskbar");

    // Track buttons by toplevel ID
    let buttons: std::rc::Rc<std::cell::RefCell<HashMap<u32, gtk::Button>>> =
        std::rc::Rc::new(std::cell::RefCell::new(HashMap::new()));

    let container_ref = container.clone();
    let buttons_ref = buttons.clone();

    super::recv_on_main_thread(event_rx, move |event| match event {
        ToplevelEvent::New(info) => {
            let button = gtk::Button::new();
            let content = make_button_content(
                &info.app_id, &info.title, &display, icon_size, max_title,
            );
            button.set_child(Some(&content));
            button.add_css_class("taskbar-button");
            if info.focused {
                button.add_css_class("active");
            }
            button.set_tooltip_text(Some(&format!("{} - {}", info.app_id, info.title)));

            // Left click: activate
            let tx = request_tx.clone();
            let id = info.id;
            button.connect_clicked(move |_| {
                let tx = tx.clone();
                glib::spawn_future_local(async move {
                    let _ = tx.send(ToplevelRequest::Activate(id)).await;
                });
            });

            // Middle click: close
            let gesture = gtk::GestureClick::builder().button(2).build();
            let tx2 = request_tx.clone();
            let id2 = info.id;
            gesture.connect_released(move |_, _, _, _| {
                let tx = tx2.clone();
                glib::spawn_future_local(async move {
                    let _ = tx.send(ToplevelRequest::Close(id2)).await;
                });
            });
            button.add_controller(gesture);

            container_ref.append(&button);
            buttons_ref.borrow_mut().insert(info.id, button);
        }
        ToplevelEvent::Update(info) => {
            if let Some(button) = buttons_ref.borrow().get(&info.id) {
                update_button_content(
                    button, &info.app_id, &info.title, &display, icon_size, max_title,
                );
                button.set_tooltip_text(Some(&format!("{} - {}", info.app_id, info.title)));

                if info.focused {
                    button.add_css_class("active");
                } else {
                    button.remove_css_class("active");
                }
            }
        }
        ToplevelEvent::Remove(id) => {
            if let Some(button) = buttons_ref.borrow_mut().remove(&id) {
                container_ref.remove(&button);
            }
        }
    });

    debug!("Taskbar module created");
    container.upcast()
}

fn truncate_title(title: &str, max_len: usize) -> String {
    if title.len() <= max_len {
        title.to_string()
    } else {
        format!("{}...", &title[..max_len.saturating_sub(3)])
    }
}

// ---- Wayland toplevel watcher (runs on blocking thread) ----

use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_registry;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1::{self, ZwlrForeignToplevelHandleV1},
    zwlr_foreign_toplevel_manager_v1::{self, ZwlrForeignToplevelManagerV1},
};

/// Internal state for the Wayland event loop
struct WaylandState {
    event_tx: mpsc::Sender<ToplevelEvent>,
    request_rx: mpsc::Receiver<ToplevelRequest>,
    handles: Vec<HandleState>,
    seat: Option<WlSeat>,
    next_id: u32,
}

struct HandleState {
    id: u32,
    handle: ZwlrForeignToplevelHandleV1,
    pending_title: String,
    pending_app_id: String,
    pending_focused: bool,
    initial_done: bool,
}

fn run_toplevel_watcher(
    event_tx: mpsc::Sender<ToplevelEvent>,
    request_rx: mpsc::Receiver<ToplevelRequest>,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<WaylandState>(&conn)?;
    let qh = queue.handle();

    // Bind to toplevel manager
    let _manager: ZwlrForeignToplevelManagerV1 = globals.bind(&qh, 1..=3, ())?;

    // Bind to seat (needed for activate)
    let seat: WlSeat = globals.bind(&qh, 1..=9, ())?;

    let mut state = WaylandState {
        event_tx,
        request_rx,
        handles: Vec::new(),
        seat: Some(seat),
        next_id: 1,
    };

    // Initial roundtrip to get existing toplevels
    queue.roundtrip(&mut state)?;

    // Event loop
    loop {
        // Process any pending requests from GTK thread (non-blocking)
        while let Ok(request) = state.request_rx.try_recv() {
            match request {
                ToplevelRequest::Activate(id) => {
                    if let Some(hs) = state.handles.iter().find(|h| h.id == id) {
                        if let Some(ref seat) = state.seat {
                            hs.handle.activate(seat);
                        }
                    }
                }
                ToplevelRequest::Close(id) => {
                    if let Some(hs) = state.handles.iter().find(|h| h.id == id) {
                        hs.handle.close();
                    }
                }
            }
        }

        // Flush outgoing requests
        conn.flush()?;

        // Block for next Wayland event (with timeout for request processing)
        queue.blocking_dispatch(&mut state)?;
    }
}

// ---- Wayland dispatch implementations ----

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &wl_registry::WlRegistry,
        _event: wl_registry::Event,
        _data: &GlobalListContents,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // Registry events handled by GlobalListContents
    }
}

delegate_noop!(WaylandState: ignore WlSeat);

impl Dispatch<ZwlrForeignToplevelManagerV1, ()> for WaylandState {
    fn event(
        _state: &mut Self,
        _proxy: &ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel: _ } => {
                // New toplevel handle - events will arrive via handle dispatch
                debug!("New toplevel handle advertised");
            }
            zwlr_foreign_toplevel_manager_v1::Event::Finished => {
                warn!("Toplevel manager finished");
            }
            _ => {}
        }
    }

    wayland_client::event_created_child!(WaylandState, ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE =>
            (ZwlrForeignToplevelHandleV1, ())
    ]);
}

impl Dispatch<ZwlrForeignToplevelHandleV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        handle: &ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        const STATE_ACTIVATED: u32 = 2;

        // Find or create handle state
        let hs_idx = state.handles.iter().position(|h| h.handle == *handle);

        match event {
            zwlr_foreign_toplevel_handle_v1::Event::Title { title } => {
                if let Some(idx) = hs_idx {
                    state.handles[idx].pending_title = title;
                } else {
                    // New handle, create state
                    let id = state.next_id;
                    state.next_id += 1;
                    state.handles.push(HandleState {
                        id,
                        handle: handle.clone(),
                        pending_title: title,
                        pending_app_id: String::new(),
                        pending_focused: false,
                        initial_done: false,
                    });
                }
            }
            zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => {
                if let Some(idx) = hs_idx {
                    state.handles[idx].pending_app_id = app_id;
                } else {
                    let id = state.next_id;
                    state.next_id += 1;
                    state.handles.push(HandleState {
                        id,
                        handle: handle.clone(),
                        pending_title: String::new(),
                        pending_app_id: app_id,
                        pending_focused: false,
                        initial_done: false,
                    });
                }
            }
            zwlr_foreign_toplevel_handle_v1::Event::State { state: wl_state } => {
                let focused = if wl_state.len() >= 4 {
                    // Parse u32 array from raw bytes
                    (0..wl_state.len() / 4).any(|i| {
                        let bytes: [u8; 4] = wl_state[i * 4..i * 4 + 4]
                            .try_into()
                            .unwrap_or([0; 4]);
                        u32::from_le_bytes(bytes) == STATE_ACTIVATED
                    })
                } else {
                    false
                };

                if let Some(idx) = state
                    .handles
                    .iter()
                    .position(|h| h.handle == *handle)
                {
                    state.handles[idx].pending_focused = focused;
                }
            }
            zwlr_foreign_toplevel_handle_v1::Event::Done => {
                if let Some(idx) = state
                    .handles
                    .iter()
                    .position(|h| h.handle == *handle)
                {
                    let hs = &mut state.handles[idx];
                    let info = ToplevelInfo {
                        id: hs.id,
                        app_id: hs.pending_app_id.clone(),
                        title: hs.pending_title.clone(),
                        focused: hs.pending_focused,
                    };

                    if hs.initial_done {
                        // Update existing
                        let _ = state.event_tx.blocking_send(ToplevelEvent::Update(info));
                    } else {
                        // Skip empty app_id (XWayland dialogs)
                        if !info.app_id.is_empty() {
                            hs.initial_done = true;
                            let _ = state.event_tx.blocking_send(ToplevelEvent::New(info));
                        }
                    }
                }
            }
            zwlr_foreign_toplevel_handle_v1::Event::Closed => {
                if let Some(idx) = state
                    .handles
                    .iter()
                    .position(|h| h.handle == *handle)
                {
                    let id = state.handles[idx].id;
                    state.handles.remove(idx);
                    let _ = state.event_tx.blocking_send(ToplevelEvent::Remove(id));
                }
            }
            _ => {}
        }
    }
}
