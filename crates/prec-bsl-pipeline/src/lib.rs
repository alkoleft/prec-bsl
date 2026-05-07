mod model;
mod registry;
mod runner;

pub use model::{
    PipelineMode, PipelineReport, PipelineRequest, ScenarioExecutionContext, ScenarioResult,
    ScenarioResultStatus, ScenarioRun, SourceSpan,
};
pub use registry::{RegisteredScenario, ScenarioDefinition, ScenarioHandler, ScenarioRegistry};
pub use runner::run_pipeline;

#[cfg(test)]
mod tests;
