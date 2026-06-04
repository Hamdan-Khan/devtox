#[derive(PartialEq, Debug)]
pub struct ScanEntry {
    pub path: String,
    pub size: u64,
}

pub enum ScanTraversalState {
    Outside,
    Inside { accumulated_size: u64, path: String },
}

#[derive(Default, PartialEq)]
pub struct ScanResult {
    pub total_size: u64,
    pub symlink_count: u64,
    pub error_count: u64,
    pub scanned_entries: Vec<ScanEntry>,
}

#[derive(PartialEq)]
pub enum ScanState {
    Idle,
    Confirmation,
    InProgress,
    Error,
    Completed(ScanResult),
}

pub struct ScanStatistics {
    pub total_size: u64,
    pub symlink_count: u64,
    pub error_count: u64,
    pub is_target_dir: bool,
    pub depth: usize,
    pub scanned_entries: Vec<ScanEntry>,
    pub traversal_state: ScanTraversalState,
}
