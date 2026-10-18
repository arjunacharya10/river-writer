//! Typed template data. Values reference the behaviour study, not Highland assets.
use river_model::{Kind, Style};
use serde::{Deserialize, Serialize};
use std::{str::FromStr, sync::OnceLock};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TemplateId {
    Screenplay,
    ScreenplayMulticam,
    Stageplay,
    Manuscript,
}
impl TemplateId {
    pub const ALL: [Self; 4] = [
        Self::Screenplay,
        Self::ScreenplayMulticam,
        Self::Stageplay,
        Self::Manuscript,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Screenplay => "screenplay",
            Self::ScreenplayMulticam => "screenplay-multicam",
            Self::Stageplay => "stageplay",
            Self::Manuscript => "manuscript",
        }
    }
    pub fn default_for(path: &std::path::Path) -> Self {
        if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("md"))
        {
            Self::Manuscript
        } else {
            Self::Screenplay
        }
    }
}
impl FromStr for TemplateId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL.into_iter().find(|i|i.name()==s).ok_or_else(||format!("Unknown template {s}; use screenplay, screenplay-multicam, stageplay, or manuscript"))
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Align {
    Left,
    Center,
    Right,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Geometry {
    pub x: f32,
    pub width: f32,
    pub align: Align,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Template {
    pub id: TemplateId,
    pub pitch: f32,
    pub dialogue_pitch: f32,
    pub first_indent: f32,
    pub cheat: f32,
    pub sentence_breaks: bool,
    pub print_sections: bool,
    pub number_first: bool,
    pub action: Geometry,
    pub scene: Geometry,
    pub cue: Geometry,
    pub parenthetical: Geometry,
    pub dialogue: Geometry,
    pub lyric: Geometry,
}
impl Template {
    pub fn get(id: TemplateId) -> &'static Self {
        static DATA: OnceLock<Vec<Template>> = OnceLock::new();
        DATA.get_or_init(|| {
            serde_json::from_str(include_str!("../templates.json"))
                .expect("valid bundled template data")
        })
        .iter()
        .find(|t| t.id == id)
        .expect("all four templates bundled")
    }
    pub fn geometry(&self, kind: Kind) -> Geometry {
        match kind {
            Kind::Scene => self.scene,
            Kind::Cue => self.cue,
            Kind::Parenthetical => self.parenthetical,
            Kind::Dialogue => self.dialogue,
            Kind::Lyric => self.lyric,
            Kind::Transition => Geometry {
                align: Align::Right,
                ..self.action
            },
            Kind::Centered | Kind::Section => Geometry {
                align: Align::Center,
                ..self.action
            },
            Kind::Blockquote => Geometry {
                x: self.action.x + 36.0,
                width: self.action.width - 36.0,
                ..self.action
            },
            _ => self.action,
        }
    }
    pub fn pitch_for(&self, kind: Kind) -> f32 {
        if matches!(kind, Kind::Dialogue | Kind::Lyric) {
            self.dialogue_pitch
        } else {
            self.pitch
        }
    }
    pub fn style(&self, kind: Kind) -> Style {
        Style {
            bold: matches!(kind, Kind::Section),
            italic: matches!(kind, Kind::Lyric | Kind::Blockquote)
                && self.id != TemplateId::Stageplay,
            ..Style::default()
        }
    }
    pub fn uppercase(&self, kind: Kind) -> bool {
        matches!(kind, Kind::Scene | Kind::Cue)
            || kind == Kind::Section
            || kind == Kind::Lyric && self.id == TemplateId::Stageplay
    }
    pub fn printable(&self, kind: Kind) -> bool {
        !matches!(
            kind,
            Kind::Note | Kind::Boneyard | Kind::Synopsis | Kind::Directive
        ) && (kind != Kind::Section || self.print_sections)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_geometry_fits_paper() {
        for id in TemplateId::ALL {
            let t = Template::get(id);
            for k in [
                Kind::Action,
                Kind::Scene,
                Kind::Cue,
                Kind::Dialogue,
                Kind::Parenthetical,
                Kind::Lyric,
            ] {
                let g = t.geometry(k);
                assert!(g.x >= 72.0 && g.width > 0.0 && g.x + g.width <= 540.01)
            }
        }
    }
    #[test]
    fn extension_defaults() {
        assert_eq!(
            TemplateId::default_for("story.MD".as_ref()),
            TemplateId::Manuscript
        );
        assert_eq!(
            TemplateId::default_for("script.fountain".as_ref()),
            TemplateId::Screenplay
        )
    }
}
