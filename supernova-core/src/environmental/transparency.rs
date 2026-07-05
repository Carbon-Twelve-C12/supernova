use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

use crate::environmental::emissions::VerificationStatus;
use crate::environmental::miner_reporting::MinerVerificationStatus;
use crate::environmental::verification::{CarbonOffset, RenewableCertificate};

/// Kilowatt-hours per megawatt-hour, used to convert certificate energy
/// amounts (recorded in kWh) into the MWh figures shown in transparency reports.
const KWH_PER_MWH: f64 = 1000.0;

/// Map an emissions [`VerificationStatus`] onto the reporting-facing
/// [`MinerVerificationStatus`] used in transparency breakdowns.
fn map_verification_status(status: VerificationStatus) -> MinerVerificationStatus {
    match status {
        VerificationStatus::Verified => MinerVerificationStatus::Verified,
        VerificationStatus::Pending => MinerVerificationStatus::Pending,
        VerificationStatus::Failed => MinerVerificationStatus::Rejected,
        VerificationStatus::None | VerificationStatus::Expired => {
            MinerVerificationStatus::Unverified
        }
    }
}

/// Level of transparency in reporting
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransparencyLevel {
    /// Basic reporting with minimal details
    Basic,
    /// Standard reporting with common metrics
    Standard,
    /// Enhanced reporting with detailed metrics
    Enhanced,
    /// Comprehensive reporting with all metrics
    Comprehensive,
}

impl fmt::Display for TransparencyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Basic => write!(f, "Basic"),
            Self::Standard => write!(f, "Standard"),
            Self::Enhanced => write!(f, "Enhanced"),
            Self::Comprehensive => write!(f, "Comprehensive"),
        }
    }
}

/// Summary of REC certificates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RECCertificateSummary {
    /// Total MWh covered by certificates
    pub total_mwh: f64,
    /// Number of certificates
    pub certificate_count: usize,
    /// Breakdown by energy type
    pub energy_type_breakdown: HashMap<String, f64>,
    /// List of verification providers
    pub verification_providers: Vec<String>,
    /// Breakdown by verification status
    pub verification_status_breakdown: HashMap<MinerVerificationStatus, usize>,
}

/// Summary of carbon offsets
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarbonOffsetSummary {
    /// Total tonnes CO2e offset
    pub total_tonnes: f64,
    /// Number of offsets
    pub offset_count: usize,
    /// Breakdown by project type
    pub project_type_breakdown: HashMap<String, f64>,
    /// List of verification providers
    pub verification_providers: Vec<String>,
    /// Breakdown by verification status
    pub verification_status_breakdown: HashMap<MinerVerificationStatus, usize>,
}

/// Data for a transparency report
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ReportData {
    /// Timestamp of the report
    timestamp: DateTime<Utc>,
    /// Total renewable energy in MWh
    total_renewable_energy_mwh: f64,
    /// Total offsets in tonnes CO2e
    total_offset_tonnes: f64,
    /// Verified renewable energy in MWh
    verified_mwh: f64,
    /// Verified offsets in tonnes CO2e
    verified_tonnes: f64,
    /// REC certificate statistics
    rec_stats: Option<RECCertificateSummary>,
    /// Carbon offset statistics
    offset_stats: Option<CarbonOffsetSummary>,
    /// Carbon negative ratio (offsets/emissions)
    carbon_negative_ratio: Option<f64>,
    /// Net carbon impact
    net_carbon_impact: Option<f64>,
}

/// Transparency report for environmental impact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransparencyReport {
    /// Timestamp of the report
    pub timestamp: DateTime<Utc>,
    /// Level of transparency
    pub transparency_level: TransparencyLevel,
    /// Total renewable energy in MWh
    pub total_renewable_energy_mwh: f64,
    /// Total offsets in tonnes CO2e
    pub total_offset_tonnes: f64,
    /// Percentage of renewable energy that is verified
    pub renewable_verification_percentage: f64,
    /// Percentage of offsets that is verified
    pub offset_verification_percentage: f64,
    /// REC certificate statistics
    pub rec_stats: Option<RECCertificateSummary>,
    /// Carbon offset statistics
    pub offset_stats: Option<CarbonOffsetSummary>,
    /// Carbon negative ratio (offsets/emissions)
    pub carbon_negative_ratio: Option<f64>,
    /// Net carbon impact
    pub net_carbon_impact: Option<f64>,
}

