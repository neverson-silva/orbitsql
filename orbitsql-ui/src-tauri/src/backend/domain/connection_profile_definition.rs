use serde::{Deserialize, Serialize};

fn default_profile_color() -> String {
    "#84cc16".to_owned()
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionProfileDefinition {
    pub id: String,
    pub name: String,
    #[serde(default = "default_profile_color", alias = "environment")]
    pub color: String,
    pub description: String,
}

pub fn built_in_connection_profile_definitions() -> Vec<ConnectionProfileDefinition> {
    vec![
        ConnectionProfileDefinition {
            id: "development".to_owned(),
            name: "Development".to_owned(),
            color: "#84cc16".to_owned(),
            description: "Local work with relaxed safety guardrails.".to_owned(),
        },
        ConnectionProfileDefinition {
            id: "testing".to_owned(),
            name: "Testing".to_owned(),
            color: "#f59e0b".to_owned(),
            description: "Controlled validation with predictable defaults.".to_owned(),
        },
        ConnectionProfileDefinition {
            id: "production".to_owned(),
            name: "Production".to_owned(),
            color: "#ef4444".to_owned(),
            description: "Protected workflows for live database environments.".to_owned(),
        },
    ]
}
