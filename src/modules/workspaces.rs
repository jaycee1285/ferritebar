use gtk::prelude::*;
use std::cell::Cell;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

use crate::config::types::WorkspacesConfig;

#[derive(Debug, Clone)]
struct WorkspaceInfo {
    id: u64,
    name: String,
    index: u32,
    group: u32,
    active: bool,
    urgent: bool,
    hidden: bool,
}

#[derive(Debug)]
enum WorkspaceEvent {
    Snapshot(Vec<WorkspaceInfo>),
    Unavailable(String),
}

#[derive(Debug)]
enum WorkspaceRequest {
    Activate(u64),
}

pub fn build(config: &WorkspacesConfig) -> gtk::Widget {
    let (event_tx, event_rx) = mpsc::channel::<WorkspaceEvent>(8);
    let (request_tx, request_rx) = mpsc::channel::<WorkspaceRequest>(8);

    // Spawn the Wayland workspace watcher on a blocking thread
    crate::spawn(async move {
        let result = tokio::task::spawn_blocking(move || {
            if let Err(e) = run_workspace_watcher(event_tx, request_rx) {
                error!("Workspace watcher failed: {e}");
            }
        })
        .await;

        if let Err(e) = result {
            error!("Workspace watcher task panicked: {e}");
        }
    });

    let container = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    container.add_css_class("module");
    container.add_css_class("workspaces");

    let format = config.format.clone();
    let show_hidden = config.show_hidden;
    let enable_scroll = config.scroll;
    let sync_command = config.sync_command.clone();
    let sync_only_active = config.sync_only_active;
    let last_synced: std::rc::Rc<Cell<Option<u64>>> = std::rc::Rc::new(Cell::new(None));

    let entries: std::rc::Rc<std::cell::RefCell<Vec<WorkspaceInfo>>> =
        std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));

    let container_ref = container.clone();
    let entries_ref = entries.clone();
    let request_tx_ref = request_tx.clone();
    let last_synced_ref = last_synced.clone();
    let sync_command_ref = sync_command.clone();

    super::recv_on_main_thread(event_rx, move |event| match event {
        WorkspaceEvent::Unavailable(reason) => {
            container_ref.remove_css_class("active");
            while let Some(child) = container_ref.first_child() {
                container_ref.remove(&child);
            }
            let label = gtk::Label::new(Some("WS"));
            label.add_css_class("module-label");
            container_ref.append(&label);
            super::set_tooltip_text(container_ref.clone(), Some(&reason));
        }
        WorkspaceEvent::Snapshot(list) => {
            let active_ws = if sync_only_active {
                list.iter().find(|w| w.active).cloned()
            } else {
                None
            };

            let mut visible = Vec::new();
            for info in list.into_iter() {
                if !show_hidden && info.hidden {
                    continue;
                }
                visible.push(info);
            }

            *entries_ref.borrow_mut() = visible.clone();

            while let Some(child) = container_ref.first_child() {
                container_ref.remove(&child);
            }

            // Auto-hide when only 1 workspace exists
            container_ref.set_visible(visible.len() > 1);

            for info in visible {
                let label_text = format_label(&format, &info);
                let label = gtk::Label::new(Some(&label_text));
                label.add_css_class("module-label");

                let button = gtk::Button::new();
                button.set_child(Some(&label));
                button.add_css_class("workspace-button");

                if info.active {
                    button.add_css_class("active");
                }
                if info.urgent {
                    button.add_css_class("urgent");
                }
                if info.hidden {
                    button.add_css_class("hidden");
                }

                let id = info.id;
                let tx = request_tx_ref.clone();
                button.connect_clicked(move |_| {
                    let tx = tx.clone();
                    glib::spawn_future_local(async move {
                        let _ = tx.send(WorkspaceRequest::Activate(id)).await;
                    });
                });

                container_ref.append(&button);
            }

            if let (Some(cmd_template), Some(active)) = (sync_command_ref.as_ref(), active_ws) {
                if last_synced_ref.get() != Some(active.id) {
                    let cmd = format_command(cmd_template, &active);
                    last_synced_ref.set(Some(active.id));
                    crate::spawn(async move {
                        let _ = tokio::process::Command::new("sh")
                            .arg("-lc")
                            .arg(&cmd)
                            .spawn();
                    });
                }
            }
        }
    });

    if enable_scroll {
        let entries_ref = entries.clone();
        let tx = request_tx.clone();
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll.connect_scroll(move |_, _dx, dy| {
            let entries = entries_ref.borrow();
            if entries.is_empty() {
                return glib::Propagation::Proceed;
            }
            let active_idx = entries
                .iter()
                .position(|w| w.active)
                .unwrap_or(0);
            let next_idx = if dy > 0.0 {
                (active_idx + 1) % entries.len()
            } else {
                (active_idx + entries.len() - 1) % entries.len()
            };
            let id = entries[next_idx].id;
            let tx = tx.clone();
            glib::spawn_future_local(async move {
                let _ = tx.send(WorkspaceRequest::Activate(id)).await;
            });
            glib::Propagation::Stop
        });
        container.add_controller(scroll);
    }

    debug!("Workspaces module created");
    container.upcast()
}

