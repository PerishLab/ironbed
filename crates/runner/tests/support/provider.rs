use std::collections::BTreeSet;

#[derive(Clone, Copy)]
pub(super) struct Facts {
    boundary: &'static str,
    temporary: &'static [&'static str],
    helpers: &'static [&'static str],
    retained: &'static [&'static str],
}

#[derive(Debug)]
pub(super) struct Cleanup {
    generation: String,
    boundary: &'static str,
    temporary: BTreeSet<&'static str>,
    helpers: BTreeSet<&'static str>,
    retained: BTreeSet<&'static str>,
}

impl Cleanup {
    pub(super) fn new(generation: impl Into<String>, facts: Facts) -> Self {
        Self {
            generation: generation.into(),
            boundary: facts.boundary,
            temporary: facts.temporary.iter().copied().collect(),
            helpers: facts.helpers.iter().copied().collect(),
            retained: facts.retained.iter().copied().collect(),
        }
    }

    pub(super) fn proves(&self, generation: &str, facts: Facts) {
        assert_eq!(self.generation, generation);
        assert_eq!(self.boundary, facts.boundary);
        assert_eq!(self.temporary, facts.temporary.iter().copied().collect());
        assert_eq!(self.helpers, facts.helpers.iter().copied().collect());
        assert_eq!(self.retained, facts.retained.iter().copied().collect());
    }
}

impl Facts {
    pub(super) const fn new(
        boundary: &'static str,
        temporary: &'static [&'static str],
        helpers: &'static [&'static str],
        retained: &'static [&'static str],
    ) -> Self {
        Self {
            boundary,
            temporary,
            helpers,
            retained,
        }
    }
}
