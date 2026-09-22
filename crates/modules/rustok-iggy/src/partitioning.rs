use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use uuid::Uuid;

pub fn partition_key(tenant_id: Uuid) -> String {
    tenant_id.to_string()
}

pub fn calculate_partition(key: &str, num_partitions: u32) -> u32 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let hash = hasher.finish();

    (hash % num_partitions as u64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partition_key_produces_consistent_format() {
        let tenant_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let key = partition_key(tenant_id);

        assert_eq!(key, "550e8400-e29b-41d4-a716-446655440000");
    }

    #[test]
    fn calculate_partition_is_deterministic() {
        let key = "tenant-123";
        let p1 = calculate_partition(key, 8);
        let p2 = calculate_partition(key, 8);

        assert_eq!(p1, p2);
    }

    #[test]
    fn calculate_partition_distributes_keys() {
        let mut partitions = std::collections::HashSet::new();

        for i in 0..100 {
            let key = format!("tenant-{}", i);
            let partition = calculate_partition(&key, 8);
            assert!(partition < 8, "Partition {} out of range", partition);
            partitions.insert(partition);
        }

        assert!(partitions.len() > 1, "All keys mapped to same partition");
    }

    #[test]
    fn calculate_partition_respects_partition_count() {
        for i in 0..1000 {
            let key = format!("key-{}", i);
            let partition = calculate_partition(&key, 16);
            assert!(
                partition < 16,
                "Partition {} out of range for 16 partitions",
                partition
            );
        }
    }
}
