use std::collections::BTreeMap;
use std::fmt;

use super::{EndpointKind, Module, emit};

/// Public event signatures, without handler bodies or generated implementation details.
pub type Inventory = BTreeMap<String, String>;

pub fn inventory(modules: &[Module]) -> Inventory {
    modules
        .iter()
        .flat_map(|module| {
            module.endpoints.iter().map(move |event| {
                let signature = event
                    .args
                    .iter()
                    .map(|arg| format!("{}: {}", arg.name, arg.ty))
                    .collect::<Vec<_>>()
                    .join(", ");
                let signature = match &event.kind {
                    EndpointKind::Event(reliability) => {
                        format!("event {} ({signature})", reliability.name())
                    }
                    EndpointKind::Request(returns) => {
                        format!("request ({signature}) -> {}", emit::returns(returns))
                    }
                };
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
        let noun = if self.count == 1 {
            "endpoint"
        } else {
            "endpoints"
        };
        write!(f, "Generated {} typed {noun}.", self.count)?;
        for (name, change) in &self.changes {
            write!(f, "\n  {change} {name}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_additions_removals_and_signature_changes_in_name_order() {
        let before = Inventory::from([
            ("server.Shop.buy".into(), "id: string".into()),
            ("server.Shop.sell".into(), "id: string".into()),
            ("server.Lobby.ready".into(), "ready: boolean".into()),
        ]);
        let after = Inventory::from([
            ("client.Notice.show".into(), "message: string".into()),
            ("server.Shop.buy".into(), "id: number".into()),
            ("server.Lobby.ready".into(), "ready: boolean".into()),
        ]);
        assert_eq!(
            Report::between(&before, &after).to_string(),
            "Generated 3 typed endpoints.\n  + client.Notice.show\n  ~ server.Shop.buy\n  - server.Shop.sell"
        );
        assert_eq!(
            Report::between(&after, &after).to_string(),
            "Generated 3 typed endpoints."
        );
    }

    #[test]
    fn reports_empty_and_single_event_inventories() {
        let empty = Inventory::new();
        let one = Inventory::from([("server.Lobby.ready".into(), String::new())]);
        assert_eq!(
            Report::between(&empty, &empty).to_string(),
            "Generated 0 typed endpoints."
        );
        assert_eq!(
            Report::between(&empty, &one).to_string(),
            "Generated 1 typed endpoint.\n  + server.Lobby.ready"
        );
        assert_eq!(
            Report::between(&one, &empty).to_string(),
            "Generated 0 typed endpoints.\n  - server.Lobby.ready"
        );
    }
}
