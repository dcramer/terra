//! The debug menu (D81): there only when the run starts with `--debug-menu`, opened and closed
//! with the backtick key, as Unreal's console and Quake's before it; drawn with egui, as Dear
//! ImGui debug windows are in shipped engines, since every build already carries egui for the
//! F1 inspector and its sliders, search field and collapsing groups come with it, where Bevy
//! UI would have each built first. Its pages sit in tabs; the first is every tunable
//! (`tunables`, D547), and warp, give, time of day and weather come next, each a [`Page`] the
//! plugin that owns its commands adds, so the menu and the agents' tools stay one list.
//!
//! Over the remote (`debug.menu {open?, search?, page?}`, `r menu`) a script opens it, searches
//! and reads where each control was drawn, in the pointer's pixels, to `click` it as a person
//! would: what it set is then what `r tune get` says.

#[cfg(test)]
mod tests;
pub mod tunables;

use crate::shell::Options;
use bevy::{
    ecs::system::SystemState,
    prelude::*,
    remote::{BrpResult, builtin_methods::parse_some},
    window::{CursorGrabMode, CursorOptions},
};
use bevy_inspector_egui::bevy_egui::{
    EguiContext, EguiGlobalSettings, EguiPrimaryContextPass, PrimaryEguiContext,
    egui::{self, Color32, RichText},
};
use serde::Deserialize;
use serde_json::{Value, json};
use tunables::{Area, TunablesPage};

/// A page of the menu: its tab's name, what draws it, and what it tells a script.
#[derive(Clone, Copy)]
pub struct Page {
    pub title: &'static str,
    pub show: fn(&mut egui::Ui, &mut World),
    pub describe: fn(&World) -> Value,
}

/// The menu, where it is on: whether it is open, on which page, and where it was last drawn.
#[derive(Resource)]
pub struct DebugMenu {
    pub open: bool,
    pub page: usize,
    pub pages: Vec<Page>,
    /// Where the window was last drawn, in the pointer's pixels; `None` until it has been.
    pub window: Option<Area>,
    /// The whole screen in the pointer's pixels (egui's points): a shot of it, whatever its size,
    /// is this scaled, so a script finds the menu in a picture.
    pub screen: [f32; 2],
}

impl DebugMenu {
    /// Adds a page, as a tab after those already there.
    pub fn add(&mut self, page: Page) {
        self.pages.push(page);
    }
}

pub fn plugin(app: &mut App) {
    let options = app.world().get_resource::<Options>().cloned();
    if options.as_ref().is_some_and(Options::unattended) {
        app.add_systems(Startup, deaf_to_the_real_mouse);
    }
    if !options.is_some_and(|o| o.debug_menu) {
        return;
    }
    app.init_resource::<TunablesPage>()
        .insert_resource(DebugMenu::with_tunables())
        .add_systems(Startup, take_the_pointer_over_the_menu)
        .add_systems(Update, toggle)
        .add_systems(EguiPrimaryContextPass, draw);
}

impl DebugMenu {
    /// Closed, with its first page: the tunables (needs [`TunablesPage`]).
    pub fn with_tunables() -> Self {
        DebugMenu {
            open: false,
            page: 0,
            pages: vec![Page {
                title: "Tunables",
                show: |ui, world| {
                    world.resource_scope(|_, mut page: Mut<TunablesPage>| {
                        tunables::show(ui, &mut page)
                    })
                },
                describe: |world| json!(world.resource::<TunablesPage>().drawn),
            }],
            window: None,
            screen: [0.0; 2],
        }
    }
}

/// The remote's method, added with the remote's own.
pub fn remote(app: &mut App) {
    crate::remote::method(app, "debug.menu", menu);
}

/// What egui is over keeps the game's mouse and keys (egui's absorb system): a click on a
/// slider does not swing the axe, nor typing in the search walk.
fn take_the_pointer_over_the_menu(mut settings: ResMut<EguiGlobalSettings>) {
    settings.enable_absorb_bevy_input_system = true;
}

/// An unattended run ignores the real keyboard and mouse (`--background`): egui reads the
/// window's own input, so it is told not to; a script's pointer reaches egui by `sandbox.click`
/// (remote/pointer.rs) instead.
fn deaf_to_the_real_mouse(mut settings: ResMut<EguiGlobalSettings>) {
    let input = &mut settings.input_system_settings;
    input.run_write_window_pointer_moved_messages = false;
    input.run_write_pointer_button_messages = false;
    input.run_write_keyboard_input_messages_system = false;
    input.run_write_mouse_wheel_messages_system = false;
}

/// The backtick opens and closes it; opening lets go of the mouse, so the pointer can use it.
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut menu: ResMut<DebugMenu>,
    mut cursor: Query<&mut CursorOptions>,
) {
    if keys.just_pressed(KeyCode::Backquote) {
        let open = !menu.open;
        set_open(&mut menu, open, &mut cursor);
    }
}

