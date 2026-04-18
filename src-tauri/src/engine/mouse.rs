use core_graphics::event::{CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::rng::FastRng;
use crate::dev_logger::DEV_LOGGER;

/// Button flags for macOS mouse events
#[inline]
pub fn get_button_flags(button: i32) -> (CGEventType, CGEventType, CGMouseButton) {
    match button {
        1 => (
            CGEventType::RightMouseDown,
            CGEventType::RightMouseUp,
            CGMouseButton::Right,
        ),
        2 => (
            CGEventType::OtherMouseDown,
            CGEventType::OtherMouseUp,
            CGMouseButton::Center,
        ),
        _ => (
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
            CGMouseButton::Left,
        ),
    }
}

/// Send batch of mouse events efficiently (similar to Windows SendInput batch)
fn send_batch(
    down_event: CGEventType,
    up_event: CGEventType,
    point: CGPoint,
    mouse_button: CGMouseButton,
    count: usize,
) {
    if count == 0 {
        return;
    }

    let source = match CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
        Ok(s) => s,
        Err(e) => {
            DEV_LOGGER.log("MOUSE", &format!("Failed to create EventSource: {:?}", e));
            return;
        }
    };

    // Create all events upfront
    let mut events: Vec<CGEvent> = Vec::with_capacity(count * 2);
    for _ in 0..count {
        if let Ok(down) = CGEvent::new_mouse_event(source.clone(), down_event, point, mouse_button)
        {
            down.set_flags(CGEventFlags::empty());
            events.push(down);
        }
        if let Ok(up) = CGEvent::new_mouse_event(source.clone(), up_event, point, mouse_button) {
            up.set_flags(CGEventFlags::empty());
            events.push(up);
        }
    }

    // Post all events
    for event in events {
        let _ = event.post(CGEventTapLocation::HID);
    }
}

/// Get current cursor position in screen coordinates (top-left origin)
/// Note: CGEvent.location() already returns screen coordinates, no flip needed
pub fn current_cursor_position() -> Option<(i32, i32)> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let event = CGEvent::new(source).ok()?;
    let loc = event.location();
    // CGEvent returns screen coordinates directly (Y=0 at top-left)
    Some((loc.x as i32, loc.y as i32))
}

#[inline]
pub fn get_cursor_pos() -> (i32, i32) {
    current_cursor_position().unwrap_or((0, 0))
}

fn create_mouse_event(
    event_type: CGEventType,
    point: CGPoint,
    button: CGMouseButton,
) -> Option<CGEvent> {
    let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState).ok()?;
    let event = CGEvent::new_mouse_event(source, event_type, point, button).ok()?;
    // Clear the synthetic flag to make event appear more like real input
    event.set_flags(CGEventFlags::empty());
    Some(event)
}

#[inline]
pub fn move_mouse(x: i32, y: i32) {
    eprintln!("[MOUSE] move_mouse to ({}, {})", x, y);
    let point = CGPoint::new(x as f64, y as f64);
    if let Some(event) = create_mouse_event(CGEventType::MouseMoved, point, CGMouseButton::Left) {
        let _ = event.post(CGEventTapLocation::HID);
    }
}

pub fn send_clicks(
    button: i32,
    count: usize,
    hold_ms: u32,
    use_double_click_gap: bool,
    double_click_delay_ms: u32,
    running: &Arc<AtomicBool>,
) {
    if count == 0 {
        return;
    }

    let (down_event, up_event, mouse_button) = get_button_flags(button);

    let loc = current_cursor_position().unwrap_or((0, 0));
    let point = CGPoint::new(loc.0 as f64, loc.1 as f64);

    DEV_LOGGER.log(
        "MOUSE",
        &format!(
            "send_clicks: button={}, count={}, loc=({}, {})",
            button, count, loc.0, loc.1
        ),
    );

    // Batch sending when no double-click gap, multiple clicks, and no hold time
    if !use_double_click_gap && count > 1 && hold_ms == 0 {
        send_batch(down_event, up_event, point, mouse_button, count);
        return;
    }

    for index in 0..count {
        let mut posted_down = false;

        if let Some(event) = create_mouse_event(down_event, point, mouse_button) {
            let result = event.post(CGEventTapLocation::HID);
            DEV_LOGGER.log(
                "MOUSE",
                &format!("Posted mouse DOWN event, result={:?}", result),
            );
            posted_down = true;
        }

        if !running.load(Ordering::SeqCst) {
            if posted_down {
                if let Some(up_ev) = create_mouse_event(up_event, point, mouse_button) {
                    let _ = up_ev.post(CGEventTapLocation::HID);
                    DEV_LOGGER.log("MOUSE", "Posted mouse UP event on early exit");
                }
            }
            return;
        }

        if hold_ms > 0 {
            sleep_interruptible(Duration::from_millis(hold_ms as u64), running);
        }

        if let Some(event) = create_mouse_event(up_event, point, mouse_button) {
            let result = event.post(CGEventTapLocation::HID);
            DEV_LOGGER.log(
                "MOUSE",
                &format!("Posted mouse UP event, result={:?}", result),
            );
        }

        if index + 1 < count && use_double_click_gap && double_click_delay_ms > 0 {
            sleep_interruptible(Duration::from_millis(double_click_delay_ms as u64), running);
        }
    }
}

fn sleep_interruptible(remaining: Duration, running: &Arc<AtomicBool>) {
    let tick = Duration::from_millis(5);
    let start = Instant::now();
    while running.load(Ordering::SeqCst) && start.elapsed() < remaining {
        let left = remaining.saturating_sub(start.elapsed());
        std::thread::sleep(left.min(tick));
    }
}

#[inline]
pub fn ease_in_out_quad(t: f64) -> f64 {
    if t < 0.5 {
        2.0 * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }
}

#[inline]
pub fn cubic_bezier(t: f64, p0: f64, p1: f64, p2: f64, p3: f64) -> f64 {
    let u = 1.0 - t;
    u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3
}

pub fn smooth_move(
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
    duration_ms: u64,
    _rng: &mut FastRng,
) {
    if duration_ms < 5 {
        move_mouse(end_x, end_y);
        return;
    }

    let (sx, sy) = (start_x as f64, start_y as f64);
    let (ex, ey) = (end_x as f64, end_y as f64);
    let (dx, dy) = (ex - sx, ey - sy);
    let distance = (dx * dx + dy * dy).sqrt();
    if distance < 1.0 {
        return;
    }

    let steps = (duration_ms as usize).clamp(10, 200);
    let step_dur = Duration::from_millis(duration_ms / steps as u64);

    for i in 0..=steps {
        let t = ease_in_out_quad(i as f64 / steps as f64);
        move_mouse(
            cubic_bezier(t, sx, sx + dx * 0.33, ex - dx * 0.33, ex) as i32,
            cubic_bezier(t, sy, sy + dy * 0.33, ey - dy * 0.33, ey) as i32,
        );
        if i < steps {
            std::thread::sleep(step_dur);
        }
    }
}
