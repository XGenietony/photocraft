//! Named Camera Raw parameter snapshots, shared by the dialog and automation clients.
use photocraft_algo::camera_raw::CameraRaw;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::commands::CommandSpec;
use crate::presets::{always, bad};
use crate::{Result, Session};

const MAX_PRESETS: usize = 128;
const MAX_NAME: usize = 255;
const MAX_SETTINGS_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CameraRawPreset {
    pub name: String,
    pub settings: CameraRaw,
}

fn check_keys(value: &Value, cmd: &str, allowed: &[&str]) -> Result<()> {
    let object = value.as_object().ok_or_else(|| bad(cmd, "expected an object"))?;
    if object.keys().any(|key| !allowed.contains(&key.as_str())) {
        return Err(bad(cmd, "unknown preset parameter"));
    }
    Ok(())
}

fn name(value: &Value, cmd: &str) -> Result<String> {
    let name = value.get("name").and_then(Value::as_str).ok_or_else(|| bad(cmd, "name must be text"))?.trim();
    if name.is_empty() || name.chars().count() > MAX_NAME || name.chars().any(char::is_control) {
        return Err(bad(cmd, "name must contain 1–255 printable characters"));
    }
    Ok(name.into())
}

fn settings(value: &Value, cmd: &str) -> Result<CameraRaw> {
    let object = value.as_object().ok_or_else(|| bad(cmd, "settings must be an object"))?;
    if value.to_string().len() > MAX_SETTINGS_BYTES {
        return Err(bad(cmd, "settings exceed the 16 KiB preset limit"));
    }
    let defaults = serde_json::to_value(CameraRaw::default()).map_err(|e| bad(cmd, e.to_string()))?;
    for key in object.keys() {
        if defaults.get(key).is_none() {
            return Err(bad(cmd, format!("unknown Camera Raw setting {key}")));
        }
    }
    let mut settings: CameraRaw = serde_json::from_value(value.clone()).map_err(|e| bad(cmd, format!("invalid Camera Raw settings: {e}")))?;
    if settings.pixel_scale != 1.0 {
        return Err(bad(cmd, "pixelScale is a preview-only value and must be 1"));
    }
    settings.validate().map_err(|e| bad(cmd, e))?;
    let bounded = [
        ("temperature", settings.temperature, -100.0, 100.0),
        ("tint", settings.tint, -100.0, 100.0),
        ("exposure", settings.exposure, -5.0, 5.0),
        ("contrast", settings.contrast, -100.0, 100.0),
        ("highlights", settings.highlights, -100.0, 100.0),
        ("shadows", settings.shadows, -100.0, 100.0),
        ("whites", settings.whites, -100.0, 100.0),
        ("blacks", settings.blacks, -100.0, 100.0),
        ("texture", settings.texture, -100.0, 100.0),
        ("clarity", settings.clarity, -100.0, 100.0),
        ("dehaze", settings.dehaze, -100.0, 100.0),
        ("vibrance", settings.vibrance, -100.0, 100.0),
        ("saturation", settings.saturation, -100.0, 100.0),
        ("curveHighlights", settings.curve_highlights, -100.0, 100.0),
        ("curveLights", settings.curve_lights, -100.0, 100.0),
        ("curveDarks", settings.curve_darks, -100.0, 100.0),
        ("curveShadows", settings.curve_shadows, -100.0, 100.0),
        ("gradeBlending", settings.grade_blending, 0.0, 100.0),
        ("gradeBalance", settings.grade_balance, -100.0, 100.0),
        ("sharpenAmount", settings.sharpen_amount, 0.0, 150.0),
        ("sharpenRadius", settings.sharpen_radius, 0.5, 3.0),
        ("sharpenDetail", settings.sharpen_detail, 0.0, 100.0),
        ("sharpenMasking", settings.sharpen_masking, 0.0, 100.0),
        ("noiseLuminance", settings.noise_luminance, 0.0, 100.0),
        ("noiseLuminanceDetail", settings.noise_luminance_detail, 0.0, 100.0),
        ("noiseColor", settings.noise_color, 0.0, 100.0),
        ("noiseColorDetail", settings.noise_color_detail, 0.0, 100.0),
        ("grainAmount", settings.grain_amount, 0.0, 100.0),
        ("grainSize", settings.grain_size, 0.0, 100.0),
        ("grainRoughness", settings.grain_roughness, 0.0, 100.0),
        ("vignetteAmount", settings.vignette_amount, -100.0, 100.0),
        ("vignetteMidpoint", settings.vignette_midpoint, 0.0, 100.0),
        ("vignetteRoundness", settings.vignette_roundness, -100.0, 100.0),
        ("vignetteFeather", settings.vignette_feather, 0.0, 100.0),
        ("vignetteHighlights", settings.vignette_highlights, 0.0, 100.0),
    ];
    for (field, value, min, max) in bounded {
        if !value.is_finite() || !(min..=max).contains(&value) {
            return Err(bad(cmd, format!("{field} must be between {min} and {max}")));
        }
    }
    for (field, values) in [("hslHue", settings.hsl_hue), ("hslSat", settings.hsl_sat), ("hslLum", settings.hsl_lum)] {
        if values.iter().any(|v| !v.is_finite() || !(-100.0..=100.0).contains(v)) {
            return Err(bad(cmd, format!("{field} entries must be between -100 and 100")));
        }
    }
    let [a, b, c] = settings.curve_splits;
    if !a.is_finite() || !b.is_finite() || !c.is_finite() || a < 0.0 || a >= b || b >= c || c > 100.0 {
        return Err(bad(cmd, "curveSplits must increase within 0–100"));
    }
    for (field, wheel) in [
        ("gradeShadows", settings.grade_shadows),
        ("gradeMidtones", settings.grade_midtones),
        ("gradeHighlights", settings.grade_highlights),
        ("gradeGlobal", settings.grade_global),
    ] {
        if !wheel.hue.is_finite()
            || !(0.0..=360.0).contains(&wheel.hue)
            || !wheel.sat.is_finite()
            || !(0.0..=100.0).contains(&wheel.sat)
            || !wheel.lum.is_finite()
            || !(-100.0..=100.0).contains(&wheel.lum)
        {
            return Err(bad(cmd, format!("{field} wheel values are out of range")));
        }
    }
    if !matches!(settings.vignette_style.as_str(), "highlightPriority" | "colorPriority" | "paintOverlay") {
        return Err(bad(cmd, "unknown vignetteStyle"));
    }
    settings.pixel_scale = 1.0;
    Ok(settings)
}

