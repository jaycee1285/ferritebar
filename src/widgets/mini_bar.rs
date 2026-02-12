use gtk::prelude::*;
use gtk::cairo;
use std::cell::Cell;
use std::rc::Rc;

use crate::theme::{self, ThemeColors};

/// A compact gradient progress bar rendered with cairo
pub struct MiniBar {
    drawing_area: gtk::DrawingArea,
    fraction: Rc<Cell<f64>>,
}

impl MiniBar {
    /// Create a new mini bar. If `vertical` is true, fills bottom-to-top.
    pub fn new(width: i32, height: i32, colors: &ThemeColors, vertical: bool) -> Self {
        let fraction = Rc::new(Cell::new(0.0));
        let drawing_area = gtk::DrawingArea::new();
        drawing_area.set_content_width(width);
        drawing_area.set_content_height(height);
        drawing_area.set_valign(gtk::Align::Center);

        let success_rgb = theme::hex_to_rgb(&colors.success).unwrap_or((0.64, 0.75, 0.55));
        let warning_rgb = theme::hex_to_rgb(&colors.warning).unwrap_or((0.98, 0.80, 0.08));
        let error_rgb = theme::hex_to_rgb(&colors.error).unwrap_or((0.97, 0.44, 0.44));
        let bg_rgb = theme::hex_to_rgb(&colors.bg).unwrap_or((0.18, 0.20, 0.25));

        let frac = fraction.clone();
        drawing_area.set_draw_func(move |_da, cr, w, h| {
            let w = w as f64;
            let h = h as f64;
            let f = frac.get();

            // Background (rounded rect)
            let radius = if vertical { w / 3.0 } else { h / 3.0 };
            rounded_rect(cr, 0.0, 0.0, w, h, radius);
            cr.set_source_rgb(bg_rgb.0 * 1.3, bg_rgb.1 * 1.3, bg_rgb.2 * 1.3);
            let _ = cr.fill();

            // Gradient fill
            if f > 0.001 {
                if vertical {
                    // Fill from bottom to top
                    let fill_height = h * f;
                    let y_start = h - fill_height;
                    let pat = cairo::LinearGradient::new(0.0, h, 0.0, 0.0);
                    pat.add_color_stop_rgb(0.0, success_rgb.0, success_rgb.1, success_rgb.2);
                    pat.add_color_stop_rgb(0.7, warning_rgb.0, warning_rgb.1, warning_rgb.2);
                    pat.add_color_stop_rgb(1.0, error_rgb.0, error_rgb.1, error_rgb.2);

                    rounded_rect(cr, 0.0, y_start, w, fill_height, radius);
                    let _ = cr.set_source(&pat);
                    let _ = cr.fill();
                } else {
                    // Fill from left to right
                    let fill_width = w * f;
                    let pat = cairo::LinearGradient::new(0.0, 0.0, w, 0.0);
                    pat.add_color_stop_rgb(0.0, success_rgb.0, success_rgb.1, success_rgb.2);
                    pat.add_color_stop_rgb(0.7, warning_rgb.0, warning_rgb.1, warning_rgb.2);
                    pat.add_color_stop_rgb(1.0, error_rgb.0, error_rgb.1, error_rgb.2);

                    rounded_rect(cr, 0.0, 0.0, fill_width, h, radius);
                    let _ = cr.set_source(&pat);
                    let _ = cr.fill();
                }
            }
        });

        Self {
            drawing_area,
            fraction,
        }
    }

    pub fn set_fraction(&self, fraction: f64) {
        self.fraction.set(fraction.clamp(0.0, 1.0));
        self.drawing_area.queue_draw();
    }

    pub fn widget(&self) -> &gtk::DrawingArea {
        &self.drawing_area
    }
}

/// Draw a rounded rectangle path
fn rounded_rect(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    cr.arc(x + r, y + r, r, std::f64::consts::PI, 3.0 * std::f64::consts::FRAC_PI_2);
    cr.close_path();
}
