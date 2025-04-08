
pub struct BucketProgress {
    pub id: u8,
    pub progress: u64
}

#[derive(Default, Clone, Copy)]
pub enum DownloadStatus {
    #[default]
    NotStarted,
    InProgress,
    Finished,
    Failed,
}