/// Dashboard for transparency reporting
pub struct TransparencyDashboard {
    /// Renewable energy certificates
    certificates: Vec<RenewableCertificate>,
    /// Carbon offsets
    offsets: Vec<CarbonOffset>,
    /// Current transparency level
    transparency_level: TransparencyLevel,
    /// Latest report
    latest_report: Option<TransparencyReport>,
    /// Report history
    report_history: Vec<TransparencyReport>,
}

impl Default for TransparencyDashboard {
    fn default() -> Self {
        Self::new()
    }
}

impl TransparencyDashboard {
    /// Create a new transparency dashboard
    pub fn new() -> Self {
        Self {
            certificates: Vec::new(),
            offsets: Vec::new(),
            transparency_level: TransparencyLevel::Standard,
            latest_report: None,
            report_history: Vec::new(),
        }
    }

    /// Generate a new transparency report
    pub fn generate_report(&mut self) -> TransparencyReport {
        let mut report_data = ReportData {
            timestamp: Utc::now(),
            total_renewable_energy_mwh: 0.0,
            total_offset_tonnes: 0.0,
            verified_mwh: 0.0,
            verified_tonnes: 0.0,
            rec_stats: None,
            offset_stats: None,
            carbon_negative_ratio: None,
            net_carbon_impact: None,
        };

        // Add certificates and offsets to the report
        self.add_certificates_to_report(&mut report_data);
        self.add_offsets_to_report(&mut report_data);

        // Calculate verification percentages
        let renewable_verification_percentage = if report_data.total_renewable_energy_mwh > 0.0 {
            (report_data.verified_mwh / report_data.total_renewable_energy_mwh) * 100.0
        } else {
            0.0
        };

        let offset_verification_percentage = if report_data.total_offset_tonnes > 0.0 {
            (report_data.verified_tonnes / report_data.total_offset_tonnes) * 100.0
        } else {
            0.0
        };

        // Create the report
        let report = TransparencyReport {
            timestamp: report_data.timestamp,
            transparency_level: self.transparency_level,
            total_renewable_energy_mwh: report_data.total_renewable_energy_mwh,
            total_offset_tonnes: report_data.total_offset_tonnes,
            renewable_verification_percentage,
            offset_verification_percentage,
            rec_stats: report_data.rec_stats,
            offset_stats: report_data.offset_stats,
            carbon_negative_ratio: report_data.carbon_negative_ratio,
            net_carbon_impact: report_data.net_carbon_impact,
        };

        // Update latest report and history
        self.latest_report = Some(report.clone());
        self.report_history.push(report.clone());

        report
    }

    /// Add renewable certificates to the report
    fn add_certificates_to_report(&self, report: &mut ReportData) {
        let certificates = &self.certificates;

        // Calculate total MWh (certificate amounts are in kWh; convert to MWh)
        let total_mwh: f64 = certificates
            .iter()
            .map(|c| c.amount_kwh / KWH_PER_MWH)
            .sum();

        report.total_renewable_energy_mwh = total_mwh;

        // Break down by energy type
        let mut energy_type_breakdown = HashMap::new();
        let mut verification_providers = Vec::new();
        let mut verification_status_breakdown = HashMap::new();

        for cert in certificates {
            // Add to energy type breakdown keyed by the certificate's actual
            // energy type (e.g. Solar/Wind/Hydro) (convert kWh to MWh)
            *energy_type_breakdown
                .entry(cert.certificate_type.clone())
                .or_insert(0.0) += cert.amount_kwh / KWH_PER_MWH;

            // Count certificates by their actual verification status
            *verification_status_breakdown
                .entry(map_verification_status(cert.verification_status))
                .or_insert(0) += 1;

            // Track verification providers
            if !verification_providers.contains(&cert.issuer) {
                verification_providers.push(cert.issuer.clone());
            }

            // Check verification status (convert kWh to MWh)
            if cert.verification_status == VerificationStatus::Verified {
                report.verified_mwh += cert.amount_kwh / KWH_PER_MWH;
            }
        }

        report.rec_stats = Some(RECCertificateSummary {
            total_mwh,
            certificate_count: certificates.len(),
            energy_type_breakdown,
            verification_providers,
            verification_status_breakdown,
        });
    }

