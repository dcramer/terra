//! Controls looked up by what they do rather than by key, so a player can set any of them
//! to another key or button (D80), and the player's other settings.
//!
//! Settings are kept in a small text file in the player's own settings folder, read at
//! start and written whenever something changes. A scripted run never reads or writes it,
//! so a script does the same thing on every machine.

use super::options::OptionsMenu;
use bevy::{
    ecs::system::SystemParam,
    prelude::*,
    reflect::{
        FromReflect,
        enums::{DynamicEnum, DynamicVariant, Enum},
    },
    window::{CursorGrabMode, CursorOptions},
};
use std::path::PathBuf;
use terra_world::{pacing::FrameLimit, quality::Quality};

/// Something the player does, which a key or a mouse button is set to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Forward,
    Back,
    Left,
    Right,
    Sprint,
    Jump,
    /// Build, take apart, cut, pick up and put down.
    Use,
    /// Held, the builder's dial of parts; tapped, put back what is carried.
    BuildMenu,
    /// Another of whatever the view is on.
    Copy,
    NextKind,
    TurnLeft,
    TurnRight,
    TakeApart,
    MoveItems,
    Grid,
    /// Held, places off the grid when it is on, and on it when it is off.
    OffGrid,
    Inventory,
    /// Use a station, talk to someone, open something.
    Interact,
    LookLeft,
    LookRight,
    LookUp,
    LookDown,
    /// Eat the most filling food carried.
    Eat,
    /// Roll clear of a blow.
    Dodge,
    /// Open and close the map.
    Map,
    /// Bring the camera nearer or take it further, as the mouse wheel does.
    ZoomIn,
    ZoomOut,
    /// Raise or lower what is being placed, as Shift and the wheel do.
    Raise,
    Lower,
    /// Undo the last thing built, laid or taken apart, within a while.
    Undo,
}

impl Action {
    pub const ALL: [Action; 30] = [
        Action::Forward,
        Action::Back,
        Action::Left,
        Action::Right,
        Action::Sprint,
        Action::Jump,
        Action::Use,
        Action::BuildMenu,
        Action::Copy,
        Action::NextKind,
        Action::TurnLeft,
        Action::TurnRight,
        Action::TakeApart,
        Action::MoveItems,
        Action::Grid,
        Action::OffGrid,
        Action::Inventory,
        Action::Interact,
        Action::LookLeft,
        Action::LookRight,
        Action::LookUp,
        Action::LookDown,
        Action::Eat,
        Action::Dodge,
        Action::Map,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::Raise,
        Action::Lower,
        Action::Undo,
    ];

