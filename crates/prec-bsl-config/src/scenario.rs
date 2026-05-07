#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScenarioSupport {
    RequiredV1,
    Compatibility,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScenarioMetadata {
    pub id: &'static str,
    pub source_file: &'static str,
    pub support: ScenarioSupport,
}

impl ScenarioMetadata {
    pub const fn required_v1(id: &'static str, source_file: &'static str) -> Self {
        Self {
            id,
            source_file,
            support: ScenarioSupport::RequiredV1,
        }
    }

    pub const fn compatibility(id: &'static str, source_file: &'static str) -> Self {
        Self {
            id,
            source_file,
            support: ScenarioSupport::Compatibility,
        }
    }

    pub const fn unsupported(id: &'static str, source_file: &'static str) -> Self {
        Self {
            id,
            source_file,
            support: ScenarioSupport::Unsupported,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ScenarioCatalog<'a> {
    scenarios: &'a [ScenarioMetadata],
}

impl<'a> ScenarioCatalog<'a> {
    pub const fn new(scenarios: &'a [ScenarioMetadata]) -> Self {
        Self { scenarios }
    }

    pub fn scenarios(self) -> &'a [ScenarioMetadata] {
        self.scenarios
    }

    pub fn find(self, id: &str) -> Option<&'a ScenarioMetadata> {
        let normalized = normalize_scenario_id(id);
        self.scenarios
            .iter()
            .find(|scenario| scenario.id == normalized)
    }

    pub fn required_v1(self) -> impl Iterator<Item = &'a ScenarioMetadata> {
        self.scenarios
            .iter()
            .filter(|scenario| scenario.support == ScenarioSupport::RequiredV1)
    }

    pub fn supported(self) -> impl Iterator<Item = &'a ScenarioMetadata> {
        self.scenarios.iter().filter(|scenario| {
            matches!(
                scenario.support,
                ScenarioSupport::RequiredV1 | ScenarioSupport::Compatibility
            )
        })
    }
}

pub const UNSUPPORTED_ORDINARY_FORMS: &str = "РазборОбычныхФормНаИсходники";

pub fn normalize_scenario_id(value: &str) -> &str {
    value.trim().strip_suffix(".os").unwrap_or(value.trim())
}
