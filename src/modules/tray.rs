use gtk::prelude::*;
use gtk::gdk_pixbuf::{Colorspace, Pixbuf};
use gtk::gdk::Texture;
use gtk_layer_shell::LayerShell;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

use crate::config::types::{Position, TrayConfig};

#[derive(Debug)]
enum TrayUpdate {
    Add {
        address: String,
        icon_name: Option<String>,
        icon_pixmap: Option<Vec<system_tray::item::IconPixmap>>,
        icon_theme_path: Option<String>,
        title: Option<String>,
        menu: Option<system_tray::menu::TrayMenu>,
        menu_path: Option<String>,
    },
    UpdateIcon {
        address: String,
        icon_name: Option<String>,
        icon_pixmap: Option<Vec<system_tray::item::IconPixmap>>,
    },
    UpdateMenu {
        address: String,
        menu: system_tray::menu::TrayMenu,
    },
    UpdateMenuDiff {
        address: String,
        diffs: Vec<system_tray::menu::MenuDiff>,
    },
    Remove {
        address: String,
    },
}

#[derive(Debug)]
enum ActivateAction {
    Primary(String),
    MenuItem {
        address: String,
        menu_path: String,
        submenu_id: i32,
    },
}

struct TrayItem {
    image: gtk::Image,
    menu: Option<system_tray::menu::TrayMenu>,
    menu_path: Option<String>,
}

