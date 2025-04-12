


#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Debug, Default)]
pub enum DownloadStatus {
    #[default]
    NotStarted,
    InProgress,
    Finished,
    Failed(String),
    Cancelled
}