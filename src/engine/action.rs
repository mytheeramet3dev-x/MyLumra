use std::collections::HashMap;

/// Parsed representation of a `lumra-action://` protocol URL
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LumraAction {
    pub name: String,
    pub params: HashMap<String, String>,
}

impl LumraAction {
    /// Parse a URL like `lumra-action://increment?step=1` or `lumra-action://reset`
    pub fn parse(url: &str) -> Option<Self> {
        let action_str = url.strip_prefix("lumra-action://")?;
        let mut parts = action_str.splitn(2, '?');
        let name = parts.next()?.trim_matches('/').to_string();
        if name.is_empty() {
            return None;
        }

        let mut params = HashMap::new();
        if let Some(query) = parts.next() {
            for pair in query.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    params.insert(k.to_string(), v.to_string());
                } else if !pair.is_empty() {
                    params.insert(pair.to_string(), "true".to_string());
                }
            }
        }
        Some(Self { name, params })
    }

    /// Mutate an in-memory state dictionary based on the action
    pub fn apply(&self, state: &mut HashMap<String, String>) {
        match self.name.as_str() {
            "increment" | "inc" => {
                let key = self.params.get("key").map(|s| s.as_str()).unwrap_or("count");
                let step: i64 = self.params.get("step").and_then(|s| s.parse().ok()).unwrap_or(1);
                let current: i64 = state.get(key).and_then(|s| s.parse().ok()).unwrap_or(0);
                state.insert(key.to_string(), (current + step).to_string());
            }
            "decrement" | "dec" => {
                let key = self.params.get("key").map(|s| s.as_str()).unwrap_or("count");
                let step: i64 = self.params.get("step").and_then(|s| s.parse().ok()).unwrap_or(1);
                let current: i64 = state.get(key).and_then(|s| s.parse().ok()).unwrap_or(0);
                state.insert(key.to_string(), (current - step).to_string());
            }
            "reset" => {
                let key = self.params.get("key").map(|s| s.as_str()).unwrap_or("count");
                let default_val = self.params.get("value").map(|s| s.as_str()).unwrap_or("0");
                state.insert(key.to_string(), default_val.to_string());
            }
            "toggle" => {
                let key = self.params.get("key").map(|s| s.as_str()).unwrap_or("toggle");
                let current = state.get(key).map(|s| s == "true").unwrap_or(false);
                state.insert(key.to_string(), (!current).to_string());
            }
            "set" => {
                if let Some(key) = self.params.get("key") {
                    let val = self.params.get("value").cloned().unwrap_or_default();
                    state.insert(key.clone(), val);
                }
            }
            other => {
                tracing::warn!("Unrecognized Lumra action: {}", other);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_action() {
        let action = LumraAction::parse("lumra-action://increment?step=2&key=counter").unwrap();
        assert_eq!(action.name, "increment");
        assert_eq!(action.params.get("step").unwrap(), "2");
        assert_eq!(action.params.get("key").unwrap(), "counter");
    }

    #[test]
    fn test_apply_counter() {
        let mut state = HashMap::new();
        let inc = LumraAction::parse("lumra-action://increment?step=5").unwrap();
        inc.apply(&mut state);
        assert_eq!(state.get("count").unwrap(), "5");

        let dec = LumraAction::parse("lumra-action://decrement?step=2").unwrap();
        dec.apply(&mut state);
        assert_eq!(state.get("count").unwrap(), "3");

        let reset = LumraAction::parse("lumra-action://reset").unwrap();
        reset.apply(&mut state);
        assert_eq!(state.get("count").unwrap(), "0");
    }
}
