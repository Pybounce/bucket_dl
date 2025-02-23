
pub struct ChunkProgress {
    pub id: u8,
    pub progress: u64
}

pub enum Update {
    Progress(ChunkProgress),
    Finished,
    Failed
}