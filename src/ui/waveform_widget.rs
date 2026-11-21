use gtk4 as gtk;
use gtk::prelude::*;
use std::cell::RefCell;
use std::f64::consts::PI;
use std::rc::Rc;

pub struct WaveformWidget {
    drawing_area: gtk::DrawingArea,
    samples: Rc<RefCell<Vec<f32>>>,
    progress: Rc<RefCell<f64>>,
    hover_pos: Rc<RefCell<Option<f64>>>,
    duration_ms: Rc<RefCell<u64>>,
    comments: Rc<RefCell<Vec<(f64, String)>>>,
    bookmarks: Rc<RefCell<Vec<(f64, String, i64)>>>,
    seek_callback: Rc<RefCell<Option<Box<dyn Fn(f64)>>>>,
}

impl WaveformWidget {
    pub fn new(height: i32) -> Self {
        let drawing_area = gtk::DrawingArea::builder()
            .content_height(height)
            .hexpand(true)
            .css_classes(["waveform-canvas"])
            .build();

        // Default wave profile
        let default_samples: Vec<f32> = (0..100)
            .map(|i| {
                let x = i as f32 / 100.0;
                0.2 + 0.6 * (x * std::f32::consts::PI).sin() * (0.8 + 0.2 * (x * 30.0).cos().abs())
            })
            .collect();

        let samples = Rc::new(RefCell::new(default_samples));
        let progress = Rc::new(RefCell::new(0.0));
        let hover_pos = Rc::new(RefCell::new(None));
        let duration_ms = Rc::new(RefCell::new(0u64));
        let comments = Rc::new(RefCell::new(Vec::new()));
        let bookmarks: Rc<RefCell<Vec<(f64, String, i64)>>> = Rc::new(RefCell::new(Vec::new()));
        let seek_callback: Rc<RefCell<Option<Box<dyn Fn(f64)>>>> = Rc::new(RefCell::new(None));

        // Setup draw function
        let samples_clone = Rc::clone(&samples);
        let progress_clone = Rc::clone(&progress);
        let hover_clone = Rc::clone(&hover_pos);
        let duration_clone = Rc::clone(&duration_ms);
        let comments_clone = Rc::clone(&comments);
        let bookmarks_clone = Rc::clone(&bookmarks);

        drawing_area.set_draw_func(move |_area, cr, width_i, height_i| {
            let w = width_i as f64;
            let h = height_i as f64;
            if w <= 10.0 || h <= 5.0 {
                return;
            }

            let samples = samples_clone.borrow();
            let current_prog = *progress_clone.borrow();
            let current_hover = *hover_clone.borrow();
            let comments = comments_clone.borrow();

            let bar_step = 4.0;
            let bar_width = 2.4;
            let num_bars = (w / bar_step).floor() as usize;

            if num_bars == 0 {
                return;
            }

            // Resample samples to num_bars
            let mut bar_values = Vec::with_capacity(num_bars);
            let s_len = samples.len();

            for i in 0..num_bars {
                if s_len == 0 {
                    bar_values.push(0.3);
                } else {
                    let s_idx = (i * s_len) / num_bars;
                    let val = samples[s_idx.min(s_len - 1)] as f64;
                    bar_values.push(val.clamp(0.08, 1.0));
                }
            }

            let baseline_y = h * 0.72; // Signature soundcloud upper/lower ratio

            // Draw bars using batched Cairo paths to eliminate GPU pipeline stalls
            // Pass 1: Upper played bars (#FFFFFF)
            cr.set_source_rgb(1.0, 1.0, 1.0);
            for (i, &val) in bar_values.iter().enumerate() {
                let bar_x = i as f64 * bar_step;
                let bar_pct = (bar_x + bar_width * 0.5) / w;
                if bar_pct <= current_prog {
                    let top_height = (val * (baseline_y - 4.0)).max(2.0);
                    cr.rectangle(bar_x, baseline_y - top_height, bar_width, top_height);
                }
            }
            let _ = cr.fill();

            // Pass 2: Upper hover scrubber preview (if any)
            if let Some(hov) = current_hover {
                if hov > current_prog {
                    cr.set_source_rgba(1.0, 1.0, 1.0, 0.55);
                    for (i, &val) in bar_values.iter().enumerate() {
                        let bar_x = i as f64 * bar_step;
                        let bar_pct = (bar_x + bar_width * 0.5) / w;
                        if bar_pct > current_prog && bar_pct <= hov {
                            let top_height = (val * (baseline_y - 4.0)).max(2.0);
                            cr.rectangle(bar_x, baseline_y - top_height, bar_width, top_height);
                        }
                    }
                    let _ = cr.fill();
                }

                // Unplayed bars beyond hover
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.18);
                for (i, &val) in bar_values.iter().enumerate() {
                    let bar_x = i as f64 * bar_step;
                    let bar_pct = (bar_x + bar_width * 0.5) / w;
                    if bar_pct > current_prog && bar_pct > hov {
                        let top_height = (val * (baseline_y - 4.0)).max(2.0);
                        cr.rectangle(bar_x, baseline_y - top_height, bar_width, top_height);
                    }
                }
                let _ = cr.fill();
            } else {
                // Pass 2b: Upper unplayed bars
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.18);
                for (i, &val) in bar_values.iter().enumerate() {
                    let bar_x = i as f64 * bar_step;
                    let bar_pct = (bar_x + bar_width * 0.5) / w;
                    if bar_pct > current_prog {
                        let top_height = (val * (baseline_y - 4.0)).max(2.0);
                        cr.rectangle(bar_x, baseline_y - top_height, bar_width, top_height);
                    }
                }
                let _ = cr.fill();
            }

            // Pass 3: Lower reflection bars (played)
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.32);
            for (i, &val) in bar_values.iter().enumerate() {
                let bar_x = i as f64 * bar_step;
                let bar_pct = (bar_x + bar_width * 0.5) / w;
                if bar_pct <= current_prog {
                    let top_height = (val * (baseline_y - 4.0)).max(2.0);
                    let bottom_height = (top_height * 0.35).min(h - baseline_y - 4.0);
                    cr.rectangle(bar_x, baseline_y + 1.5, bar_width, bottom_height);
                }
            }
            let _ = cr.fill();

