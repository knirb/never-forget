use iced::window;
use iced::Task;

use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
};

/// Apply a macOS vibrancy blur effect behind the window content.
///
/// Inserts an NSVisualEffectView into the window's theme frame (the
/// superview of the contentView), positioned below iced's content view.
/// This avoids reparenting iced's view which would break its Metal renderer.
pub fn apply_blur(window_id: window::Id) -> Task<()> {
    window::run_with_handle(window_id, |handle| {
        let raw = handle.as_raw();
        if let iced::window::raw_window_handle::RawWindowHandle::AppKit(appkit) = raw {
            let ns_view_ptr = appkit.ns_view.as_ptr() as *const NSView;
            unsafe {
                let iced_view: &NSView = &*ns_view_ptr;

                let Some(window) = iced_view.window() else {
                    return;
                };

                let frame = window.frame();
                // Use a rect covering the full window in local coordinates
                let local_rect = objc2_foundation::NSRect::new(
                    objc2_foundation::NSPoint::new(0.0, 0.0),
                    frame.size,
                );

                let mtm = MainThreadMarker::new_unchecked();
                let blur_view = NSVisualEffectView::initWithFrame(
                    mtm.alloc(),
                    local_rect,
                );
                blur_view.setMaterial(NSVisualEffectMaterial::FullScreenUI);
                blur_view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
                blur_view.setState(NSVisualEffectState::Active);
                blur_view.setAutoresizingMask(
                    NSAutoresizingMaskOptions::ViewWidthSizable
                        | NSAutoresizingMaskOptions::ViewHeightSizable,
                );

                // Insert into the theme frame (superview of contentView),
                // positioned below iced's content view
                if let Some(superview) = iced_view.superview() {
                    superview.addSubview_positioned_relativeTo(
                        &blur_view,
                        NSWindowOrderingMode::Below,
                        Some(iced_view),
                    );
                } else {
                    // Fallback: add as subview of iced's view at the back
                    iced_view.addSubview_positioned_relativeTo(
                        &blur_view,
                        NSWindowOrderingMode::Below,
                        None,
                    );
                }
            }
        }
    })
}
