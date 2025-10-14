/**
 * SuperNova Environmental Dashboard
 * JavaScript for data fetching and visualization
 */

// Configuration
const API_ENDPOINT = 'http://localhost:8000/api';
const UPDATE_INTERVAL = 60000; // 1 minute
const DEBUG_MODE = false;

// Charts
let emissionsChart;
let treasuryChart;
let hardwareChart;
let energySourcesChart;

// Dashboard state
const dashboardState = {
    emissions: {
        total: 0,
        byCountry: {},
        history: []
    },
    miners: [],
    treasury: {
        balance: 0,
        recAllocation: 75,
        offsetAllocation: 15,
        researchAllocation: 5,
        communityAllocation: 5
    },
    hardware: {},
    energySources: {}
};

// Emissions factors database
const emissionsFactors = {
    // Sample of key countries - this would be expanded with the complete dataset
    USA: { iea2021: 366.5, ifi2020: 416, watttime: 1136 },
    CHN: { iea2021: 446.3, ifi2020: 899 },
    RUS: { iea2020: 359, ifi2020: 476 },
    DEU: { iea2021: 356.1, ifi2020: 650, watttime: 1685 },
    GBR: { iea2021: 220, ifi2020: 380, watttime: 943 },
    IND: { iea2021: 691.5, ifi2020: 951 },
    JPN: { iea2021: 461.3, ifi2020: 471 },
    CAN: { iea2021: 121.5, ifi2020: 372, watttime: 1069 },
    BRA: { iea2021: 132.8, ifi2020: 284 },
    FRA: { iea2021: 54.1, ifi2020: 158, watttime: 846 },
    AUS: { iea2021: 649.1, ifi2020: 808, watttime: 1409 },
    ZAF: { iea2021: 890.6, ifi2020: 1070 },
    ISL: { iea2021: 0.1, ifi2020: 0 },
    NOR: { iea2021: 4.1, ifi2020: 47, watttime: 536 },
    SWE: { iea2021: 14.4, ifi2020: 68, watttime: 1724 }
};

// Hardware efficiency data in J/TH (Joules per TeraHash)
const hardwareEfficiency = {
    'Antminer S19 Pro': 29.5,
    'Antminer S19 XP': 21.5,
    'Whatsminer M30S++': 31.0,
    'Avalon A1246': 38.0,
    'Antminer S9': 98.0,
    'MicroBT M50': 26.0,
    'Custom ASIC': 24.0
};

// Chart instances
let charts = {};

let chartInstances = {};
let refreshTimer = null;

// DOM Elements
document.addEventListener('DOMContentLoaded', () => {
    // Initialize UI
    initializeUI();
    
    // Initial data fetch
    fetchDashboardData();
    
    // Setup periodic refresh
    refreshTimer = setInterval(fetchDashboardData, UPDATE_INTERVAL);
    
    // Event listeners
    document.getElementById('refreshButton').addEventListener('click', manualRefresh);
    document.getElementById('debugToggle').addEventListener('change', toggleDebugMode);
    
    // Chart tab events
    document.getElementById('dailyTab').addEventListener('click', (e) => {
        e.preventDefault();
        activateTab('dailyTab');
        updateEmissionsChart('daily');
    });
    
    document.getElementById('weeklyTab').addEventListener('click', (e) => {
        e.preventDefault();
        activateTab('weeklyTab');
        updateEmissionsChart('weekly');
    });
    
    document.getElementById('monthlyTab').addEventListener('click', (e) => {
        e.preventDefault();
        activateTab('monthlyTab');
        updateEmissionsChart('monthly');
    });
    
    // Toggle verified-only miners
    document.getElementById('show-verified-only').addEventListener('change', (e) => {
        updateMinersTable(dashboardState.miners, e.target.checked);
    });
});

/**
 * Initialize the UI components
 */