    /// What the options menu calls it.
    pub fn name(self) -> &'static str {
        match self {
            Action::Forward => "Forward",
            Action::Back => "Back",
            Action::Left => "Left",
            Action::Right => "Right",
            Action::Sprint => "Sprint",
            Action::Jump => "Jump",
            Action::Use => "Use tool",
            Action::BuildMenu => "Build menu",
            Action::Copy => "Copy what you look at",
            Action::NextKind => "Next kind or size",
            Action::TurnLeft => "Turn left",
            Action::TurnRight => "Turn right",
            Action::TakeApart => "Take apart",
            Action::MoveItems => "Move items",
            Action::Grid => "Grid on or off",
            Action::OffGrid => "Hold to leave the grid",
            Action::Inventory => "Inventory",
            Action::Interact => "Interact",
            Action::LookLeft => "Look left",
            Action::LookRight => "Look right",
            Action::LookUp => "Look up",
            Action::LookDown => "Look down",
            Action::Eat => "Eat",
            Action::Dodge => "Roll",
            Action::Map => "Map",
            Action::ZoomIn => "Camera nearer",
            Action::ZoomOut => "Camera further",
            Action::Raise => "Raise what you place",
            Action::Lower => "Lower what you place",
            Action::Undo => "Undo the last thing built",
        }
    }

    /// Its name in the settings file.
    fn file_name(self) -> &'static str {
        match self {
            Action::Forward => "forward",
            Action::Back => "back",
            Action::Left => "left",
            Action::Right => "right",
            Action::Sprint => "sprint",
            Action::Jump => "jump",
            Action::Use => "use",
            Action::BuildMenu => "build_menu",
            Action::Copy => "copy",
            Action::NextKind => "next_kind",
            Action::TurnLeft => "turn_left",
            Action::TurnRight => "turn_right",
            Action::TakeApart => "take_apart",
            Action::MoveItems => "move_items",
            Action::Grid => "grid",
            Action::OffGrid => "off_grid",
            Action::Inventory => "inventory",
            Action::Interact => "interact",
            Action::LookLeft => "look_left",
            Action::LookRight => "look_right",
            Action::LookUp => "look_up",
            Action::LookDown => "look_down",
            Action::Eat => "eat",
            Action::Dodge => "dodge",
            Action::Map => "map",
            Action::ZoomIn => "zoom_in",
            Action::ZoomOut => "zoom_out",
            Action::Raise => "raise",
            Action::Lower => "lower",
            Action::Undo => "undo",
        }
    }

    /// What it is set to until the player says otherwise. An action can have two.
    fn defaults(self) -> [Option<Binding>; 2] {
        use Binding::{Key, Mouse};
        let (first, second) = match self {
            Action::Forward => (Key(KeyCode::KeyW), None),
            Action::Back => (Key(KeyCode::KeyS), None),
            Action::Left => (Key(KeyCode::KeyA), None),
            Action::Right => (Key(KeyCode::KeyD), None),
            Action::Sprint => (Key(KeyCode::ShiftLeft), None),
            Action::Jump => (Key(KeyCode::Space), None),
            Action::Use => (Mouse(MouseButton::Left), Some(Key(KeyCode::KeyE))),
            Action::BuildMenu => (Mouse(MouseButton::Right), Some(Key(KeyCode::KeyB))),
            Action::Copy => (Mouse(MouseButton::Middle), Some(Key(KeyCode::KeyV))),
            Action::NextKind => (Key(KeyCode::KeyR), None),
            Action::TurnLeft => (Key(KeyCode::KeyZ), None),
            Action::TurnRight => (Key(KeyCode::KeyC), None),
            Action::TakeApart => (Key(KeyCode::KeyQ), None),
            Action::MoveItems => (Key(KeyCode::KeyG), None),
            Action::Grid => (Key(KeyCode::KeyX), None),
            Action::OffGrid => (Key(KeyCode::AltLeft), None),
            Action::Inventory => (Key(KeyCode::Tab), Some(Key(KeyCode::KeyI))),
            Action::Interact => (Key(KeyCode::KeyF), None),
            Action::LookLeft => (Key(KeyCode::ArrowLeft), None),
            Action::LookRight => (Key(KeyCode::ArrowRight), None),
            Action::LookUp => (Key(KeyCode::ArrowUp), None),
            Action::LookDown => (Key(KeyCode::ArrowDown), None),
            Action::Eat => (Key(KeyCode::KeyH), None),
            Action::Dodge => (Key(KeyCode::ControlLeft), None),
            Action::Map => (Key(KeyCode::KeyM), None),
            Action::ZoomIn => (Key(KeyCode::Equal), None),
            Action::ZoomOut => (Key(KeyCode::Minus), None),
            Action::Raise => (Key(KeyCode::PageUp), None),
            Action::Lower => (Key(KeyCode::PageDown), None),
            // Enshrouded's key. Ctrl and Z would roll, Ctrl being the roll.
            Action::Undo => (Key(KeyCode::KeyY), None),
        };
        [Some(first), second]
    }

    fn index(self) -> usize {
        Action::ALL
            .iter()
            .position(|&action| action == self)
            .unwrap()
    }
}

/// A key or a mouse button.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Binding {
    Key(KeyCode),
    Mouse(MouseButton),
}

