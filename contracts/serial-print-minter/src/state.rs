use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Timestamp};
use cw_storage_plus::{Item, Map};
use sg4::{MinterConfig, Status};

#[cw_serde]
pub struct ConfigExtension {
    pub admin: Addr,
    pub payment_address: Option<Addr>,
    pub base_token_uri: String,
    pub num_tokens: u32,
    pub whitelist: Option<Addr>,
    pub start_time: Timestamp,
    pub per_address_limit: u32,
}
pub type Config = MinterConfig<ConfigExtension>;

pub const CONFIG: Item<Config> = Item::new("config");
pub const SG721_ADDRESS: Item<Addr> = Item::new("sg721_address");
// map of token ids. Bool is just a placeholder
pub const MINTABLE_TOKEN_IDS: Map<u32, bool> = Map::new("mt");
pub const MINTABLE_NUM_TOKENS: Item<u32> = Item::new("mintable_num_tokens");
pub const MINTER_ADDRS: Map<&Addr, u32> = Map::new("ma");

/// Holds the status of the minter. Can be changed with on-chain governance proposals.
pub const STATUS: Item<Status> = Item::new("status");

/// Set New URI
pub const BASE_TOKEN_ID: Item<u32> = Item::new("base_token_id");
pub const MINTED_NUM_TOKENS: Item<u32> = Item::new("minted_num_tokens");

/// Set Pause
pub const MINTING_PAUSED: Item<bool> = Item::new("mintable on/off");
