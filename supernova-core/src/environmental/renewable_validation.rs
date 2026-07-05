// Renewable Energy Validation System for Supernova
// Implements green mining incentives and renewable energy certificate verification
// Leveraging Nova Energy expertise for world-class renewable energy integration

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::{Arc, RwLock};

use crate::environmental::{
    oracle::{EnvironmentalOracle, OracleError},
    types::{EnergySourceType, Region},
    verification::{CarbonOffset, RenewableCertificate, VerificationProvider, VerificationService},
};

/// Renewable energy validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenewableValidationResult {
    /// Validated renewable energy percentage
    pub renewable_percentage: f64,

    /// Green mining score (0-100)
    pub green_mining_score: f64,

    /// Renewable energy certificates validated
    pub validated_certificates: Vec<ValidatedREC>,

    /// Green mining incentive earned
    pub green_incentive_nova: f64,

    /// Carbon negativity achieved
    pub is_carbon_negative: bool,

    /// Validation timestamp
    pub timestamp: DateTime<Utc>,

    /// Environmental impact assessment
    pub impact_assessment: EnvironmentalImpact,
}

/// Validated renewable energy certificate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatedREC {
    pub certificate_id: String,
    pub energy_amount_mwh: f64,
    pub energy_type: EnergySourceType,
    pub generation_period: (DateTime<Utc>, DateTime<Utc>),
    pub issuer: String,
    pub validation_status: ValidationStatus,
    pub blockchain_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ValidationStatus {
    Valid,
    Expired,
    Revoked,
    Pending,
    Invalid,
}

/// Environmental impact assessment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalImpact {
    /// CO2 avoided (tonnes)
    pub co2_avoided: f64,

    /// Equivalent trees planted
    pub trees_equivalent: u64,

    /// Cars removed from road equivalent
    pub cars_removed_equivalent: u64,

    /// Environmental benefit score
    pub benefit_score: f64,
}

/// Green mining incentive structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GreenMiningIncentive {
    /// Base reward multiplier for green mining
    pub base_multiplier: f64,

    /// Bonus for 100% renewable
    pub full_renewable_bonus: f64,

    /// Carbon negative bonus
    pub carbon_negative_bonus: f64,

    /// Regional incentives
    pub regional_multipliers: HashMap<Region, f64>,

    /// Time-based incentives (peak renewable hours)
    pub time_based_incentives: TimeBasedIncentives,
}

/// Time-based incentive structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBasedIncentives {
    /// Solar peak hours bonus
    pub solar_peak_bonus: f64,

    /// Wind peak hours bonus
    pub wind_peak_bonus: f64,

    /// Off-peak penalty
    pub off_peak_penalty: f64,
}

/// Renewable Energy Validator
pub struct RenewableEnergyValidator {
    /// Verification service
    verification_service: Arc<VerificationService>,

    /// Environmental oracle
    oracle: Arc<EnvironmentalOracle>,

    /// Validated certificates cache
    validated_certificates: Arc<RwLock<HashMap<String, ValidatedREC>>>,

    /// Ledger of consumed REC certificate ids -> claiming miner id.
    ///
    /// Prevents a single renewable-energy certificate from being credited to
    /// more than one miner (cross-miner double-claim). Mirrors
    /// `MinerReportingManager::consumed_rec_certificates`.
    consumed_rec_certificates: Arc<RwLock<HashMap<String, String>>>,

    /// Green mining incentives
    incentive_structure: Arc<RwLock<GreenMiningIncentive>>,

    /// Renewable energy registries
    registries: Arc<RwLock<HashMap<String, RenewableRegistry>>>,

    /// Real-time grid data
    grid_data: Arc<RwLock<GridDataCache>>,

    /// Performance metrics
    metrics: Arc<RwLock<ValidationMetrics>>,
}

/// Renewable energy registry connection
#[derive(Debug, Clone)]
struct RenewableRegistry {
    pub registry_id: String,
    pub name: String,
    pub api_endpoint: String,
    pub supported_types: HashSet<EnergySourceType>,
    pub regions: HashSet<Region>,
}

