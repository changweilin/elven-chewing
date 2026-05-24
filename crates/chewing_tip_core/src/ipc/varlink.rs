use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Default, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct MethodCall {
    pub method: String,
    pub parameters: Value,
    pub oneway: Option<bool>,
    pub more: Option<bool>,
    pub upgrade: Option<bool>,
}

#[derive(Default, Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct MethodReply {
    pub parameters: Value,
    pub continues: Option<bool>,
    pub error: Option<String>,
}