impl Binding {
    /// As a player reads it: "E", "Left Shift", "left click".
    pub fn label(self) -> String {
        match self {
            Binding::Mouse(MouseButton::Left) => "left click".into(),
            Binding::Mouse(MouseButton::Right) => "right click".into(),
            Binding::Mouse(MouseButton::Middle) => "middle click".into(),
            Binding::Mouse(MouseButton::Back) => "mouse 4".into(),
            Binding::Mouse(MouseButton::Forward) => "mouse 5".into(),
            Binding::Mouse(MouseButton::Other(number)) => format!("mouse {number}"),
            Binding::Key(key) => {
                let name = key.variant_name();
                if let Some(single) = name
                    .strip_prefix("Key")
                    .or_else(|| name.strip_prefix("Digit"))
                    .filter(|rest| rest.len() == 1)
                {
                    return single.to_string();
                }
                // ShiftLeft reads better as Left Shift, and PageUp as Page Up.
                let (side, name) = match (name.strip_suffix("Left"), name.strip_suffix("Right")) {
                    (Some(rest), _) if !rest.is_empty() => ("Left ", rest),
                    (_, Some(rest)) if !rest.is_empty() => ("Right ", rest),
                    _ => ("", name),
                };
                let mut words = String::from(side);
                for (i, c) in name.chars().enumerate() {
                    if i > 0 && c.is_ascii_uppercase() {
                        words.push(' ');
                    }
                    words.push(c);
                }
                words
            }
        }
    }

    /// As the settings file has it: `KeyE`, `ShiftLeft`, `MouseLeft`.
    fn write(self) -> String {
        match self {
            Binding::Key(key) => key.variant_name().to_string(),
            Binding::Mouse(MouseButton::Other(number)) => format!("Mouse{number}"),
            Binding::Mouse(button) => format!("Mouse{button:?}"),
        }
    }

    fn read(text: &str) -> Option<Self> {
        if let Some(button) = text.strip_prefix("Mouse") {
            return Some(Binding::Mouse(match button {
                "Left" => MouseButton::Left,
                "Right" => MouseButton::Right,
                "Middle" => MouseButton::Middle,
                "Back" => MouseButton::Back,
                "Forward" => MouseButton::Forward,
                number => MouseButton::Other(number.parse().ok()?),
            }));
        }
        KeyCode::from_reflect(&DynamicEnum::new(text, DynamicVariant::Unit)).map(Binding::Key)
    }

    /// Keys kept for other things, which no action may be set to: Esc closes menus, the
    /// numbers pick slots of the bar and parts of the dial, and the function keys belong
    /// to developer tools.
    pub fn reserved(self) -> bool {
        let Binding::Key(key) = self else {
            return false;
        };
        let name = key.variant_name();
        key == KeyCode::Escape
            || name.starts_with("Digit")
            || name
                .strip_prefix('F')
                .is_some_and(|number| number.parse::<u8>().is_ok())
    }

    fn held(self, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>) -> bool {
        match self {
            Binding::Key(key) => keys.pressed(key),
            Binding::Mouse(button) => mouse.pressed(button),
        }
    }

    fn went_down(self, keys: &ButtonInput<KeyCode>, mouse: &ButtonInput<MouseButton>) -> bool {
        match self {
            Binding::Key(key) => keys.just_pressed(key),
            Binding::Mouse(button) => mouse.just_pressed(button),
        }
    }
}

/// How a setting with a range of values moves: its least, its most, and one step.
pub struct Range {
    pub least: f32,
    pub most: f32,
    pub step: f32,
}

impl Range {
    /// One step up or down from a value, kept in range and to a whole number of steps.
    pub fn nudge(&self, value: f32, up: bool) -> f32 {
        let steps = libm::roundf(value / self.step) + if up { 1.0 } else { -1.0 };
        // Rounded again so a tenth stays a tenth, not a tenth and a hair.
        (libm::roundf(steps * self.step * 1000.0) / 1000.0).clamp(self.least, self.most)
    }
}

pub const LOOK_SPEED: Range = Range {
    least: 0.2,
    most: 3.0,
    step: 0.1,
};

/// Loudness, for each part of the mix: from silent to full, in tenths.
pub const VOLUME: Range = Range {
    least: 0.0,
    most: 1.0,
    step: 0.1,
};

pub const FIELD_OF_VIEW: Range = Range {
    least: 40.0,
    most: 90.0,
    step: 5.0,
};