/// Load entries independently so a corrupt saved preset does not hide valid presets.
pub(crate) fn load(value: Value) -> Vec<CameraRawPreset> {
    let Some(entries) = value.as_array() else { return Vec::new() };
    let mut result: Vec<CameraRawPreset> = Vec::new();
    for item in entries.iter().take(MAX_PRESETS) {
        let Ok(name) = name(item, "cameraRaw.presets.load") else { continue };
        let Some(value) = item.get("settings") else { continue };
        let Ok(settings) = settings(value, "cameraRaw.presets.load") else { continue };
        if result.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) {
            continue;
        }
        result.push(CameraRawPreset { name, settings });
    }
    result
}

fn save(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cameraRaw.presets.save";
    check_keys(p, CMD, &["name", "settings", "replace"])?;
    let name = name(p, CMD)?;
    let settings = settings(p.get("settings").unwrap_or(&Value::Null), CMD)?;
    let replace = p.get("replace").map(Value::as_bool).unwrap_or(Some(false)).ok_or_else(|| bad(CMD, "replace must be boolean"))?;
    let found = s.presets.camera_raw.iter().position(|v| v.name.eq_ignore_ascii_case(&name));
    if found.is_some() && !replace {
        return Err(bad(CMD, format!("A preset named \"{name}\" already exists")));
    }
    if found.is_none() && s.presets.camera_raw.len() >= MAX_PRESETS {
        return Err(bad(CMD, format!("At most {MAX_PRESETS} Camera Raw presets can be saved")));
    }
    let preset = CameraRawPreset { name, settings };
    if let Some(index) = found {
        s.presets.camera_raw[index] = preset.clone();
    } else {
        s.presets.camera_raw.push(preset.clone());
    }
    s.presets_changed();
    Ok(json!({"preset":preset,"replaced":found.is_some()}))
}

