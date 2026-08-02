use super::MutationMemoryRecord;

pub fn find_similar(
    records: &[MutationMemoryRecord],
    function: &str,
) -> Vec<MutationMemoryRecord> {
    records.iter().filter(|r| r.target_function == function).cloned().collect()
}