/// What the player has chosen in the options menu.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Settings {
    bindings: [[Option<Binding>; 2]; Action::ALL.len()],
    /// How fast the view turns with the mouse, as a multiple of the usual speed.
    pub look_speed: f32,
    /// Moving the mouse up looks down.
    pub invert_look: bool,
    /// From the bottom of the view to the top, in degrees.
    pub field_of_view: f32,
    pub full_screen: bool,
    pub vsync: bool,
    /// The most frames a second, with vsync off (D25).
    pub frame_limit: FrameLimit,
    /// The quality preset, or `None` for the one chosen for the GPU (D239).
    pub graphics: Option<Quality>,
    /// How loud everything is, and each part of it, from 0 to 1.
    pub master_volume: f32,
    pub music_volume: f32,
    pub ambience_volume: f32,
    pub effects_volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            bindings: Action::ALL.map(Action::defaults),
            look_speed: 1.0,
            invert_look: false,
            // A little wider than the engine's default, which suits a view from behind a
            // figure.
            field_of_view: 55.0,
            full_screen: false,
            vsync: true,
            frame_limit: FrameLimit::Off,
            graphics: None,
            master_volume: 1.0,
            // Music is meant to sit under the world, not over it.
            music_volume: 0.7,
            ambience_volume: 1.0,
            effects_volume: 1.0,
        }
    }
}

impl Settings {
    pub fn bindings(&self, action: Action) -> [Option<Binding>; 2] {
        self.bindings[action.index()]
    }

    /// Sets one of an action's two bindings, or clears it. Whatever had that key or button
    /// before takes this one's old binding in exchange, so nothing is left without a
    /// control by accident and no key does two things.
    pub fn bind(&mut self, action: Action, slot: usize, binding: Option<Binding>) {
        let old = self.bindings[action.index()][slot];
        if let Some(binding) = binding {
            for other in self.bindings.iter_mut().flatten() {
                if *other == Some(binding) {
                    *other = old;
                }
            }
        }
        self.bindings[action.index()][slot] = binding;
    }

    /// The first binding of an action, as a prompt would name it.
    pub fn key(&self, action: Action) -> String {
        self.bindings(action)
            .into_iter()
            .flatten()
            .next()
            .map_or("(not set)".to_string(), Binding::label)
    }

    /// "Hold left click", or "Hold E".
    pub fn hold(&self, action: Action) -> String {
        format!("Hold {}", self.key(action))
    }

    /// Whether any binding of an action is held down. Mouse buttons count only once the
    /// game has the mouse, so the click that takes it does nothing else.
    pub fn held(
        &self,
        action: Action,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
        captured: bool,
    ) -> bool {
        self.bindings(action)
            .into_iter()
            .flatten()
            .filter(|binding| captured || matches!(binding, Binding::Key(_)))
            .any(|binding| binding.held(keys, mouse))
    }

    /// Whether any binding of an action went down this frame, on the same terms.
    pub fn went_down(
        &self,
        action: Action,
        keys: &ButtonInput<KeyCode>,
        mouse: &ButtonInput<MouseButton>,
        captured: bool,
    ) -> bool {
        self.bindings(action)
            .into_iter()
            .flatten()
            .filter(|binding| captured || matches!(binding, Binding::Key(_)))
            .any(|binding| binding.went_down(keys, mouse))
    }

    /// The settings file's text: one setting a line, as `name = value`.
    pub fn write(&self) -> String {
        let mut text = String::from(
            "# Terra settings, written by the game. Delete this file to go back to the defaults.\n",
        );
        text += &format!("look_speed = {}\n", self.look_speed);
        text += &format!("invert_look = {}\n", self.invert_look);
        text += &format!("field_of_view = {}\n", self.field_of_view);
        text += &format!("full_screen = {}\n", self.full_screen);
        text += &format!("vsync = {}\n", self.vsync);
        text += &format!("frame_limit = {}\n", self.frame_limit.name());
        text += &format!(
            "graphics = {}\n",
            self.graphics.map_or("auto", Quality::name)
        );
        text += &format!("master_volume = {}\n", self.master_volume);
        text += &format!("music_volume = {}\n", self.music_volume);
        text += &format!("ambience_volume = {}\n", self.ambience_volume);
        text += &format!("effects_volume = {}\n", self.effects_volume);
        for action in Action::ALL {
            let bound: Vec<String> = self
                .bindings(action)
                .into_iter()
                .map(|binding| binding.map_or("-".to_string(), Binding::write))
                .collect();
            text += &format!("{} = {}\n", action.file_name(), bound.join(", "));
        }
        text
    }