/// Grid data cache for real-time validation
#[derive(Debug, Clone)]
struct GridDataCache {
    pub grid_mix: HashMap<Region, GridMixData>,
    pub last_update: DateTime<Utc>,
    pub update_frequency: Duration,
}

/// Grid mix data
#[derive(Debug, Clone)]
struct GridMixData {
    pub renewable_percentage: f64,
    pub carbon_intensity: f64,
    pub energy_sources: HashMap<EnergySourceType, f64>,
    pub timestamp: DateTime<Utc>,
}

/// Validation metrics
#[derive(Debug, Clone, Default)]
struct ValidationMetrics {
    pub total_validations: u64,
    pub successful_validations: u64,
    pub failed_validations: u64,
    pub total_mwh_validated: f64,
    pub total_incentives_paid: f64,
}

impl RenewableEnergyValidator {
    /// Create a new renewable energy validator
    pub fn new(
        verification_service: Arc<VerificationService>,
        oracle: Arc<EnvironmentalOracle>,
    ) -> Self {
        let default_incentives = GreenMiningIncentive {
            base_multiplier: 1.2,       // 20% bonus for renewable energy
            full_renewable_bonus: 0.5,  // 50% bonus for 100% renewable
            carbon_negative_bonus: 0.3, // 30% bonus for carbon negative
            regional_multipliers: Self::initialize_regional_multipliers(),
            time_based_incentives: TimeBasedIncentives {
                solar_peak_bonus: 0.15,
                wind_peak_bonus: 0.10,
                off_peak_penalty: -0.05,
            },
        };

        Self {
            verification_service,
            oracle,
            validated_certificates: Arc::new(RwLock::new(HashMap::new())),
            consumed_rec_certificates: Arc::new(RwLock::new(HashMap::new())),
            incentive_structure: Arc::new(RwLock::new(default_incentives)),
            registries: Arc::new(RwLock::new(HashMap::new())),
            grid_data: Arc::new(RwLock::new(GridDataCache {
                grid_mix: HashMap::new(),
                last_update: Utc::now(),
                update_frequency: Duration::hours(1),
            })),
            metrics: Arc::new(RwLock::new(ValidationMetrics::default())),
        }
    }

    /// Validate renewable energy certificates
    pub async fn validate_renewable_energy_certificates(
        &self,
        miner_id: &str,
        certificates: Vec<RenewableCertificate>,
        energy_consumption_mwh: f64,
    ) -> Result<RenewableValidationResult, OracleError> {

        let mut validated_certificates = Vec::new();
        let mut total_renewable_mwh = 0.0;
        let mut energy_by_type: HashMap<EnergySourceType, f64> = HashMap::new();
        // Certificate ids already credited within THIS call, to reject a
        // duplicate id submitted twice in a single request.
        let mut claimed_in_call: HashSet<String> = HashSet::new();

        // Validate each certificate
        for cert in certificates {
            match self.validate_single_certificate(&cert).await {
                Ok(validated) => {
                    if validated.validation_status == ValidationStatus::Valid {
                        // Reject a certificate already consumed by a *different*
                        // miner (cross-miner double-claim) or already credited
                        // earlier in this same call (intra-call duplicate). On
                        // success the claim is recorded against `miner_id`.
                        if !self.try_claim_certificate(
                            miner_id,
                            &validated.certificate_id,
                            &mut claimed_in_call,
                        ) {
                            continue;
                        }

                        // Persist to the validated-certificate cache so dashboard
                        // totals reflect real validated energy.
                        {
                            let mut cache = self
                                .validated_certificates
                                .write()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            cache.insert(validated.certificate_id.clone(), validated.clone());
                        }

                        total_renewable_mwh += validated.energy_amount_mwh;
                        *energy_by_type.entry(validated.energy_type).or_insert(0.0) +=
                            validated.energy_amount_mwh;
                        validated_certificates.push(validated);
                    }
                }
                Err(_e) => {
                }
            }
        }

        // Calculate renewable percentage
        let renewable_percentage = if energy_consumption_mwh > 0.0 {
            (total_renewable_mwh / energy_consumption_mwh * 100.0).min(100.0)
        } else {
            0.0
        };

        // Calculate green mining score
        let green_mining_score = self.calculate_green_mining_score(
            renewable_percentage,
            &energy_by_type,
            energy_consumption_mwh,
        );

        // Calculate incentives
        let green_incentive_nova = self.calculate_green_incentives(
            renewable_percentage,
            green_mining_score,
            energy_consumption_mwh,
        );

        // Assess environmental impact
        let impact_assessment =
            self.assess_environmental_impact(total_renewable_mwh, energy_consumption_mwh);

        // Check if carbon negative
        let is_carbon_negative = renewable_percentage >= 100.0;

        let result = RenewableValidationResult {
            renewable_percentage,
            green_mining_score,
            validated_certificates,
            green_incentive_nova,
            is_carbon_negative,
            timestamp: Utc::now(),
            impact_assessment,
        };

        // Update metrics
        self.update_metrics(&result);


        Ok(result)
    }

