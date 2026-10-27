//! The window: tab strip on top, then Home (sidebar + page) or the active module tab, with menus,
//! dialogs and the toast floating above.

mod ask;
mod home;
mod menus;
mod module_tab;
mod sidebar;
mod tabs;

use dioxus::prelude::*;
use solar_core::settings::{HomeState, Store};
use solar_core::{AppKind, now_ms};

use crate::state::{Ctx, Menu, MenuKind, Modal, Page};

/// Tailwind's output (`assets/tailwind.css`, built from `tailwind.css`; `dx serve` rebuilds it).
const CSS: &str = include_str!("../../assets/tailwind.css");

/// Whether ⌘ (macOS) or Ctrl (elsewhere) is held.
pub fn primary(m: Modifiers) -> bool {
    if cfg!(target_os = "macos") { m.contains(Modifiers::META) } else { m.contains(Modifiers::CONTROL) }
}

/// First launch: copy the example documents into `<save folder>/Examples` and list them.
fn install_examples(state: &mut HomeState) {
    if state.settings.examples_installed {
        return;
    }
    let Some(src) = crate::files::bundled_examples() else { return };
    let save = state.settings.save_folder();
    let dest = save.join("Examples");
    match solar_core::folders::copy_missing(&src, &dest) {
        Ok(_) => {
            state.settings.examples_installed = true;
            let now = now_ms();
            let listing = solar_core::folders::list(&dest);
            for (i, (p, _)) in listing.files.iter().enumerate().rev() {
                state.library.touch(p, now - i as i64);
            }
            state.settings.set_expanded(&save, true);
        }
        Err(e) => log::warn!("examples: {e}"),
    }
}

#[component]
pub fn App() -> Element {
    let mut ctx = use_hook(|| {
        let store = Store::default_location();
        let mut state = store.load();
        install_examples(&mut state);
        let _ = std::fs::create_dir_all(state.settings.save_folder());
        let system_dark = matches!(dioxus::desktop::window().window.theme(), dioxus::desktop::tao::window::Theme::Dark);
        Ctx::new(state, store, system_dark)
    });
    use_context_provider(|| ctx);

    // Persist the home state whenever it changes.
    use_effect(move || {
        let state = ctx.home.read();
        if let Err(e) = ctx.store.peek().save(&state) {
            log::warn!("saving home.json: {e}");
        }
    });

    let dark = ctx.dark();
    let theme_class = if dark { "dark" } else { "" };
    let on_home = ctx.active.read().is_none();

    let onkeydown = move |evt: KeyboardEvent| {
        let m = evt.modifiers();
        let shift = m.contains(Modifiers::SHIFT);
        let alt = m.contains(Modifiers::ALT);
        if evt.key() == Key::Escape {
            if ctx.menu.peek().is_some() || ctx.modal.peek().is_some() {
                ctx.close_overlays();
            } else if ctx.renaming.peek().is_some() {
                ctx.renaming.set(None);
            }
            return;
        }
        // ⌃Tab / ⌃⇧Tab cycle tabs on every platform.
        if evt.key() == Key::Tab && m.contains(Modifiers::CONTROL) {
            evt.prevent_default();
            ctx.cycle(if shift { -1 } else { 1 });
            return;
        }
        if !primary(m) {
            return;
        }
        let digit = match evt.code() {
            Code::Digit1 => Some(1),
            Code::Digit2 => Some(2),
            Code::Digit3 => Some(3),
            Code::Digit4 => Some(4),
            Code::Digit5 => Some(5),
            Code::Digit6 => Some(6),
            Code::Digit7 => Some(7),
            Code::Digit8 => Some(8),
            Code::Digit9 => Some(9),
            _ => None,
        };
        if let Some(d) = digit {
            evt.prevent_default();
            if shift {
                // ⌘⇧1–7: a new file per module.
                if let Some(k) = AppKind::ALL.get(d - 1) {
                    ctx.new_file(*k);
                }
            } else if d == 9 {
                let n = ctx.tabs.peek().len();
                ctx.select_index(n);
            } else {
                ctx.select_index(d - 1);
            }
            return;
        }
        // Shift and Option variants first: the plain letter also matches them.
        match evt.code() {
            Code::KeyT if shift => ctx.reopen_closed(),
            Code::KeyT => ctx.menu.set(Some(Menu { kind: MenuKind::NewFile, x: 0.0, y: 0.0 })),
            Code::KeyW => {
                let active = *ctx.active.peek();
                if let Some(id) = active {
                    ctx.close_tab(id);
                }
            }
            Code::KeyO => ctx.open_dialog(),
            Code::KeyN => ctx.new_file(AppKind::Word),
            Code::Comma => ctx.modal.set(Some(Modal::Settings)),
            Code::ArrowRight if alt => ctx.cycle(1),
            Code::ArrowLeft if alt => ctx.cycle(-1),
            Code::KeyF if ctx.active.peek().is_none() => {
                if ctx.page.peek().clone() == Page::AskAi {
                    ctx.go(Page::Recent);
                }
                if let Some(b) = ctx.search_box.peek().clone() {
                    spawn(async move {
                        let _ = b.set_focus(true).await;
                    });
                }
            }
            _ => return,
        }
        evt.prevent_default();
    };

    rsx! {
        style { dangerous_inner_html: CSS }
        div {
            class: "{theme_class} h-screen w-screen overflow-hidden",
            div {
                class: "flex flex-col h-full bg-app text-ink font-sans text-[13px] antialiased select-none outline-none",
                tabindex: "-1",
                onmounted: move |e| async move {
                    let _ = e.set_focus(true).await;
                },
                onkeydown,
                tabs::TabStrip {}
                div { class: "flex flex-1 min-h-0",
                    if on_home {
                        sidebar::Sidebar {}
                        main { class: "flex-1 min-w-0 overflow-y-auto",
                            match ctx.page.read().clone() {
                                Page::AskAi => rsx! { ask::AskAi {} },
                                _ => rsx! { home::HomePage {} },
                            }
                        }
                    } else {
                        module_tab::ModuleTab {}
                    }
                }
                menus::Overlays {}
            }
        }
    }
}
