use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnergyMetrics {
    pub duration: Duration,
    pub estimated_joules: f64,
    pub estimated_watt_hours: f64,
    pub carbon_grams_co2: f64,
    pub cpu_cores_utilized: f64,
    pub is_hardware_measured: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareProfile {
    pub tdp_watts: f64,
    pub idle_power_watts: f64,
    pub core_count: usize,
}

impl Default for HardwareProfile {
    fn default() -> Self {
        Self {
            tdp_watts: 65.0,
            idle_power_watts: 10.0,
            core_count: std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(4),
        }
    }
}

pub struct LinuxRaplReader;

impl LinuxRaplReader {
    pub fn read_energy_microjoules() -> Option<u64> {
        let rapl_path = Path::new("/sys/class/powercap/intel-rapl/intel-rapl:0/energy_uj");
        if rapl_path.exists() {
            if let Ok(content) = fs::read_to_string(rapl_path) {
                if let Ok(val) = content.trim().parse::<u64>() {
                    return Some(val);
                }
            }
        }
        None
    }
}

pub struct EnergyMeter {
    profile: HardwareProfile,
    start_time: Option<Instant>,
    start_microjoules: Option<u64>,
    grid_intensity_g_per_kwh: f64,
}

impl EnergyMeter {
    pub fn new(profile: HardwareProfile, grid_intensity_g_per_kwh: f64) -> Self {
        Self {
            profile,
            start_time: None,
            start_microjoules: None,
            grid_intensity_g_per_kwh,
        }
    }

    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        self.start_microjoules = LinuxRaplReader::read_energy_microjoules();
    }

    pub fn stop(&self, cpu_utilization_ratio: f64) -> EnergyMetrics {
        let duration = self
            .start_time
            .map(|t| t.elapsed())
            .unwrap_or(Duration::from_secs(0));

        let duration_secs = duration.as_secs_f64();
        let util = cpu_utilization_ratio.clamp(0.0, 1.0);

        let end_microjoules = LinuxRaplReader::read_energy_microjoules();
        let (estimated_joules, is_hw) =
            if let (Some(start_uj), Some(end_uj)) = (self.start_microjoules, end_microjoules) {
                if end_uj >= start_uj {
                    ((end_uj - start_uj) as f64 / 1_000_000.0, true)
                } else {
                    let active_power = self.profile.idle_power_watts
                        + (self.profile.tdp_watts - self.profile.idle_power_watts) * util;
                    (active_power * duration_secs, false)
                }
            } else {
                let active_power = self.profile.idle_power_watts
                    + (self.profile.tdp_watts - self.profile.idle_power_watts) * util;
                (active_power * duration_secs, false)
            };

        let estimated_watt_hours = estimated_joules / 3600.0;
        let carbon_grams_co2 = (estimated_watt_hours / 1000.0) * self.grid_intensity_g_per_kwh;
        let cpu_cores_utilized = self.profile.core_count as f64 * util;

        EnergyMetrics {
            duration,
            estimated_joules,
            estimated_watt_hours,
            carbon_grams_co2,
            cpu_cores_utilized,
            is_hardware_measured: is_hw,
        }
    }
}

pub struct GreenCarbonCalculator;

impl GreenCarbonCalculator {
    pub fn calculate_carbon_offset(grams_co2: f64) -> f64 {
        grams_co2 / 21770.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_energy_meter_measurement() {
        let profile = HardwareProfile {
            tdp_watts: 100.0,
            idle_power_watts: 20.0,
            core_count: 8,
        };

        let mut meter = EnergyMeter::new(profile, 300.0);
        meter.start();
        std::thread::sleep(Duration::from_millis(50));

        let metrics = meter.stop(0.8);
        assert!(metrics.estimated_joules > 0.0);
        assert!(metrics.estimated_watt_hours > 0.0);
        assert!(metrics.carbon_grams_co2 > 0.0);
        assert_eq!(metrics.cpu_cores_utilized, 6.4);
    }
}
