use serde_derive::{
    Deserialize,
    Serialize,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JIRCustomOp {
    pub name: String,
}
