use serde::{
    Deserialize,
    Serialize,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct JIRCustomOp {
    pub name: String,
}