function initializeUI() {
    // Set initial values
    document.getElementById('lastUpdatedTime').textContent = getFormattedTime();
    
    // For demonstration, we'll use simulated data
    initializeCharts();
    
    // Hide loading overlay after initialization
    setTimeout(() => {
        document.getElementById('loadingOverlay').style.display = 'none';
    }, 1500);
}

/**
 * Initialize charts with empty data
 */
function initializeCharts() {
    // Emissions chart
    charts.emissions = new Chart(
        document.getElementById('emissions-chart').getContext('2d'),
        {
            type: 'line',
            data: {
                labels: generateTimeLabels(12),
                datasets: [{
                    label: 'Carbon Emissions (tCO2e)',
                    data: generateRandomData(12, 500, 1000),
                    borderColor: '#ffc107',
                    backgroundColor: 'rgba(255, 193, 7, 0.2)',
                    tension: 0.4,
                    fill: true
                }]
            },
            options: {
                responsive: true,
                plugins: {
                    legend: {
                        position: 'top',
                    }
                },
                scales: {
                    y: {
                        beginAtZero: true,
                        title: {
                            display: true,
                            text: 'Metric Tons CO2e'
                        }
                    },
                    x: {
                        title: {
                            display: true,
                            text: 'Time'
                        }
                    }
                }
            }
        }
    );
    
    // Treasury allocation chart
    charts.treasury = new Chart(
        document.getElementById('treasury-chart').getContext('2d'),
        {
            type: 'doughnut',
            data: {
                labels: ['RECs', 'Carbon Offsets', 'Research', 'Community'],
                datasets: [{
                    data: [75, 15, 5, 5],
                    backgroundColor: [
                        'rgba(40, 167, 69, 0.8)',
                        'rgba(255, 193, 7, 0.8)',
                        'rgba(13, 110, 253, 0.8)',
                        'rgba(108, 117, 125, 0.8)'
                    ],
                    borderWidth: 1
                }]
            },
            options: {
                responsive: true,
                plugins: {
                    legend: {
                        position: 'right',
                    }
                }
            }
        }
    );
    
    // Hardware distribution chart
    charts.hardware = new Chart(
        document.getElementById('hardware-chart').getContext('2d'),
        {
            type: 'bar',
            data: {
                labels: Object.keys(hardwareEfficiency),
                datasets: [{
                    label: 'Miners Count',
                    data: generateRandomData(Object.keys(hardwareEfficiency).length, 5, 50),
                    backgroundColor: 'rgba(13, 110, 253, 0.8)'
                }]
            },
            options: {
                indexAxis: 'y',
                responsive: true,
                plugins: {
                    legend: {
                        display: false,
                    }
                }
            }
        }
    );
    
    // Energy sources chart
    charts.energy = new Chart(
        document.getElementById('energy-chart').getContext('2d'),
        {
            type: 'pie',
            data: {
                labels: ['Solar', 'Hydro', 'Wind', 'Nuclear', 'Natural Gas', 'Coal', 'Other'],
                datasets: [{
                    data: [20, 25, 15, 10, 15, 10, 5],
                    backgroundColor: [
                        '#ffc107', // Solar - yellow
                        '#0dcaf0', // Hydro - blue
                        '#20c997', // Wind - teal
                        '#6c757d', // Nuclear - gray
                        '#0d6efd', // Natural Gas - blue
                        '#212529', // Coal - dark
                        '#fd7e14'  // Other - orange
                    ]
                }]
            },
            options: {
                responsive: true,
                plugins: {
                    legend: {
                        position: 'right',
                    }
                }
            }
        }
    );
}

/**
 * Fetch dashboard data from the API
 * For MVP, we're using simulated data
 */