    /// Reads a settings file's text. Anything missing, unknown or unreadable keeps its
    /// default, so an old or hand-edited file still works.
    pub fn read(text: &str) -> Self {
        let mut settings = Settings::default();
        for line in text.lines() {
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            let (name, value) = (name.trim(), value.trim());
            let number = |range: &Range| {
                value
                    .parse::<f32>()
                    .ok()
                    .map(|number| number.clamp(range.least, range.most))
            };
            match name {
                "look_speed" => settings.look_speed = number(&LOOK_SPEED).unwrap_or(1.0),
                "field_of_view" => {
                    settings.field_of_view = number(&FIELD_OF_VIEW).unwrap_or(55.0);
                }
                "invert_look" => settings.invert_look = value == "true",
                "full_screen" => settings.full_screen = value == "true",
                "vsync" => settings.vsync = value != "false",
                "frame_limit" => {
                    settings.frame_limit = FrameLimit::from_name(value).unwrap_or_default();
                }
                "graphics" => settings.graphics = Quality::from_name(value),
                "master_volume" => settings.master_volume = number(&VOLUME).unwrap_or(1.0),
                "music_volume" => settings.music_volume = number(&VOLUME).unwrap_or(0.7),
                "ambience_volume" => settings.ambience_volume = number(&VOLUME).unwrap_or(1.0),
                "effects_volume" => settings.effects_volume = number(&VOLUME).unwrap_or(1.0),
                _ => {
                    let Some(action) = Action::ALL.into_iter().find(|a| a.file_name() == name)
                    else {
                        continue;
                    };
                    let mut bound = [None; 2];
                    for (slot, text) in value.split(',').take(2).enumerate() {
                        bound[slot] = Binding::read(text.trim()).filter(|b| !b.reserved());
                    }
                    settings.bindings[action.index()] = bound;
                }
            }
        }
        settings
    }

    /// Where the settings are kept: the player's own settings folder for this system.
    pub fn path() -> Option<PathBuf> {
        let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
        let folder = if cfg!(target_os = "windows") {
            var("APPDATA")?.join("Terra")
        } else if cfg!(target_os = "macos") {
            var("HOME")?.join("Library/Application Support/Terra")
        } else {
            var("XDG_CONFIG_HOME")
                .or_else(|| var("HOME").map(|home| home.join(".config")))?
                .join("terra")
        };
        Some(folder.join("settings.txt"))
    }

    pub fn load() -> Self {
        Settings::path()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map_or_else(Settings::default, |text| Settings::read(&text))
    }

    pub fn save(&self) {
        let Some(path) = Settings::path() else {
            warn!("Nowhere to save settings: no settings folder was found.");
            return;
        };
        let saved = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&path, self.write()));
        if let Err(problem) = saved {
            warn!("Could not save settings to {}: {problem}", path.display());
        }
    }
}

/// A key's name at the start of a line: "Left click", not "left click".
pub fn capitalised(text: String) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => text,
    }
}

/// The controls as gameplay reads them. Nothing counts while the options menu is open.
#[derive(SystemParam)]
pub struct Controls<'w, 's> {
    settings: Res<'w, Settings>,
    keys: Res<'w, ButtonInput<KeyCode>>,
    mouse: Res<'w, ButtonInput<MouseButton>>,
    cursor: Single<'w, 's, &'static CursorOptions>,
    menu: Res<'w, OptionsMenu>,
}

impl Controls<'_, '_> {
    /// Whether the game has the mouse.
    pub fn captured(&self) -> bool {
        self.cursor.grab_mode != CursorGrabMode::None
    }

    pub fn held(&self, action: Action) -> bool {
        !self.menu.open
            && self
                .settings
                .held(action, &self.keys, &self.mouse, self.captured())
    }

