// supernova Emissions Visualizer
// A command line tool that visualizes emissions data using ASCII/Unicode art charts
// This demonstrates the use of the environmental module's emissions tracking capabilities

extern crate supernova_core as btclib;

use btclib::environmental::emissions::EmissionsTracker;
use btclib::environmental::types::{HardwareType, Region};
use chrono::{Duration, TimeZone, Utc};
use rand::thread_rng;
use rand::Rng;
use std::collections::HashMap;
use std::io::{self, Write};

struct ASCIIChartConfig {
    width: usize,
    height: usize,
    title: String,
    x_label: String,
    y_label: String,
    max_value: f64,
    min_value: f64,
}

impl Default for ASCIIChartConfig {
    fn default() -> Self {
        Self {
            width: 60,
            height: 20,
            title: "Emissions Chart".to_string(),
            x_label: "Time".to_string(),
            y_label: "Value".to_string(),
            max_value: 100.0,
            min_value: 0.0,
        }
    }
}

struct MinerSimulation {
    id: String,
    region: Region,
    hardware_type: HardwareType,
    unit_count: usize,
    renewable_percentage: f64,
    daily_emissions: Vec<f64>,
    energy_consumption: Vec<f64>,
}

impl MinerSimulation {
    fn new(
        id: &str,
        region: Region,
        hardware_type: HardwareType,
        unit_count: usize,
        renewable_percentage: f64,
    ) -> Self {
        Self {
            id: id.to_string(),
            region,
            hardware_type,
            unit_count,
            renewable_percentage,
            daily_emissions: Vec::new(),
            energy_consumption: Vec::new(),
        }
    }

    fn simulate_emissions(&mut self, days: usize, volatility: f64) {
        // Start with base emissions based on hardware type and region
        let base_emissions_per_unit = match &self.hardware_type {
            HardwareType::ASIC(model) if model.contains("S19") => 35.0, // kg CO2/day
            HardwareType::ASIC(model) if model.contains("M30") => 42.0,
            HardwareType::ASIC(_) => 40.0,
            HardwareType::GPU(_) => 28.0,
            HardwareType::CPU(_) => 15.0,
            HardwareType::Other(_) => 30.0,
        };

        // Apply regional factor
        let regional_factor = match self.region.country_code.as_str() {
            "US" => 1.0,
            "CN" => 1.4,
            "EU" => 0.7,
            "IN" => 1.5,
            _ => 1.0,
        };

        // Apply renewable percentage reduction
        let renewable_factor = 1.0 - (self.renewable_percentage / 100.0);

        // Calculate base daily emissions
        let base_daily =
            base_emissions_per_unit * self.unit_count as f64 * regional_factor * renewable_factor;

        // Simulate daily variations
        let mut rng = rand::thread_rng();

        for day in 0..days {
            // Add some randomness to simulate daily variations
            let random_factor = 1.0 + (rand::random::<f64>() - 0.5) * volatility;
            let daily_emissions = base_daily * random_factor;

            // Also simulate energy consumption (kWh)
            let base_energy = match &self.hardware_type {
                HardwareType::ASIC(model) if model.contains("S19") => 3.0, // kWh per unit per day
                HardwareType::ASIC(model) if model.contains("M30") => 3.5,
                HardwareType::ASIC(_) => 3.3,
                HardwareType::GPU(_) => 2.5,
                HardwareType::CPU(_) => 1.2,
                HardwareType::Other(_) => 2.8,
            };

            let daily_energy = base_energy * self.unit_count as f64 * random_factor;

            self.daily_emissions.push(daily_emissions);
            self.energy_consumption.push(daily_energy);
        }
    }
}

fn draw_bar_chart(data: &[f64], config: &ASCIIChartConfig) {
    println!("\n{:^width$}", config.title, width = config.width);
    println!("{:^width$}", "".to_string(), width = config.width);

    // Find max value for scaling if not specified
    let max_value = if config.max_value > 0.0 {
        config.max_value
    } else {
        data.iter().copied().fold(0.0, f64::max)
    };

    // Draw y-axis and bars
    for row in (0..config.height).rev() {
        let threshold = max_value * (row as f64 / config.height as f64);
        let y_label = if row % 5 == 0 {
            format!("{:>5.1}", threshold)
        } else {
            "     ".to_string()
        };

        print!("{} │", y_label);

        for &value in data {
            let scaled_value = (value / max_value) * config.height as f64;
            if scaled_value >= row as f64 {
                print!("█");
            } else {
                print!(" ");
            }
        }
        println!();
    }

    // Draw x-axis
    print!("      └");
    for _ in 0..data.len() {
        print!("─");
    }
    println!();

    // X-axis labels
    print!("       ");
    for i in 0..data.len() {
        if i % 5 == 0 {
            print!("{}", i % 10);
        } else {
            print!(" ");
        }
    }
    println!("\n       {}", config.x_label);
}