function fetchDashboardData() {
    // Show loading overlay
    document.getElementById('loading-overlay').style.display = 'flex';
    
    // Simulate API calls
    Promise.all([
        simulateFetchEmissionsData(),
        simulateFetchMinerData(),
        simulateFetchTreasuryData(),
        simulateFetchHardwareData()
    ])
    .then(([emissions, miners, treasury, hardware]) => {
        // Update dashboard state
        dashboardState.emissions = emissions;
        dashboardState.miners = miners;
        dashboardState.treasury = treasury;
        dashboardState.hardware = hardware;
        
        // Update UI components
        updateDashboardUI();
        
        // Hide loading overlay
        document.getElementById('loading-overlay').style.display = 'none';
    })
    .catch(error => {
        console.error('Error fetching data:', error);
        
        // Show error toast
        const toast = new bootstrap.Toast(document.getElementById('error-message'));
        document.getElementById('error-text').textContent = 'Error loading dashboard data';
        toast.show();
        
        // Hide loading overlay
        document.getElementById('loading-overlay').style.display = 'none';
    });
}

/**
 * Simulate fetching emissions data
 */
function simulateFetchEmissionsData() {
    return new Promise(resolve => {
        setTimeout(() => {
            const totalEmissions = Math.round(Math.random() * 500 + 800);
            const data = {
                total: totalEmissions,
                change: Math.round((Math.random() * 20) - 10), // -10% to +10%
                byCountry: {
                    USA: Math.round(totalEmissions * 0.3),
                    CHN: Math.round(totalEmissions * 0.25),
                    RUS: Math.round(totalEmissions * 0.15),
                    DEU: Math.round(totalEmissions * 0.1),
                    GBR: Math.round(totalEmissions * 0.05),
                    Other: Math.round(totalEmissions * 0.15)
                },
                history: generateRandomData(12, totalEmissions * 0.8, totalEmissions * 1.2)
            };
            resolve(data);
        }, 300);
    });
}

/**
 * Simulate fetching miner data
 */
function simulateFetchMinerData() {
    return new Promise(resolve => {
        setTimeout(() => {
            const minerCount = 25;
            const miners = [];
            
            for (let i = 0; i < minerCount; i++) {
                const country = getRandomKey(emissionsFactors);
                const hardware = getRandomKey(hardwareEfficiency);
                const renewablePercentage = Math.random() > 0.5 ? Math.round(Math.random() * 100) : 0;
                const isREC = renewablePercentage > 0 && Math.random() > 0.3;
                
                let status;
                if (renewablePercentage > 80) {
                    status = Math.random() > 0.2 ? 'Verified' : 'Pending';
                } else if (renewablePercentage > 30) {
                    status = Math.random() > 0.5 ? 'Pending' : 'Verified';
                } else {
                    status = Math.random() > 0.7 ? 'Pending' : 'None';
                }
                
                const energySource = getEnergySourceFromRenewable(renewablePercentage);
                const footprint = calculateFootprint(hardware, country, renewablePercentage);
                
                miners.push({
                    id: `miner_${i}_${Math.random().toString(36).substring(2, 7)}`,
                    country,
                    hardware,
                    energySource,
                    renewablePercentage,
                    isREC,
                    status,
                    carbonFootprint: footprint,
                    feeDiscount: calculateFeeDiscount(renewablePercentage, isREC, status)
                });
            }
            
            // Sort by renewable percentage (high to low)
            miners.sort((a, b) => b.renewablePercentage - a.renewablePercentage);
            
            resolve(miners);
        }, 300);
    });
}

/**
 * Simulate fetching treasury data
 */
function simulateFetchTreasuryData() {
    return new Promise(resolve => {
        setTimeout(() => {
            const balance = (Math.random() * 0.5 + 0.1).toFixed(4);
            const data = {
                balance,
                change: Math.round((Math.random() * 30) - 5), // -5% to +25%
                recAllocation: 75,
                offsetAllocation: 15,
                researchAllocation: 5,
                communityAllocation: 5,
                recPurchased: (balance * 0.75 * 0.8).toFixed(4), // 80% of allocation used
                offsetPurchased: (balance * 0.15 * 0.7).toFixed(4) // 70% of allocation used
            };
            resolve(data);
        }, 300);
    });
}

