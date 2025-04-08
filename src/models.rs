


#[derive(Default, Clone, Copy, Debug)]
pub enum DownloadStatus {
    #[default]
    NotStarted,
    InProgress,
    Finished,
    Failed,
}