use std::ops::Deref;

use egui::Pos2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub struct SPos2(pub Pos2);

impl Serialize for SPos2 {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        (self.0.x, self.0.y).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SPos2 {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (x, y) = Deserialize::deserialize(deserializer)?;

        Ok(Self(Pos2::new(x, y)))
    }
}

impl Deref for SPos2 {
    type Target = Pos2;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, Clone)]
pub struct SendLines {
    pub lines: Vec<Vec<SPos2>>,
    pub line_ids: Vec<usize>,
}

impl SendLines {
    pub fn merge(&mut self, other: Self) {
        for (i, line_id) in other.line_ids.iter().enumerate() {
            if !self.line_ids.contains(line_id) {
                self.line_ids.push(*line_id);

                self.lines.push(other.lines[i].clone());
            }
        }
    }
}
