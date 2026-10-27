//! The debug menu's first page (D81): every tunable (D547) the run knows, grouped by the words
//! before its last (`sky.cloud`), each a slider within its range with its help, its default
//! and what last set it, and a reset for each and for each group, as Unreal's console
//! variables are shown in its editor. It reads each value every frame it is drawn and sets
//! through `crate::tune::set_logged`, as the remote's `tune.set` does, so a slider and
//! `r tune set` each see what the other set.

use bevy::prelude::*;
use bevy_inspector_egui::egui::{self, Color32, RichText, Ui};
use serde::Serialize;
use terra_gen::tune::{self, SetBy, Tunable};

/// What the page keeps between frames: the search, and where it drew its controls, in the
/// pointer's pixels (egui's points), for a script to aim at (`debug.menu`).
#[derive(Resource, Default)]
pub struct TunablesPage {
    pub search: String,
    pub drawn: Drawn,
}

#[derive(Default, Clone, Serialize)]
pub struct Drawn {
    pub search: Option<Area>,
    pub reset_all: Option<Area>,
    pub groups: Vec<GroupDrawn>,
    pub tunables: Vec<TunableDrawn>,
    /// What the page says instead of a list, when it shows none.
    pub note: Option<String>,
}

#[derive(Clone, Serialize)]
pub struct GroupDrawn {
    pub prefix: String,
    pub shown: usize,
    pub reset: Area,
}

#[derive(Clone, Serialize)]
pub struct TunableDrawn {
    pub name: &'static str,
    pub value: f32,
    pub set_by: &'static str,
    /// The slider's rail; `track` is where its handle's middle is at the least and the most.
    pub slider: Area,
    pub track: [f32; 2],
    pub reset: Area,
}

/// A rectangle: left, top, width and height.
#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
pub struct Area(pub [f32; 4]);

impl From<egui::Rect> for Area {
    fn from(r: egui::Rect) -> Self {
        Area([r.left(), r.top(), r.width(), r.height()])
    }
}

/// Its group: every word but the last.
pub fn group(name: &str) -> &str {
    name.rsplit_once('.').map_or(name, |(group, _)| group)
}

/// Those the search finds, in groups by name: every word of it in the name or the help.
pub fn grouped(search: &str) -> Vec<(&'static str, Vec<&'static Tunable>)> {
    let words: Vec<String> = search.split_whitespace().map(str::to_lowercase).collect();
    let mut groups: Vec<(&'static str, Vec<&'static Tunable>)> = Vec::new();
    for t in tune::all() {
        let text = format!("{} {}", t.name, t.doc).to_lowercase();
        if !words.iter().all(|w| text.contains(w.as_str())) {
            continue;
        }
        match groups.last_mut() {
            Some((g, list)) if *g == group(t.name) => list.push(t),
            _ => groups.push((group(t.name), vec![t])),
        }
    }
    groups
}

/// What last set it, in plain words, and a colour to tell them apart at a glance.
fn set_by(t: &Tunable) -> (&'static str, Color32) {
    match t.set_by() {
        SetBy::Default => ("its default", Color32::from_gray(150)),
        SetBy::CommandLine => ("set at launch (--tune)", Color32::from_rgb(120, 175, 255)),
        SetBy::Remote => ("set over the remote", Color32::from_rgb(255, 170, 80)),
        SetBy::Menu => ("set here", Color32::from_rgb(130, 215, 130)),
    }
}

const CHANGED: Color32 = Color32::from_rgb(255, 214, 102);
/// Help reads at a glance; the numbers under it a little quieter.
const HELP: Color32 = Color32::from_gray(200);
const QUIET: Color32 = Color32::from_gray(160);

/// Draws the page into `ui`, records where its controls are, and sets what is moved.
pub fn show(ui: &mut Ui, page: &mut TunablesPage) {
    let mut drawn = Drawn::default();
    let all = tune::all();
    ui.horizontal(|ui| {
        let search = egui::TextEdit::singleline(&mut page.search)
            .hint_text("Search names and help")
            .desired_width(ui.available_width() - 90.0);
        drawn.search = Some(ui.add(search).rect.into());
        // The key that closes the menu never reaches the search (`closes_on_backtick`).
        page.search.retain(|c| c != '`');
        let changed = all.iter().any(|t| t.get() != t.default);
        let reset = ui
            .add_enabled(changed, egui::Button::new("Reset all"))
            .on_hover_text("Every changed tunable back to its default");
        if reset.clicked() {
            crate::tune::reset_logged(&None);
        }
        drawn.reset_all = Some(reset.rect.into());
    });
    let groups = grouped(&page.search);
    let shown: usize = groups.iter().map(|(_, list)| list.len()).sum();
    ui.label(
        RichText::new(format!(
            "{shown} of {} tunables. Drag a slider to try a value: it lasts until the game closes; \
             when one is right, change its default in the code.",
            all.len()
        ))
        .small()
        .color(QUIET),
    );
    ui.add_space(4.0);
    if all.is_empty() {
        drawn.note = Some(
            "No tunables are registered in this run. A tunable is a static in its owner's code, \
             registered by its plugin (terra_gen::tune, D547)."
                .into(),
        );
    } else if groups.is_empty() {
        drawn.note = Some(format!(
            "Nothing has \"{}\" in its name or help. Clear the search to see all {}.",
            page.search.trim(),
            all.len()
        ));
    }
    if let Some(note) = &drawn.note {
        ui.label(note.as_str());
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for (prefix, list) in &groups {
                show_group(ui, prefix, list, &mut drawn);
            }
        });
    page.drawn = drawn;
}

