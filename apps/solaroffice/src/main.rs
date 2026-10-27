//! SolarOffice — the desktop app. Dioxus (desktop webview) + TailwindCSS; the home screen is the
//! first milestone, the module editors follow.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai_state;
mod chat;
mod files;
mod icons;
mod state;
mod ui;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};

fn main() {
    let mut window = WindowBuilder::new()
        .with_title("SolarOffice")
        .with_inner_size(LogicalSize::new(1360.0, 860.0))
        .with_min_inner_size(LogicalSize::new(820.0, 540.0));

    #[cfg(target_os = "macos")]
    {
        use dioxus::desktop::tao::platform::macos::WindowBuilderExtMacOS;
        // The tab strip is the title bar: content runs under the traffic lights.
        window = window.with_titlebar_transparent(true).with_fullsize_content_view(true).with_title_hidden(true);
    }
    #[cfg(not(target_os = "macos"))]
    {
        window = window.with_decorations(true);
    }

    let mut config = Config::new().with_window(window).with_background_color((250, 250, 250, 255)).with_disable_context_menu(!cfg!(debug_assertions));
    // macOS keeps the standard app menu (Edit › Copy/Paste, ⌘Q); elsewhere a menu bar inside the
    // window would sit above the tab strip.
    if !cfg!(target_os = "macos") {
        config = config.with_menu(None);
    }

    dioxus::LaunchBuilder::desktop().with_cfg(config).launch(ui::App);
}
