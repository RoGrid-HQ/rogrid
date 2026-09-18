use std::collections::BTreeMap;
use std::fmt;

use super::{Module, emit};

/// Public event signatures, without handler bodies or generated implementation details.
pub type Inventory = BTreeMap<String, String>;

pub fn inventory(modules: &[Module]) -> Inventory {
    modules
        .iter()
        .flat_map(|module| {
            module.events.iter().map(move |event| {
                let signature = event
                    .args
                    .iter()
                    .map(|arg| format!("{}: {}", arg.name, arg.ty))
                    .collect::<Vec<_>>()
                    .join(", ");
                (emit::id(module, event), signature)
            })
        })
        .collect()
}

pub struct Report {
    count: usize,
    changes: BTreeMap<String, char>,
}

impl Report {
    pub fn between(previous: &Inventory, current: &Inventory) -> Self {
        let mut changes = BTreeMap::new();
        for (name, signature) in current {
            match previous.get(name) {
                None => {
                    changes.insert(name.clone(), '+');
                }
                Some(old) if old != signature => {
                    changes.insert(name.clone(), '~');
                }
                Some(_) => {}
            }
        }
        for name in previous.keys() {
            if !current.contains_key(name) {
                changes.insert(name.clone(), '-');
            }
        }
        Self {
            count: current.len(),
            changes,
        }
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let noun = if self.count == 1 { "event" } else { "events" };
        write!(f, "Generated {} typed {noun}.", self.count)?;
        for (name, change) in &self.changes {
            write!(f, "\n  {change} {name}")?;
        }
        Ok(())
    }
}
