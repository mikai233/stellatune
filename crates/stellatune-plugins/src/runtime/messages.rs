#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerControlMessage {
    Recreate { reason: String, seq: u64 },
    Destroy { reason: String, seq: u64 },
}