fn show_group(ui: &mut Ui, prefix: &str, list: &[&'static Tunable], drawn: &mut Drawn) {
    let id = ui.make_persistent_id(("tunable group", prefix));
    let state =
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true);
    let mut reset_area = None;
    state
        .show_header(ui, |ui| {
            ui.label(RichText::new(format!("{prefix}.*")).strong().monospace());
            ui.label(RichText::new(format!("{}", list.len())).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let changed = list.iter().any(|t| t.get() != t.default);
                let reset = ui
                    .add_enabled(changed, egui::Button::new("Reset group").small())
                    .on_hover_text(format!("Every {prefix}.* back to its default"));
                if reset.clicked() {
                    crate::tune::reset_logged(&Some(format!("{prefix}.")));
                }
                reset_area = Some(reset.rect.into());
            });
        })
        .body(|ui| {
            for &t in list {
                show_tunable(ui, t, drawn);
            }
        });
    drawn.groups.push(GroupDrawn {
        prefix: prefix.to_string(),
        shown: list.len(),
        reset: reset_area.unwrap_or(Area([0.0; 4])),
    });
}

fn show_tunable(ui: &mut Ui, t: &'static Tunable, drawn: &mut Drawn) {
    let mut value = t.get();
    let changed = value != t.default;
    let short = t.name.rsplit_once('.').map_or(t.name, |(_, last)| last);
    ui.add_space(2.0);
    let (slider, track, reset) = ui
        .horizontal(|ui| {
            let name = RichText::new(short).monospace();
            let name = if changed { name.color(CHANGED) } else { name };
            ui.add_sized([150.0, 18.0], egui::Label::new(name).truncate())
                .on_hover_text(t.name);
            let slider = ui.add(
                egui::Slider::new(&mut value, t.min..=t.max)
                    .show_value(false)
                    .clamping(egui::SliderClamping::Always),
            );
            // Where the handle's middle is at each end: the rail less the handle's radius, as
            // egui's slider places it (`Slider::handle_radius`).
            let radius = slider.rect.height() / 2.5;
            let track = [slider.rect.left() + radius, slider.rect.right() - radius];
            let range = t.min..=t.max;
            let speed = (t.max - t.min) / 500.0;
            ui.add(
                egui::DragValue::new(&mut value)
                    .range(range)
                    .speed(speed)
                    .max_decimals(3),
            );
            let reset = ui
                .add_enabled(changed, egui::Button::new("Reset").small())
                .on_hover_text(format!("Back to its default, {}", t.default));
            (slider.rect, track, reset)
        })
        .inner;
    // A reset leaves it set by its default, as `r tune set NAME default` does.
    let to = if reset.clicked() {
        Some(None)
    } else {
        (value != t.get()).then_some(Some(value))
    };
    if let Some(to) = to {
        // Within its range, as the slider clamps; a refusal would be a bug, so it is logged.
        if let Err(why) = crate::tune::set_logged(t, to, SetBy::Menu) {
            error!("Debug menu: {why}");
        }
    }
    let reset = reset.rect;
    let (by, colour) = set_by(t);
    ui.horizontal_wrapped(|ui| {
        ui.add_space(154.0);
        ui.label(RichText::new(t.doc).small().color(HELP));
    });
    ui.horizontal(|ui| {
        ui.add_space(154.0);
        ui.label(
            RichText::new(format!("default {}, {} to {}", t.default, t.min, t.max))
                .small()
                .color(QUIET),
        );
        ui.label(RichText::new(by).small().color(colour));
    });
    drawn.tunables.push(TunableDrawn {
        name: t.name,
        value: t.get(),
        set_by: t.set_by().name(),
        slider: slider.into(),
        track,
        reset: reset.into(),
    });
}