    /// Attempt to claim a validated certificate for `miner_id`.
    ///
    /// Returns `false` (no credit) when the certificate id was already consumed
    /// by a *different* miner, or has already been credited earlier in the same
    /// call (`claimed_in_call`). On a `true` result the id is recorded in both
    /// `claimed_in_call` and the persistent `consumed_rec_certificates` ledger
    /// so it can never be double-claimed across miners.
    fn try_claim_certificate(
        &self,
        miner_id: &str,
        certificate_id: &str,
        claimed_in_call: &mut HashSet<String>,
    ) -> bool {
        let claimed_by_other = {
            let consumed = self
                .consumed_rec_certificates
                .read()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            matches!(consumed.get(certificate_id), Some(owner) if owner != miner_id)
        };
        if claimed_by_other || !claimed_in_call.insert(certificate_id.to_string()) {
            return false;
        }

        let mut consumed = self
            .consumed_rec_certificates
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        consumed.insert(certificate_id.to_string(), miner_id.to_string());
        true
    }

    /// Implement green mining incentives
    pub fn implement_green_mining_incentives(
        &self,
        new_incentives: GreenMiningIncentive,
    ) -> Result<(), OracleError> {
        let mut incentives = self
            .incentive_structure
            .write()
            .map_err(|_| OracleError::LockPoisoned)?;
        *incentives = new_incentives;

        Ok(())
    }

    /// Verify carbon-negative operations.
    ///
    /// Renewable energy MUST be substantiated by validated certificates rather
    /// than trusted from a caller-supplied MWh figure: only certificates whose
    /// verification returns `Valid` count toward the miner's renewable total.
    ///
    /// Emissions are charged solely on the non-renewable portion of consumption
    /// (renewable MWh is already zero-rated there). Renewables are therefore
    /// **not** subtracted a second time as "emissions avoided" — doing so
    /// double-counted them and let any miner claiming renewables above half of
    /// consumption be certified carbon-negative with no verified offsets at all.
    /// Carbon-negativity now requires verified offsets to strictly exceed the
    /// residual non-renewable emissions.
    pub async fn verify_carbon_negative_operations(
        &self,
        miner_id: &str,
        renewable_certificates: Vec<RenewableCertificate>,
        total_consumption_mwh: f64,
        carbon_offsets: Vec<CarbonOffset>,
    ) -> Result<bool, OracleError> {
        // Substantiate renewable energy through certificate validation; only
        // certificates that pass verification contribute renewable MWh.
        let validation = self
            .validate_renewable_energy_certificates(
                miner_id,
                renewable_certificates,
                total_consumption_mwh,
            )
            .await?;
        let verified_renewable_mwh: f64 = validation
            .validated_certificates
            .iter()
            .map(|c| c.energy_amount_mwh)
            .sum();

        // Validate and sum carbon offsets. An offset that fails verification is
        // simply not counted, so an unverified offset can never help a claim.
        let mut total_offset_tonnes = 0.0;
        for offset in carbon_offsets {
            if self
                .verification_service
                .verify_offset(&offset)
                .await
                .is_ok()
            {
                total_offset_tonnes += offset.amount_tonnes;
            }
        }

        // Charge emissions only on the non-renewable portion; renewables are not
        // credited a second time.
        let non_renewable_mwh = (total_consumption_mwh - verified_renewable_mwh).max(0.0);
        let estimated_emissions = non_renewable_mwh * 0.5; // Average emission factor
        let net_emissions = estimated_emissions - total_offset_tonnes;

        Ok(net_emissions < 0.0)
    }

