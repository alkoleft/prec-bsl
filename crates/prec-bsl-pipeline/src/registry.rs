use std::collections::BTreeMap;

use crate::model::{ScenarioExecutionContext, ScenarioResult, ScenarioRun};
use prec_bsl_config::{ScenarioCatalog, ScenarioMetadata, normalize_scenario_id};

pub type ScenarioHandler = fn(&ScenarioExecutionContext<'_>) -> ScenarioRun;

#[derive(Debug, Clone, Copy)]
pub struct ScenarioDefinition {
    pub metadata: ScenarioMetadata,
    pub handler: ScenarioHandler,
    pub handles_deleted_files: bool,
}

impl ScenarioDefinition {
    pub const fn new(metadata: ScenarioMetadata, handler: ScenarioHandler) -> Self {
        Self {
            metadata,
            handler,
            handles_deleted_files: false,
        }
    }

    pub const fn with_deleted_files(mut self) -> Self {
        self.handles_deleted_files = true;
        self
    }

    pub const fn required_v1(
        id: &'static str,
        source_file: &'static str,
        handler: ScenarioHandler,
    ) -> Self {
        Self::new(ScenarioMetadata::required_v1(id, source_file), handler)
    }

    pub const fn compatibility(
        id: &'static str,
        source_file: &'static str,
        handler: ScenarioHandler,
    ) -> Self {
        Self::new(ScenarioMetadata::compatibility(id, source_file), handler)
    }
}

#[derive(Debug, Clone)]
pub struct ScenarioRegistry {
    scenarios: BTreeMap<String, RegisteredScenario>,
}

impl ScenarioRegistry {
    pub fn reference(catalog: ScenarioCatalog<'_>) -> Self {
        let scenarios = catalog
            .supported()
            .map(|scenario| {
                (
                    scenario.id.to_owned(),
                    RegisteredScenario {
                        id: scenario.id.to_owned(),
                        metadata: Some(*scenario),
                        handler: handler_not_registered,
                        handles_deleted_files: false,
                        handler_registered: false,
                    },
                )
            })
            .collect();

        Self { scenarios }
    }

    pub fn empty() -> Self {
        Self {
            scenarios: BTreeMap::new(),
        }
    }

    pub fn with_definition(mut self, definition: ScenarioDefinition) -> Self {
        let normalized = normalize_scenario_id(definition.metadata.id).to_owned();
        self.scenarios.insert(
            normalized.clone(),
            RegisteredScenario {
                id: normalized,
                metadata: Some(definition.metadata),
                handler: definition.handler,
                handles_deleted_files: definition.handles_deleted_files,
                handler_registered: true,
            },
        );
        self
    }

    pub fn with_definitions(
        mut self,
        definitions: impl IntoIterator<Item = ScenarioDefinition>,
    ) -> Self {
        for definition in definitions {
            self = self.with_definition(definition);
        }
        self
    }

    pub fn get(&self, scenario_id: &str) -> Option<&RegisteredScenario> {
        self.scenarios.get(normalize_scenario_id(scenario_id))
    }
}

impl Default for ScenarioRegistry {
    fn default() -> Self {
        Self::empty()
    }
}

#[derive(Debug, Clone)]
pub struct RegisteredScenario {
    pub id: String,
    pub metadata: Option<ScenarioMetadata>,
    pub(crate) handler: ScenarioHandler,
    pub(crate) handles_deleted_files: bool,
    handler_registered: bool,
}

impl RegisteredScenario {
    pub fn has_registered_handler(&self) -> bool {
        self.handler_registered
    }
}

fn handler_not_registered(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    ScenarioRun::single(ScenarioResult::hard_failure(
        context.rule_id,
        context.file.repo_path.clone(),
        "scenario handler is not registered",
    ))
}