/**
 * Simulate fetching hardware data
 */
function simulateFetchHardwareData() {
    return new Promise(resolve => {
        setTimeout(() => {
            const data = {};
            const hwKeys = Object.keys(hardwareEfficiency);
            
            hwKeys.forEach(hw => {
                data[hw] = Math.floor(Math.random() * 50) + 5;
            });
            
            resolve(data);
        }, 300);
    });
}

/**
 * Update all dashboard UI components
 */
function updateDashboardUI() {
    // Update summary cards
    document.getElementById('total-energy').textContent = `${Math.round(dashboardState.emissions.total * 1.5)} MWh`;
    document.getElementById('energy-change').textContent = 
        `${dashboardState.emissions.change > 0 ? '+' : ''}${dashboardState.emissions.change}%`;
    
    const renewablePercentage = calculateNetworkRenewablePercentage(dashboardState.miners);
    document.getElementById('renewable-percentage').textContent = `${renewablePercentage}%`;
    document.getElementById('renewable-change').textContent = 
        `${Math.round((Math.random() * 10) - 2)}%`;
    
    document.getElementById('total-emissions').textContent = `${dashboardState.emissions.total} tCO2e`;
    document.getElementById('emissions-change').textContent = 
        `${dashboardState.emissions.change > 0 ? '+' : ''}${dashboardState.emissions.change}%`;
    
    document.getElementById('treasury-balance').textContent = `${dashboardState.treasury.balance} SNV`;
    document.getElementById('treasury-change').textContent = 
        `${dashboardState.treasury.change > 0 ? '+' : ''}${dashboardState.treasury.change}%`;
    
    // Update charts
    updateEmissionsChart();
    updateTreasuryChart();
    updateHardwareChart();
    updateEnergyChart();
    
    // Update miners table
    updateMinersTable(dashboardState.miners, false);
    
    // Update treasury allocations
    updateTreasuryAllocations();
}

/**
 * Update emissions chart
 */
function updateEmissionsChart() {
    charts.emissions.data.datasets[0].data = dashboardState.emissions.history;
    charts.emissions.update();
}

/**
 * Update treasury chart
 */
function updateTreasuryChart() {
    charts.treasury.data.datasets[0].data = [
        dashboardState.treasury.recAllocation,
        dashboardState.treasury.offsetAllocation,
        dashboardState.treasury.researchAllocation,
        dashboardState.treasury.communityAllocation
    ];
    charts.treasury.update();
}

/**
 * Update hardware chart
 */
function updateHardwareChart() {
    const hwLabels = [];
    const hwData = [];
    
    for (const [hw, count] of Object.entries(dashboardState.hardware)) {
        hwLabels.push(hw);
        hwData.push(count);
    }
    
    charts.hardware.data.labels = hwLabels;
    charts.hardware.data.datasets[0].data = hwData;
    charts.hardware.update();
}

/**
 * Update energy sources chart
 */
function updateEnergyChart() {
    // Calculate energy sources from miners
    const sources = {};
    
    dashboardState.miners.forEach(miner => {
        if (!sources[miner.energySource]) {
            sources[miner.energySource] = 0;
        }
        sources[miner.energySource]++;
    });
    
    const sourceLabels = [];
    const sourceData = [];
    
    for (const [source, count] of Object.entries(sources)) {
        sourceLabels.push(source);
        sourceData.push(count);
    }
    
    charts.energy.data.labels = sourceLabels;
    charts.energy.data.datasets[0].data = sourceData;
    charts.energy.update();
}

/**
 * Update miners table
 */