    /// Create environmental impact dashboard data. Read-only; recovers
    /// from lock poisoning so the dashboard endpoint never panics on a
    /// prior writer's panic.
    pub fn create_environmental_impact_dashboard(&self) -> EnvironmentalDashboard {
        let metrics = self
            .metrics
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let certificates = self
            .validated_certificates
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // Calculate totals
        let total_renewable_mwh: f64 = certificates
            .values()
            .filter(|c| c.validation_status == ValidationStatus::Valid)
            .map(|c| c.energy_amount_mwh)
            .sum();

        let co2_avoided = self.calculate_emissions_avoided(total_renewable_mwh);

        EnvironmentalDashboard {
            total_validations: metrics.total_validations,
            successful_validations: metrics.successful_validations,
            total_renewable_mwh,
            total_co2_avoided: co2_avoided,
            total_incentives_paid: metrics.total_incentives_paid,
            // Not derivable from held state: a renewable percentage needs a
            // total-consumption denominator that is not tracked here, and no
            // per-miner carbon-negative registry is held. Report as explicitly
            // unavailable rather than emitting fabricated placeholder constants.
            average_renewable_percentage: None,
            carbon_negative_miners: None,
            timestamp: Utc::now(),
        }
    }

    // Helper methods

    async fn validate_single_certificate(
        &self,
        cert: &RenewableCertificate,
    ) -> Result<ValidatedREC, OracleError> {
        // Check if certificate is expired
        let now = Utc::now();
        let is_expired = cert.generation_end < now;

        if is_expired {
            return Ok(ValidatedREC {
                certificate_id: cert.certificate_id.clone(),
                energy_amount_mwh: cert.amount_kwh / 1000.0, // Convert kWh to MWh
                energy_type: EnergySourceType::from_str(&cert.certificate_type)
                    .unwrap_or(EnergySourceType::Other),
                generation_period: (cert.generation_start, cert.generation_end),
                issuer: cert.issuer.clone(),
                validation_status: ValidationStatus::Expired,
                blockchain_hash: self.calculate_certificate_hash(cert),
            });
        }

        // Verify with verification service
        match self.verification_service.verify_certificate(cert).await {
            Ok(status) => {
                let validation_status = match status {
                    crate::environmental::emissions::VerificationStatus::Verified => {
                        ValidationStatus::Valid
                    }
                    crate::environmental::emissions::VerificationStatus::Failed => {
                        ValidationStatus::Invalid
                    }
                    crate::environmental::emissions::VerificationStatus::Pending => {
                        ValidationStatus::Pending
                    }
                    crate::environmental::emissions::VerificationStatus::Expired => {
                        ValidationStatus::Expired
                    }
                    crate::environmental::emissions::VerificationStatus::None => {
                        ValidationStatus::Invalid
                    }
                };

                Ok(ValidatedREC {
                    certificate_id: cert.certificate_id.clone(),
                    energy_amount_mwh: cert.amount_kwh / 1000.0, // Convert kWh to MWh
                    energy_type: EnergySourceType::from_str(&cert.certificate_type)
                        .unwrap_or(EnergySourceType::Other),
                    generation_period: (cert.generation_start, cert.generation_end),
                    issuer: cert.issuer.clone(),
                    validation_status,
                    blockchain_hash: self.calculate_certificate_hash(cert),
                })
            }
            Err(e) => Err(OracleError::VerificationFailed(e.to_string())),
        }
    }

