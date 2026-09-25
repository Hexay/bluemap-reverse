//! Block-state interning: hot paths (fill, chunk building) work on `StateId`s instead of cloning,
//! hashing and comparing `BlockState` strings per cell.

use rustc_hash::FxHashMap;

use crate::chunk::BlockState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StateId(pub u32);

#[derive(Default)]
pub struct StateTable {
    states: Vec<BlockState>,
    air: Vec<bool>,
    ids: FxHashMap<BlockState, StateId>,
}

impl StateTable {
    pub fn intern(&mut self, s: &BlockState) -> StateId {
        if let Some(&id) = self.ids.get(s) {
            return id;
        }
        let id = StateId(self.states.len() as u32);
        self.states.push(s.clone());
        self.air.push(s.is_air());
        self.ids.insert(s.clone(), id);
        id
    }

    pub fn get(&self, id: StateId) -> &BlockState {
        &self.states[id.0 as usize]
    }

    pub fn is_air(&self, id: StateId) -> bool {
        self.air[id.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (StateId, &BlockState)> {
        self.states.iter().enumerate().map(|(i, s)| (StateId(i as u32), s))
    }
}