    /// Add carbon offsets to the report
    fn add_offsets_to_report(&self, report: &mut ReportData) {
        let offsets = &self.offsets;

        // Calculate total tonnes CO2e
        let total_tonnes: f64 = offsets.iter().map(|o| o.amount_tonnes).sum();

        report.total_offset_tonnes = total_tonnes;

        // Break down by project type
        let mut project_type_breakdown = HashMap::new();
        let mut verification_providers = Vec::new();
        let mut verification_status_breakdown = HashMap::new();

        for offset in offsets {
            // Add to project type breakdown keyed by the offset's actual
            // project type (e.g. Reforestation/DAC)
            *project_type_breakdown
                .entry(offset.offset_type.clone())
                .or_insert(0.0) += offset.amount_tonnes;

            // Track verification providers
            if !verification_providers.contains(&offset.issuer) {
                verification_providers.push(offset.issuer.clone());
            }

            // Count offsets by their actual verification status
            *verification_status_breakdown
                .entry(map_verification_status(offset.verification_status))
                .or_insert(0) += 1;

            // Check verification status
            if offset.verification_status == VerificationStatus::Verified {
                report.verified_tonnes += offset.amount_tonnes;
            }
        }

        report.offset_stats = Some(CarbonOffsetSummary {
            total_tonnes,
            offset_count: offsets.len(),
            project_type_breakdown,
            verification_providers,
            verification_status_breakdown,
        });
    }

    /// Get the latest report
    pub fn get_latest_report(&self) -> Option<&TransparencyReport> {
        self.latest_report.as_ref()
    }

    /// Get the full report history
    pub fn get_report_history(&self) -> &[TransparencyReport] {
        &self.report_history
    }

    /// Get the current transparency level
    pub fn get_transparency_level(&self) -> TransparencyLevel {
        self.transparency_level
    }

