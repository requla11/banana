use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnergyMetrics {
    pub duration: Duration,
    pub estimated_joules: f64,
    pub estimated_watt_hours: f64,
    pub carbon_grams_co2: f64,
    pub cpu_cores_utilized: f64,
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

pub struct EnergyMeter {
    profile: HardwareProfile,
    start_time: Option<Instant>,
    grid_intensity_g_per_kwh: f64,
}

impl EnergyMeter {
    pub fn new(profile: HardwareProfile, grid_intensity_g_per_kwh: f64) -> Self {
        Self {
            profile,
            start_time: None,
            grid_intensity_g_per_kwh,
        }
    }

    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
    }

    pub fn stop(&self, cpu_utilization_ratio: f64) -> EnergyMetrics {
        let duration = self
            .start_time
            .map(|t| t.elapsed())
            .unwrap_or(Duration::from_secs(0));

        let duration_secs = duration.as_secs_f64();
        let util = cpu_utilization_ratio.clamp(0.0, 1.0);
        let active_power = self.profile.idle_power_watts
            + (self.profile.tdp_watts - self.profile.idle_power_watts) * util;

        let estimated_joules = active_power * duration_secs;
        let estimated_watt_hours = estimated_joules / 3600.0;
        let carbon_grams_co2 = (estimated_watt_hours / 1000.0) * self.grid_intensity_g_per_kwh;
        let cpu_cores_utilized = self.profile.core_count as f64 * util;

        EnergyMetrics {
            duration,
            estimated_joules,
            estimated_watt_hours,
            carbon_grams_co2,
            cpu_cores_utilized,
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
