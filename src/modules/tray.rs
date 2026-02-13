use gtk::prelude::*;
use gtk::gdk_pixbuf::{Colorspace, Pixbuf};
use gtk::gdk::Texture;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

use crate::config::types::TrayConfig;

#[derive(Debug)]
enum TrayUpdate {
    Add {
        address: String,
        icon_name: Option<String>,
        icon_pixmap: Option<Vec<system_tray::item::IconPixmap>>,
        icon_theme_path: Option<String>,
        title: Option<String>,
    },
    UpdateIcon {
        address: String,
        icon_name: Option<String>,
        icon_pixmap: Option<Vec<system_tray::item::IconPixmap>>,
    },
    Remove {
        address: String,
    },
}

#[derive(Debug)]
enum ActivateAction {
    Primary(String),
    Secondary(String),
}

pub fn build(config: &TrayConfig) -> gtk::Widget {
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
                        .map(|(address, (item, _menu))| TrayUpdate::Add {
                            address: address.clone(),
                            icon_name: item.icon_name.clone(),
                            icon_pixmap: item.icon_pixmap.clone(),
                            icon_theme_path: item
                                .icon_theme_path
                                .as_ref()
                                .map(|p| p.clone()),
                            title: item.title.clone(),
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
                                        })
                                        .await;
                                }
                                system_tray::client::Event::Update(address, update) => {
                                    use system_tray::client::UpdateEvent;
                                    if let UpdateEvent::Icon {
                                        icon_name,
                                        icon_pixmap,
                                    } = update
                                    {
                                        let _ = tx
                                            .send(TrayUpdate::UpdateIcon {
                                                address,
                                                icon_name,
                                                icon_pixmap,
                                            })
                                            .await;
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
                                ActivateAction::Secondary(address) => {
                                    system_tray::client::ActivateRequest::Secondary {
                                        address,
                                        x: 0,
                                        y: 0,
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

    // Track tray items by address -> widget
    let items: std::rc::Rc<std::cell::RefCell<std::collections::HashMap<String, gtk::Image>>> =
        std::rc::Rc::new(std::cell::RefCell::new(std::collections::HashMap::new()));

    let container_ref = container.clone();
    let items_ref = items.clone();

    super::recv_on_main_thread(rx, move |update| match update {
        TrayUpdate::Add {
            address,
            icon_name,
            mut icon_pixmap,
            icon_theme_path,
            title,
        } => {
            let image = gtk::Image::new();
            image.set_pixel_size(icon_size);
            image.add_css_class("tray-icon");

            if let Some(ref name) = icon_name {
                if !name.is_empty() {
                    // Add custom icon search path if provided
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
                image.set_tooltip_text(Some(t));
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

            // Right-click: secondary activate
            let right_click = gtk::GestureClick::new();
            right_click.set_button(3);
            let addr = address.clone();
            let tx = activate_tx.clone();
            right_click.connect_released(move |_, _, _, _| {
                let _ = tx.try_send(ActivateAction::Secondary(addr.clone()));
            });
            image.add_controller(right_click);

            container_ref.append(&image);
            items_ref.borrow_mut().insert(address, image);
            container_ref.set_visible(true);
        }
        TrayUpdate::UpdateIcon {
            address,
            icon_name,
            mut icon_pixmap,
        } => {
            if let Some(image) = items_ref.borrow().get(&address) {
                if let Some(ref name) = icon_name {
                    if !name.is_empty() {
                        image.set_icon_name(Some(name));
                        return;
                    }
                }
                if let Some(ref mut pixmaps) = icon_pixmap {
                    if let Some(texture) = pixmap_to_texture(pixmaps, icon_size as u32) {
                        image.set_paintable(Some(&texture));
                    }
                }
            }
        }
        TrayUpdate::Remove { address } => {
            if let Some(image) = items_ref.borrow_mut().remove(&address) {
                container_ref.remove(&image);
            }
            if items_ref.borrow().is_empty() {
                container_ref.set_visible(false);
            }
        }
    });

    debug!("Tray module created");
    container.upcast()
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
