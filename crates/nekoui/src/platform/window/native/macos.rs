use std::ffi::CStr;

use objc2::msg_send;
use winit::platform::macos::WindowAttributesExtMacOS;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window as WinitWindow;
use winit::window::WindowAttributes;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::{NSView, NSWindow, NSWindowButton, NSWindowTitleVisibility};
use objc2_foundation::{
    NSDictionary, NSGlobalDomain, NSPoint, NSRect, NSSize, NSString, NSUserDefaults,
};

use crate::element::WindowFrameArea;
use crate::platform::window::WindowOptions;
use crate::scene::LayoutBox;

pub(crate) fn decorate_attributes(
    mut attributes: WindowAttributes,
    options: &WindowOptions,
) -> WindowAttributes {
    log::debug!(
        "macos decorate_attributes show_titlebar={}",
        options.show_titlebar
    );
    attributes = attributes.with_decorations(true);
    if !options.show_titlebar {
        attributes = attributes
            .with_movable_by_window_background(true)
            .with_title_hidden(true)
            .with_fullsize_content_view(true)
            .with_titlebar_transparent(true)
            .with_has_shadow(true);
    }
    attributes
}

pub(crate) fn apply_post_create(window: &WinitWindow, options: &WindowOptions) {
    if !options.show_titlebar {
        with_ns_window(window, |ns_window| {
            ns_window.setTitlebarAppearsTransparent(true);
            ns_window.setTitleVisibility(NSWindowTitleVisibility::NSWindowTitleHidden);
            ns_window.setMovableByWindowBackground(true);
            ns_window.setHasShadow(true);
        });
    }
}

pub(crate) fn perform_window_frame_action(window: &WinitWindow, area: WindowFrameArea) -> bool {
    with_ns_window(window, |ns_window| match area {
        WindowFrameArea::Drag => {}
        WindowFrameArea::Close => ns_window.close(),
        WindowFrameArea::Minimize => ns_window.miniaturize(None),
        WindowFrameArea::Maximize => ns_window.zoom(None),
    })
    .is_some()
}

pub(crate) fn titlebar_double_click(window: &WinitWindow) -> bool {
    with_ns_window(window, |ns_window| {
        let defaults = unsafe { NSUserDefaults::standardUserDefaults() };
        let key = NSString::from_str("AppleActionOnDoubleClick");
        let action = unsafe {
            defaults.persistentDomainForName(NSGlobalDomain).and_then(
                |dict: Retained<NSDictionary<NSString, AnyObject>>| dict.objectForKey(&key),
            )
        };

        let action_string = action
            .map(|obj| unsafe {
                let utf8: *const i8 = msg_send![&*obj, UTF8String];
                if utf8.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(utf8).to_string_lossy().into_owned()
                }
            })
            .unwrap_or_default();

        match action_string.as_str() {
            "Minimize" => ns_window.miniaturize(None),
            "Maximize" => ns_window.zoom(None),
            _ => {}
        }
    })
    .is_some()
}

pub(crate) fn update_standard_window_buttons(
    window: &WinitWindow,
    scale_factor: f64,
    areas: &[(WindowFrameArea, LayoutBox)],
) {
    with_ns_window(window, |ns_window| {
        let Some(content_view) = ns_window.contentView() else {
            return;
        };
        let content_bounds = content_view.bounds();
        let content_height = content_bounds.size.height;

        for (area, bounds) in areas {
            let button_kind = match area {
                WindowFrameArea::Close => Some(NSWindowButton::NSWindowCloseButton),
                WindowFrameArea::Minimize => Some(NSWindowButton::NSWindowMiniaturizeButton),
                WindowFrameArea::Maximize => Some(NSWindowButton::NSWindowZoomButton),
                WindowFrameArea::Drag => None,
            };

            let Some(button_kind) = button_kind else {
                continue;
            };
            let Some(button) = ns_window.standardWindowButton(button_kind) else {
                continue;
            };

            let frame = button_frame_for_area(*bounds, content_height, scale_factor);
            unsafe {
                button.setFrame(frame);
            }
        }
    });
}

fn button_frame_for_area(bounds: LayoutBox, content_height: f64, scale_factor: f64) -> NSRect {
    let scale = scale_factor as f32;
    let origin_x = bounds.x / scale;
    let origin_y = content_height as f32 - ((bounds.y + bounds.height) / scale);
    let width = bounds.width / scale;
    let height = bounds.height / scale;
    NSRect::new(
        NSPoint::new(origin_x as f64, origin_y as f64),
        NSSize::new(width as f64, height as f64),
    )
}

fn with_ns_window<R>(window: &WinitWindow, f: impl FnOnce(&NSWindow) -> R) -> Option<R> {
    let ns_view = match window.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::AppKit(handle) => handle.ns_view,
            _ => {
                log::debug!("macos native window access skipped: non-AppKit raw handle");
                return None;
            }
        },
        Err(error) => {
            log::debug!("macos native window access failed: {error}");
            return None;
        }
    };

    let Some(ns_view) = (unsafe { Retained::<NSView>::retain(ns_view.as_ptr().cast()) }) else {
        log::debug!("macos native window access failed: NSView retain returned null");
        return None;
    };
    let Some(ns_window) = ns_view.window() else {
        log::debug!("macos native window access failed: NSView had no NSWindow");
        return None;
    };
    Some(f(&ns_window))
}
