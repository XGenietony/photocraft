//! Camera Raw preset picker and naming form; mutations go through engine commands.
use egui::{TextEdit, Ui, vec2};
use photocraft_algo::camera_raw::CameraRaw;
use photocraft_engine::camera_raw_preset_cmds::CameraRawPreset;

use crate::{theme::Tokens, widgets};

#[derive(Default)]
pub(super) struct State {
    pub selected: Option<String>,
    pub modified: bool,
    pub saving: bool,
    pub draft: String,
    pub error: Option<String>,
}

pub(super) enum Action {
    Apply(String),
    Save { name: String, settings: Box<CameraRaw>, replace: bool },
    Delete(String),
}

pub(super) fn panel(ui: &mut Ui, state: &mut State, presets: &[CameraRawPreset], settings: &CameraRaw) -> Option<Action> {
    ui.label(tl!("Presets"));
    let mut selected = state.selected.clone();
    let mut options: Vec<(Option<String>, &str)> = vec![(None, tl!("Custom"))];
    options.extend(presets.iter().map(|preset| (Some(preset.name.clone()), preset.name.as_str())));
    if widgets::dropdown(ui, "camera-raw-preset-list", &mut selected, &options, 285.0) {
        state.error = None;
        state.modified = false;
        state.selected = selected.clone();
        if let Some(name) = selected {
            return Some(Action::Apply(name));
        }
    }
    ui.horizontal(|ui| {
        if state.modified {
            ui.label("*");
        }
        if widgets::secondary_button(ui, tl!("Save Preset…"), 150.0).clicked() {
            state.draft = state.selected.clone().unwrap_or_default();
            state.saving = true;
            state.error = None;
        }
    });
    if let Some(selected) = &state.selected
        && widgets::secondary_button(ui, tl!("Delete"), 100.0).clicked()
    {
        return Some(Action::Delete(selected.clone()));
    }
    if state.saving {
        let enter = ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        let label = ui.label(tl!("Preset Name"));
        ui.add(TextEdit::singleline(&mut state.draft).id_salt("camera-raw-preset-name").desired_width(280.0).char_limit(255)).labelled_by(label.id);
        let mut action = None;
        ui.horizontal(|ui| {
            if widgets::secondary_button(ui, tl!("Save"), 110.0).clicked() || enter {
                let name = state.draft.trim().to_string();
                action = Some(Action::Save {
                    replace: state.selected.as_ref().is_some_and(|current| current.eq_ignore_ascii_case(&name)),
                    name,
                    settings: Box::new(settings.clone()),
                });
            }
            if widgets::secondary_button(ui, tl!("Cancel"), 110.0).clicked() {
                state.saving = false;
                state.error = None;
            }
        });
        if action.is_some() {
            return action;
        }
    }
    if let Some(error) = &state.error {
        ui.colored_label(Tokens::get(ui.ctx()).danger, error);
    }
    ui.allocate_space(vec2(0.0, 4.0));
    None
}
