use cosmwasm_std::Storage;
use cosmwasm_storage::{
    ReadonlySingleton, singleton, Singleton,
    singleton_read,
};

use serde::{Deserialize, Serialize};

const CONFIG_KEY: &[u8] = b"config";

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct State {
    pub state: ContractState,
    pub player_1: <Option>DiceRoller,
    pub player_2: <Option>DiceRoller
    pub dice_roll: u8,
    pub winner: <Option>DiceRoller,
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub enum ContractState {
    Init,
    Got1,
    Done
}

impl Default for ContractState {
    fn default() -> Self {
        Self::Init
    }
}

impl From<u8> for ContractState {
    fn from(num: u8) -> Self {
        match num {
            0 => ContractState::Init,
            1 => ContractState::Got1,
            2 => ContractState::Done,
            _ => ContractState::Init
        }
    }
}

impl From<ContractState> for u8 {
    fn from(state: ContractState) -> Self {
        match state {
            ContractState::Init => 0,
            ContractState::Got1 => 1,
            ContractState::Done => 2
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, Eq)]
pub struct DiceRoller {
    addr: Addr,
    secret: UInt128
}

impl DiceRoller {
    /// Constructor function. Takes input parameters and initializes a struct containing both
    /// those items
    pub fn new(addr: Addr, secret: UInt128) -> DiceRoller {
        return DiceRoller {
            addr,
            secret
        }
    }

    /// Viewer function to read the private member of the DiceRoller struct.
    /// We could make the member public instead and access it directly if we wanted to simplify
    /// access patterns
    pub fn addr(&self) -> &Addr {
        &self.addr
    }

    pub fn secret(&self) -> &UInt128 {
        &self.secret
    }
}

pub fn config(storage: &mut dyn Storage) -> Singleton<State> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read(storage: &dyn Storage) -> ReadonlySingleton<State> {
    singleton_read(storage, CONFIG_KEY)
}