            // Pass 4: Lower reflection bars (unplayed)
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.07);
            for (i, &val) in bar_values.iter().enumerate() {
                let bar_x = i as f64 * bar_step;
                let bar_pct = (bar_x + bar_width * 0.5) / w;
                if bar_pct > current_prog {
                    let top_height = (val * (baseline_y - 4.0)).max(2.0);
                    let bottom_height = (top_height * 0.35).min(h - baseline_y - 4.0);
                    cr.rectangle(bar_x, baseline_y + 1.5, bar_width, bottom_height);
                }
            }
            let _ = cr.fill();

            // Draw comment markers along timeline
            for (comment_pct, _text) in comments.iter() {
                let marker_x = comment_pct * w;
                cr.set_source_rgb(1.0, 1.0, 1.0);
                cr.arc(marker_x, h - 3.5, 2.5, 0.0, 2.0 * PI);
                let _ = cr.fill();
            }

            let bookmarks = bookmarks_clone.borrow();
            // Draw personal bookmark markers (cyan diamond + subtle vertical hairline)
            for (bm_pct, _note, _id) in bookmarks.iter() {
                let bm_x = bm_pct * w;
                // Diamond top flag
                cr.set_source_rgb(0.35, 0.70, 1.0);
                cr.move_to(bm_x, 1.0);
                cr.line_to(bm_x + 3.0, 4.5);
                cr.line_to(bm_x, 8.0);
                cr.line_to(bm_x - 3.0, 4.5);
                cr.close_path();
                let _ = cr.fill();

                // Subtle hairline down the waveform
                cr.set_source_rgba(0.35, 0.70, 1.0, 0.45);
                cr.set_line_width(0.75);
                cr.move_to(bm_x, 8.0);
                cr.line_to(bm_x, h - 2.0);
                let _ = cr.stroke();
            }

            // Draw hover scrub indicator line and time indicator pill
            if let Some(hov) = current_hover {
                let hov_x = hov * w;
                cr.set_source_rgba(1.0, 1.0, 1.0, 0.85);
                cr.set_line_width(1.0);
                cr.move_to(hov_x, 0.0);
                cr.line_to(hov_x, h);
                let _ = cr.stroke();

                let dur = *duration_clone.borrow();
                if dur > 0 {
                    let hover_sec = (hov * dur as f64 / 1000.0) as u64;
                    let mins = hover_sec / 60;
                    let secs = hover_sec % 60;
                    let mut time_str = format!("{}:{:02}", mins, secs);

                    for item in bookmarks.iter() {
                        let bm_pct: f64 = item.0;
                        let note: &str = item.1.as_str();
                        if f64::abs(hov - bm_pct) < 0.015 {
                            let clean_note = if note.len() > 18 {
                                format!("{}...", &note[..18])
                            } else {
                                note.to_string()
                            };
                            time_str = format!("{} • {}", time_str, clean_note);
                            break;
                        }
                    }

                    cr.set_font_size(9.0);
                    cr.select_font_face("monospace", gtk::cairo::FontSlant::Normal, gtk::cairo::FontWeight::Bold);
                    if let Ok(ext) = cr.text_extents(&time_str) {
                        let pill_w = ext.width() + 10.0;
                        let pill_h = ext.height() + 5.0;
                        let pill_x = (hov_x - pill_w * 0.5).clamp(2.0, (w - pill_w - 2.0).max(2.0));
                        let pill_y = 1.0;

                        // Pill background
                        cr.set_source_rgba(0.06, 0.07, 0.09, 0.94);
                        cr.rectangle(pill_x, pill_y, pill_w, pill_h);
                        let _ = cr.fill();

                        // Pill crisp 1px border
                        cr.set_source_rgba(1.0, 1.0, 1.0, 0.28);
                        cr.set_line_width(0.75);
                        cr.rectangle(pill_x, pill_y, pill_w, pill_h);
                        let _ = cr.stroke();

                        // Time text
                        cr.set_source_rgb(1.0, 1.0, 1.0);
                        cr.move_to(pill_x + 5.0, pill_y + pill_h - 2.5);
                        let _ = cr.show_text(&time_str);
                    }
                }
            }
        });

        // Click Gesture
        let click_gesture = gtk::GestureClick::new();
        let da_clone = drawing_area.clone();
        let prog_clone = Rc::clone(&progress);
        let seek_clone = Rc::clone(&seek_callback);

        click_gesture.connect_pressed(move |_gesture, _n_press, x, _y| {
            let width = da_clone.width() as f64;
            if width > 0.0 {
                let pct = (x / width).clamp(0.0, 1.0);
                *prog_clone.borrow_mut() = pct;
                da_clone.queue_draw();
                if let Some(ref cb) = *seek_clone.borrow() {
                    cb(pct);
                }
            }
        });
        drawing_area.add_controller(click_gesture);

        // Drag Gesture for smooth real-time scrubbing
        let drag_gesture = gtk::GestureDrag::new();
        let drag_start_x = Rc::new(RefCell::new(0.0));
        let drag_start_x_clone = Rc::clone(&drag_start_x);

        drag_gesture.connect_drag_begin(move |_gesture, start_x, _start_y| {
            *drag_start_x_clone.borrow_mut() = start_x;
        });

        let da_drag_update = drawing_area.clone();
        let prog_drag_update = Rc::clone(&progress);
        let seek_drag_update = Rc::clone(&seek_callback);
        let drag_start_x_update = Rc::clone(&drag_start_x);
        let last_seek_time = Rc::new(RefCell::new(std::time::Instant::now()));
        let last_seek_time_clone = Rc::clone(&last_seek_time);

        drag_gesture.connect_drag_update(move |_gesture, offset_x, _offset_y| {
            let width = da_drag_update.width() as f64;
            if width > 0.0 {
                let current_x = *drag_start_x_update.borrow() + offset_x;
                let pct = (current_x / width).clamp(0.0, 1.0);
                *prog_drag_update.borrow_mut() = pct;
                da_drag_update.queue_draw();

                // Throttle seeking during continuous mouse drag to prevent flooding MPV demuxer
                let now = std::time::Instant::now();
                if now.duration_since(*last_seek_time_clone.borrow()) >= std::time::Duration::from_millis(200) {
                    *last_seek_time_clone.borrow_mut() = now;
                    if let Some(ref cb) = *seek_drag_update.borrow() {
                        cb(pct);
                    }
                }
            }
        });

        let da_drag_end = drawing_area.clone();
        let seek_drag_end = Rc::clone(&seek_callback);
        let drag_start_x_end = Rc::clone(&drag_start_x);

        drag_gesture.connect_drag_end(move |_gesture, offset_x, _offset_y| {
            let width = da_drag_end.width() as f64;
            if width > 0.0 {
                let current_x = *drag_start_x_end.borrow() + offset_x;
                let pct = (current_x / width).clamp(0.0, 1.0);
                if let Some(ref cb) = *seek_drag_end.borrow() {
                    cb(pct);
                }
            }
        });
        drawing_area.add_controller(drag_gesture);

        // Motion Controller for hover line
        let motion_ctrl = gtk::EventControllerMotion::new();
        let da_motion = drawing_area.clone();
        let hover_motion = Rc::clone(&hover_pos);

        motion_ctrl.connect_motion(move |_ctrl, x, _y| {
            let width = da_motion.width() as f64;
            if width > 0.0 {
                *hover_motion.borrow_mut() = Some((x / width).clamp(0.0, 1.0));
                da_motion.queue_draw();
            }
        });

        let da_leave = drawing_area.clone();
        let hover_leave = Rc::clone(&hover_pos);
        motion_ctrl.connect_leave(move |_ctrl| {
            *hover_leave.borrow_mut() = None;
            da_leave.queue_draw();
        });
        drawing_area.add_controller(motion_ctrl);

        Self {
            drawing_area,
            samples,
            progress,
            hover_pos,
            duration_ms,
            comments,
            bookmarks,
            seek_callback,
        }
    }

    pub fn widget(&self) -> &gtk::DrawingArea {
        &self.drawing_area
    }

    pub fn set_duration_ms(&self, ms: u64) {
        *self.duration_ms.borrow_mut() = ms;
        self.drawing_area.queue_draw();
    }

    pub fn set_samples(&self, new_samples: Vec<f32>) {
        *self.samples.borrow_mut() = new_samples;
        self.drawing_area.queue_draw();
    }

    pub fn set_progress(&self, prog: f64) {
        let clamped = prog.clamp(0.0, 1.0);
        let mut curr = self.progress.borrow_mut();
        let w = self.drawing_area.width() as f64;
        if w > 0.0 {
            let old_px = (*curr * w).round();
            let new_px = (clamped * w).round();
            if old_px == new_px {
                *curr = clamped;
                return; // Has not moved by even 1 visual pixel on screen
            }
        }
        *curr = clamped;
        self.drawing_area.queue_draw();
    }

    pub fn set_comments(&self, comment_list: Vec<(f64, String)>) {
        *self.comments.borrow_mut() = comment_list;
        self.drawing_area.queue_draw();
    }

    pub fn set_bookmarks(&self, bookmark_list: Vec<(f64, String, i64)>) {
        *self.bookmarks.borrow_mut() = bookmark_list;
        self.drawing_area.queue_draw();
    }

    pub fn connect_seek<F>(&self, callback: F)
    where
        F: Fn(f64) + 'static,
    {
        *self.seek_callback.borrow_mut() = Some(Box::new(callback));
    }
}
