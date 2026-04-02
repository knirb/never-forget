use iced::window;
use iced::Task;

use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
};

/// Apply a macOS vibrancy blur effect behind the window content.
/// Inserts an NSVisualEffectView with dark material behind iced's content view.
pub fn apply_blur(window_id: window::Id) -> Task<()> {
    window::run_with_handle(window_id, |handle| {
        let raw = handle.as_raw();
        if let iced::window::raw_window_handle::RawWindowHandle::AppKit(appkit) = raw {
            let ns_view_ptr = appkit.ns_view.as_ptr() as *const NSView;
            unsafe {
                let ns_view: &NSView = &*ns_view_ptr;
                let frame = ns_view.frame();

                // We're on the main thread (iced's event loop runs on main thread)
                let mtm = MainThreadMarker::new_unchecked();
                let blur_view = NSVisualEffectView::initWithFrame(
                    mtm.alloc(),
                    frame,
                );

                blur_view.setMaterial(NSVisualEffectMaterial::FullScreenUI);
                blur_view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
                blur_view.setState(NSVisualEffectState::Active);
                blur_view.setAutoresizingMask(
                    NSAutoresizingMaskOptions::ViewWidthSizable
                        | NSAutoresizingMaskOptions::ViewHeightSizable,
                );

                // Insert behind all other subviews so iced content draws on top
                ns_view.addSubview_positioned_relativeTo(
                    &blur_view,
                    NSWindowOrderingMode::Below,
                    None,
                );
            }
        }
    })
}
