
pub struct BucketProgress {
    pub id: u8,
    pub progress: u64
}

pub enum Update {
    Progress(BucketProgress),
    Finished,
    Failed,
    NotStarted
}