use serde::{
    Deserialize,
    Serialize,
};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct EdgeFeedbackMetadata {
    pub hit_map: Vec<u64>,
}
