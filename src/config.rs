use crate::auth;

fn default_id_length() -> usize {
    4
}

fn default_database_connections() -> u32 {
    10
}

fn default_max_entries() -> i32 {
    10000
}

fn default_database_file() -> String {
    ":memory:".to_owned()
}

#[derive(serde::Deserialize)]
pub struct BibinConfig {
    pub password: auth::AuthKey,
    pub prefix: String,
    #[serde(default = "default_id_length")]
    pub id_length: usize,
    #[serde(default = "default_database_file")]
    pub database_file: String,
    #[serde(default = "default_database_connections")]
    pub database_connections: u32,
    #[serde(default = "default_max_entries")]
    pub max_entries: i32,
}