fn get(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cameraRaw.presets.get";
    check_keys(p, CMD, &["name"])?;
    let name = name(p, CMD)?;
    let preset = s.presets.camera_raw.iter().find(|pr| pr.name.eq_ignore_ascii_case(&name)).ok_or_else(|| bad(CMD, "No such Camera Raw preset"))?;
    Ok(json!({"preset":preset,"settings":preset.settings}))
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "cameraRaw.presets.delete";
    check_keys(p, CMD, &["name"])?;
    let name = name(p, CMD)?;
    let index = s.presets.camera_raw.iter().position(|pr| pr.name.eq_ignore_ascii_case(&name)).ok_or_else(|| bad(CMD, "No such Camera Raw preset"))?;
    let removed = s.presets.camera_raw.remove(index);
    s.presets_changed();
    Ok(json!({"deleted":removed.name}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "cameraRaw.presets.list",
            label: "List Camera Raw Presets",
            menu: &[],
            shortcut: None,
            params: "{} -> {presets:[{name,settings}]}",
            enabled: always,
            run: |s, p| {
                check_keys(p, "cameraRaw.presets.list", &[])?;
                Ok(json!({"presets":s.presets.camera_raw}))
            },
            journal: false,
        },
        CommandSpec {
            id: "cameraRaw.presets.get",
            label: "Get Camera Raw Preset",
            menu: &[],
            shortcut: None,
            params: r#"{"name":str} -> {preset,settings}; retrieving does not edit the document"#,
            enabled: always,
            run: get,
            journal: false,
        },
        CommandSpec {
            id: "cameraRaw.presets.save",
            label: "Save Camera Raw Preset",
            menu: &[],
            shortcut: None,
            params: r#"{"name":str,"settings":CameraRaw params,"replace":bool=false}; up to 128 snapshots persist with preferences"#,
            enabled: always,
            run: save,
            journal: true,
        },
        CommandSpec {
            id: "cameraRaw.presets.delete",
            label: "Delete Camera Raw Preset",
            menu: &[],
            shortcut: None,
            params: r#"{"name":str}"#,
            enabled: always,
            run: delete,
            journal: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reopened(s: &Session) -> Session {
        let mut next = Session::new();
        next.load_prefs_json(&s.prefs_to_json()).unwrap();
        next
    }

    #[test]
    fn snapshots_survive_restart_and_can_be_replaced_or_deleted() {
        let mut s = Session::new();
        let settings = json!({"temperature":-20,"exposure":0.75,"dehaze":25,"hslSat":[1,2,3,4,5,6,7,8],
            "pointCurve":[[0,0],[255,255]],"gradeShadows":{"hue":210,"sat":30,"lum":-10},"vignetteStyle":"colorPriority"});
        let saved = s.execute("cameraRaw.presets.save", json!({"name":"  Misty morning  ","settings":settings})).unwrap();
        assert_eq!(saved["preset"]["name"], "Misty morning");
        assert_eq!(saved["preset"]["settings"]["pixelScale"], json!(1.0));
        assert!(s.active().is_none(), "saving a preset does not edit a document");
        let mut s = reopened(&s);
        let rev = s.prefs.rev();
        let loaded = s.execute("cameraRaw.presets.get", json!({"name":"MISTY MORNING"})).unwrap();
        assert_eq!(loaded["settings"]["hslSat"], json!([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]));
        assert_eq!(loaded["settings"]["gradeShadows"]["hue"], json!(210.0));
        assert_eq!(s.prefs.rev(), rev, "queries do not write preferences");
        assert!(s.execute("cameraRaw.presets.save", json!({"name":"misty morning","settings":{}})).is_err());
        s.execute("cameraRaw.presets.save", json!({"name":"Misty morning","settings":{"dehaze":-30},"replace":true})).unwrap();
        assert_eq!(s.presets.camera_raw.len(), 1);
        assert_eq!(reopened(&s).presets.camera_raw[0].settings.dehaze, -30.0);
        s.execute("cameraRaw.presets.delete", json!({"name":"  misty MORNING "})).unwrap();
        assert!(reopened(&s).presets.camera_raw.is_empty());
    }

    #[test]
    fn malformed_entries_and_invalid_commands_never_poison_other_presets() {
        let mut original = Session::new();
        original.execute("cameraRaw.presets.save", json!({"name":"Valid","settings":{"exposure":1}})).unwrap();
        let valid = json!(original.presets.camera_raw[0]);
        let mut prefs = original.prefs_value();
        prefs["presets"]["cameraRaw"] = json!([
            {"name":"bad","settings":{"pixelScale":0}},
            {"name":"bad curve","settings":{"pointCurve":[[255,255],[0,0]]}},
            valid, valid,
            {"name":"range","settings":{"noiseLuminance":999999}},
        ]);
        let mut s = Session::new();
        s.load_prefs_json(&prefs.to_string()).unwrap();
        assert_eq!(s.presets.camera_raw.len(), 1);
        assert_eq!(s.presets.camera_raw[0].name, "Valid");
        for (cmd, bad_params) in [
            ("cameraRaw.presets.list", json!(null)),
            ("cameraRaw.presets.list", json!({"extra":true})),
            ("cameraRaw.presets.get", json!({"name":3})),
            ("cameraRaw.presets.delete", json!({"name":"missing"})),
            ("cameraRaw.presets.save", json!({"name":" ","settings":{}})),
            ("cameraRaw.presets.save", json!({"name":"Bad","settings":{"exposure":999}})),
            ("cameraRaw.presets.save", json!({"name":"Bad","settings":{"unknown":1}})),
            ("cameraRaw.presets.save", json!({"name":"Bad","settings":{"curveSplits":[0,0,100]}})),
            ("cameraRaw.presets.save", json!({"name":"Bad","settings":{},"replace":"yes"})),
        ] {
            assert!(s.execute(cmd, bad_params).is_err(), "{cmd} should reject malformed params");
        }
        assert_eq!(s.presets.camera_raw.len(), 1);
    }
}
