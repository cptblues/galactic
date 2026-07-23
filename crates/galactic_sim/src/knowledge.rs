use std::fmt;

use galactic_domain::{PlanetId, SystemId};

/// Progressive information available to the player.
///
/// Knowledge is monotone: simulation code may only keep or increase a level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum KnowledgeLevel {
    #[default]
    Unknown = 0,
    Detected = 1,
    Probed = 2,
    Analyzed = 3,
    Colonized = 4,
}

impl KnowledgeLevel {
    pub const fn is_visible(self) -> bool {
        self as u8 >= Self::Detected as u8
    }

    pub const fn reveals_identity(self) -> bool {
        self as u8 >= Self::Probed as u8
    }

    pub const fn reveals_exact_details(self) -> bool {
        self as u8 >= Self::Analyzed as u8
    }

    pub const fn can_enter_system(self) -> bool {
        self.reveals_identity()
    }

    /// Temporary progression used by the MVP debug key until probe missions
    /// become available. Colonization is never granted by this method.
    pub const fn next_exploration_level(self) -> Option<Self> {
        match self {
            Self::Unknown => Some(Self::Detected),
            Self::Detected => Some(Self::Probed),
            Self::Probed => Some(Self::Analyzed),
            Self::Analyzed | Self::Colonized => None,
        }
    }
}

impl fmt::Display for KnowledgeLevel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Unknown => "unknown",
            Self::Detected => "detected",
            Self::Probed => "probed",
            Self::Analyzed => "analyzed",
            Self::Colonized => "colonized",
        };
        formatter.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SystemKnowledge {
    pub system_id: SystemId,
    pub level: KnowledgeLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PlanetKnowledge {
    pub planet_id: PlanetId,
    pub level: KnowledgeLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnowledgeTarget {
    System(SystemId),
    Planet(PlanetId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KnowledgeChange {
    pub target: KnowledgeTarget,
    pub previous: KnowledgeLevel,
    pub current: KnowledgeLevel,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct KnowledgeCounts {
    pub detected: usize,
    pub probed: usize,
    pub analyzed: usize,
    pub colonized: usize,
}

impl KnowledgeCounts {
    pub fn include(&mut self, level: KnowledgeLevel) {
        match level {
            KnowledgeLevel::Unknown => {}
            KnowledgeLevel::Detected => self.detected += 1,
            KnowledgeLevel::Probed => self.probed += 1,
            KnowledgeLevel::Analyzed => self.analyzed += 1,
            KnowledgeLevel::Colonized => self.colonized += 1,
        }
    }

    pub const fn visible_total(self) -> usize {
        self.detected + self.probed + self.analyzed + self.colonized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exploration_progression_stops_before_colonization() {
        assert_eq!(
            KnowledgeLevel::Unknown.next_exploration_level(),
            Some(KnowledgeLevel::Detected)
        );
        assert_eq!(
            KnowledgeLevel::Detected.next_exploration_level(),
            Some(KnowledgeLevel::Probed)
        );
        assert_eq!(
            KnowledgeLevel::Probed.next_exploration_level(),
            Some(KnowledgeLevel::Analyzed)
        );
        assert_eq!(KnowledgeLevel::Analyzed.next_exploration_level(), None);
    }

    #[test]
    fn information_capabilities_follow_level_order() {
        assert!(!KnowledgeLevel::Detected.reveals_identity());
        assert!(KnowledgeLevel::Probed.reveals_identity());
        assert!(!KnowledgeLevel::Probed.reveals_exact_details());
        assert!(KnowledgeLevel::Analyzed.reveals_exact_details());
    }
}
