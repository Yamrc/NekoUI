use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use winit::platform::windows::WindowExtWindows;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window as WinitWindow, WindowAttributes};

use windows_sys::Win32::Foundation::{POINT, RECT};
use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GWL_STYLE, GWLP_WNDPROC, GetWindowLongPtrW, GetWindowRect,
    HTCAPTION, HTCLOSE, HTMAXBUTTON, HTMINBUTTON, HTTOP, HTTOPLEFT, HTTOPRIGHT, IsZoomed,
    NCCALCSIZE_PARAMS, PostMessageW, SM_CXPADDEDBORDER, SM_CXSIZEFRAME, SM_CYSIZEFRAME,
    SW_MAXIMIZE, SW_MINIMIZE, SW_NORMAL, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SetWindowLongPtrW, SetWindowPos, ShowWindowAsync, WM_CLOSE, WM_NCCALCSIZE, WM_NCDESTROY,
    WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WS_CAPTION,
};

use crate::element::WindowFrameArea;
use crate::geometry::{Point, Px};
use crate::platform::window::WindowOptions;
use crate::scene::LayoutBox;

pub(crate) fn decorate_attributes(
    attributes: WindowAttributes,
    _options: &WindowOptions,
) -> WindowAttributes {
    attributes.with_decorations(true)
}

pub(crate) fn apply_post_create(window: &WinitWindow, options: &WindowOptions) {
    if options.show_titlebar {
        return;
    }

    let hwnd = match window.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::Win32(handle) => handle.hwnd.get(),
            _ => return,
        },
        Err(_) => return,
    };

    unsafe {
        let hwnd = hwnd as *mut core::ffi::c_void;
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        if style != 0 {
            let new_style = style & !(WS_CAPTION as isize);
            SetWindowLongPtrW(hwnd, GWL_STYLE, new_style);
            SetWindowPos(
                hwnd,
                core::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER,
            );
        }
        install_hidden_titlebar_subclass(hwnd);
        window.set_undecorated_shadow(true);
    }
}

fn hidden_titlebar_wndproc_map() -> &'static Mutex<HashMap<isize, isize>> {
    static MAP: OnceLock<Mutex<HashMap<isize, isize>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone)]
struct HiddenTitlebarHitTestState {
    scale_factor: f64,
    areas: Vec<(WindowFrameArea, LayoutBox)>,
    nc_button_pressed: Option<u32>,
}

fn hidden_titlebar_hit_test_map() -> &'static Mutex<HashMap<isize, HiddenTitlebarHitTestState>> {
    static MAP: OnceLock<Mutex<HashMap<isize, HiddenTitlebarHitTestState>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(crate) fn update_hidden_titlebar_hit_test_state(
    window: &WinitWindow,
    scale_factor: f64,
    areas: &[(WindowFrameArea, LayoutBox)],
) {
    let hwnd = match window.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::Win32(handle) => handle.hwnd.get(),
            _ => return,
        },
        Err(_) => return,
    };

    hidden_titlebar_hit_test_map()
        .lock()
        .expect("hidden titlebar hit-test map lock")
        .entry(hwnd)
        .and_modify(|state| {
            state.scale_factor = scale_factor;
            state.areas = areas.to_vec();
        })
        .or_insert(HiddenTitlebarHitTestState {
            scale_factor,
            areas: areas.to_vec(),
            nc_button_pressed: None,
        });
}

