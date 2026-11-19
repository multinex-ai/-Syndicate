//! Machine-local opt-out; shared installations keep memory guards enabled.
use serde_json::Value;
use std::path::Path;

pub fn memory_guard_enabled(root: &Path) -> bool {
    std::fs::read_to_string(root.join("shared/runtime/resource-policy.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| value.get("memory_guard_enabled").and_then(Value::as_bool))
        .unwrap_or(true)
}

#[cfg(test)]
mod tests {
    use crate::runtime::{admission_with_memory_policy, runtime_with_memory_policy, Sample};
    #[test]
    fn memory_opt_out_retains_disk_reserve() {
        let gib = 1u64 << 30;
        let s = Sample {
            time: 0.0,
            physical_bytes: 18 * gib,
            available_bytes: gib,
            pressure_level: 4,
            swap_used_bytes: 20 * gib,
            disk_free_bytes: 30 * gib,
            worker_rss_bytes: 12 * gib,
            worker_footprint_bytes: 12 * gib,
        };
        assert_eq!(admission_with_memory_policy(&s, false), None);
        assert_eq!(runtime_with_memory_policy(&s, &s, false), None);
        assert!(admission_with_memory_policy(&s, true).is_some());
        let low_disk = Sample {
            disk_free_bytes: gib,
            ..s
        };
        assert!(admission_with_memory_policy(&low_disk, false).is_some());
        assert!(runtime_with_memory_policy(&low_disk, &low_disk, false).is_some());
    }
}