pub fn build(
    config: &TrayConfig,
    app: &gtk::Application,
    bar_position: Position,
    bar_height: u32,
    bar_edge_margin: i32,
) -> gtk::Widget {
    let (tx, rx) = mpsc::channel::<TrayUpdate>(32);
    let (activate_tx, mut activate_rx) = mpsc::channel::<ActivateAction>(16);
    let icon_size = config.icon_size;

    // Spawn tray client controller
    crate::spawn(async move {
        match system_tray::client::Client::new().await {
            Ok(client) => {
                let mut event_rx = client.subscribe();

                // Send initial items - collect under lock, then send
                let initial: Vec<TrayUpdate> = {
                    let items = client.items();
                    let items_lock = items.lock().unwrap();
                    items_lock
                        .iter()
                        .map(|(address, (item, menu))| TrayUpdate::Add {
                            address: address.clone(),
                            icon_name: item.icon_name.clone(),
                            icon_pixmap: item.icon_pixmap.clone(),
                            icon_theme_path: item
                                .icon_theme_path
                                .as_ref()
                                .map(|p| p.clone()),
                            title: item.title.clone(),
                            menu: menu.clone(),
                            menu_path: item.menu.clone(),
                        })
                        .collect()
                };
                for update in initial {
                    let _ = tx.send(update).await;
                }

                // Listen for events and activation requests
                loop {
                    tokio::select! {
                        event = event_rx.recv() => {
                            let event = match event {
                                Ok(event) => event,
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                    warn!("Tray receiver lagged, missed {n} events");
                                    continue;
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                            };
                            match event {
                                system_tray::client::Event::Add(address, item) => {
                                    let _ = tx
                                        .send(TrayUpdate::Add {
                                            address,
                                            icon_name: item.icon_name.clone(),
                                            icon_pixmap: item.icon_pixmap.clone(),
                                            icon_theme_path: item
                                                .icon_theme_path
                                                .as_ref()
                                                .map(|p| p.clone()),
                                            title: item.title.clone(),
                                            menu: None,
                                            menu_path: item.menu.clone(),
                                        })
                                        .await;
                                }
                                system_tray::client::Event::Update(address, update) => {
                                    use system_tray::client::UpdateEvent;
                                    match update {
                                        UpdateEvent::Icon {
                                            icon_name,
                                            icon_pixmap,
                                        } => {
                                            let _ = tx
                                                .send(TrayUpdate::UpdateIcon {
                                                    address,
                                                    icon_name,
                                                    icon_pixmap,
                                                })
                                                .await;
                                        }
                                        UpdateEvent::Menu(menu) => {
                                            let _ = tx
                                                .send(TrayUpdate::UpdateMenu { address, menu })
                                                .await;
                                        }
                                        UpdateEvent::MenuDiff(diffs) => {
                                            let _ = tx
                                                .send(TrayUpdate::UpdateMenuDiff { address, diffs })
                                                .await;
                                        }
                                        _ => {}
                                    }
                                }
                                system_tray::client::Event::Remove(address) => {
                                    let _ = tx.send(TrayUpdate::Remove { address }).await;
                                }
                            }
                        }
                        action = activate_rx.recv() => {
                            let Some(action) = action else { break };
                            let req = match action {
                                ActivateAction::Primary(address) => {
                                    system_tray::client::ActivateRequest::Default {
                                        address,
                                        x: 0,
                                        y: 0,
                                    }
                                }
                                ActivateAction::MenuItem { address, menu_path, submenu_id } => {
                                    system_tray::client::ActivateRequest::MenuItem {
                                        address,
                                        menu_path,
                                        submenu_id,
                                    }
                                }
                            };
                            if let Err(e) = client.activate(req).await {
                                debug!("Tray activate failed: {e}");
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to create tray client: {e}");
            }
        }
    });

    // Build widget (hidden until items arrive)
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    container.add_css_class("module");
    container.add_css_class("tray");
    container.set_visible(false);

    // Create the context menu popup window (reused for all tray items)
    let popup = create_popup_window(app, bar_position, bar_height, bar_edge_margin);
    let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    menu_box.add_css_class("tray-menu");
    popup.set_child(Some(&menu_box));
    popup.set_visible(false);

    // Dismiss on Escape
    let key_ctrl = gtk::EventControllerKey::new();
    let popup_esc = popup.clone();
    key_ctrl.connect_key_pressed(move |_, key, _, _| {
        if key == gtk::gdk::Key::Escape {
            popup_esc.set_visible(false);
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });
    popup.add_controller(key_ctrl);

    // Dismiss on focus loss (click outside)
    let popup_focus = popup.clone();
    popup.connect_is_active_notify(move |_| {
        // When window loses active status, hide it
        // Use idle callback to avoid re-entrancy issues
        let p = popup_focus.clone();
        glib::idle_add_local_once(move || {
            if !p.is_active() {
                p.set_visible(false);
            }
        });
    });

    // Track tray items by address
    let items: Rc<RefCell<HashMap<String, TrayItem>>> =
        Rc::new(RefCell::new(HashMap::new()));

    let container_ref = container.clone();
    let items_ref = items.clone();
    let popup_ref = popup.clone();
    let menu_box_ref = menu_box.clone();

    super::recv_on_main_thread(rx, move |update| match update {
        TrayUpdate::Add {
            address,
            icon_name,
            mut icon_pixmap,
            icon_theme_path,
            title,
            menu,
            menu_path,
        } => {
            let image = gtk::Image::new();
            image.set_pixel_size(icon_size);
            image.add_css_class("tray-icon");

            if let Some(ref name) = icon_name {
                if !name.is_empty() {
                    if let Some(ref path) = icon_theme_path {
                        if !path.is_empty() {
                            let icon_theme = gtk::IconTheme::for_display(
                                &gtk::gdk::Display::default().unwrap(),
                            );
                            icon_theme.add_search_path(path);
                        }
                    }
                    image.set_icon_name(Some(name));
                }
            } else if let Some(ref mut pixmaps) = icon_pixmap {
                if let Some(texture) = pixmap_to_texture(pixmaps, icon_size as u32) {
                    image.set_paintable(Some(&texture));
                }
            }

            if let Some(ref t) = title {
                super::set_tooltip_text(image.clone(), Some(t));
            }

            // Left-click: primary activate
            let left_click = gtk::GestureClick::new();
            left_click.set_button(1);
            let addr = address.clone();
            let tx = activate_tx.clone();
            left_click.connect_released(move |_, _, _, _| {
                let _ = tx.try_send(ActivateAction::Primary(addr.clone()));
            });
            image.add_controller(left_click);

            // Right-click: show context menu
            let right_click = gtk::GestureClick::new();
            right_click.set_button(3);
            let addr = address.clone();
            let items_rc = items_ref.clone();
            let popup_rc = popup_ref.clone();
            let menu_box_rc = menu_box_ref.clone();
            let tx = activate_tx.clone();
            right_click.connect_released(move |gesture, _, _, _| {
                let Some(widget) = gesture.widget() else { return };
                show_context_menu(
                    &popup_rc,
                    &menu_box_rc,
                    &items_rc,
                    &addr,
                    &widget,
                    &tx,
                );
            });
            image.add_controller(right_click);

            container_ref.append(&image);
            items_ref.borrow_mut().insert(address, TrayItem {
                image,
                menu,
                menu_path,
            });
            container_ref.set_visible(true);
        }
        TrayUpdate::UpdateIcon {
            address,
            icon_name,
            mut icon_pixmap,
        } => {
            if let Some(item) = items_ref.borrow().get(&address) {
                if let Some(ref name) = icon_name {
                    if !name.is_empty() {
                        item.image.set_icon_name(Some(name));
                        return;
                    }
                }
                if let Some(ref mut pixmaps) = icon_pixmap {
                    if let Some(texture) = pixmap_to_texture(pixmaps, icon_size as u32) {
                        item.image.set_paintable(Some(&texture));
                    }
                }
            }
        }
        TrayUpdate::UpdateMenu { address, menu } => {
            if let Some(item) = items_ref.borrow_mut().get_mut(&address) {
                item.menu = Some(menu);
            }
        }
        TrayUpdate::UpdateMenuDiff { address, diffs } => {
            if let Some(item) = items_ref.borrow_mut().get_mut(&address) {
                if let Some(ref mut menu) = item.menu {
                    system_tray::data::apply_menu_diffs(menu, &diffs);
                }
            }
        }
        TrayUpdate::Remove { address } => {
            if let Some(item) = items_ref.borrow_mut().remove(&address) {
                container_ref.remove(&item.image);
            }
            if items_ref.borrow().is_empty() {
                container_ref.set_visible(false);
            }
            // Hide popup if it was showing this item's menu
            popup_ref.set_visible(false);
        }
    });

    debug!("Tray module created");
    container.upcast()
}

fn create_popup_window(
    app: &gtk::Application,
    bar_position: Position,
    bar_height: u32,
    bar_edge_margin: i32,
) -> gtk::ApplicationWindow {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .default_width(200)
        .default_height(0)
        .build();

    window.init_layer_shell();
    window.set_layer(gtk_layer_shell::Layer::Overlay);
    window.set_namespace(Some("ferritebar-tray-menu"));
    window.set_keyboard_mode(gtk_layer_shell::KeyboardMode::OnDemand);

    // Anchor to bar edge + left (not right, so it doesn't stretch)
    match bar_position {
        Position::Top => {
            window.set_anchor(gtk_layer_shell::Edge::Top, true);
            window.set_anchor(gtk_layer_shell::Edge::Bottom, false);
            window.set_margin(
                gtk_layer_shell::Edge::Top,
                bar_height as i32 + bar_edge_margin,
            );
        }
        Position::Bottom => {
            window.set_anchor(gtk_layer_shell::Edge::Top, false);
            window.set_anchor(gtk_layer_shell::Edge::Bottom, true);
            window.set_margin(
                gtk_layer_shell::Edge::Bottom,
                bar_height as i32 + bar_edge_margin,
            );
        }
    }
    window.set_anchor(gtk_layer_shell::Edge::Left, true);
    window.set_anchor(gtk_layer_shell::Edge::Right, false);

    window
}

fn show_context_menu(
    popup: &gtk::ApplicationWindow,
    menu_box: &gtk::Box,
    items: &Rc<RefCell<HashMap<String, TrayItem>>>,
    address: &str,
    icon_widget: &gtk::Widget,
    activate_tx: &mpsc::Sender<ActivateAction>,
) {
    let items_borrow = items.borrow();
    let Some(tray_item) = items_borrow.get(address) else {
        return;
    };
    let Some(ref tray_menu) = tray_item.menu else {
        debug!("No menu data for tray item {address}");
        return;
    };
    let Some(ref menu_path) = tray_item.menu_path else {
        debug!("No menu path for tray item {address}");
        return;
    };

    // Clear previous menu contents
    while let Some(child) = menu_box.first_child() {
        menu_box.remove(&child);
    }

    // Build menu items from TrayMenu
    build_menu_items(
        menu_box,
        &tray_menu.submenus,
        address,
        menu_path,
        activate_tx,
        popup,
        0,
    );

    // Position popup at the icon's X coordinate
    if let Some(root) = icon_widget.root() {
        if let Some(point) = icon_widget.compute_point(
            &root.upcast::<gtk::Widget>(),
            &gtk::graphene::Point::new(0.0, 0.0),
        ) {
            popup.set_margin(gtk_layer_shell::Edge::Left, point.x() as i32);
        }
    }

    popup.present();
}

fn build_menu_items(
    container: &gtk::Box,
    items: &[system_tray::menu::MenuItem],
    address: &str,
    menu_path: &str,
    activate_tx: &mpsc::Sender<ActivateAction>,
    popup: &gtk::ApplicationWindow,
    depth: u32,
) {
    use system_tray::menu::{MenuType, ToggleType, ToggleState};

    for item in items {
        if !item.visible {
            continue;
        }

        match item.menu_type {
            MenuType::Separator => {
                let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
                container.append(&sep);
            }
            MenuType::Standard => {
                // If this item has children, render as a submenu header + children
                if !item.submenu.is_empty() {
                    if let Some(ref label) = item.label {
                        let header = gtk::Label::new(Some(&strip_underscores(label)));
                        header.set_halign(gtk::Align::Start);
                        header.add_css_class("submenu-header");
                        container.append(&header);
                    }
                    build_menu_items(
                        container,
                        &item.submenu,
                        address,
                        menu_path,
                        activate_tx,
                        popup,
                        depth + 1,
                    );
                } else {
                    // Leaf item — clickable button
                    let label_text = item.label.as_deref().unwrap_or("");
                    let display_text = format_menu_label(
                        &strip_underscores(label_text),
                        &item.toggle_type,
                        &item.toggle_state,
                    );

                    let btn = gtk::Button::with_label(&display_text);
                    btn.set_halign(gtk::Align::Fill);
                    btn.child()
                        .and_then(|c| c.downcast::<gtk::Label>().ok())
                        .map(|l| l.set_halign(gtk::Align::Start));

                    if !item.enabled {
                        btn.set_sensitive(false);
                    }

                    if depth > 0 {
                        btn.add_css_class("submenu-item");
                    }

                    if matches!(item.toggle_type, ToggleType::Checkmark | ToggleType::Radio)
                        && matches!(item.toggle_state, ToggleState::On)
                    {
                        btn.add_css_class("toggle-on");
                    }

                    let addr = address.to_string();
                    let mpath = menu_path.to_string();
                    let id = item.id;
                    let tx = activate_tx.clone();
                    let popup_ref = popup.clone();
                    btn.connect_clicked(move |_| {
                        debug!("Tray menu click: address={addr}, submenu_id={id}");
                        let _ = tx.try_send(ActivateAction::MenuItem {
                            address: addr.clone(),
                            menu_path: mpath.clone(),
                            submenu_id: id,
                        });
                        popup_ref.set_visible(false);
                    });

                    container.append(&btn);
                }
            }
        }
    }
}

/// Strip DBusMenu underscore mnemonics (e.g. "_File" -> "File")
fn strip_underscores(label: &str) -> String {
    label.replace('_', "")
}

/// Format menu label with toggle indicator
fn format_menu_label(
    label: &str,
    toggle_type: &system_tray::menu::ToggleType,
    toggle_state: &system_tray::menu::ToggleState,
) -> String {
    use system_tray::menu::{ToggleType, ToggleState};

    match toggle_type {
        ToggleType::Checkmark => {
            let check = match toggle_state {
                ToggleState::On => "\u{2713} ",
                _ => "  ",
            };
            format!("{check}{label}")
        }
        ToggleType::Radio => {
            let radio = match toggle_state {
                ToggleState::On => "\u{25cf} ",
                _ => "\u{25cb} ",
            };
            format!("{radio}{label}")
        }
        ToggleType::CannotBeToggled => label.to_string(),
    }
}

/// Convert ARGB32 pixmap to GTK Texture, taking ownership to avoid cloning pixel data
fn pixmap_to_texture(
    pixmaps: &mut Vec<system_tray::item::IconPixmap>,
    target_size: u32,
) -> Option<Texture> {
    // Find index of best size match
    let best_idx = pixmaps
        .iter()
        .enumerate()
        .filter(|(_, p)| p.width > 0 && p.height > 0)
        .min_by_key(|(_, p)| {
            let diff = (p.width as i32 - target_size as i32).abs();
            // Prefer sizes >= target
            if p.width as u32 >= target_size {
                diff
            } else {
                diff + 1000
            }
        })
        .map(|(i, _)| i)?;

    let pixmap = pixmaps.swap_remove(best_idx);

    if pixmap.pixels.is_empty() {
        return None;
    }

    // Convert ARGB32 to RGBA32 in-place (we own the data)
    let mut pixels = pixmap.pixels;
    for chunk in pixels.chunks_exact_mut(4) {
        let a = chunk[0];
        chunk[0] = chunk[1]; // R
        chunk[1] = chunk[2]; // G
        chunk[2] = chunk[3]; // B
        chunk[3] = a; // A
    }

    let row_stride = pixmap.width * 4;
    let bytes = glib::Bytes::from(&pixels);

    let pixbuf = Pixbuf::from_bytes(
        &bytes,
        Colorspace::Rgb,
        true, // has_alpha
        8,    // bits_per_sample
        pixmap.width,
        pixmap.height,
        row_stride,
    );

    Some(Texture::for_pixbuf(&pixbuf))
}