function updateMinersTable(miners, verifiedOnly) {
    const tableBody = document.getElementById('miners-table-body');
    tableBody.innerHTML = '';
    
    const filteredMiners = verifiedOnly 
        ? miners.filter(m => m.status === 'Verified') 
        : miners;
    
    filteredMiners.forEach(miner => {
        // Create badge for verification status
        let statusBadge;
        switch (miner.status) {
            case 'Verified':
                statusBadge = '<span class="badge bg-success">Verified</span>';
                break;
            case 'Pending':
                statusBadge = '<span class="badge bg-warning text-dark">Pending</span>';
                break;
            case 'None':
            default:
                statusBadge = '<span class="badge bg-secondary">None</span>';
        }
        
        // Create badge for REC/offset
        let energyBadge = '';
        if (miner.renewablePercentage > 0) {
            energyBadge = miner.isREC 
                ? '<span class="badge bg-success ms-1">REC</span>' 
                : '<span class="badge bg-info ms-1">Offset</span>';
        }
        
        const row = document.createElement('tr');
        row.innerHTML = `
            <td><small>${miner.id.substring(0, 10)}</small></td>
            <td>${miner.hardware}</td>
            <td>${miner.energySource} ${energyBadge}</td>
            <td>${miner.renewablePercentage}%</td>
            <td>${miner.carbonFootprint} kg</td>
            <td>${statusBadge}</td>
            <td>${miner.feeDiscount}%</td>
        `;
        
        tableBody.appendChild(row);
    });
}

/**
 * Update treasury allocations
 */
function updateTreasuryAllocations() {
    // Update progress bars
    document.getElementById('rec-progress').style.width = `${dashboardState.treasury.recAllocation}%`;
    document.getElementById('rec-progress').textContent = `${dashboardState.treasury.recAllocation}%`;
    
    document.getElementById('offset-progress').style.width = `${dashboardState.treasury.offsetAllocation}%`;
    document.getElementById('offset-progress').textContent = `${dashboardState.treasury.offsetAllocation}%`;
    
    document.getElementById('research-progress').style.width = `${dashboardState.treasury.researchAllocation}%`;
    document.getElementById('research-progress').textContent = `${dashboardState.treasury.researchAllocation}%`;
    
    document.getElementById('community-progress').style.width = `${dashboardState.treasury.communityAllocation}%`;
    document.getElementById('community-progress').textContent = `${dashboardState.treasury.communityAllocation}%`;
}

/**
 * Calculate network-wide renewable percentage
 */
function calculateNetworkRenewablePercentage(miners) {
    if (!miners.length) return 0;
    
    let totalHashrate = 0;
    let renewableHashrate = 0;
    
    miners.forEach(miner => {
        // Estimate hashrate based on hardware type
        const hashrate = getEstimatedHashrate(miner.hardware);
        totalHashrate += hashrate;
        renewableHashrate += hashrate * (miner.renewablePercentage / 100);
    });
    
    return Math.round((renewableHashrate / totalHashrate) * 100);
}

/**
 * Get estimated hashrate for hardware type
 */
function getEstimatedHashrate(hardware) {
    // Simplified hashrate estimates in TH/s
    const hashrates = {
        'Antminer S19 Pro': 110,
        'Antminer S19 XP': 140,
        'Whatsminer M30S++': 112,
        'Avalon A1246': 90,
        'Antminer S9': 14,
        'MicroBT M50': 126,
        'Custom ASIC': 130
    };
    
    return hashrates[hardware] || 50; // 50 TH/s default
}

/**
 * Calculate carbon footprint based on hardware, country, and renewable %
 */
