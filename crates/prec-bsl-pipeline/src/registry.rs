use std::collections::BTreeMap;

use crate::model::{ScenarioExecutionContext, ScenarioResult, ScenarioRun};
use prec_bsl_scenarios::{
    REFERENCE_SCENARIOS, ScenarioDefinition, ScenarioSupport, find_reference_scenario,
    normalize_scenario_id,
};

pub type ScenarioHandler = fn(&ScenarioExecutionContext<'_>) -> ScenarioRun;

#[derive(Debug, Clone)]
pub struct ScenarioRegistry {
    scenarios: BTreeMap<String, RegisteredScenario>,
}

impl ScenarioRegistry {
    pub fn reference() -> Self {
        let scenarios = REFERENCE_SCENARIOS
            .iter()
            .filter(|scenario| {
                matches!(
                    scenario.support,
                    ScenarioSupport::RequiredV1 | ScenarioSupport::Compatibility
                )
            })
            .map(|scenario| {
                (
                    scenario.id.to_owned(),
                    RegisteredScenario {
                        id: scenario.id.to_owned(),
                        definition: Some(scenario),
                        handler: handler_not_registered,
                        handles_deleted_files: false,
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

    pub fn with_handler(mut self, scenario_id: &str, handler: ScenarioHandler) -> Self {
        let normalized = normalize_scenario_id(scenario_id).to_owned();
        let definition = find_reference_scenario(&normalized);
        self.scenarios.insert(
            normalized.clone(),
            RegisteredScenario {
                id: normalized,
                definition,
                handler,
                handles_deleted_files: false,
            },
        );
        self
    }

    pub fn with_deleted_file_handler(
        mut self,
        scenario_id: &str,
        handler: ScenarioHandler,
    ) -> Self {
        let normalized = normalize_scenario_id(scenario_id).to_owned();
        let definition = find_reference_scenario(&normalized);
        self.scenarios.insert(
            normalized.clone(),
            RegisteredScenario {
                id: normalized,
                definition,
                handler,
                handles_deleted_files: true,
            },
        );
        self
    }

    pub fn get(&self, scenario_id: &str) -> Option<&RegisteredScenario> {
        self.scenarios.get(normalize_scenario_id(scenario_id))
    }
}

impl Default for ScenarioRegistry {
    fn default() -> Self {
        Self::reference()
    }
}

#[derive(Debug, Clone)]
pub struct RegisteredScenario {
    pub id: String,
    pub definition: Option<&'static ScenarioDefinition>,
    pub(crate) handler: ScenarioHandler,
    pub(crate) handles_deleted_files: bool,
}

fn handler_not_registered(context: &ScenarioExecutionContext<'_>) -> ScenarioRun {
    ScenarioRun::single(ScenarioResult::hard_failure(
        context.rule_id,
        context.file.repo_path.clone(),
        "scenario handler is not registered",
    ))
}