fn format_label(format: &str, info: &WorkspaceInfo) -> String {
    let mut out = String::with_capacity(format.len());
    for part in format.split('{') {
        if let Some(rest) = part.strip_prefix("name}") {
            if info.name.is_empty() {
                out.push_str(&info.index.to_string());
            } else {
                out.push_str(&info.name);
            }
            out.push_str(rest);
        } else if let Some(rest) = part.strip_prefix("index}") {
            out.push_str(&info.index.to_string());
            out.push_str(rest);
        } else if let Some(rest) = part.strip_prefix("group}") {
            out.push_str(&info.group.to_string());
            out.push_str(rest);
        } else {
            out.push_str(part);
        }
    }

    if out.is_empty() {
        if info.name.is_empty() {
            info.index.to_string()
        } else {
            info.name.clone()
        }
    } else {
        out
    }
}

fn format_command(format: &str, info: &WorkspaceInfo) -> String {
    let mut out = String::with_capacity(format.len());
    for part in format.split('{') {
        if let Some(rest) = part.strip_prefix("name}") {
            if info.name.is_empty() {
                out.push_str(&info.index.to_string());
            } else {
                out.push_str(&info.name);
            }
            out.push_str(rest);
        } else if let Some(rest) = part.strip_prefix("index}") {
            out.push_str(&info.index.to_string());
            out.push_str(rest);
        } else if let Some(rest) = part.strip_prefix("group}") {
            out.push_str(&info.group.to_string());
            out.push_str(rest);
        } else {
            out.push_str(part);
        }
    }
    out
}

// ---- Wayland workspace watcher (runs on blocking thread) ----

use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_registry;
use wayland_client::{Connection, Dispatch, QueueHandle, delegate_noop};
use wayland_protocols::ext::workspace::v1::client::{
    ext_workspace_group_handle_v1::{self, ExtWorkspaceGroupHandleV1},
    ext_workspace_handle_v1::{self, ExtWorkspaceHandleV1},
    ext_workspace_manager_v1::{self, ExtWorkspaceManagerV1},
};

struct WorkspaceState {
    internal_id: u64,
    handle: ExtWorkspaceHandleV1,
    name: String,
    id: Option<String>,
    coords: Vec<u32>,
    state: u32,
    capabilities: u32,
    group_id: Option<u32>,
    removed: bool,
    serial: u64,
}

struct WorkspaceGroupState {
    id: u32,
    handle: ExtWorkspaceGroupHandleV1,
    outputs: Vec<WlOutput>,
    removed: bool,
    capabilities: u32,
}

/// Internal state for the Wayland event loop
struct WaylandState {
    event_tx: mpsc::Sender<WorkspaceEvent>,
    request_rx: mpsc::Receiver<WorkspaceRequest>,
    manager: Option<ExtWorkspaceManagerV1>,
    workspaces: Vec<WorkspaceState>,
    groups: Vec<WorkspaceGroupState>,
    next_workspace_id: u64,
    next_group_id: u32,
    next_serial: u64,
    finished: bool,
}

const STATE_ACTIVE: u32 = 1;
const STATE_URGENT: u32 = 2;
const STATE_HIDDEN: u32 = 4;