pub(crate) unsafe extern "system" fn hidden_titlebar_wndproc(
    hwnd: *mut core::ffi::c_void,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match msg {
        WM_NCCALCSIZE if wparam != 0 => {
            let params = lparam as *mut NCCALCSIZE_PARAMS;
            if !params.is_null() {
                let saved_top = unsafe { (*params).rgrc[0].top };
                let result = unsafe { DefWindowProcW(hwnd, WM_NCCALCSIZE, wparam, lparam) };
                unsafe {
                    (*params).rgrc[0].top = saved_top;
                }
                return result;
            }
        }
        WM_NCHITTEST => {
            if let Some(hit) = unsafe { hidden_titlebar_hit_test(hwnd, lparam) } {
                return hit;
            }
        }
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK => {
            if let Some(result) = hidden_titlebar_nc_button_down(hwnd, wparam as u32) {
                return result;
            }
        }
        WM_NCLBUTTONUP => {
            if let Some(result) = hidden_titlebar_nc_button_up(hwnd, wparam as u32) {
                return result;
            }
        }
        WM_NCDESTROY => {
            if let Some(original_proc) = hidden_titlebar_wndproc_map()
                .lock()
                .expect("window proc map lock")
                .remove(&(hwnd as isize))
            {
                unsafe {
                    SetWindowLongPtrW(hwnd, GWLP_WNDPROC, original_proc);
                }
                return unsafe {
                    CallWindowProcW(
                        Some(std::mem::transmute::<
                            isize,
                            unsafe extern "system" fn(
                                *mut core::ffi::c_void,
                                u32,
                                usize,
                                isize,
                            ) -> isize,
                        >(original_proc)),
                        hwnd,
                        msg,
                        wparam,
                        lparam,
                    )
                };
            }
            hidden_titlebar_hit_test_map()
                .lock()
                .expect("hidden titlebar hit-test map lock")
                .remove(&(hwnd as isize));
        }
        _ => {}
    }

    let original_proc = hidden_titlebar_wndproc_map()
        .lock()
        .expect("window proc map lock")
        .get(&(hwnd as isize))
        .copied();

    if let Some(original_proc) = original_proc {
        unsafe {
            CallWindowProcW(
                Some(std::mem::transmute::<
                    isize,
                    unsafe extern "system" fn(*mut core::ffi::c_void, u32, usize, isize) -> isize,
                >(original_proc)),
                hwnd,
                msg,
                wparam,
                lparam,
            )
        }
    } else {
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}

fn hidden_titlebar_nc_button_down(hwnd: *mut core::ffi::c_void, hit: u32) -> Option<isize> {
    match hit {
        HTMINBUTTON | HTMAXBUTTON | HTCLOSE => {
            if let Some(state) = hidden_titlebar_hit_test_map()
                .lock()
                .expect("hidden titlebar hit-test map lock")
                .get_mut(&(hwnd as isize))
            {
                state.nc_button_pressed = Some(hit);
            }
            Some(0)
        }
        _ => None,
    }
}

fn hidden_titlebar_nc_button_up(hwnd: *mut core::ffi::c_void, hit: u32) -> Option<isize> {
    let last_pressed = hidden_titlebar_hit_test_map()
        .lock()
        .expect("hidden titlebar hit-test map lock")
        .get_mut(&(hwnd as isize))
        .and_then(|state| state.nc_button_pressed.take())?;

    match (hit, last_pressed) {
        (HTMINBUTTON, HTMINBUTTON) => {
            unsafe {
                ShowWindowAsync(hwnd, SW_MINIMIZE);
            }
            Some(0)
        }
        (HTMAXBUTTON, HTMAXBUTTON) => {
            let is_maximized = unsafe { IsZoomed(hwnd) != 0 };
            unsafe {
                ShowWindowAsync(hwnd, if is_maximized { SW_NORMAL } else { SW_MAXIMIZE });
            }
            Some(0)
        }
        (HTCLOSE, HTCLOSE) => {
            unsafe {
                PostMessageW(hwnd, WM_CLOSE, 0, 0);
            }
            Some(0)
        }
        _ => None,
    }
}

unsafe fn hidden_titlebar_hit_test(hwnd: *mut core::ffi::c_void, lparam: isize) -> Option<isize> {
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let frame_y = frame_thickness_y(dpi);
    let frame_x = frame_thickness_x(dpi);

    let mut cursor_point = POINT {
        x: (lparam as i32 & 0xFFFF) as i16 as i32,
        y: ((lparam as i32 >> 16) & 0xFFFF) as i16 as i32,
    };
    unsafe {
        ScreenToClient(hwnd, &mut cursor_point);
    }

    let mut drag_area = None;
    if let Some(state) = hidden_titlebar_hit_test_map()
        .lock()
        .expect("hidden titlebar hit-test map lock")
        .get(&(hwnd as isize))
        .cloned()
    {
        let logical_x = cursor_point.x as f32 / state.scale_factor as f32;
        let logical_y = cursor_point.y as f32 / state.scale_factor as f32;
        let point = Point::new(Px(logical_x), Px(logical_y));
        if let Some(area) = window_frame_area_at_point(&state.areas, point) {
            match area {
                WindowFrameArea::Drag => drag_area = Some(HTCAPTION as isize),
                WindowFrameArea::Close => return Some(HTCLOSE as isize),
                WindowFrameArea::Maximize => return Some(HTMAXBUTTON as isize),
                WindowFrameArea::Minimize => return Some(HTMINBUTTON as isize),
            }
        }
    }

    if cursor_point.y < 0 || cursor_point.y > frame_y {
        return drag_area;
    }

    let mut rect = RECT::default();
    unsafe {
        GetWindowRect(hwnd, &mut rect);
    }
    let right = rect.right - rect.left - 1;

    Some(if cursor_point.x <= 0 {
        HTTOPLEFT as isize
    } else if right - 2 * frame_x <= cursor_point.x {
        HTTOPRIGHT as isize
    } else {
        HTTOP as isize
    })
    .or(drag_area)
}

fn frame_thickness_x(dpi: u32) -> i32 {
    let resize_frame_thickness = unsafe { GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi) };
    let padding_thickness = unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    resize_frame_thickness + padding_thickness
}

fn frame_thickness_y(dpi: u32) -> i32 {
    let resize_frame_thickness = unsafe { GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) };
    let padding_thickness = unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    resize_frame_thickness + padding_thickness
}

fn window_frame_area_at_point(
    areas: &[(WindowFrameArea, LayoutBox)],
    point: Point<Px>,
) -> Option<WindowFrameArea> {
    areas.iter().rev().find_map(|(area, bounds)| {
        let x = point.x.get();
        let y = point.y.get();
        (x >= bounds.x
            && x <= bounds.x + bounds.width
            && y >= bounds.y
            && y <= bounds.y + bounds.height)
            .then_some(*area)
    })
}

unsafe fn install_hidden_titlebar_subclass(hwnd: *mut core::ffi::c_void) {
    let mut map = hidden_titlebar_wndproc_map()
        .lock()
        .expect("window proc map lock");
    let key = hwnd as isize;
    if map.contains_key(&key) {
        return;
    }

    let original_proc = unsafe { GetWindowLongPtrW(hwnd, GWLP_WNDPROC) };
    map.insert(key, original_proc);
    unsafe {
        SetWindowLongPtrW(
            hwnd,
            GWLP_WNDPROC,
            hidden_titlebar_wndproc as *const () as isize,
        );
    }
}
