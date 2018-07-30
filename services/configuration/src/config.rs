/// Config for Configuration service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Number of votes required to commit the new configuration.
    /// This value should be greater than 2/3 and less or equal to the
    /// validators count.
    pub majority_count: Option<u16>,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            majority_count: None,
        }
    }
}
