// MVP-012: deterministic production, storage and energy throttling.
use galactic_domain::{ColonyId, ResourceStock};

use crate::{
    BuildingLevels, ColonyState, PlanetResourceProfile, STRATEGIC_TICKS_PER_SECOND,
    StrategicDuration,
};

/// Fixed-point scale used for sub-unit production.
pub const PRODUCTION_SCALE: u64 = 1_000;

/// Temporary centralized rules. MVP-013 will replace these constants with
/// data from the building catalog without changing the simulation loop.
pub const BASE_METAL_MILLI_PER_TICK: u64 = 250;
pub const BASE_CRYSTAL_MILLI_PER_TICK: u64 = 125;
pub const BASE_FUEL_MILLI_PER_TICK: u64 = 75;

pub const BASE_STORAGE_CAPACITY: ResourceStock = ResourceStock::new(1_000, 800, 600);
pub const WAREHOUSE_CAPACITY_PER_LEVEL: ResourceStock = ResourceStock::new(4_000, 3_200, 2_400);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProductionRemainder {
    metal_milli: u16,
    crystal_milli: u16,
    fuel_milli: u16,
}

impl ProductionRemainder {
    pub const ZERO: Self = Self::new_unchecked(0, 0, 0);

    const fn new_unchecked(metal_milli: u16, crystal_milli: u16, fuel_milli: u16) -> Self {
        Self {
            metal_milli,
            crystal_milli,
            fuel_milli,
        }
    }

    pub fn from_parts(
        metal_milli: u16,
        crystal_milli: u16,
        fuel_milli: u16,
    ) -> Result<Self, ProductionRemainderError> {
        let scale = PRODUCTION_SCALE as u16;
        if metal_milli >= scale {
            return Err(ProductionRemainderError::OutOfRange {
                resource: ProductionResource::Metal,
                value: metal_milli,
            });
        }
        if crystal_milli >= scale {
            return Err(ProductionRemainderError::OutOfRange {
                resource: ProductionResource::Crystal,
                value: crystal_milli,
            });
        }
        if fuel_milli >= scale {
            return Err(ProductionRemainderError::OutOfRange {
                resource: ProductionResource::Fuel,
                value: fuel_milli,
            });
        }

        Ok(Self::new_unchecked(metal_milli, crystal_milli, fuel_milli))
    }

    pub const fn metal_milli(self) -> u16 {
        self.metal_milli
    }

    pub const fn crystal_milli(self) -> u16 {
        self.crystal_milli
    }