    fn calculate_green_mining_score(
        &self,
        renewable_percentage: f64,
        energy_by_type: &HashMap<EnergySourceType, f64>,
        total_consumption: f64,
    ) -> f64 {
        let mut score = renewable_percentage; // Base score is renewable percentage

        // Bonus for diverse renewable sources
        let diversity_bonus = (energy_by_type.len() as f64) * 2.0;
        score += diversity_bonus.min(10.0);

        // Apply efficiency factor
        if total_consumption > 0.0 {
            let efficiency_factor = 100.0 / total_consumption.sqrt();
            score += efficiency_factor.min(10.0);
        }

        score.min(100.0)
    }

    fn calculate_green_incentives(
        &self,
        renewable_percentage: f64,
        green_mining_score: f64,
        energy_consumption_mwh: f64,
    ) -> f64 {
        // Read-only; recover from lock poisoning so incentive
        // calculation never panics during a reward computation.
        let incentives = self
            .incentive_structure
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let mut total_incentive = 100.0; // Base reward

        // Apply renewable percentage multiplier
        total_incentive *= 1.0 + (renewable_percentage / 100.0 * incentives.base_multiplier);

        // Apply full renewable bonus
        if renewable_percentage >= 100.0 {
            total_incentive *= 1.0 + incentives.full_renewable_bonus;
        }

        // Apply green mining score factor
        total_incentive *= green_mining_score / 100.0;

        // Scale by energy consumption
        total_incentive *= energy_consumption_mwh.sqrt();

        total_incentive
    }

    fn assess_environmental_impact(
        &self,
        renewable_mwh: f64,
        total_mwh: f64,
    ) -> EnvironmentalImpact {
        // Calculate CO2 avoided (assumes 0.5 tonnes CO2/MWh for grid average)
        let co2_avoided = renewable_mwh * 0.5;

        // Environmental equivalents
        let trees_equivalent = (co2_avoided * 50.0) as u64; // ~50 trees per tonne CO2
        let cars_removed_equivalent = (co2_avoided / 4.6) as u64; // ~4.6 tonnes CO2 per car/year

        // Benefit score (0-100)
        let renewable_ratio = renewable_mwh / total_mwh.max(1.0);
        let benefit_score = (renewable_ratio * 100.0).min(100.0);

        EnvironmentalImpact {
            co2_avoided,
            trees_equivalent,
            cars_removed_equivalent,
            benefit_score,
        }
    }

    fn calculate_emissions_avoided(&self, renewable_mwh: f64) -> f64 {
        // Average grid emission factor: 0.5 tonnes CO2/MWh
        renewable_mwh * 0.5
    }

    fn calculate_certificate_hash(&self, cert: &RenewableCertificate) -> String {
        let mut hasher = Sha256::new();
        hasher.update(cert.certificate_id.as_bytes());
        hasher.update(cert.amount_kwh.to_string().as_bytes());
        format!("{:x}", hasher.finalize())
    }

    fn update_metrics(&self, result: &RenewableValidationResult) {
        // Best-effort metrics update; recover from lock poisoning so a
        // prior panic in the metrics path doesn't cascade.
        let mut metrics = self
            .metrics
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        metrics.total_validations += 1;
        metrics.successful_validations += 1;
        metrics.total_mwh_validated += result
            .validated_certificates
            .iter()
            .map(|c| c.energy_amount_mwh)
            .sum::<f64>();
        metrics.total_incentives_paid += result.green_incentive_nova;
    }

