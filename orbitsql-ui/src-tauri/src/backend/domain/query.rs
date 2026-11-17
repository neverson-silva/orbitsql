use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct QueryColumn {
    pub name: String,
    pub type_name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<QueryColumn>,
    pub rows: Vec<Vec<Value>>,
    pub row_count: usize,
    pub affected_rows: Option<u64>,
    pub elapsed_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueryResultBatch {
    pub rows: Vec<Vec<Value>>,
    pub row_count: usize,
    pub has_more: bool,
    pub total_fetched: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct QueryResultSessionStart {
    pub session_id: String,
    pub columns: Vec<QueryColumn>,
    pub first_batch: QueryResultBatch,
    pub elapsed_ms: u128,
}