function calculateFootprint(hardware, country, renewablePercentage) {
    // Get hardware efficiency in J/TH
    const efficiency = hardwareEfficiency[hardware] || 40;
    
    // Get country emission factor (gCO2/kWh) - prefer WattTime > IFI > IEA
    let emissionFactor;
    if (emissionsFactors[country]) {
        emissionFactor = emissionsFactors[country].watttime || 
                         emissionsFactors[country].ifi2020 || 
                         emissionsFactors[country].iea2021 || 
                         600; // Default if no specific factor
    } else {
        emissionFactor = 600; // Global average as fallback
    }
    
    // Get hashrate in TH/s
    const hashrate = getEstimatedHashrate(hardware);
    
    // Calculate daily energy in kWh
    // Energy (kWh) = Hashrate (TH/s) * Efficiency (J/TH) * 86400 (seconds/day) / 3,600,000 (J/kWh)
    const dailyEnergyKWh = hashrate * efficiency * 86400 / 3600000;
    
    // Apply renewable percentage reduction
    const effectiveEnergyKWh = dailyEnergyKWh * (1 - (renewablePercentage / 100));
    
    // Calculate emissions in kg CO2e
    // Emissions (kg CO2e) = Energy (kWh) * Emission Factor (gCO2/kWh) / 1000 (g/kg)
    const dailyEmissions = effectiveEnergyKWh * emissionFactor / 1000;
    
    return Math.round(dailyEmissions * 10) / 10; // Round to 1 decimal place
}

/**
 * Calculate fee discount based on renewable percentage and verification
 */
function calculateFeeDiscount(renewablePercentage, isREC, status) {
    // Base discount based on renewable percentage
    let discount = Math.min(Math.floor(renewablePercentage / 5), 20);
    
    // Additional discount if using RECs (prioritized)
    if (isREC && status === 'Verified') {
        discount += 10;
    } else if (status === 'Verified') {
        discount += 5;
    }
    
    return Math.min(discount, 30); // Cap at 30%
}

/**
 * Helper function to determine energy source based on renewable percentage
 */
function getEnergySourceFromRenewable(renewablePercentage) {
    if (renewablePercentage === 0) {
        const nonRenewables = ['Coal', 'Natural Gas', 'Mixed Grid'];
        return nonRenewables[Math.floor(Math.random() * nonRenewables.length)];
    } else if (renewablePercentage < 30) {
        return 'Mixed Grid';
    } else if (renewablePercentage < 70) {
        const mixedRenewables = ['Mixed Renewables', 'Hydro/Solar', 'Wind/Solar'];
        return mixedRenewables[Math.floor(Math.random() * mixedRenewables.length)];
    } else {
        const renewables = ['Solar', 'Hydro', 'Wind', 'Geothermal'];
        return renewables[Math.floor(Math.random() * renewables.length)];
    }
}

/**
 * Generate random data points
 */
function generateRandomData(count, min, max) {
    return Array(count).fill().map(() => Math.floor(Math.random() * (max - min + 1)) + min);
}

/**
 * Generate time labels for charts
 */
function generateTimeLabels(count) {
    const now = new Date();
    const labels = [];
    
    for (let i = count - 1; i >= 0; i--) {
        const date = new Date(now);
        date.setDate(date.getDate() - i);
        labels.push(date.toLocaleDateString('en-US', { month: 'short', day: 'numeric' }));
    }
    
    return labels;
}

/**
 * Get a random key from an object
 */
function getRandomKey(obj) {
    const keys = Object.keys(obj);
    return keys[Math.floor(Math.random() * keys.length)];
}

/**
 * Handle manual refresh button click
 */
function manualRefresh() {
    fetchDashboardData();
}

/**
 * Toggle debug mode
 */
function toggleDebugMode(event) {
    const debugPanel = document.getElementById('debugPanel');
    const isDebug = event.target.checked;
    
    if (isDebug) {
        debugPanel.style.display = 'block';
        debugPanel.innerHTML = '<h3>Debug Panel</h3><pre>' + JSON.stringify(dashboardState, null, 2) + '</pre>';
    } else {
        debugPanel.style.display = 'none';
    }
}

/**
 * Activate a tab and deactivate others
 */
function activateTab(tabId) {
    const tabs = ['dailyTab', 'weeklyTab', 'monthlyTab'];
    tabs.forEach(tab => {
        const element = document.getElementById(tab);
        if (tab === tabId) {
            element.classList.add('active');
        } else {
            element.classList.remove('active');
        }
    });
} 