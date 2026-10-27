//! The Settings window: a section list on the left (General, AI providers, AI models) and the
//! section on the right. Adding or editing a provider, and adding a model, open a second screen
//! inside the section with a way back. General changes apply at once; AI changes are saved when
//! you click Done (Cancel drops them).

use dioxus::prelude::*;
use solar_ai::{Category, KeySource, ModelRef, Protocol, ProviderConfig, ProviderKind, Region, Scope, Service};
use solar_core::settings::ThemePref;

use super::pickers::{Dropdown, Entry, model_entries, provider_model_entries};
use crate::ai_state::*;
use crate::icons::{self, Icon};
use crate::state::{Ctx, Modal};

const INPUT: &str = "h-9 px-3 rounded-lg border border-line bg-app outline-none focus:border-accent select-text placeholder:text-muted";
const BTN: &str = "px-3.5 h-8 rounded-lg border border-line hover:bg-hover";
const BTN_PRIMARY: &str = "px-3.5 h-8 rounded-lg bg-accent text-on-accent font-medium hover:brightness-110";

/// `~/Documents/SolarOffice` for display.
fn tilde(p: &std::path::Path) -> String {
    let s = p.display().to_string();
    match dirs_home() {
        Some(h) if s.starts_with(&h) => format!("~{}", &s[h.len()..]),
        _ => s,
    }
}

fn dirs_home() -> Option<String> {
    std::env::var("HOME").ok().filter(|h| !h.is_empty())
}

#[component]
pub fn SettingsDialog() -> Element {
    let mut ctx = use_context::<Ctx>();
    let tab = *ctx.settings_tab.read();
    let has_draft = ctx.ai_ui.read().draft.is_some();
    let error = ctx.ai_ui.read().error.clone();

    // Finished connection tests land in the draft's state.
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            if ctx.ai_ui.peek().any_running() {
                ctx.ai_ui.write().poll_tests();
            }
        }
    });

    let nav = |t: SettingsTab, icon: &'static str, label: &'static str| {
        let on = tab == t;
        let look = if on { "bg-sel text-ink font-medium" } else { "text-ink2 hover:bg-hover hover:text-ink" };
        rsx! {
            button {
                class: "flex items-center gap-2.5 px-2.5 py-[7px] rounded-md text-left {look}",
                onclick: move |_| {
                    ctx.settings_tab.set(t);
                    ctx.ai_ui.write().screen = AiScreen::List;
                    if t != SettingsTab::General {
                        ctx.ensure_draft();
                    }
                },
                Icon { svg: icon, size: 16, class: if on { "text-accent" } else { "" } }
                "{label}"
            }
        }
    };

    rsx! {
        div { class: "w-[min(960px,calc(100vw-80px))] h-[min(720px,calc(100vh-80px))] flex flex-col",
            div { class: "flex items-center px-6 pt-5 pb-3",
                span { class: "flex-1 text-[17px] font-semibold", "Settings" }
                button { class: "p-1 rounded text-muted hover:text-ink hover:bg-hover", title: "Close without saving AI changes", onclick: move |_| ctx.close_settings(), Icon { svg: icons::X, size: 16 } }
            }
            div { class: "flex flex-1 min-h-0 border-t border-line",
                div { class: "w-[196px] shrink-0 p-3 flex flex-col gap-0.5 border-r border-line",
                    {nav(SettingsTab::General, icons::SETTINGS, "General")}
                    {nav(SettingsTab::Providers, icons::SPARKLES, "AI providers")}
                    {nav(SettingsTab::Models, icons::CHAT, "AI models")}
                }
                div { class: "flex-1 min-w-0 overflow-y-auto px-6 py-5",
                    match tab {
                        SettingsTab::General => rsx! { General {} },
                        SettingsTab::Providers | SettingsTab::Models => rsx! { AiSection { tab } },
                    }
                }
            }
            if let Some(e) = error {
                div { class: "px-6 pt-3 text-[#D92D20] dark:text-[#FF7A6E]", "{e}" }
            }
            div { class: "flex items-center gap-3 px-6 py-4 border-t border-line",
                if has_draft {
                    span { class: "text-[12px] text-muted", "AI changes are saved when you click Done." }
                }
                div { class: "flex-1" }
                if has_draft {
                    button { class: BTN, onclick: move |_| ctx.close_settings(), "Cancel" }
                }
                button { class: BTN_PRIMARY, onclick: move |_| { ctx.save_settings(); }, "Done" }
            }
        }
    }
}

#[component]
fn Heading(title: &'static str, sub: &'static str) -> Element {
    rsx! {
        div { class: "text-[18px] font-semibold", "{title}" }
        if !sub.is_empty() {
            div { class: "mt-0.5 mb-3 text-[12.5px] text-muted max-w-[640px]", "{sub}" }
        }
    }
}

#[component]
fn BackLink(label: &'static str, onclick: EventHandler<()>) -> Element {
    rsx! {
        button { class: "mb-2 text-accent font-medium hover:underline", onclick: move |_| onclick.call(()), "‹  {label}" }
    }
}