    fn initialize_regional_multipliers() -> HashMap<Region, f64> {
        let mut multipliers = HashMap::new();

        // Regions with high renewable potential get higher multipliers
        multipliers.insert(Region::NorthAmerica, 1.1);
        multipliers.insert(Region::Europe, 1.2); // Strong renewable policies
        multipliers.insert(Region::AsiaPacific, 1.0);
        // Note: Oceania (Australia, NZ) is included in AsiaPacific region
        multipliers.insert(Region::Africa, 1.25); // Encourage renewable development
        multipliers.insert(Region::SouthAmerica, 1.1);

        multipliers
    }
}

/// Environmental dashboard data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentalDashboard {
    pub total_validations: u64,
    pub successful_validations: u64,
    pub total_renewable_mwh: f64,
    pub total_co2_avoided: f64,
    pub total_incentives_paid: f64,
    /// Average renewable percentage across the network.
    ///
    /// `None` when the validator does not currently hold the data required to
    /// derive this figure honestly (a renewable percentage requires a total
    /// energy-consumption denominator, which is not tracked here). Emitting a
    /// fabricated constant would misrepresent the network's environmental
    /// status, so the field is reported as explicitly unavailable instead.
    pub average_renewable_percentage: Option<f64>,
    /// Number of miners operating carbon-negative.
    ///
    /// `None` when no per-miner carbon-negative registry is held by this
    /// validator, so the count cannot be derived from verified data.
    pub carbon_negative_miners: Option<u64>,
    pub timestamp: DateTime<Utc>,
}

/// Public API functions

pub async fn validate_renewable_energy_certificates(
    validator: &RenewableEnergyValidator,
    miner_id: &str,
    certificates: Vec<RenewableCertificate>,
    energy_consumption_mwh: f64,
) -> Result<RenewableValidationResult, OracleError> {
    validator
        .validate_renewable_energy_certificates(miner_id, certificates, energy_consumption_mwh)
        .await
}

pub fn implement_green_mining_incentives(
    validator: &RenewableEnergyValidator,
    incentives: GreenMiningIncentive,
) -> Result<(), OracleError> {
    validator.implement_green_mining_incentives(incentives)
}

pub async fn verify_carbon_negative_operations(
    validator: &RenewableEnergyValidator,
    miner_id: &str,
    renewable_certificates: Vec<RenewableCertificate>,
    total_consumption_mwh: f64,
    carbon_offsets: Vec<CarbonOffset>,
) -> Result<bool, OracleError> {
    validator
        .verify_carbon_negative_operations(
            miner_id,
            renewable_certificates,
            total_consumption_mwh,
            carbon_offsets,
        )
        .await
}

