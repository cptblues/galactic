use galactic_domain::FactionId;

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub enum DiplomaticRelation {
    #[default]
    Unknown,
    Neutral,
    Hostile,
    Allied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FactionRelation {
    pub first: FactionId,
    pub second: FactionId,
    pub relation: DiplomaticRelation,
}

impl FactionRelation {
    pub fn new(
        first: FactionId,
        second: FactionId,
        relation: DiplomaticRelation,
    ) -> Result<Self, DiplomacyError> {
        if first == second {
            return Err(DiplomacyError::SelfRelation(first));
        }
        let (first, second) = canonical_pair(first, second);
        Ok(Self {
            first,
            second,
            relation,
        })
    }

    pub const fn involves(self, faction: FactionId) -> bool {
        self.first.raw() == faction.raw() || self.second.raw() == faction.raw()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DiplomacyState {
    default_relation: DiplomaticRelation,
    relations: Vec<FactionRelation>,
}

impl DiplomacyState {
    pub fn new(
        default_relation: DiplomaticRelation,
        relations: impl IntoIterator<Item = FactionRelation>,
    ) -> Result<Self, DiplomacyError> {
        let mut relations = relations
            .into_iter()
            .map(|entry| FactionRelation::new(entry.first, entry.second, entry.relation))
            .collect::<Result<Vec<_>, _>>()?;
        relations.sort_by_key(|entry| (entry.first, entry.second));
        for window in relations.windows(2) {
            if window[0].first == window[1].first && window[0].second == window[1].second {
                return Err(DiplomacyError::DuplicateRelation {
                    first: window[0].first,
                    second: window[0].second,
                });
            }
        }
        if let Some(redundant) = relations
            .iter()
            .find(|entry| entry.relation == default_relation)
        {
            return Err(DiplomacyError::RedundantDefaultRelation {
                first: redundant.first,
                second: redundant.second,
            });
        }
        Ok(Self {
            default_relation,
            relations,
        })
    }

    pub const fn default_relation(&self) -> DiplomaticRelation {
        self.default_relation
    }

    pub fn relations(&self) -> &[FactionRelation] {
        &self.relations
    }

    pub fn relation_between(&self, first: FactionId, second: FactionId) -> DiplomaticRelation {
        if first == second {
            return DiplomaticRelation::Allied;
        }
        let (first, second) = canonical_pair(first, second);
        self.relations
            .binary_search_by_key(&(first, second), |entry| (entry.first, entry.second))
            .ok()
            .map(|index| self.relations[index].relation)
            .unwrap_or(self.default_relation)
    }

    pub fn set_relation(
        &mut self,
        first: FactionId,
        second: FactionId,
        relation: DiplomaticRelation,
    ) -> Result<bool, DiplomacyError> {
        if first == second {
            return Err(DiplomacyError::SelfRelation(first));
        }
        let (first, second) = canonical_pair(first, second);
        match self
            .relations
            .binary_search_by_key(&(first, second), |entry| (entry.first, entry.second))
        {
            Ok(index) if relation == self.default_relation => {
                self.relations.remove(index);
                Ok(true)
            }
            Ok(index) if self.relations[index].relation == relation => Ok(false),
            Ok(index) => {
                self.relations[index].relation = relation;
                Ok(true)
            }
            Err(_) if relation == self.default_relation => Ok(false),
            Err(index) => {
                self.relations.insert(
                    index,
                    FactionRelation {
                        first,
                        second,
                        relation,
                    },
                );
                Ok(true)
            }
        }
    }
}

impl Default for DiplomacyState {
    fn default() -> Self {
        Self {
            default_relation: DiplomaticRelation::Unknown,
            relations: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiplomacyError {
    UnknownFaction(FactionId),
    SelfRelation(FactionId),
    DuplicateRelation { first: FactionId, second: FactionId },
    RedundantDefaultRelation { first: FactionId, second: FactionId },
}

fn canonical_pair(first: FactionId, second: FactionId) -> (FactionId, FactionId) {
    if first <= second {
        (first, second)
    } else {
        (second, first)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relations_are_symmetric_and_sorted() {
        let player = FactionId::new(0);
        let foreign = FactionId::new(2);
        let relation =
            FactionRelation::new(foreign, player, DiplomaticRelation::Hostile).expect("valid");
        let diplomacy =
            DiplomacyState::new(DiplomaticRelation::Unknown, [relation]).expect("valid");

        assert_eq!(
            diplomacy.relation_between(player, foreign),
            DiplomaticRelation::Hostile,
        );
        assert_eq!(
            diplomacy.relation_between(foreign, player),
            DiplomaticRelation::Hostile,
        );
        assert_eq!(
            diplomacy.relation_between(player, player),
            DiplomaticRelation::Allied,
        );
    }

    #[test]
    fn setting_default_removes_the_explicit_override() {
        let first = FactionId::new(0);
        let second = FactionId::new(1);
        let mut diplomacy = DiplomacyState::default();

        assert!(
            diplomacy
                .set_relation(first, second, DiplomaticRelation::Neutral)
                .expect("valid"),
        );
        assert!(
            diplomacy
                .set_relation(first, second, DiplomaticRelation::Unknown)
                .expect("valid"),
        );
        assert!(diplomacy.relations().is_empty());
    }
}