#[component]
fn Field(label: &'static str, #[props(default)] hint: &'static str, children: Element) -> Element {
    rsx! {
        div { class: "grid grid-cols-[150px_minmax(0,1fr)] gap-x-4 items-start py-1.5",
            div { class: "pt-2 text-ink2", "{label}" }
            div {
                {children}
                if !hint.is_empty() {
                    div { class: "mt-1 text-[12px] text-muted", "{hint}" }
                }
            }
        }
    }
}

// ---- General ------------------------------------------------------------------------------

#[component]
fn General() -> Element {
    let mut ctx = use_context::<Ctx>();
    let settings = ctx.home.read().settings.clone();
    let save = settings.save_folder();
    let name = settings.display_name.clone().unwrap_or_default();
    let placeholder = solar_core::settings::capitalise_login(&std::env::var("USER").unwrap_or_default()).unwrap_or_else(|| "Your name".into());
    let data = ctx.store.read().path();
    rsx! {
        Heading { title: "General", sub: "" }
        div { class: "max-w-[640px] flex flex-col gap-3",
            div {
                div { class: "mb-1.5 font-medium", "Theme" }
                div { class: "inline-flex p-1 rounded-lg bg-pill gap-0.5",
                    for pref in ThemePref::ALL {
                        button {
                            key: "{pref.label()}",
                            class: if settings.theme == pref { "px-3 h-7 rounded-md bg-card shadow-[0_1px_2px_rgba(0,0,0,.08)] font-medium" } else { "px-3 h-7 rounded-md text-ink2 hover:text-ink" },
                            onclick: move |_| ctx.home.write().settings.theme = pref,
                            "{pref.label()}"
                        }
                    }
                }
            }
            div {
                div { class: "mb-1.5 font-medium", "Name in the greeting" }
                input {
                    class: "w-full {INPUT}",
                    initial_value: "{name}",
                    placeholder: "{placeholder}",
                    oninput: move |e| {
                        let v = e.value();
                        ctx.home.write().settings.display_name = if v.trim().is_empty() { None } else { Some(v) };
                    },
                }
            }
            div {
                div { class: "mb-1.5 font-medium", "Save new files in" }
                div { class: "flex items-center gap-2",
                    div { class: "flex-1 min-w-0 h-9 px-3 flex items-center rounded-lg border border-line bg-app text-ink2 truncate", title: "{save.display()}", "{tilde(&save)}" }
                    button {
                        class: "flex items-center gap-1.5 {BTN} h-9 shrink-0",
                        onclick: move |_| {
                            spawn(async move {
                                if let Some(dir) = rfd::AsyncFileDialog::new().set_title("Save new files in").pick_folder().await {
                                    ctx.home.write().settings.default_folder = Some(dir.path().to_path_buf());
                                }
                            });
                        },
                        Icon { svg: icons::FOLDER, size: 14 }
                        "Change…"
                    }
                }
                div { class: "mt-1 text-[12px] text-muted", "Also the first folder in the sidebar." }
            }
            div { class: "mt-2 text-[12px] text-muted leading-relaxed",
                div { "Data: {tilde(&data)}" }
                div { "AI settings: {tilde(&solar_ai::AiConfig::file(&solar_core::settings::config_dir()))}" }
                div {
                    "SolarOffice {solar_core::VERSION} · "
                    button { class: "text-accent hover:underline", onclick: move |_| ctx.modal.set(Some(Modal::About)), "About" }
                }
            }
        }
    }
}

// ---- AI sections --------------------------------------------------------------------------

#[component]
fn AiSection(tab: SettingsTab) -> Element {
    let ctx = use_context::<Ctx>();
    let screen = ctx.ai_ui.read().screen.clone();
    match (tab, screen) {
        (SettingsTab::Providers, AiScreen::List) => rsx! { ProvidersPage {} },
        (SettingsTab::Providers, AiScreen::Edit(id)) => rsx! { ProviderEditPage { id } },
        (SettingsTab::Providers, AiScreen::AddKind) => rsx! { AddKindPage {} },
        (SettingsTab::Providers, AiScreen::AddForm(_)) => rsx! { AddFormPage {} },
        (SettingsTab::Models, AiScreen::AddModel(_)) => rsx! { AddModelPage {} },
        (SettingsTab::Models, _) => rsx! { ModelsPage {} },
        _ => rsx! { ProvidersPage {} },
    }
}

fn badge(kind: ProviderKind, size: u32) -> Element {
    let (bg, letter) = kind_badge(kind);
    let fs = if letter.chars().count() > 1 { size as f32 * 0.36 } else { size as f32 * 0.44 };
    rsx! {
        span {
            class: "shrink-0 inline-flex items-center justify-center rounded-[8px] text-white font-semibold",
            style: "width:{size}px;height:{size}px;background:{bg};font-size:{fs}px",
            "{letter}"
        }
    }
}

