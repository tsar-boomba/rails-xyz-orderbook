use ecow::EcoString;
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawOrder {
    pub quantity: EcoString,
    pub price: EcoString,
    pub order_type: OrderType,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawOrderDelta {
    pub quantity: EcoString,
    pub price: EcoString,
    pub order_type: OrderType,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawCompletedOrder {
	pub price: EcoString,
	pub quantity: EcoString,
	pub match_id: Uuid,
	pub updated_at: u64,
	pub order_type: OrderType,
	pub execution_type: ExecutionType,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderType {
	Buy,
	Sell
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExecutionType {
	Maker,
	Taker
}
