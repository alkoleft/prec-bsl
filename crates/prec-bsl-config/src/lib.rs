mod error;
mod model;
mod path;
mod raw;
mod resolve;
mod validation;

pub use error::ConfigError;
pub use model::{
    ConfigSource, ConfigWarning, GlobalConfig, ProjectScenarioConfig, RepositoryScenarioSettings,
    ResolvedConfig, ScenarioConfig,
};
pub use resolve::{ConfigResolveRequest, built_in_defaults, parse_config_str, resolve_config};

#[cfg(test)]
mod tests;