fn region_heading(region: Region, count: usize) -> Element {
    rsx! {
        div { class: "mt-4 mb-1.5 flex items-center gap-2 text-[11.5px] font-semibold tracking-wide text-muted",
            span { "{region.label().to_uppercase()}" }
            span { class: "font-medium", "{count}" }
        }
    }
}

fn tone_class(t: Tone) -> &'static str {
    match t {
        Tone::Good => "bg-accent-soft text-accent",
        Tone::Bad => "bg-[#FEE4E2] text-[#D92D20] dark:bg-[#4A1D1A] dark:text-[#FF7A6E]",
        Tone::Muted => "bg-hover text-muted",
    }
}

#[component]
fn ProvidersPage() -> Element {
    let mut ctx = use_context::<Ctx>();
    let hub = ctx.ai.read().clone();
    let ui = ctx.ai_ui.read();
    let draft = ui.draft.clone().unwrap_or_default();
    let rows: Vec<(Region, Vec<(ProviderConfig, String)>)> = Region::ALL
        .into_iter()
        .map(|r| {
            let list = draft
                .providers
                .iter()
                .filter(|p| p.region() == r)
                .map(|p| {
                    let model = p.default_model.clone().or_else(|| provider_fallback(&hub, &ui.tests, p));
                    let line = match &model {
                        Some(m) => {
                            let info = hub.catalog().info(p.kind, m);
                            let auto = if p.default_model.is_none() { " (automatic)" } else { "" };
                            format!("Default: {}{auto} · {}", info.name, info.price_label())
                        }
                        None if p.kind.needs_endpoint() => format!("Open it to enter your endpoint ({})", p.kind.endpoint_hint()),
                        None => "No models yet — open it and click Test connection".into(),
                    };
                    (p.clone(), line)
                })
                .collect();
            (r, list)
        })
        .filter(|(_, l): &(Region, Vec<_>)| !l.is_empty())
        .collect();
    drop(ui);

    rsx! {
        div { class: "flex items-start gap-4",
            div { class: "flex-1 min-w-0",
                Heading { title: "AI providers", sub: "The services and local servers SolarOffice can use, by region. Click one to change its key, endpoint or default model." }
            }
            button {
                class: "flex items-center gap-1.5 {BTN_PRIMARY} shrink-0",
                onclick: move |_| ctx.ai_ui.write().screen = AiScreen::AddKind,
                Icon { svg: icons::PLUS, size: 14 }
                "Add provider"
            }
        }
        for (region, list) in rows {
            {region_heading(region, list.len())}
            for (p, line) in list {
                {
                    let id = p.id.clone();
                    let (text, tone) = status(&p);
                    let name = p.display_name();
                    rsx! {
                        button {
                            key: "{p.id}",
                            class: "w-full flex items-center gap-3 px-3.5 h-16 mb-2 rounded-xl bg-hover/60 hover:bg-hover text-left",
                            onclick: move |_| ctx.ai_ui.write().screen = AiScreen::Edit(id.clone()),
                            {badge(p.kind, 36)}
                            div { class: "flex-1 min-w-0",
                                div { class: "flex items-center gap-2",
                                    span { class: if p.enabled { "font-semibold text-[14px]" } else { "font-semibold text-[14px] text-muted" }, "{name}" }
                                    span { "{flag_emoji(&p.country())}" }
                                    if let Some(tag) = service_tag(p.service()) {
                                        span { class: "px-1.5 rounded text-[11px] leading-[16px] font-medium bg-hover text-ink2", "{tag}" }
                                    }
                                }
                                div { class: "mt-0.5 text-[12.5px] text-ink2 truncate", "{line}" }
                            }
                            span { class: "shrink-0 px-2.5 h-[22px] leading-[22px] rounded-full text-[12px] font-medium {tone_class(tone)}", "{text}" }
                            Icon { svg: icons::CHEVRON_RIGHT, size: 14, class: "text-muted shrink-0" }
                        }
                    }
                }
            }
        }
    }
}

/// Apply `f` to the provider being edited: the draft's entry, or the one on the Add screen.
fn mutate_provider(ctx: &mut Ctx, adding: bool, id: &str, f: impl FnOnce(&mut ProviderConfig)) {
    let mut ui = ctx.ai_ui.write();
    if adding {
        if let AiScreen::AddForm(p) = &mut ui.screen {
            f(p);
        }
    } else if let Some(p) = ui.draft.as_mut().and_then(|d| d.provider_mut(id)) {
        f(p);
    }
}