    pub const fn fuel_milli(self) -> u16 {
        self.fuel_milli
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionResource {
    Metal,
    Crystal,
    Fuel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionRemainderError {
    OutOfRange {
        resource: ProductionResource,
        value: u16,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProductionRate {
    pub metal_milli_per_tick: u64,
    pub crystal_milli_per_tick: u64,
    pub fuel_milli_per_tick: u64,
}

impl ProductionRate {
    pub const ZERO: Self = Self {
        metal_milli_per_tick: 0,
        crystal_milli_per_tick: 0,
        fuel_milli_per_tick: 0,
    };

    pub fn for_colony(buildings: BuildingLevels, profile: PlanetResourceProfile) -> Self {
        Self {
            metal_milli_per_tick: modified_rate(
                BASE_METAL_MILLI_PER_TICK,
                buildings.metal_mine,
                profile.metal,
            ),
            crystal_milli_per_tick: modified_rate(
                BASE_CRYSTAL_MILLI_PER_TICK,
                buildings.crystal_extractor,
                profile.crystal,
            ),
            fuel_milli_per_tick: modified_rate(
                BASE_FUEL_MILLI_PER_TICK,
                buildings.fuel_refinery,
                profile.fuel,
            ),
        }
    }

    pub fn scaled_by_permille(self, efficiency_per_mille: u16) -> Self {
        Self {
            metal_milli_per_tick: scale_rate(self.metal_milli_per_tick, efficiency_per_mille),
            crystal_milli_per_tick: scale_rate(self.crystal_milli_per_tick, efficiency_per_mille),
            fuel_milli_per_tick: scale_rate(self.fuel_milli_per_tick, efficiency_per_mille),
        }
    }

    pub fn metal_per_second(self) -> f64 {
        per_second(self.metal_milli_per_tick)
    }

    pub fn crystal_per_second(self) -> f64 {
        per_second(self.crystal_milli_per_tick)
    }

    pub fn fuel_per_second(self) -> f64 {
        per_second(self.fuel_milli_per_tick)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaturationTime {
    Full,
    Never,
    In(StrategicDuration),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaturationEstimate {
    pub metal: SaturationTime,
    pub crystal: SaturationTime,
    pub fuel: SaturationTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColonyProductionSnapshot {
    pub capacity: ResourceStock,
    pub nominal_rate: ProductionRate,
    pub effective_rate: ProductionRate,
    pub nominal_energy_production: u64,
    pub effective_energy_production: u64,
    pub energy_efficiency_per_mille: u16,
    pub saturation: SaturationEstimate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColonyProductionReport {
    pub colony_id: ColonyId,
    pub ticks: StrategicDuration,
    pub produced: ResourceStock,
    pub blocked_by_storage: ResourceStock,
    pub energy_efficiency_per_mille: u16,
}

pub fn storage_capacity(buildings: BuildingLevels) -> ResourceStock {
    let warehouse_level = u64::from(buildings.warehouse);
    ResourceStock::new(
        BASE_STORAGE_CAPACITY.metal.saturating_add(
            WAREHOUSE_CAPACITY_PER_LEVEL
                .metal
                .saturating_mul(warehouse_level),
        ),
        BASE_STORAGE_CAPACITY.crystal.saturating_add(
            WAREHOUSE_CAPACITY_PER_LEVEL
                .crystal
                .saturating_mul(warehouse_level),
        ),
        BASE_STORAGE_CAPACITY.fuel.saturating_add(
            WAREHOUSE_CAPACITY_PER_LEVEL
                .fuel
                .saturating_mul(warehouse_level),
        ),
    )
}

pub fn colony_production_snapshot(colony: &ColonyState) -> ColonyProductionSnapshot {
    let capacity = storage_capacity(colony.buildings);
    let nominal_rate = ProductionRate::for_colony(colony.buildings, colony.resource_profile);
    let nominal_energy_production = colony.energy.production();
    let effective_energy_production =
        apply_modifier(nominal_energy_production, colony.resource_profile.energy);
    let energy_efficiency_per_mille =
        energy_efficiency_per_mille(effective_energy_production, colony.energy.consumption());
    let effective_rate = nominal_rate.scaled_by_permille(energy_efficiency_per_mille);
    let stock = colony.resources.stock();
    let remainder = colony.production_remainder;

    ColonyProductionSnapshot {
        capacity,
        nominal_rate,
        effective_rate,
        nominal_energy_production,
        effective_energy_production,
        energy_efficiency_per_mille,
        saturation: SaturationEstimate {
            metal: saturation_time(
                stock.metal,
                capacity.metal,
                effective_rate.metal_milli_per_tick,
                remainder.metal_milli(),
            ),
            crystal: saturation_time(
                stock.crystal,
                capacity.crystal,
                effective_rate.crystal_milli_per_tick,
                remainder.crystal_milli(),
            ),
            fuel: saturation_time(
                stock.fuel,
                capacity.fuel,
                effective_rate.fuel_milli_per_tick,
                remainder.fuel_milli(),
            ),
        },
    }
}

pub fn apply_colony_production(
    colony: &mut ColonyState,
    ticks: StrategicDuration,
) -> ColonyProductionReport {
    let snapshot = colony_production_snapshot(colony);
    let tick_count = ticks.ticks();

    let (metal, next_metal) = generated_units(
        snapshot.effective_rate.metal_milli_per_tick,
        tick_count,
        colony.production_remainder.metal_milli(),
    );
    let (crystal, next_crystal) = generated_units(
        snapshot.effective_rate.crystal_milli_per_tick,
        tick_count,
        colony.production_remainder.crystal_milli(),
    );
    let (fuel, next_fuel) = generated_units(
        snapshot.effective_rate.fuel_milli_per_tick,
        tick_count,
        colony.production_remainder.fuel_milli(),
    );

    let requested = ResourceStock::new(metal, crystal, fuel);
    let produced = colony.resources.credit_capped(requested, snapshot.capacity);
    let blocked_by_storage = requested.saturating_sub(produced);

    colony.production_remainder = ProductionRemainder::new_unchecked(
        if produced.metal < requested.metal {
            0
        } else {
            next_metal
        },
        if produced.crystal < requested.crystal {
            0
        } else {
            next_crystal
        },
        if produced.fuel < requested.fuel {
            0
        } else {
            next_fuel
        },
    );

    ColonyProductionReport {
        colony_id: colony.id,
        ticks,
        produced,
        blocked_by_storage,
        energy_efficiency_per_mille: snapshot.energy_efficiency_per_mille,
    }
}

fn modified_rate(base_milli_per_tick: u64, level: u8, modifier_percent: u16) -> u64 {
    let value = u128::from(base_milli_per_tick)
        .saturating_mul(u128::from(level))
        .saturating_mul(u128::from(modifier_percent))
        / 100;
    value.min(u128::from(u64::MAX)) as u64
}

fn scale_rate(rate: u64, efficiency_per_mille: u16) -> u64 {
    let value = u128::from(rate).saturating_mul(u128::from(efficiency_per_mille))
        / u128::from(PRODUCTION_SCALE);
    value.min(u128::from(u64::MAX)) as u64
}

fn apply_modifier(value: u64, modifier_percent: u16) -> u64 {
    let modified = u128::from(value).saturating_mul(u128::from(modifier_percent)) / 100;
    modified.min(u128::from(u64::MAX)) as u64
}

fn energy_efficiency_per_mille(effective_production: u64, consumption: u64) -> u16 {
    if consumption == 0 || effective_production >= consumption {
        return PRODUCTION_SCALE as u16;
    }

    let value = u128::from(effective_production).saturating_mul(u128::from(PRODUCTION_SCALE))
        / u128::from(consumption);
    value.min(u128::from(PRODUCTION_SCALE)) as u16
}

fn generated_units(rate_milli_per_tick: u64, ticks: u64, previous_remainder: u16) -> (u64, u16) {
    let total = u128::from(rate_milli_per_tick)
        .saturating_mul(u128::from(ticks))
        .saturating_add(u128::from(previous_remainder));
    let units = total / u128::from(PRODUCTION_SCALE);
    let remainder = (total % u128::from(PRODUCTION_SCALE)) as u16;

    (units.min(u128::from(u64::MAX)) as u64, remainder)
}

fn saturation_time(
    stock: u64,
    capacity: u64,
    rate_milli_per_tick: u64,
    remainder_milli: u16,
) -> SaturationTime {
    if stock >= capacity {
        return SaturationTime::Full;
    }
    if rate_milli_per_tick == 0 {
        return SaturationTime::Never;
    }

    let missing_units = capacity - stock;
    let required_milli = u128::from(missing_units)
        .saturating_mul(u128::from(PRODUCTION_SCALE))
        .saturating_sub(u128::from(remainder_milli));
    let rate = u128::from(rate_milli_per_tick);
    let ticks = required_milli.saturating_add(rate - 1) / rate;

    SaturationTime::In(StrategicDuration::from_ticks(
        ticks.min(u128::from(u64::MAX)) as u64,
    ))
}

fn per_second(rate_milli_per_tick: u64) -> f64 {
    rate_milli_per_tick as f64 * f64::from(STRATEGIC_TICKS_PER_SECOND) / PRODUCTION_SCALE as f64
}

#[cfg(test)]
mod tests {
    use galactic_domain::{EnergyGrid, ResourceLedger, UniverseConfig};

    use crate::Simulation;

    use super::*;

    fn home_colony() -> ColonyState {
        Simulation::new(UniverseConfig::mvp())
            .state()
            .player_home_colony()
            .expect("home colony exists")
            .clone()
    }

    #[test]
    fn starting_colony_has_expected_rates_and_capacity() {
        let colony = home_colony();
        let snapshot = colony_production_snapshot(&colony);

        assert_eq!(snapshot.capacity, ResourceStock::new(5_000, 4_000, 3_000));
        assert_eq!(
            snapshot.nominal_rate,
            ProductionRate {
                metal_milli_per_tick: 250,
                crystal_milli_per_tick: 125,
                fuel_milli_per_tick: 75,
            }
        );
        assert_eq!(snapshot.energy_efficiency_per_mille, 1_000);
    }

    #[test]
    fn tick_batches_produce_identical_state() {
        let mut batched = home_colony();
        let mut incremental = batched.clone();

        apply_colony_production(&mut batched, StrategicDuration::from_ticks(100));
        for _ in 0..10 {
            apply_colony_production(&mut incremental, StrategicDuration::from_ticks(10));
        }

        assert_eq!(batched, incremental);
        assert_eq!(batched.resources.stock(), ResourceStock::new(625, 312, 227));
        assert_eq!(
            batched.production_remainder,
            ProductionRemainder::from_parts(0, 500, 500,).expect("valid remainder")
        );
    }

    #[test]
    fn energy_deficit_throttles_all_extractors() {
        let mut colony = home_colony();
        colony.energy = EnergyGrid::new(15, 30);

        let report = apply_colony_production(&mut colony, StrategicDuration::from_ticks(1_000));

        assert_eq!(report.energy_efficiency_per_mille, 500);
        assert_eq!(report.produced, ResourceStock::new(125, 62, 37));
    }

    #[test]
    fn full_storage_discards_blocked_output() {
        let mut colony = home_colony();
        let capacity = storage_capacity(colony.buildings);
        colony.resources = ResourceLedger::new(ResourceStock::new(
            capacity.metal - 1,
            capacity.crystal - 1,
            capacity.fuel - 1,
        ));

        let report = apply_colony_production(&mut colony, StrategicDuration::from_ticks(1_000));

        assert_eq!(colony.resources.stock(), capacity);
        assert!(!report.blocked_by_storage.is_zero());
        assert_eq!(colony.production_remainder, ProductionRemainder::ZERO);
    }

    #[test]
    fn planet_profile_applies_simple_modifiers() {
        let mut colony = home_colony();
        colony.resource_profile = PlanetResourceProfile::new(150, 80, 50, 50);
        let snapshot = colony_production_snapshot(&colony);

        assert_eq!(snapshot.nominal_rate.metal_milli_per_tick, 375);
        assert_eq!(snapshot.nominal_rate.crystal_milli_per_tick, 100);
        assert_eq!(snapshot.nominal_rate.fuel_milli_per_tick, 37);
        assert_eq!(snapshot.effective_energy_production, 40);
        assert_eq!(snapshot.energy_efficiency_per_mille, 1_000);
    }

    #[test]
    fn zero_energy_blocks_production_without_losing_remainder() {
        let mut colony = home_colony();
        colony.production_remainder =
            ProductionRemainder::from_parts(900, 800, 700).expect("valid remainder");
        colony.energy = EnergyGrid::new(0, 30);
        let before = colony.production_remainder;

        let report = apply_colony_production(&mut colony, StrategicDuration::from_ticks(100));

        assert!(report.produced.is_zero());
        assert_eq!(report.energy_efficiency_per_mille, 0);
        assert_eq!(colony.production_remainder, before);
    }
}