    pub fn went_down(&self, action: Action) -> bool {
        !self.menu.open
            && self
                .settings
                .went_down(action, &self.keys, &self.mouse, self.captured())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_survive_being_written_and_read_back() {
        let mut settings = Settings {
            look_speed: 1.7,
            invert_look: true,
            field_of_view: 70.0,
            full_screen: true,
            vsync: false,
            frame_limit: FrameLimit::Fps(75),
            ..default()
        };
        settings.bind(Action::Jump, 1, Some(Binding::Mouse(MouseButton::Other(7))));
        settings.bind(Action::Use, 1, None);
        settings.bind(Action::Sprint, 0, Some(Binding::Key(KeyCode::ControlLeft)));
        // Any key that is not kept for something else, Backspace included.
        settings.bind(Action::Back, 1, Some(Binding::Key(KeyCode::Backspace)));
        assert_eq!(Settings::read(&settings.write()), settings);
        assert_eq!(
            Settings::read(&Settings::default().write()),
            Settings::default()
        );
    }

    #[test]
    fn a_file_that_is_missing_things_or_wrong_keeps_the_defaults() {
        let settings = Settings::read(
            "# a comment\nlook_speed = fast\nfield_of_view = 500\nnonsense\nwings = KeyQ\n\
             jump = Escape, KeyNope\nforward = KeyT\n",
        );
        assert_eq!(settings.look_speed, 1.0);
        assert_eq!(settings.field_of_view, FIELD_OF_VIEW.most);
        // Esc is kept for menus, and there is no such key as KeyNope.
        assert_eq!(settings.bindings(Action::Jump), [None, None]);
        assert_eq!(
            settings.bindings(Action::Forward),
            [Some(Binding::Key(KeyCode::KeyT)), None]
        );
        assert_eq!(settings.bindings(Action::Back), Action::Back.defaults());
    }

    #[test]
    fn taking_another_actions_key_gives_it_this_ones_old_key() {
        let mut settings = Settings::default();
        settings.bind(Action::Forward, 0, Some(Binding::Key(KeyCode::KeyS)));
        assert_eq!(
            settings.bindings(Action::Forward)[0],
            Some(Binding::Key(KeyCode::KeyS))
        );
        assert_eq!(
            settings.bindings(Action::Back)[0],
            Some(Binding::Key(KeyCode::KeyW))
        );
        // Every key still does exactly one thing.
        let mut all: Vec<_> = settings.bindings.iter().flatten().flatten().collect();
        let count = all.len();
        all.sort_by_key(|binding| format!("{binding:?}"));
        all.dedup();
        assert_eq!(all.len(), count);
    }

    #[test]
    fn mouse_buttons_count_only_once_the_game_has_the_mouse() {
        let settings = Settings::default();
        let keys = ButtonInput::<KeyCode>::default();
        let mut mouse = ButtonInput::<MouseButton>::default();
        mouse.press(MouseButton::Left);
        assert!(!settings.held(Action::Use, &keys, &mouse, false));
        assert!(settings.held(Action::Use, &keys, &mouse, true));
        assert!(settings.went_down(Action::Use, &keys, &mouse, true));
        let mut keys = ButtonInput::<KeyCode>::default();
        keys.press(KeyCode::KeyE);
        assert!(settings.held(Action::Use, &keys, &mouse, false));
    }

    #[test]
    fn keys_and_buttons_have_names_a_player_can_read() {
        let label = Binding::label;
        assert_eq!(label(Binding::Key(KeyCode::KeyE)), "E");
        assert_eq!(label(Binding::Key(KeyCode::Digit4)), "4");
        assert_eq!(label(Binding::Key(KeyCode::ShiftLeft)), "Left Shift");
        assert_eq!(label(Binding::Key(KeyCode::PageUp)), "Page Up");
        assert_eq!(label(Binding::Key(KeyCode::Space)), "Space");
        assert_eq!(label(Binding::Mouse(MouseButton::Right)), "right click");
        assert_eq!(Settings::default().hold(Action::Use), "Hold left click");
        assert!(Binding::Key(KeyCode::Escape).reserved());
        assert!(Binding::Key(KeyCode::Digit0).reserved());
        assert!(Binding::Key(KeyCode::F3).reserved());
        assert!(!Binding::Key(KeyCode::KeyF).reserved());
        assert!(!Binding::Key(KeyCode::Backspace).reserved());
        assert!(!Binding::Mouse(MouseButton::Left).reserved());
    }

    #[test]
    fn ranged_settings_move_in_whole_steps_and_stay_in_range() {
        assert_eq!(LOOK_SPEED.nudge(1.0, true), 1.1);
        assert_eq!(LOOK_SPEED.nudge(0.2, false), 0.2);
        assert_eq!(FIELD_OF_VIEW.nudge(55.0, true), 60.0);
        assert_eq!(FIELD_OF_VIEW.nudge(90.0, true), 90.0);
    }
}