fn set_open(menu: &mut DebugMenu, open: bool, cursor: &mut Query<&mut CursorOptions>) {
    if menu.open == open {
        return;
    }
    menu.open = open;
    if !open {
        menu.window = None;
    }
    info!("Debug menu: {}", if open { "open" } else { "closed" });
    for mut cursor in cursor
        .iter_mut()
        .filter(|c| c.grab_mode != CursorGrabMode::None)
    {
        cursor.grab_mode = CursorGrabMode::None;
        cursor.visible = true;
    }
}

const WIDTH: f32 = 520.0;
/// Below the frame-time readout and its graph, in the top-left corner (D77).
const AT: [f32; 2] = [16.0, 116.0];

fn draw(world: &mut World) {
    if !world.resource::<DebugMenu>().open {
        return;
    }
    let Ok(mut context) = world
        .query_filtered::<&mut EguiContext, With<PrimaryEguiContext>>()
        .single_mut(world)
    else {
        return;
    };
    let ctx = context.get_mut().clone();
    let (window, close) = show(&ctx, world);
    // egui is drawn over the whole screen (`overlay`), so its rect is the screen's.
    let screen = ctx.content_rect().size().into();
    let mut menu = world.resource_mut::<DebugMenu>();
    menu.window = window;
    menu.screen = screen;
    if close {
        menu.open = false;
        menu.window = None;
        info!("Debug menu: closed");
    }
}

/// Draws the menu's window and its page; answers where the window is and whether the
/// backtick was pressed while typing (egui then has the keys, so `toggle` never saw it).
pub fn show(ctx: &egui::Context, world: &mut World) -> (Option<Area>, bool) {
    let close =
        ctx.egui_wants_keyboard_input() && ctx.input(|i| i.key_pressed(egui::Key::Backtick));
    let height = (ctx.content_rect().height() - AT[1] - 24.0).max(200.0);
    let frame = egui::Frame::window(&ctx.global_style())
        .fill(Color32::from_rgba_unmultiplied(16, 18, 22, 238))
        .inner_margin(12.0)
        .corner_radius(6.0);
    let shown = egui::Window::new("Debug menu")
        .id(egui::Id::new("terra debug menu"))
        .anchor(egui::Align2::LEFT_TOP, AT)
        .fixed_size([WIDTH, height])
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .frame(frame)
        .show(ctx, |ui| {
            let style = ui.style_mut();
            style
                .text_styles
                .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
            style
                .text_styles
                .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
            style.spacing.slider_width = 190.0;
            ui.horizontal(|ui| {
                ui.label(RichText::new("Debug menu").heading().strong());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("` closes").color(Color32::from_gray(170)));
                });
            });
            let page = tabs(ui, world);
            ui.separator();
            (page.show)(ui, world);
        });
    (shown.map(|s| s.response.rect.into()), close)
}

/// A tab a page; answers the page chosen.
fn tabs(ui: &mut egui::Ui, world: &mut World) -> Page {
    let mut menu = world.resource_mut::<DebugMenu>();
    ui.horizontal(|ui| {
        for (i, page) in menu.pages.clone().iter().enumerate() {
            if ui.selectable_label(menu.page == i, page.title).clicked() {
                menu.page = i;
            }
        }
    });
    menu.pages[menu.page.min(menu.pages.len() - 1)]
}

#[derive(Deserialize, Default)]
struct Asked {
    open: Option<bool>,
    search: Option<String>,
    page: Option<String>,
}

/// `debug.menu {open?, search?, page?}`: opens or closes it, searches its tunables or turns to a
/// page, and answers what it shows and where each control was drawn on its last frame.
fn menu(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    let asked: Asked = parse_some(params.or(Some(json!({}))))?;
    if !world.contains_resource::<DebugMenu>() {
        return Err(crate::remote::refused(
            "the debug menu is off in this run: it is there only when the run starts with \
             --debug-menu (D81). Start one with it: `r start -- --debug-menu`.",
        ));
    }
    if let Some(open) = asked.open {
        let mut state = SystemState::<(ResMut<DebugMenu>, Query<&mut CursorOptions>)>::new(world);
        if let Ok((mut menu, mut cursor)) = state.get_mut(world) {
            set_open(&mut menu, open, &mut cursor);
        }
    }
    if let Some(search) = asked.search {
        world.resource_mut::<TunablesPage>().search = search;
    }
    if let Some(title) = asked.page {
        let mut menu = world.resource_mut::<DebugMenu>();
        let Some(i) = menu
            .pages
            .iter()
            .position(|p| p.title.eq_ignore_ascii_case(&title))
        else {
            let titles: Vec<_> = menu.pages.iter().map(|p| p.title).collect();
            return Err(crate::remote::refused(format!(
                "the debug menu has no page called {title}; its pages are {}",
                titles.join(", ")
            )));
        };
        menu.page = i;
    }
    let menu = world.resource::<DebugMenu>();
    let page = menu.pages[menu.page];
    Ok(json!({
        "open": menu.open,
        "page": page.title,
        "pages": menu.pages.iter().map(|p| p.title).collect::<Vec<_>>(),
        "search": world.resource::<TunablesPage>().search,
        "screen": menu.screen,
        "window": menu.window,
        "shown": if menu.window.is_some() { (page.describe)(world) } else { Value::Null },
    }))
}