fn draw_line_chart(data: &[f64], config: &ASCIIChartConfig) {
    println!("\n{:^width$}", config.title, width = config.width);
    println!("{:^width$}", "".to_string(), width = config.width);

    // Find max value for scaling if not specified
    let max_value = if config.max_value > 0.0 {
        config.max_value
    } else {
        data.iter().copied().fold(0.0, f64::max) * 1.1 // Add 10% headroom
    };

    // Min value
    let min_value = if config.min_value < max_value {
        config.min_value
    } else {
        0.0
    };

    // Create matrix for the chart area
    let mut chart_matrix = vec![vec![' '; data.len()]; config.height];

    // Calculate points
    let range = max_value - min_value;
    for (x, &value) in data.iter().enumerate() {
        if x >= data.len() || value.is_nan() {
            continue;
        }

        let normalized = (value - min_value) / range;
        let y = ((1.0 - normalized) * (config.height as f64 - 1.0)) as usize;

        if y < config.height {
            chart_matrix[y][x] = '•';
        }
    }

    // Connect points with lines
    for x in 1..data.len() {
        let prev_value = data[x - 1];
        let curr_value = data[x];

        if prev_value.is_nan() || curr_value.is_nan() {
            continue;
        }

        let prev_normalized = (prev_value - min_value) / range;
        let curr_normalized = (curr_value - min_value) / range;

        let prev_y = ((1.0 - prev_normalized) * (config.height as f64 - 1.0)) as usize;
        let curr_y = ((1.0 - curr_normalized) * (config.height as f64 - 1.0)) as usize;

        if prev_y == curr_y {
            continue;
        }

        let (start_y, end_y) = if prev_y < curr_y {
            (prev_y + 1, curr_y)
        } else {
            (curr_y + 1, prev_y)
        };

        for y in start_y..end_y {
            if y < config.height {
                chart_matrix[y][x] = '│';
            }
        }
    }

    // Draw y-axis and chart
    for (row, line) in chart_matrix.iter().enumerate() {
        let y_value = max_value - (row as f64 * range / (config.height as f64 - 1.0));
        let y_label = if row % 5 == 0 || row == config.height - 1 {
            format!("{:>6.1}", y_value)
        } else {
            "      ".to_string()
        };

        print!("{} │", y_label);

        for &c in line {
            print!("{}", c);
        }
        println!();
    }

    // Draw x-axis
    print!("       └");
    for _ in 0..data.len() {
        print!("─");
    }
    println!();

    // X-axis labels
    print!("        ");
    for i in 0..data.len() {
        if i % 5 == 0 {
            print!("{}", i % 10);
        } else {
            print!(" ");
        }
    }
    println!("\n        {}", config.x_label);
}

fn draw_comparative_chart(label: &str, data_sets: &[(&str, &[f64])]) {
    println!("\n{:^60}", label);
    println!("{:^60}", "".to_string());

    // Find max value across all datasets
    let max_value = data_sets
        .iter()
        .flat_map(|(_, data)| data.iter())
        .copied()
        .fold(0.0, f64::max)
        * 1.1; // Add 10% headroom

    let height = 20;

    // Symbols for different data sets
    let symbols = ['*', '+', '•', 'x', '□', '■', '◆'];

    // Create legend
    println!("Legend:");
    for (i, (name, _)) in data_sets.iter().enumerate() {
        println!("  {} - {}", symbols[i % symbols.len()], name);
    }
    println!();

    // Create matrix for the chart area
    let mut chart_matrix = vec![vec![' '; data_sets[0].1.len()]; height];

    // Fill in the chart matrix with data points
    for (dataset_idx, (_, data)) in data_sets.iter().enumerate() {
        let symbol = symbols[dataset_idx % symbols.len()];

        for (x, &value) in data.iter().enumerate() {
            if x >= data.len() || value.is_nan() {
                continue;
            }

            let normalized = value / max_value;
            let y = ((1.0 - normalized) * (height as f64 - 1.0)) as usize;

            if y < height {
                chart_matrix[y][x] = symbol;
            }
        }
    }

    // Draw y-axis and chart
    for row in 0..height {
        let y_value = max_value - (row as f64 * max_value / (height as f64 - 1.0));
        let y_label = if row % 5 == 0 || row == height - 1 {
            format!("{:>6.1}", y_value)
        } else {
            "      ".to_string()
        };

        print!("{} │", y_label);

        for &c in &chart_matrix[row] {
            print!("{}", c);
        }
        println!();
    }

    // Draw x-axis
    print!("       └");
    for _ in 0..data_sets[0].1.len() {
        print!("─");
    }
    println!();

    // X-axis labels
    print!("        ");
    for i in 0..data_sets[0].1.len() {
        if i % 5 == 0 {
            print!("{}", i % 10);
        } else {
            print!(" ");
        }
    }
    println!("\n        Days");
}

