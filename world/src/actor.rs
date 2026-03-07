use std::fmt;

use serde::{Deserialize, Serialize};

use crate::econ::EconState;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ActorId(pub u64);

impl fmt::Display for ActorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Actor({})", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActorKind {
    State,
    Organization,
    SubState,
    NonStateArmed,
    Economic,
    Transnational,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Actor {
    pub id: ActorId,
    pub kind: ActorKind,
    pub name: String,
    pub econ: EconState,
}

impl Actor {
    pub fn new(id: ActorId, kind: ActorKind, name: String, econ: EconState) -> Self {
        Self {
            id,
            kind,
            name,
            econ,
        }
    }
}