fn run_workspace_watcher(
    event_tx: mpsc::Sender<WorkspaceEvent>,
    request_rx: mpsc::Receiver<WorkspaceRequest>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::io::{AsFd, AsRawFd};

    let conn = Connection::connect_to_env()?;
    let (globals, mut queue) = registry_queue_init::<WaylandState>(&conn)?;
    let qh = queue.handle();

    let manager: ExtWorkspaceManagerV1 = match globals.bind(&qh, 1..=1, ()) {
        Ok(manager) => manager,
        Err(_) => {
            let _ = event_tx.blocking_send(WorkspaceEvent::Unavailable(
                "ext_workspace_v1 not supported by compositor".to_string(),
            ));
            return Ok(());
        }
    };

    let mut state = WaylandState {
        event_tx,
        request_rx,
        manager: Some(manager),
        workspaces: Vec::new(),
        groups: Vec::new(),
        next_workspace_id: 1,
        next_group_id: 1,
        next_serial: 1,
        finished: false,
    };

    // Initial roundtrip to get existing workspaces
    queue.roundtrip(&mut state)?;

    let raw_fd = conn.as_fd().as_raw_fd();

    loop {
        let mut needs_commit = false;

        while let Ok(request) = state.request_rx.try_recv() {
            match request {
                WorkspaceRequest::Activate(id) => {
                    if let Some(ws) = state.workspaces.iter().find(|w| w.internal_id == id) {
                        if (ws.capabilities & 1) != 0 {
                            ws.handle.activate();
                            needs_commit = true;
                        }
                    }
                }
            }
        }

        if needs_commit {
            if let Some(ref manager) = state.manager {
                manager.commit();
            }
        }

        conn.flush()?;
        queue.dispatch_pending(&mut state)?;

        if state.finished {
            break;
        }

        if let Some(guard) = queue.prepare_read() {
            if poll_readable(raw_fd, 50) {
                guard.read()?;
            }
        }

        queue.dispatch_pending(&mut state)?;
    }

    Ok(())
}

/// Poll a file descriptor for readability with a timeout.
fn poll_readable(raw_fd: i32, timeout_ms: i32) -> bool {
    use std::os::raw::{c_int, c_short};

    #[repr(C)]
    struct PollFd {
        fd: c_int,
        events: c_short,
        revents: c_short,
    }

    extern "C" {
        fn poll(fds: *mut PollFd, nfds: u64, timeout: c_int) -> c_int;
    }

    const POLLIN: c_short = 0x001;
    let mut pfd = PollFd {
        fd: raw_fd,
        events: POLLIN,
        revents: 0,
    };
    unsafe { poll(&mut pfd, 1, timeout_ms) > 0 }
}

fn parse_u32_array(data: &[u8]) -> Vec<u32> {
    let mut out = Vec::new();
    let mut idx = 0;
    while idx + 4 <= data.len() {
        let bytes: [u8; 4] = data[idx..idx + 4].try_into().unwrap_or([0; 4]);
        out.push(u32::from_ne_bytes(bytes));
        idx += 4;
    }
    out
}

