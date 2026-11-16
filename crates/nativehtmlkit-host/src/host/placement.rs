//! Initial window placement. A new window must fit the work area of the monitor it opens on:
//! the default 900x560 CSS px window (or a requested size) at a scale factor above 100 %, or the
//! Windows cascade offset of a later window, otherwise leaves the frame partly below the
//! taskbar or the screen edge.
//!
//! - Windows: after creation (the window is still hidden) the visible frame is clamped to the
//!   work area of the nearest monitor and moved inside it ([`fit_into`]). Positions a client
//!   sets later with `window.set {x, y}` are not touched.
//! - Wayland: clients cannot position windows and the work area is not known; the compositor
//!   places the window. Only the requested surface size is clamped to the smallest output's
//!   logical size ([`clamp_logical_size`]), so the window cannot exceed any output.
//!
//! The math is pure and works in one consistent unit (Windows: physical pixels of a
//! per-monitor-DPI-aware process, where each monitor reports its rectangles in physical pixels).
//! The rectangle math is used by the Windows build only; Linux compiles it for its unit tests.
#![cfg_attr(not(target_os = "windows"), allow(dead_code))]

/// A rectangle, edges exclusive at `right`/`bottom` (Win32 `RECT` semantics).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(self) -> i32 {
        self.right - self.left
    }
    pub fn height(self) -> i32 {
        self.bottom - self.top
    }
    fn intersection_area(self, other: Rect) -> i64 {
        let w = self.right.min(other.right) - self.left.max(other.left);
        let h = self.bottom.min(other.bottom) - self.top.max(other.top);
        if w <= 0 || h <= 0 {
            0
        } else {
            i64::from(w) * i64::from(h)
        }
    }
    /// Squared distance between the two rectangles (0 when they touch or overlap).
    fn distance_sq(self, other: Rect) -> i64 {
        let gap = |a0: i32, a1: i32, b0: i32, b1: i32| i64::from((b0 - a1).max(a0 - b1).max(0));
        let dx = gap(self.left, self.right, other.left, other.right);
        let dy = gap(self.top, self.bottom, other.top, other.bottom);
        dx * dx + dy * dy
    }
}

/// One display: its full bounds and its work area (bounds minus taskbar/docked bars).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Monitor {
    pub bounds: Rect,
    pub work: Rect,
}

/// The monitor a window frame belongs to, like `MONITOR_DEFAULTTONEAREST`: the largest
/// intersection with the monitor bounds, otherwise the closest monitor. Ties keep the first
/// (the enumeration order puts the primary monitor first on Windows).
pub(crate) fn target_monitor(frame: Rect, monitors: &[Monitor]) -> Option<Monitor> {
    let best_overlap = monitors
        .iter()
        .map(|m| (m.bounds.intersection_area(frame), *m))
        .filter(|(area, _)| *area > 0)
        .fold(None::<(i64, Monitor)>, |best, (area, m)| match best {
            Some((a, _)) if a >= area => best,
            _ => Some((area, m)),
        });
    if let Some((_, monitor)) = best_overlap {
        return Some(monitor);
    }
    monitors
        .iter()
        .map(|m| (m.bounds.distance_sq(frame), *m))
        .fold(None::<(i64, Monitor)>, |best, (distance, m)| match best {
            Some((d, _)) if d <= distance => best,
            _ => Some((distance, m)),
        })
        .map(|(_, monitor)| monitor)
}

/// `frame` shrunk to at most the work area's size and moved by the smallest offset that puts
/// it completely inside the work area. A frame that already fits is returned unchanged.
pub(crate) fn fit_into(frame: Rect, work: Rect) -> Rect {
    if work.width() <= 0 || work.height() <= 0 {
        return frame;
    }
    let width = frame.width().clamp(1, work.width());
    let height = frame.height().clamp(1, work.height());
    let left = frame.left.clamp(work.left, work.right - width);
    let top = frame.top.clamp(work.top, work.bottom - height);
    Rect {
        left,
        top,
        right: left + width,
        bottom: top + height,
    }
}

/// The new outer window rectangle for a visible frame that was fitted from `visible` to
/// `fitted`, given the outer rectangle `outer` (which includes invisible resize borders).
pub(crate) fn outer_for(outer: Rect, visible: Rect, fitted: Rect) -> Rect {
    Rect {
        left: fitted.left - (visible.left - outer.left),
        top: fitted.top - (visible.top - outer.top),
        right: fitted.right + (outer.right - visible.right),
        bottom: fitted.bottom + (outer.bottom - visible.bottom),
    }
}

