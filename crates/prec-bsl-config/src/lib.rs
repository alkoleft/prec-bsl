mod error;
mod model;
mod path;
mod raw;
mod resolve;
mod scenario;
mod validation;

pub use error::ConfigError;
pub use model::{
    ConfigSource, ConfigWarning, GlobalConfig, ProjectScenarioConfig, RepositoryScenarioSettings,
    ResolvedConfig, ScenarioConfig,
};
pub use resolve::{
    ConfigResolveRequest, built_in_defaults_with_catalog, parse_config_str_with_catalog,
    resolve_config_with_catalog,
};
pub use scenario::{
    ScenarioCatalog, ScenarioMetadata, ScenarioSupport, UNSUPPORTED_ORDINARY_FORMS,
    normalize_scenario_id,
};

#[cfg(test)]
mod tests;
