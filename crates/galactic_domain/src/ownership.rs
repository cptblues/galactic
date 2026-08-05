use crate::FactionId;

/// Stable ownership reference shared by every possessable domain object.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Owner {
    Unowned,
    Faction(FactionId),
}

impl Owner {
    pub const fn faction(self) -> Option<FactionId> {
        match self {
            Self::Unowned => None,
            Self::Faction(faction_id) => Some(faction_id),
        }
    }

    pub const fn is_faction(self, faction_id: FactionId) -> bool {
        matches!(self, Self::Faction(owner_id) if owner_id.raw() == faction_id.raw())
    }
}

impl From<FactionId> for Owner {
    fn from(faction_id: FactionId) -> Self {
        Self::Faction(faction_id)
    }
}

/// Implemented by domain objects whose management is controlled by an owner.
pub trait Owned {
    fn owner(&self) -> Owner;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faction_owner_exposes_its_stable_id() {
        let faction_id = FactionId::new(7);
        let owner = Owner::from(faction_id);

        assert_eq!(owner.faction(), Some(faction_id));
        assert!(owner.is_faction(faction_id));
        assert!(!Owner::Unowned.is_faction(faction_id));
    }
}