#[component]
fn ProviderForm(p: ProviderConfig, adding: bool) -> Element {
    let mut ctx = use_context::<Ctx>();
    let hub = ctx.ai.read().clone();
    let ui = ctx.ai_ui.read();
    let entries = provider_model_entries(&hub, &ui.tests, &p);
    let test = match ui.tests.get(&p.id) {
        Some(TestState::Running(_)) => Some((None, true)),
        Some(TestState::Ok(list)) => {
            let text = match ui.notes.get(&p.id) {
                Some(note) => format!("✓ Connected — {note}"),
                None => {
                    let shown: Vec<&str> = list.iter().take(5).map(String::as_str).collect();
                    let more = if list.len() > 5 { format!(" and {} more", list.len() - 5) } else { String::new() };
                    format!("✓ Connected — {} models ({}{more})", list.len(), shown.join(", "))
                }
            };
            Some((Some((text, true)), false))
        }
        Some(TestState::Err(e)) => Some((Some((format!("✗ {e}"), false)), false)),
        None => None,
    };
    drop(ui);
    let running = test.as_ref().is_some_and(|(_, r)| *r);
    let id = p.id.clone();
    let default_label = {
        let fallback = provider_fallback(&hub, &ctx.ai_ui.read().tests, &p);
        match (&p.default_model, fallback) {
            (Some(m), _) => {
                let info = hub.catalog().info(p.kind, m);
                format!("{} · {}", info.name, info.price_label())
            }
            (None, Some(m)) => {
                let info = hub.catalog().info(p.kind, &m);
                format!("{} (automatic) · {}", info.name, info.price_label())
            }
            (None, None) if p.kind == ProviderKind::Microsoft => "Add your deployment names above".into(),
            (None, None) => "Test the connection to list models".into(),
        }
    };
    let key_hint = match p.key_source() {
        KeySource::Env(v) => format!("Using ${v}"),
        _ if p.kind.needs_key() => match p.kind.key_env().first() {
            Some(v) => format!("Required (or set ${v})"),
            None => "Required".into(),
        },
        _ => "Optional".into(),
    };
    let url_hint = if p.kind.needs_endpoint() { format!("Required, e.g. {}", p.kind.endpoint_hint()) } else { p.base_url() };
    let service_use = if p.service() == Service::Translate { "translate documents and selections" } else { "create and edit images" };
    let models_text = p.models.join(", ");
    let models_label = if p.kind == ProviderKind::Microsoft { "Deployments" } else { "Models" };
    let models_hint = if p.kind == ProviderKind::Microsoft { "Your deployment names, separated by commas" } else { "Filled in by Test connection, or type ids separated by commas" };

    let (i1, i2, i3, i4, i5, i6, i7, i8, i9) = (id.clone(), id.clone(), id.clone(), id.clone(), id.clone(), id.clone(), id.clone(), id.clone(), id.clone());
    let p_test = p.clone();

    rsx! {
        div { class: "flex items-center gap-3",
            {badge(p.kind, 40)}
            div {
                div { class: "flex items-center gap-2",
                    span { class: "text-[18px] font-semibold", if adding { "Add {p.kind.label()}" } else { "{p.display_name()}" } }
                    span { "{flag_emoji(&p.country())}" }
                    if p.region() != Region::Local {
                        span { class: "text-[11.5px] font-medium text-muted", "{p.region().short()}" }
                    }
                }
                div { class: "text-[12.5px] text-muted max-w-[560px]", "{p.kind.help()}" }
            }
        }
        div { class: "mt-4 max-w-[720px]",
            if !adding {
                label { class: "flex items-center gap-2 mb-2",
                    input {
                        r#type: "checkbox",
                        class: "accent-[var(--so-accent)]",
                        checked: p.enabled,
                        onchange: move |e| {
                            let on = e.checked();
                            mutate_provider(&mut ctx, false, &i1, |p| p.enabled = on);
                        },
                    }
                    "Use this provider"
                }
            }
            Field { label: "Name",
                input {
                    class: "w-full {INPUT}",
                    initial_value: "{p.name.clone().unwrap_or_default()}",
                    placeholder: "{p.kind.label()}",
                    oninput: move |e| {
                        let v = e.value();
                        mutate_provider(&mut ctx, adding, &i2, |p| p.name = Some(v).filter(|n| !n.trim().is_empty()));
                    },
                }
            }
            if p.kind == ProviderKind::Custom {
                Field { label: "API",
                    select {
                        class: "{INPUT} w-full",
                        onchange: move |e| {
                            let v = e.value();
                            let proto = Protocol::CHAT.into_iter().find(|x| x.label() == v);
                            mutate_provider(&mut ctx, adding, &i3, |p| p.protocol = proto);
                        },
                        for proto in Protocol::CHAT {
                            option { value: "{proto.label()}", selected: p.protocol() == proto, "{proto.label()}" }
                        }
                    }
                }
                Field { label: "Region",
                    div { class: "flex items-center gap-2",
                        select {
                            class: "{INPUT} w-[220px]",
                            onchange: move |e| {
                                let v = e.value();
                                let r = Region::ALL.into_iter().find(|x| x.label() == v);
                                mutate_provider(&mut ctx, adding, &i4, |p| p.region = r);
                            },
                            for r in Region::ALL {
                                option { value: "{r.label()}", selected: p.region() == r, "{r.label()}" }
                            }
                        }
                        input {
                            class: "{INPUT} w-[130px]",
                            maxlength: "2",
                            placeholder: "Country, e.g. NL",
                            initial_value: "{p.country.clone().unwrap_or_default()}",
                            oninput: move |e| {
                                let v = e.value().trim().to_ascii_uppercase();
                                mutate_provider(&mut ctx, adding, &i5, |p| p.country = Some(v).filter(|c| !c.is_empty()));
                            },
                        }
                        span { class: "text-[18px]", "{flag_emoji(&p.country())}" }
                    }
                }
            }
            Field { label: "Endpoint",
                input {
                    class: "w-full {INPUT}",
                    initial_value: "{p.base_url.clone().unwrap_or_default()}",
                    placeholder: "{url_hint}",
                    spellcheck: "false",
                    oninput: move |e| {
                        let v = e.value();
                        mutate_provider(&mut ctx, adding, &i6, |p| p.base_url = Some(v).filter(|u| !u.trim().is_empty()));
                    },
                }
            }
            Field { label: "API key",
                input {
                    class: "w-full {INPUT}",
                    r#type: "password",
                    initial_value: "{p.api_key.clone().unwrap_or_default()}",
                    placeholder: "{key_hint}",
                    autocomplete: "off",
                    spellcheck: "false",
                    oninput: move |e| {
                        let v = e.value();
                        mutate_provider(&mut ctx, adding, &i7, |p| p.api_key = Some(v).filter(|k| !k.trim().is_empty()));
                    },
                }
            }
            if p.kind.lists_own_models() {
                Field { label: models_label,
                    input {
                        class: "w-full {INPUT}",
                        initial_value: "{models_text}",
                        placeholder: "{models_hint}",
                        spellcheck: "false",
                        oninput: move |e| {
                            let v = e.value();
                            mutate_provider(&mut ctx, adding, &i8, |p| p.models = v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect());
                        },
                    }
                }
            }
            Field { label: "Default model",
                Dropdown {
                    label: default_label,
                    entries,
                    width: 480,
                    on_pick: move |key: String| {
                        mutate_provider(&mut ctx, adding, &i9, |p| p.default_model = Some(key).filter(|k| !k.is_empty()));
                    },
                }
            }
            Field { label: "Id", hint: "",
                div { class: "pt-2 text-[12px] text-muted", "{p.id}  (used in ai.toml and model references like {p.id}:<model>)" }
            }
        }
        div { class: "mt-3 flex items-center gap-3 flex-wrap",
            button {
                class: BTN,
                disabled: running,
                onclick: move |_| ctx.ai_ui.write().start_test(&p_test),
                if running { "Testing…" } else { "Test connection" }
            }
            if let Some((Some((text, ok)), _)) = &test {
                span { class: if *ok { "text-[12.5px] text-accent" } else { "text-[12.5px] text-[#D92D20] dark:text-[#FF7A6E]" }, "{text}" }
            }
        }
        if let Some(tag) = service_tag(p.service()) {
            div { class: "mt-2 text-[12px] text-muted max-w-[640px]",
                "{tag} service: modules use it to {service_use}. It doesn't appear in the chat model pickers."
            }
        }
    }
}