fn emit_snapshot(state: &mut WaylandState) {
    let mut grouped: std::collections::BTreeMap<u32, Vec<&WorkspaceState>> =
        std::collections::BTreeMap::new();

    for ws in &state.workspaces {
        if ws.removed {
            continue;
        }
        let group_id = ws.group_id.unwrap_or(0);
        grouped.entry(group_id).or_default().push(ws);
    }

    let mut snapshot = Vec::new();

    for (group_id, mut list) in grouped {
        list.sort_by(|a, b| {
            if !a.coords.is_empty() || !b.coords.is_empty() {
                match a.coords.cmp(&b.coords) {
                    std::cmp::Ordering::Equal => a.serial.cmp(&b.serial),
                    other => other,
                }
            } else {
                a.serial.cmp(&b.serial)
            }
        });

        for (idx, ws) in list.iter().enumerate() {
            snapshot.push(WorkspaceInfo {
                id: ws.internal_id,
                name: ws.name.clone(),
                index: (idx + 1) as u32,
                group: group_id,
                active: (ws.state & STATE_ACTIVE) != 0,
                urgent: (ws.state & STATE_URGENT) != 0,
                hidden: (ws.state & STATE_HIDDEN) != 0,
            });
        }
    }

    state.workspaces.retain(|w| !w.removed);
    state.groups.retain(|g| !g.removed);

    let _ = state
        .event_tx
        .blocking_send(WorkspaceEvent::Snapshot(snapshot));
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

delegate_noop!(WaylandState: ignore WlOutput);

impl Dispatch<ExtWorkspaceManagerV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        _proxy: &ExtWorkspaceManagerV1,
        event: ext_workspace_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            ext_workspace_manager_v1::Event::WorkspaceGroup { workspace_group } => {
                let id = state.next_group_id;
                state.next_group_id += 1;
                state.groups.push(WorkspaceGroupState {
                    id,
                    handle: workspace_group,
                    outputs: Vec::new(),
                    removed: false,
                    capabilities: 0,
                });
            }
            ext_workspace_manager_v1::Event::Workspace { workspace } => {
                let id = state.next_workspace_id;
                state.next_workspace_id += 1;
                let serial = state.next_serial;
                state.next_serial += 1;
                state.workspaces.push(WorkspaceState {
                    internal_id: id,
                    handle: workspace,
                    name: String::new(),
                    id: None,
                    coords: Vec::new(),
                    state: 0,
                    capabilities: 0,
                    group_id: None,
                    removed: false,
                    serial,
                });
            }
            ext_workspace_manager_v1::Event::Done => {
                emit_snapshot(state);
            }
            ext_workspace_manager_v1::Event::Finished => {
                warn!("Workspace manager finished");
                state.finished = true;
            }
            _ => {}
        }
    }

    wayland_client::event_created_child!(WaylandState, ExtWorkspaceManagerV1, [
        ext_workspace_manager_v1::EVT_WORKSPACE_GROUP_OPCODE => (ExtWorkspaceGroupHandleV1, ()),
        ext_workspace_manager_v1::EVT_WORKSPACE_OPCODE => (ExtWorkspaceHandleV1, ())
    ]);
}

impl Dispatch<ExtWorkspaceGroupHandleV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        group: &ExtWorkspaceGroupHandleV1,
        event: ext_workspace_group_handle_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some(g_idx) = state.groups.iter().position(|g| g.handle == *group) else {
            return;
        };

        match event {
            ext_workspace_group_handle_v1::Event::Capabilities { capabilities } => {
                state.groups[g_idx].capabilities = capabilities.into();
            }
            ext_workspace_group_handle_v1::Event::OutputEnter { output } => {
                state.groups[g_idx].outputs.push(output);
            }
            ext_workspace_group_handle_v1::Event::OutputLeave { output } => {
                state.groups[g_idx]
                    .outputs
                    .retain(|o| o != &output);
            }
            ext_workspace_group_handle_v1::Event::WorkspaceEnter { workspace } => {
                if let Some(ws) = state.workspaces.iter_mut().find(|w| w.handle == workspace) {
                    ws.group_id = Some(state.groups[g_idx].id);
                }
            }
            ext_workspace_group_handle_v1::Event::WorkspaceLeave { workspace } => {
                if let Some(ws) = state.workspaces.iter_mut().find(|w| w.handle == workspace) {
                    ws.group_id = None;
                }
            }
            ext_workspace_group_handle_v1::Event::Removed => {
                state.groups[g_idx].removed = true;
            }
            _ => {}
        }
    }
}

impl Dispatch<ExtWorkspaceHandleV1, ()> for WaylandState {
    fn event(
        state: &mut Self,
        workspace: &ExtWorkspaceHandleV1,
        event: ext_workspace_handle_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let Some(ws_idx) = state.workspaces.iter().position(|w| w.handle == *workspace) else {
            return;
        };

        match event {
            ext_workspace_handle_v1::Event::Id { id } => {
                state.workspaces[ws_idx].id = Some(id);
            }
            ext_workspace_handle_v1::Event::Name { name } => {
                state.workspaces[ws_idx].name = name;
            }
            ext_workspace_handle_v1::Event::Coordinates { coordinates } => {
                state.workspaces[ws_idx].coords = parse_u32_array(&coordinates);
            }
            ext_workspace_handle_v1::Event::State { state: ws_state } => {
                state.workspaces[ws_idx].state = ws_state.into();
            }
            ext_workspace_handle_v1::Event::Capabilities { capabilities } => {
                state.workspaces[ws_idx].capabilities = capabilities.into();
            }
            ext_workspace_handle_v1::Event::Removed => {
                state.workspaces[ws_idx].removed = true;
            }
            _ => {}
        }
    }
}
