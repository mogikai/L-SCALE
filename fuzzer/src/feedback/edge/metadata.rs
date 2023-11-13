use serde::{
    Deserialize,
    Serialize,
};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct EdgeFeedbackMetadata {}

libafl_bolts::impl_serdeany!(EdgeFeedbackMetadata);