#[component]
fn ProviderEditPage(id: String) -> Element {
    let mut ctx = use_context::<Ctx>();
    let p = ctx.ai_ui.read().draft.as_ref().and_then(|d| d.provider(&id).cloned());
    let Some(p) = p else {
        ctx.ai_ui.write().screen = AiScreen::List;
        return rsx! {};
    };
    let rid = id.clone();
    rsx! {
        BackLink { label: "All providers", onclick: move |_| ctx.ai_ui.write().screen = AiScreen::List }
        ProviderForm { p, adding: false }
        div { class: "mt-5",
            button {
                class: "flex items-center gap-1.5 {BTN}",
                onclick: move |_| {
                    let mut ui = ctx.ai_ui.write();
                    if let Some(d) = ui.draft.as_mut() {
                        remove_provider(d, &rid);
                    }
                    ui.screen = AiScreen::List;
                },
                Icon { svg: icons::TRASH, size: 14 }
                "Remove this provider"
            }
        }
    }
}

#[component]
fn AddKindPage() -> Element {
    let mut ctx = use_context::<Ctx>();
    let draft = ctx.ai_ui.read().draft.clone().unwrap_or_default();
    rsx! {
        BackLink { label: "All providers", onclick: move |_| ctx.ai_ui.write().screen = AiScreen::List }
        Heading { title: "Add an AI provider", sub: "Choose a provider by region, or a server on your own machine. You can add the same one more than once (a second Ollama server, a work and a personal account)." }
        for region in Region::ALL {
            {
                let kinds = ProviderKind::in_region(region);
                rsx! {
                    {region_heading(region, kinds.len())}
                    div { class: "grid grid-cols-2 xl:grid-cols-3 gap-2",
                        for kind in kinds {
                            {
                                let added = draft.providers.iter().filter(|x| x.kind == kind).count();
                                let local = kind.region() == Region::Local;
                                let title = if local { format!("+ {}", kind_title(kind)) } else { kind_title(kind).to_string() };
                                let sub = match (service_tag(kind.service()), added) {
                                    (_, n) if n > 0 => format!("✓ Added{} · {}", if n > 1 { format!(" ×{n}") } else { String::new() }, kind.blurb()),
                                    (Some(tag), _) => format!("{tag} · {}", kind.blurb()),
                                    (None, _) => kind.blurb().to_string(),
                                };
                                let free = draft.free_id(kind.default_id());
                                rsx! {
                                    button {
                                        key: "{kind.default_id()}",
                                        class: "flex items-center gap-3 px-3 h-[54px] rounded-xl border border-line bg-card hover:bg-hover hover:border-[color:var(--so-line-strong)] text-left min-w-0",
                                        title: "{kind.help()}",
                                        onclick: move |_| {
                                            let mut p = ProviderConfig::new(kind);
                                            p.id = free.clone();
                                            if p.id != kind.default_id() {
                                                p.name = Some(format!("{} ({})", kind.label(), p.id));
                                            }
                                            ctx.ai_ui.write().screen = AiScreen::AddForm(Box::new(p));
                                        },
                                        {badge(kind, 30)}
                                        div { class: "min-w-0 flex-1",
                                            div { class: "flex items-center gap-1.5",
                                                span { class: "font-semibold text-[13.5px] truncate", "{title}" }
                                                span { "{flag_emoji(kind.country())}" }
                                            }
                                            div { class: if added > 0 { "text-[11.5px] text-accent truncate" } else { "text-[11.5px] text-muted truncate" }, "{sub}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn AddFormPage() -> Element {
    let mut ctx = use_context::<Ctx>();
    let AiScreen::AddForm(p) = ctx.ai_ui.read().screen.clone() else { return rsx! {} };
    let p = *p;
    let p_add = p.clone();
    rsx! {
        BackLink { label: "Choose another provider", onclick: move |_| ctx.ai_ui.write().screen = AiScreen::AddKind }
        ProviderForm { p, adding: true }
        div { class: "mt-5 flex gap-2",
            button {
                class: BTN_PRIMARY,
                onclick: move |_| {
                    let mut p = p_add.clone();
                    let mut ui = ctx.ai_ui.write();
                    // A test's model list becomes the provider's list for local servers and endpoints.
                    if p.models.is_empty()
                        && p.kind.lists_own_models()
                        && let Some(TestState::Ok(list)) = ui.tests.get(&p.id)
                    {
                        p.models = list.clone();
                    }
                    if let Some(d) = ui.draft.as_mut() {
                        d.providers.push(p);
                    }
                    ui.screen = AiScreen::List;
                },
                "Add provider"
            }
            button { class: BTN, onclick: move |_| ctx.ai_ui.write().screen = AiScreen::List, "Cancel" }
        }
    }
}

// ---- AI models ----------------------------------------------------------------------------

#[component]
fn ModelsPage() -> Element {
    let mut ctx = use_context::<Ctx>();
    let hub = ctx.ai.read().clone();
    let ui = ctx.ai_ui.read();
    let draft = ui.draft.clone().unwrap_or_default();
    let tests = &ui.tests;
    let label_of = |v: &Option<ModelRef>, none: &str| v.as_ref().map(|m| model_summary(&hub, &draft, m)).unwrap_or_else(|| none.to_string());

    let default_entries = model_entries(&hub, &draft, tests, draft.default_model.as_ref(), Some("Automatic (first ready provider's default)"));
    let default_label = label_of(&draft.default_model, "Automatic (first ready provider's default)");
    let scopes: Vec<(Scope, String, Vec<Entry>)> = Scope::ALL
        .into_iter()
        .map(|s| {
            let v = draft.models.get(s.id()).cloned();
            (s, label_of(&v, "Same as all of SolarOffice"), model_entries(&hub, &draft, tests, v.as_ref(), Some("Same as all of SolarOffice")))
        })
        .collect();
    let mut added: Vec<(String, String, solar_ai::ModelInfo, String)> = Vec::new();
    for p in &draft.providers {
        for m in &p.models {
            let info = draft.catalog.iter().find(|c| c.provider == p.kind && &c.id == m).cloned().unwrap_or_else(|| hub.catalog().info(p.kind, m));
            added.push((p.id.clone(), m.clone(), info, p.display_name()));
        }
    }
    let first_provider = draft.providers.iter().find(|p| p.usable_for_chat()).or(draft.providers.first()).map(|p| p.id.clone()).unwrap_or_default();
    drop(ui);

    rsx! {
        div { class: "flex items-start gap-4",
            div { class: "flex-1 min-w-0",
                Heading { title: "AI models", sub: "Which model answers where. Prices are per million tokens (about 750,000 words); $ to $$$$ compares output prices. The icons show what a model is good at." }
            }
            button {
                class: "flex items-center gap-1.5 {BTN_PRIMARY} shrink-0",
                onclick: move |_| ctx.ai_ui.write().screen = AiScreen::AddModel(Box::new(ModelForm { provider: first_provider.clone(), tools: true, ..ModelForm::default() })),
                Icon { svg: icons::PLUS, size: 14 }
                "Add model"
            }
        }
        div { class: "flex flex-wrap gap-x-3 gap-y-1 mb-4 text-[12px] text-muted",
            for c in Category::ALL {
                span { key: "{c.label()}", title: "{c.description()}", "{c.emoji()} {c.label()}" }
            }
        }
        div { class: "grid grid-cols-[170px_minmax(0,1fr)] gap-x-4 gap-y-2.5 items-center",
            div { class: "font-medium", "All of SolarOffice" }
            Dropdown {
                label: default_label,
                entries: default_entries,
                width: 480,
                on_pick: move |key: String| {
                    if let Some(d) = ctx.ai_ui.write().draft.as_mut() {
                        d.default_model = ModelRef::parse(&key);
                    }
                },
            }
            for (scope, label, entries) in scopes {
                div { key: "l{scope.id()}", class: "text-ink2", "{scope.label()}" }
                Dropdown {
                    label,
                    entries,
                    width: 480,
                    on_pick: move |key: String| {
                        if let Some(d) = ctx.ai_ui.write().draft.as_mut() {
                            match ModelRef::parse(&key) {
                                Some(m) => d.models.insert(scope.id().into(), m),
                                None => d.models.remove(scope.id()),
                            };
                        }
                    },
                }
            }
        }

        div { class: "mt-7 text-[15px] font-semibold", "Models you added" }
        div { class: "mt-0.5 mb-2 text-[12.5px] text-muted", "Extra models offered for a provider, with the details and prices you entered." }
        if added.is_empty() {
            div { class: "text-muted", "None yet." }
        }
        for (pid, model, info, provider) in added {
            {
                let (rp, rm) = (pid.clone(), model.clone());
                rsx! {
                    div { key: "{pid}:{model}", class: "flex items-center gap-3 py-1.5 border-b border-line last:border-b-0",
                        span { class: "font-medium", "{info.name}" }
                        span { class: "text-[12px]", title: "{info.categories.iter().map(|c| c.label()).collect::<Vec<_>>().join(\", \")}",
                            for c in info.categories.clone() { "{c.emoji()}" }
                        }
                        span { class: "flex-1 text-[12.5px] text-muted truncate", "{provider} · {info.price_label()}" }
                        button {
                            class: "px-2 h-7 rounded-md border border-line hover:bg-hover text-[12.5px]",
                            onclick: move |_| {
                                if let Some(d) = ctx.ai_ui.write().draft.as_mut() {
                                    remove_model(d, &rp, &rm);
                                }
                            },
                            "Remove"
                        }
                    }
                }
            }
        }
    }
}

fn form_mut(ctx: &mut Ctx, f: impl FnOnce(&mut ModelForm)) {
    if let AiScreen::AddModel(form) = &mut ctx.ai_ui.write().screen {
        f(form);
    }
}

#[component]
fn AddModelPage() -> Element {
    let mut ctx = use_context::<Ctx>();
    let hub = ctx.ai.read().clone();
    let AiScreen::AddModel(form) = ctx.ai_ui.read().screen.clone() else { return rsx! {} };
    let f = *form;
    let ui = ctx.ai_ui.read();
    let draft = ui.draft.clone().unwrap_or_default();
    let kind = draft.provider(&f.provider).map(|p| p.kind);
    let pick_list: Vec<String> = draft.provider(&f.provider).map(|p| offered(&hub, &ui.tests, p)).unwrap_or_default();
    drop(ui);
    let providers: Vec<(String, String, String)> = by_region(&draft.providers).into_iter().map(|p| (p.id.clone(), p.display_name(), flag_emoji(&p.country()))).collect();
    let full = f.categories.len() >= Category::MAX_PER_MODEL;
    let f_add = f.clone();

    rsx! {
        BackLink { label: "AI models", onclick: move |_| ctx.ai_ui.write().screen = AiScreen::List }
        Heading { title: "Add a model", sub: "Offer a model that isn't listed yet: a new release, a fine-tune, or a model on your own server. Details and prices are optional; they're used for limits and cost estimates." }
        div { class: "max-w-[720px]",
            Field { label: "Provider",
                select {
                    class: "{INPUT} w-full",
                    onchange: move |e| {
                        let v = e.value();
                        form_mut(&mut ctx, |f| f.provider = v);
                    },
                    for (id , name , flag) in providers {
                        option { key: "{id}", value: "{id}", selected: f.provider == id, "{flag} {name}" }
                    }
                }
            }
            Field { label: "Model id",
                div { class: "flex items-center gap-2",
                    input {
                        id: "model-id",
                        class: "{INPUT} flex-1",
                        initial_value: "{f.id}",
                        placeholder: "e.g. claude-sonnet-5-5, llama3.3:70b",
                        spellcheck: "false",
                        oninput: move |e| {
                            let v = e.value();
                            form_mut(&mut ctx, |f| f.id = v);
                        },
                    }
                    if !pick_list.is_empty() {
                        select {
                            class: "{INPUT} w-[110px]",
                            onchange: move |e| {
                                let v = e.value();
                                if !v.is_empty() {
                                    let js = format!("let e=document.getElementById('model-id'); if(e) e.value={};", serde_json::to_string(&v).unwrap_or_default());
                                    let _ = document::eval(&js);
                                    form_mut(&mut ctx, |f| f.id = v);
                                }
                            },
                            option { value: "", "Pick…" }
                            for m in pick_list {
                                option { key: "{m}", value: "{m}", "{m}" }
                            }
                        }
                    }
                }
            }
            Field { label: "Display name",
                input {
                    class: "{INPUT} w-full",
                    initial_value: "{f.name}",
                    placeholder: "Optional; defaults to the id",
                    oninput: move |e| {
                        let v = e.value();
                        form_mut(&mut ctx, |f| f.name = v);
                    },
                }
            }
            Field { label: "Price (USD per 1M tokens)",
                div { class: "flex items-center gap-2",
                    input {
                        class: "{INPUT} w-[140px]",
                        initial_value: "{f.input}",
                        placeholder: "input, e.g. 2",
                        oninput: move |e| {
                            let v = e.value();
                            form_mut(&mut ctx, |f| f.input = v);
                        },
                    }
                    input {
                        class: "{INPUT} w-[140px]",
                        initial_value: "{f.output}",
                        placeholder: "output, e.g. 10",
                        oninput: move |e| {
                            let v = e.value();
                            form_mut(&mut ctx, |f| f.output = v);
                        },
                    }
                    if kind.is_some_and(|k| k.is_local()) {
                        span { class: "text-[12px] text-muted", "Leave empty: free" }
                    }
                }
            }
            Field { label: "Limits (tokens)",
                div { class: "flex items-center gap-2",
                    input {
                        class: "{INPUT} w-[170px]",
                        initial_value: "{f.context}",
                        placeholder: "context, e.g. 128000",
                        oninput: move |e| {
                            let v = e.value();
                            form_mut(&mut ctx, |f| f.context = v);
                        },
                    }
                    input {
                        class: "{INPUT} w-[170px]",
                        initial_value: "{f.max_output}",
                        placeholder: "max output, e.g. 8192",
                        oninput: move |e| {
                            let v = e.value();
                            form_mut(&mut ctx, |f| f.max_output = v);
                        },
                    }
                }
            }
            Field { label: "Can handle",
                div { class: "flex items-center gap-4 pt-2",
                    label { class: "flex items-center gap-1.5",
                        input { r#type: "checkbox", class: "accent-[var(--so-accent)]", checked: f.tools, onchange: move |e| { let v = e.checked(); form_mut(&mut ctx, |f| f.tools = v); } }
                        "Tools"
                    }
                    label { class: "flex items-center gap-1.5",
                        input { r#type: "checkbox", class: "accent-[var(--so-accent)]", checked: f.vision, onchange: move |e| { let v = e.checked(); form_mut(&mut ctx, |f| f.vision = v); } }
                        "Images"
                    }
                    label { class: "flex items-center gap-1.5",
                        input { r#type: "checkbox", class: "accent-[var(--so-accent)]", checked: f.pdf, onchange: move |e| { let v = e.checked(); form_mut(&mut ctx, |f| f.pdf = v); } }
                        "PDFs"
                    }
                }
            }
            Field { label: "Good at (up to 5)",
                div { class: "flex flex-wrap gap-2",
                    for c in Category::ALL {
                        {
                            let on = f.categories.contains(&c);
                            let enabled = on || !full;
                            let look = if on {
                                "bg-accent-soft text-accent border-accent"
                            } else if enabled {
                                "bg-card text-ink2 border-line hover:bg-hover"
                            } else {
                                "bg-card text-muted border-line opacity-60"
                            };
                            rsx! {
                                button {
                                    key: "{c.label()}",
                                    class: "px-3 h-[26px] rounded-full border text-[12.5px] {look}",
                                    title: "{c.description()}",
                                    disabled: !enabled,
                                    onclick: move |_| form_mut(&mut ctx, |f| {
                                        if on {
                                            f.categories.retain(|x| *x != c);
                                        } else {
                                            f.categories.push(c);
                                        }
                                    }),
                                    "{c.emoji()} {c.label()}"
                                }
                            }
                        }
                    }
                }
            }
        }
        if let Some(e) = &f.error {
            div { class: "mt-2 text-[#D92D20] dark:text-[#FF7A6E]", "{e}" }
        }
        div { class: "mt-5 flex gap-2",
            button {
                class: BTN_PRIMARY,
                onclick: move |_| match model_from_form(&f_add, kind) {
                    Ok(info) => {
                        let mut ui = ctx.ai_ui.write();
                        if let Some(d) = ui.draft.as_mut() {
                            add_model(d, &f_add.provider, info);
                        }
                        ui.screen = AiScreen::List;
                    }
                    Err(e) => form_mut(&mut ctx, |f| f.error = Some(e)),
                },
                "Add model"
            }
            button { class: BTN, onclick: move |_| ctx.ai_ui.write().screen = AiScreen::List, "Cancel" }
        }
    }
}
