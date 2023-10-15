pub mod config;

use std::{collections::HashMap, fmt::Display, ops::Deref};

use egui::Pos2;
use serde::{Deserialize, Serialize};

use anyhow::Result;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Copy, Eq, Hash)]
pub struct ClientID(pub usize);

impl Display for ClientID {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

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

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default)]
#[serde(rename_all = "lowercase")]
pub enum Flag {
    #[default]
    None,
    Clear,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, Clone)]
pub struct SendLines {
    pub lines: HashMap<usize, Vec<SPos2>>,
    pub flag: Flag,
}

impl SendLines {
    pub fn merge(&mut self, other: Self) {
        for (line_id, line) in other.lines.iter() {
            match self.lines.get_mut(line_id) {
                Some(_) => (),
                None => {
                    self.lines.insert(*line_id, line.clone());
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct Peer(pub String);

impl Peer {
    pub fn ip(&self) -> Result<&str> {
        match self.0.split_once(':') {
            Some((ip, _)) => Ok(ip),
            None => Err(anyhow::anyhow!("Failed to get ip from peer")),
        }
    }
}

impl Display for Peer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0.split_once(':') {
            Some((ip, port)) => write!(f, "{}:{}", ip, port),
            None => Err(std::fmt::Error),
        }
    }
}