fn print_miner_summary(miners: &[MinerSimulation]) {
    println!("\nMiner Summary");
    println!("=============");

    for miner in miners {
        println!("\nMiner ID: {}", miner.id);
        println!("Region: {:?}", miner.region.country_code);
        println!("Hardware: {:?}", miner.hardware_type);
        println!("Units: {}", miner.unit_count);
        println!("Renewable %: {:.1}%", miner.renewable_percentage);

        // Calculate total emissions
        let total_emissions: f64 = miner.daily_emissions.iter().sum();
        let avg_daily_emissions = total_emissions / miner.daily_emissions.len() as f64;

        let total_energy: f64 = miner.energy_consumption.iter().sum();
        let avg_daily_energy = total_energy / miner.energy_consumption.len() as f64;

        println!("Average Daily Emissions: {:.2} kg CO2", avg_daily_emissions);
        println!("Average Daily Energy: {:.2} kWh", avg_daily_energy);
        println!("Total Emissions (period): {:.2} kg CO2", total_emissions);
        println!("Total Energy (period): {:.2} kWh", total_energy);

        // Calculate emissions intensity
        let emissions_intensity = avg_daily_emissions / avg_daily_energy;
        println!("Emissions Intensity: {:.3} kg CO2/kWh", emissions_intensity);

        // Compare to industry average
        let industry_avg = 0.5; // kg CO2/kWh (example value)
        let comparison = (emissions_intensity / industry_avg - 1.0) * 100.0;

        if comparison < 0.0 {
            println!(
                "Performance: {:.1}% better than industry average",
                -comparison
            );
        } else {
            println!(
                "Performance: {:.1}% worse than industry average",
                comparison
            );
        }
    }
}

fn main() {
    println!("supernova Emissions Visualizer");
    println!("==============================");

    // Configure simulation parameters
    let simulation_days = 30;

    // Create sample miners for simulation
    let mut miners = vec![
        MinerSimulation::new(
            "GreenMiner-US",
            Region {
                country_code: "US".to_string(),
                sub_region: Some("WA".to_string()),
            },
            HardwareType::ASIC("S19 Pro".to_string()),
            50,
            80.0, // 80% renewable
        ),
        MinerSimulation::new(
            "CoalMiner-CN",
            Region {
                country_code: "CN".to_string(),
                sub_region: None,
            },
            HardwareType::ASIC("S19j".to_string()),
            100,
            10.0, // 10% renewable
        ),
        MinerSimulation::new(
            "EuropeMiner",
            Region {
                country_code: "EU".to_string(),
                sub_region: None,
            },
            HardwareType::ASIC("M30S+".to_string()),
            75,
            60.0, // 60% renewable
        ),
    ];

    // Run simulations
    for miner in &mut miners {
        println!(
            "Simulating emissions for {} over {} days...",
            miner.id, simulation_days
        );
        miner.simulate_emissions(simulation_days, 0.2); // 20% volatility
    }

    // Visualize the data

    // Individual charts for each miner
    for miner in &miners {
        // Emissions chart
        let emissions_config = ASCIIChartConfig {
            title: format!("{} - Daily Emissions (kg CO2)", miner.id),
            x_label: "Days".to_string(),
            y_label: "kg CO2".to_string(),
            ..Default::default()
        };

        draw_line_chart(&miner.daily_emissions, &emissions_config);

        // Energy consumption chart
        let energy_config = ASCIIChartConfig {
            title: format!("{} - Daily Energy Consumption (kWh)", miner.id),
            x_label: "Days".to_string(),
            y_label: "kWh".to_string(),
            ..Default::default()
        };

        draw_line_chart(&miner.energy_consumption, &energy_config);
    }

    // Comparative chart of emissions
    let emissions_data: Vec<(&str, &[f64])> = miners
        .iter()
        .map(|m| (m.id.as_str(), m.daily_emissions.as_slice()))
        .collect();

    draw_comparative_chart("Comparative Daily Emissions (kg CO2)", &emissions_data);

    // Print summary statistics
    print_miner_summary(&miners);
}