/// A requested logical surface size clamped to the smallest output, given each output's
/// physical size and scale factor. Unknown outputs (none, or a zero/invalid entry) leave the
/// size unchanged; the result is never below 1x1 or below `min` (the client's own minimum).
#[cfg_attr(target_os = "windows", allow(dead_code))]
pub(crate) fn clamp_logical_size(
    size: (u32, u32),
    min: Option<(u32, u32)>,
    outputs: &[(u32, u32, f64)],
) -> (u32, u32) {
    let logical: Vec<(u32, u32)> = outputs
        .iter()
        .filter(|(w, h, s)| *w > 0 && *h > 0 && s.is_finite() && *s > 0.0)
        .map(|(w, h, s)| {
            (
                (f64::from(*w) / s).floor().max(1.0) as u32,
                (f64::from(*h) / s).floor().max(1.0) as u32,
            )
        })
        .collect();
    let (Some(max_w), Some(max_h)) = (
        logical.iter().map(|o| o.0).min(),
        logical.iter().map(|o| o.1).min(),
    ) else {
        return size;
    };
    let min = min.unwrap_or((1, 1));
    (
        size.0.min(max_w).max(min.0).max(1),
        size.1.min(max_h).max(min.1).max(1),
    )
}

/// Windows: fit a newly created (still hidden) window into the work area of its monitor.
/// Returns the new outer rectangle when the window was moved or resized.
#[cfg(target_os = "windows")]
pub(crate) fn fit_new_window(window: &dyn winit::window::Window) -> Option<Rect> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, SetWindowPos, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER,
    };

    fn rect(r: RECT) -> Rect {
        Rect {
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        }
    }
    unsafe extern "system" fn collect(
        monitor: HMONITOR,
        _: HDC,
        _: *mut RECT,
        data: LPARAM,
    ) -> windows_sys::core::BOOL {
        // SAFETY: `data` is the `&mut Vec<Monitor>` passed below, valid for the whole
        // synchronous enumeration; `info` is a properly sized MONITORINFO.
        let monitors = unsafe { &mut *(data as *mut Vec<Monitor>) };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(monitor, &mut info) } != 0 {
            monitors.push(Monitor {
                bounds: rect(info.rcMonitor),
                work: rect(info.rcWork),
            });
        }
        1
    }

    let handle = window.window_handle().ok()?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return None;
    };
    let hwnd = handle.hwnd.get() as HWND;
    let mut outer = RECT::default();
    // SAFETY: `hwnd` is the live window owned by this (event-loop) thread; `outer` is writable.
    if unsafe { GetWindowRect(hwnd, &mut outer) } == 0 {
        return None;
    }
    let outer = rect(outer);
    // The visible frame excludes the invisible resize borders; fall back to the outer
    // rectangle when DWM does not report it (the window is hidden or composition is off).
    let mut bounds = RECT::default();
    // SAFETY: the out pointer and size describe `bounds`.
    let visible = if unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS as u32,
            (&mut bounds as *mut RECT).cast(),
            std::mem::size_of::<RECT>() as u32,
        )
    } >= 0
        && bounds.right > bounds.left
        && bounds.bottom > bounds.top
    {
        rect(bounds)
    } else {
        outer
    };
    let mut monitors: Vec<Monitor> = Vec::new();
    // SAFETY: the callback only runs during this call and receives `&mut monitors`.
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(collect),
            &mut monitors as *mut Vec<Monitor> as LPARAM,
        )
    };
    let monitor = target_monitor(visible, &monitors)?;
    let fitted = fit_into(visible, monitor.work);
    if fitted == visible {
        return None;
    }
    let next = outer_for(outer, visible, fitted);
    // SAFETY: plain Win32 call on our own window; no z-order or activation change.
    let moved = unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            next.left,
            next.top,
            next.width(),
            next.height(),
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOOWNERZORDER,
        )
    } != 0;
    moved.then_some(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(left: i32, top: i32, right: i32, bottom: i32) -> Rect {
        Rect {
            left,
            top,
            right,
            bottom,
        }
    }

    /// The reported case: 1280x800 desktop with a 40 px taskbar, the default 900x560 CSS px
    /// window at 100 % (frame about 916x599) cascaded to y=230 ends 69 px below the work area.
    #[test]
    fn a_cascaded_window_below_the_taskbar_is_moved_inside_the_work_area() {
        let work = r(0, 0, 1280, 760);
        let frame = r(260, 230, 1176, 829);
        let fitted = fit_into(frame, work);
        assert_eq!(fitted, r(260, 161, 1176, 760));
        assert_eq!((fitted.width(), fitted.height()), (916, 599), "size kept");
    }

    #[test]
    fn a_fitting_window_is_unchanged_and_an_oversized_one_is_clamped() {
        let work = r(0, 0, 1280, 760);
        assert_eq!(fit_into(r(10, 20, 926, 619), work), r(10, 20, 926, 619));
        // 900x560 CSS px at 150 % is 1350x840 client px plus the frame: larger than the area.
        let fitted = fit_into(r(100, 50, 1474, 940), work);
        assert_eq!(fitted, work);
        // Off the left/top edge.
        assert_eq!(fit_into(r(-50, -30, 450, 270), work), r(0, 0, 500, 300));
    }

    /// Multi-monitor: secondary monitors left of and above the primary (negative coordinates),
    /// with different scale factors (physical-pixel rectangles) and taskbars on other edges.
    #[test]
    fn the_nearest_monitor_and_its_own_work_area_are_used() {
        let primary = Monitor {
            bounds: r(0, 0, 1280, 800),
            work: r(0, 0, 1280, 760),
        };
        // 2560x1440 at 150 %, taskbar on the left, placed left of the primary.
        let left = Monitor {
            bounds: r(-2560, -200, 0, 1240),
            work: r(-2500, -200, 0, 1240),
        };
        // 1920x1080 at 125 %, taskbar at the top, above the primary.
        let top = Monitor {
            bounds: r(0, -1080, 1920, 0),
            work: r(0, -1040, 1920, 0),
        };
        let monitors = [primary, left, top];
        // Mostly on the left monitor, overlapping its taskbar and its bottom edge.
        let frame = r(-2540, 900, -1190, 1740);
        let target = target_monitor(frame, &monitors).unwrap();
        assert_eq!(target, left);
        let fitted = fit_into(frame, target.work);
        assert_eq!(fitted, r(-2500, 400, -1150, 1240));
        // Straddling the top monitor and the primary: the larger overlap (top) wins.
        let frame = r(100, -500, 1000, 100);
        assert_eq!(target_monitor(frame, &monitors), Some(top));
        assert_eq!(fit_into(frame, top.work), r(100, -600, 1000, 0));
        // Entirely in a gap between monitors: the closest one.
        let frame = r(1400, 900, 1600, 1000);
        assert_eq!(target_monitor(frame, &monitors), Some(primary));
        assert_eq!(target_monitor(frame, &[]), None);
    }

    #[test]
    fn the_outer_rectangle_keeps_the_invisible_borders() {
        let outer = r(253, 230, 1183, 836);
        let visible = r(260, 230, 1176, 829);
        let fitted = fit_into(visible, r(0, 0, 1280, 760));
        assert_eq!(outer_for(outer, visible, fitted), r(253, 161, 1183, 767));
    }

    #[test]
    fn wayland_sizes_are_clamped_to_the_smallest_output_in_logical_px() {
        // 1280x800 at 100 % and 2560x1600 at 200 %: both 1280x800 logical.
        let outputs = [(1280, 800, 1.0), (2560, 1600, 2.0)];
        assert_eq!(clamp_logical_size((900, 560), None, &outputs), (900, 560));
        assert_eq!(
            clamp_logical_size((2000, 1200), None, &outputs),
            (1280, 800)
        );
        // 1920x1080 at 150 % is 1280x720 logical.
        assert_eq!(
            clamp_logical_size((1600, 900), None, &[(1920, 1080, 1.5)]),
            (1280, 720)
        );
        // The client's minimum wins over the output; unknown outputs change nothing.
        assert_eq!(
            clamp_logical_size((2000, 1200), Some((1500, 900)), &[(1280, 800, 1.0)]),
            (1500, 900)
        );
        assert_eq!(clamp_logical_size((2000, 1200), None, &[]), (2000, 1200));
        assert_eq!(
            clamp_logical_size((2000, 1200), None, &[(0, 0, 1.0), (1280, 800, f64::NAN)]),
            (2000, 1200)
        );
    }
}
