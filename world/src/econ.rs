use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaxRates {
    pub income: f64,
    pub corporate: f64,
    pub vat: f64,
    pub social_employer: f64,
    pub social_employee: f64,
    pub export: f64,
    pub capital_formation: f64,
}

impl Default for TaxRates {
    fn default() -> Self {
        Self {
            income: 0.0,
            corporate: 0.0,
            vat: 0.0,
            social_employer: 0.0,
            social_employee: 0.0,
            export: 0.0,
            capital_formation: 0.0,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EconSnapshot {
    #[serde(default)]
    pub quarter: u32,
    #[serde(default)]
    pub real_gdp: f64,
    #[serde(default)]
    pub nominal_gdp: f64,
    #[serde(default)]
    pub real_gdp_growth: f64,
    #[serde(default)]
    pub nominal_gdp_growth: f64,
    #[serde(default)]
    pub inflation: f64,
    #[serde(default)]
    pub unemployment: f64,
    #[serde(default)]
    pub euribor: f64,
    #[serde(default)]
    pub government_spending: f64,
    #[serde(default)]
    pub government_revenue: f64,
    #[serde(default)]
    pub government_debt: f64,
    #[serde(default)]
    pub consumption: f64,
    #[serde(default)]
    pub investment: f64,
    #[serde(default)]
    pub exports: f64,
    #[serde(default)]
    pub imports: f64,
    #[serde(default)]
    pub wage_growth: f64,
    #[serde(default)]
    pub price_level: f64,
    #[serde(default)]
    pub money_supply: f64,
    #[serde(default)]
    pub bank_deposits: f64,
    #[serde(default)]
    pub bank_loans: f64,
    #[serde(default)]
    pub equity_index: f64,
    #[serde(default)]
    pub housing_price: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct EconState {
    pub tax_rates: TaxRates,
    pub history: Vec<EconSnapshot>,
}

impl EconState {
    pub fn latest(&self) -> Option<&EconSnapshot> {
        self.history.last()
    }
}