    /// Export the latest report as JSON
    pub fn export_latest_report_json(&self) -> Result<String, String> {
        match &self.latest_report {
            Some(report) => match serde_json::to_string_pretty(report) {
                Ok(json) => Ok(json),
                Err(e) => Err(format!("Failed to serialize report to JSON: {}", e)),
            },
            None => Err("No report available".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environmental::types::Region;
    use std::collections::HashMap;

    fn make_cert(issuer: &str, kwh: f64, status: VerificationStatus) -> RenewableCertificate {
        RenewableCertificate {
            certificate_id: format!("cert-{}", issuer),
            issuer: issuer.to_string(),
            certificate_type: "Solar".to_string(),
            amount_kwh: kwh,
            generation_start: Utc::now(),
            generation_end: Utc::now(),
            location: Region::Global,
            verification_status: status,
            verification_url: None,
            metadata: HashMap::new(),
        }
    }

    fn make_offset(issuer: &str, tonnes: f64, status: VerificationStatus) -> CarbonOffset {
        CarbonOffset {
            offset_id: format!("offset-{}", issuer),
            issuer: issuer.to_string(),
            offset_type: "Reforestation".to_string(),
            amount_tonnes: tonnes,
            period_start: Utc::now(),
            period_end: None,
            location: Region::Global,
            verification_status: status,
            verification_url: None,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_verification_status_mapping() {
        assert_eq!(
            map_verification_status(VerificationStatus::Verified),
            MinerVerificationStatus::Verified
        );
        assert_eq!(
            map_verification_status(VerificationStatus::Pending),
            MinerVerificationStatus::Pending
        );
        assert_eq!(
            map_verification_status(VerificationStatus::Failed),
            MinerVerificationStatus::Rejected
        );
        assert_eq!(
            map_verification_status(VerificationStatus::None),
            MinerVerificationStatus::Unverified
        );
        assert_eq!(
            map_verification_status(VerificationStatus::Expired),
            MinerVerificationStatus::Unverified
        );
    }

    #[test]
    fn test_breakdown_reflects_actual_status() {
        let mut dashboard = TransparencyDashboard::new();
        // One verified, one pending, one failed certificate.
        dashboard
            .certificates
            .push(make_cert("issuer-a", 100.0, VerificationStatus::Verified));
        dashboard
            .certificates
            .push(make_cert("issuer-b", 50.0, VerificationStatus::Pending));
        dashboard
            .certificates
            .push(make_cert("issuer-c", 25.0, VerificationStatus::Failed));
        // One verified, one unverified (None) offset.
        dashboard
            .offsets
            .push(make_offset("issuer-d", 10.0, VerificationStatus::Verified));
        dashboard
            .offsets
            .push(make_offset("issuer-e", 5.0, VerificationStatus::None));

        let report = dashboard.generate_report();

        let rec_breakdown = report
            .rec_stats
            .expect("rec_stats present")
            .verification_status_breakdown;
        assert_eq!(
            rec_breakdown.get(&MinerVerificationStatus::Verified),
            Some(&1)
        );
        assert_eq!(
            rec_breakdown.get(&MinerVerificationStatus::Pending),
            Some(&1)
        );
        assert_eq!(
            rec_breakdown.get(&MinerVerificationStatus::Rejected),
            Some(&1)
        );
        // Not every certificate should be labeled Verified.
        assert_ne!(
            rec_breakdown.get(&MinerVerificationStatus::Verified),
            Some(&3)
        );

        let offset_breakdown = report
            .offset_stats
            .expect("offset_stats present")
            .verification_status_breakdown;
        assert_eq!(
            offset_breakdown.get(&MinerVerificationStatus::Verified),
            Some(&1)
        );
        assert_eq!(
            offset_breakdown.get(&MinerVerificationStatus::Unverified),
            Some(&1)
        );
    }

    #[test]
    fn test_breakdown_keyed_by_actual_type() {
        let mut dashboard = TransparencyDashboard::new();
        // Two distinct energy types: Solar (3.0 MWh) and Wind (2.0 MWh).
        let mut solar = make_cert("issuer-a", 3000.0, VerificationStatus::Verified);
        solar.certificate_type = "Solar".to_string();
        let mut wind = make_cert("issuer-b", 2000.0, VerificationStatus::Verified);
        wind.certificate_type = "Wind".to_string();
        dashboard.certificates.push(solar);
        dashboard.certificates.push(wind);

        // Two distinct project types: Reforestation (10t) and DAC (4t).
        let mut reforest = make_offset("issuer-c", 10.0, VerificationStatus::Verified);
        reforest.offset_type = "Reforestation".to_string();
        let mut dac = make_offset("issuer-d", 4.0, VerificationStatus::Verified);
        dac.offset_type = "DirectAirCapture".to_string();
        dashboard.offsets.push(reforest);
        dashboard.offsets.push(dac);

        let report = dashboard.generate_report();

        let energy = report
            .rec_stats
            .expect("rec_stats present")
            .energy_type_breakdown;
        // Breakdown must reflect real types, not a single "Renewable" bucket.
        assert!(!energy.contains_key("Renewable"));
        assert!((energy.get("Solar").copied().unwrap_or(0.0) - 3.0).abs() < 1e-9);
        assert!((energy.get("Wind").copied().unwrap_or(0.0) - 2.0).abs() < 1e-9);

        let projects = report
            .offset_stats
            .expect("offset_stats present")
            .project_type_breakdown;
        // Breakdown must reflect real project types, not a single bucket.
        assert!(!projects.contains_key("Carbon Offset"));
        assert!((projects.get("Reforestation").copied().unwrap_or(0.0) - 10.0).abs() < 1e-9);
        assert!((projects.get("DirectAirCapture").copied().unwrap_or(0.0) - 4.0).abs() < 1e-9);
    }

    #[test]
    fn test_renewable_energy_reported_in_mwh() {
        let mut dashboard = TransparencyDashboard::new();
        // 3,000 kWh verified + 1,500 kWh verified = 4,500 kWh = 4.5 MWh.
        dashboard
            .certificates
            .push(make_cert("issuer-a", 3000.0, VerificationStatus::Verified));
        dashboard
            .certificates
            .push(make_cert("issuer-b", 1500.0, VerificationStatus::Verified));

        let report = dashboard.generate_report();

        // MWh-labeled fields must be in MWh, not kWh (no 1000x inflation).
        assert!(
            (report.total_renewable_energy_mwh - 4.5).abs() < 1e-9,
            "expected 4.5 MWh, got {}",
            report.total_renewable_energy_mwh
        );

        let rec_stats = report.rec_stats.expect("rec_stats present");
        assert!(
            (rec_stats.total_mwh - 4.5).abs() < 1e-9,
            "expected 4.5 MWh summary, got {}",
            rec_stats.total_mwh
        );
        let breakdown_mwh = rec_stats
            .energy_type_breakdown
            .get("Solar")
            .copied()
            .expect("solar breakdown present");
        assert!(
            (breakdown_mwh - 4.5).abs() < 1e-9,
            "expected 4.5 MWh breakdown, got {}",
            breakdown_mwh
        );

        // Both certificates verified, so verification percentage is 100%.
        assert!(
            (report.renewable_verification_percentage - 100.0).abs() < 1e-9,
            "expected 100% verified, got {}",
            report.renewable_verification_percentage
        );
    }
}