pub fn create_environmental_impact_dashboard(
    validator: &RenewableEnergyValidator,
) -> EnvironmentalDashboard {
    validator.create_environmental_impact_dashboard()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environmental::verification::VerificationService;

    fn make_validator() -> RenewableEnergyValidator {
        let verification_service = Arc::new(VerificationService::default());
        let oracle = Arc::new(EnvironmentalOracle::new(0));
        RenewableEnergyValidator::new(verification_service, oracle)
    }

    /// The dashboard must never emit fabricated headline figures. Fields that
    /// cannot be derived from held state are reported as explicitly
    /// unavailable (`None`) rather than plausible-looking constants.
    #[test]
    fn dashboard_reports_unavailable_instead_of_fabricated_constants() {
        let validator = make_validator();
        let dashboard = validator.create_environmental_impact_dashboard();

        // These two figures have no honest source in the validator's held
        // state and must not be fabricated.
        assert_eq!(dashboard.average_renewable_percentage, None);
        assert_eq!(dashboard.carbon_negative_miners, None);

        // Fields that ARE derived from held state remain populated (and are
        // zero on a fresh validator with no validated certificates).
        assert_eq!(dashboard.total_validations, 0);
        assert_eq!(dashboard.total_renewable_mwh, 0.0);
    }

    /// `implement_green_mining_incentives` must persist the supplied structure
    /// so subsequent reward calculations use the updated parameters.
    #[test]
    fn implement_green_mining_incentives_stores_new_structure() {
        let validator = make_validator();

        // A distinct structure so we can detect it was actually stored.
        let new_incentives = GreenMiningIncentive {
            base_multiplier: 2.0,
            full_renewable_bonus: 1.0,
            carbon_negative_bonus: 0.5,
            regional_multipliers: HashMap::new(),
            time_based_incentives: TimeBasedIncentives {
                solar_peak_bonus: 0.0,
                wind_peak_bonus: 0.0,
                off_peak_penalty: 0.0,
            },
        };

        validator
            .implement_green_mining_incentives(new_incentives)
            .expect("storing incentives should succeed");

        let stored = validator
            .incentive_structure
            .read()
            .expect("incentive lock should not be poisoned");
        assert_eq!(stored.base_multiplier, 2.0);
        assert_eq!(stored.full_renewable_bonus, 1.0);
        assert_eq!(stored.carbon_negative_bonus, 0.5);
    }

    /// A miner with no verified renewable energy (e.g. all RECs invalid or
    /// unverified, yielding 0% renewable) earns only the base reward: no
    /// renewable multiplier and no full-renewable bonus tier.
    #[test]
    fn unverified_rec_yields_base_reward_only() {
        let validator = make_validator();

        // score = 100, consumption = 100 (sqrt = 10) makes the base reward an
        // exact, deterministic 100 * 1.0 * 1.0 * 10 = 1000.0.
        let none = validator.calculate_green_incentives(0.0, 100.0, 100.0);
        assert!(
            (none - 1000.0).abs() < 1e-9,
            "0% renewable must earn only the base reward, got {none}"
        );
    }

    /// A fully-renewable miner must earn strictly more than an
    /// otherwise-identical miner with 0% renewable, and crossing the 100%
    /// threshold must engage the discrete full-renewable bonus tier.
    #[test]
    fn full_renewable_out_earns_nonrenewable_and_applies_bonus() {
        let validator = make_validator();

        let none = validator.calculate_green_incentives(0.0, 100.0, 100.0);
        let full = validator.calculate_green_incentives(100.0, 100.0, 100.0);

        assert!(
            full > none,
            "100% renewable ({full}) must out-earn 0% renewable ({none})"
        );

        // The full-renewable tier is a discrete multiplier applied only at
        // >= 100%; a value just below the threshold must not receive it, so
        // the jump from 99.9% to 100% exceeds the marginal linear increase.
        let below = validator.calculate_green_incentives(99.9, 100.0, 100.0);
        let just_above = validator.calculate_green_incentives(100.0, 100.0, 100.0);
        let bonus = validator
            .incentive_structure
            .read()
            .expect("incentive lock should not be poisoned")
            .full_renewable_bonus;
        assert!(
            just_above > below * (1.0 + bonus * 0.5),
            "crossing 100% must engage the full-renewable bonus tier"
        );
    }

    /// The consumed-certificate ledger must credit a REC to exactly one miner:
    /// once `miner-a` has claimed it, `miner-b` presenting the same id gets no
    /// credit, closing the cross-miner double-claim. The rightful owner may
    /// still re-present it.
    #[test]
    fn certificate_cannot_be_claimed_by_a_second_miner() {
        let validator = make_validator();
        let mut call_a: HashSet<String> = HashSet::new();
        let mut call_b: HashSet<String> = HashSet::new();

        // First miner claims the certificate: credited.
        assert!(
            validator.try_claim_certificate("miner-a", "REC-1", &mut call_a),
            "first miner to present a certificate must be credited"
        );

        // A different miner presenting the SAME certificate id: rejected.
        assert!(
            !validator.try_claim_certificate("miner-b", "REC-1", &mut call_b),
            "a second miner must not be able to double-claim the same certificate"
        );

        // The original owner re-presenting its own certificate: still credited.
        let mut call_a2: HashSet<String> = HashSet::new();
        assert!(
            validator.try_claim_certificate("miner-a", "REC-1", &mut call_a2),
            "the certificate's rightful owner may re-present it"
        );
    }

    /// The same certificate id submitted twice within a single call must be
    /// credited only once (intra-call duplicate guard).
    #[test]
    fn duplicate_certificate_within_one_call_is_credited_once() {
        let validator = make_validator();
        let mut in_call: HashSet<String> = HashSet::new();

        assert!(
            validator.try_claim_certificate("miner-a", "REC-DUP", &mut in_call),
            "first occurrence in a call must be credited"
        );
        assert!(
            !validator.try_claim_certificate("miner-a", "REC-DUP", &mut in_call),
            "a duplicate id in the same call must not be credited twice"
        );
    }

    fn make_certificate(id: &str, amount_kwh: f64) -> RenewableCertificate {
        RenewableCertificate {
            certificate_id: id.to_string(),
            issuer: "Test Issuer".to_string(),
            certificate_type: "Solar".to_string(),
            amount_kwh,
            generation_start: Utc::now() - Duration::days(30),
            generation_end: Utc::now() + Duration::days(30),
            location: Region::new("US"),
            verification_status:
                crate::environmental::emissions::VerificationStatus::Pending,
            verification_url: None,
            metadata: HashMap::new(),
        }
    }

    fn make_offset(id: &str, amount_tonnes: f64) -> CarbonOffset {
        CarbonOffset {
            offset_id: id.to_string(),
            issuer: "Test Issuer".to_string(),
            offset_type: "Reforestation".to_string(),
            amount_tonnes,
            period_start: Utc::now() - Duration::days(30),
            period_end: Some(Utc::now() + Duration::days(30)),
            location: Region::new("US"),
            verification_status:
                crate::environmental::emissions::VerificationStatus::Pending,
            verification_url: None,
            metadata: HashMap::new(),
        }
    }

    /// The headline exploit: a miner presenting renewable certificates for more
    /// than half of consumption but ZERO carbon offsets must NOT be certified
    /// carbon-negative. Renewable energy is only credited when substantiated by
    /// certificate validation (the default validator has no verification
    /// endpoint, so certificates stay unverified and contribute no MWh), and it
    /// is never double-counted as "emissions avoided" on top of already being
    /// zero-rated. With no offsets there is nothing to drive net emissions below
    /// zero.
    #[tokio::test]
    async fn unverified_renewables_without_offsets_are_not_carbon_negative() {
        let validator = make_validator();

        // 100 MWh of renewable claims against 100 MWh consumption (i.e. 100%,
        // far above the old > total/2 trigger), but no carbon offsets at all.
        let certs = vec![make_certificate("CERT-1", 100_000.0)];

        let is_negative = validator
            .verify_carbon_negative_operations("miner-1", certs, 100.0, Vec::new())
            .await
            .expect("verification should succeed");

        assert!(
            !is_negative,
            "unverified renewables with zero offsets must not be certified carbon-negative"
        );
    }

    /// Carbon-negativity is driven strictly by verified offsets exceeding the
    /// residual non-renewable emissions (0.5 t/MWh). With no renewables, 100 MWh
    /// of consumption carries 50 t of emissions: 60 t of offsets clears it
    /// (net -10 t), 40 t does not (net +10 t).
    #[tokio::test]
    async fn carbon_negative_requires_offsets_to_exceed_residual_emissions() {
        let validator = make_validator();

        let sufficient = validator
            .verify_carbon_negative_operations(
                "miner-1",
                Vec::new(),
                100.0,
                vec![make_offset("OFF-A", 60.0)],
            )
            .await
            .expect("verification should succeed");
        assert!(
            sufficient,
            "offsets (60 t) exceeding residual emissions (50 t) must be carbon-negative"
        );

        let insufficient = validator
            .verify_carbon_negative_operations(
                "miner-1",
                Vec::new(),
                100.0,
                vec![make_offset("OFF-B", 40.0)],
            )
            .await
            .expect("verification should succeed");
        assert!(
            !insufficient,
            "offsets (40 t) below residual emissions (50 t) must not be carbon-negative"
        );
    }
}
