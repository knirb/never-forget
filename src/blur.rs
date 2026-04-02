use iced::window;
use iced::Task;

use objc2::MainThreadMarker;
use objc2_app_kit::{
    NSAutoresizingMaskOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
    NSVisualEffectState, NSVisualEffectView,
};

/// Apply a macOS vibrancy blur effect behind the window content.
///
/// Restructures the window's view hierarchy:
///   NSWindow.contentView = blur_view (NSVisualEffectView)
///     └── iced_view (original content view, now a subview)
///
/// This way the blur renders behind iced's Metal/wgpu content.
pub fn apply_blur(window_id: window::Id) -> Task<()> {
    window::run_with_handle(window_id, |handle| {
        let raw = handle.as_raw();
        if let iced::window::raw_window_handle::RawWindowHandle::AppKit(appkit) = raw {
            let ns_view_ptr = appkit.ns_view.as_ptr() as *const NSView;
            unsafe {
                let iced_view: &NSView = &*ns_view_ptr;

                let Some(window) = iced_view.window() else {
                    tracing::warn!("Could not get NSWindow from iced view");
                    return;
                };

                let frame = iced_view.frame();
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

                // Make iced's view auto-resize within the blur view
                iced_view.setAutoresizingMask(
                    NSAutoresizingMaskOptions::ViewWidthSizable
                        | NSAutoresizingMaskOptions::ViewHeightSizable,
                );

                // Swap: set blur_view as the window's contentView,
                // then add iced's view as a subview on top
                blur_view.addSubview(iced_view);
                window.setContentView(Some(&blur_view));
            }
        }
    })
}
