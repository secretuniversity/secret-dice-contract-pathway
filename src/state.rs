use cosmwasm_std::{Addr, Uint128, Storage};
use cosmwasm_storage::{
    ReadonlySingleton, singleton, Singleton,
    singleton_read,
};

use serde::{Deserialize, Serialize};

const CONFIG_KEY: &[u8] = b"config";
const BLOCK_HEIGHT_KEY: &[u8] = b"block_height";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct State {
    pub state: ContractState,
    pub player_1: Option<DiceRoller>,
    pub player_2: Option<DiceRoller>,
    pub dice_roll: Option<u8>,
    pub winner: Option<Winner>,
}

impl State {
    pub fn default() -> State {
        return State {
            state: ContractState::default(),
            player_1: None,
            player_2: None,
            dice_roll: None,
            winner: None,
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq, Clone, Debug)]
pub enum ContractState {
    Init,
    Got1,
    Got2,
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
            2 => ContractState::Got2,
            3 => ContractState::Done,
            _ => ContractState::Init
        }
    }
}

impl From<ContractState> for u8 {
    fn from(state: ContractState) -> Self {
        match state {
            ContractState::Init => 0,
            ContractState::Got1 => 1,
            ContractState::Got2 => 2,
            ContractState::Done => 3,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiceRoller {
    name: String,
    addr: Addr,
    secret: Uint128
}

impl Default for DiceRoller {
    fn default() -> DiceRoller {
        return DiceRoller {
            name: String::from(""),
            addr: Addr::unchecked(""),
            secret: Uint128::from(0u32)
        }
    }
}

impl DiceRoller {
    /// Constructor function. Takes input parameters and initializes a struct containing both
    /// those 
    pub fn new(name: String, addr: Addr, secret: Uint128) -> DiceRoller {
        return DiceRoller {
            name,
            addr,
            secret
        }
    }

    /// Viewer function to read the private member of the DiceRoller struct.
    /// We could make the member public instead and access it directly if we wanted to simplify
    /// access patterns
    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn addr(&self) -> &Addr {
        &self.addr
    }

    pub fn secret(&self) -> &Uint128 {
        &self.secret
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Winner {
    name: String,
    addr: Addr,
}

impl Winner {
    /// Constructor function. Takes input parameters and initializes a struct containing both
    /// those items
    pub fn new(name: String, addr: Addr) -> Winner {
        return Winner {
            name,
            addr
        }
    }

    /// Viewer function to read the private member of the Winner struct.
    /// We could make the member public instead and access it directly if we wanted to simplify
    /// access patterns
    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn addr(&self) -> &Addr {
        &self.addr
    }
}

pub fn config(storage: &mut dyn Storage) -> Singleton<State> {
    singleton(storage, CONFIG_KEY)
}

pub fn config_read(storage: &dyn Storage) -> ReadonlySingleton<State> {
    singleton_read(storage, CONFIG_KEY)
}

pub fn block_height(storage: &mut dyn Storage) -> Singleton<u64> {
    singleton(storage, BLOCK_HEIGHT_KEY)
}

pub fn block_height_read(storage: &dyn Storage) -> ReadonlySingleton<u64> {
    singleton_read(storage, BLOCK_HEIGHT_KEY)
}
