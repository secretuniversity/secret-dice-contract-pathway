use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    // let thiserror implement From<StdError> for you
    Std(#[from] StdError),

    #[error("The game is full."]
    GameIsFull,

    #[error("The game is already over. The winner was: {addr})")]
    GameIsAlreadyOver { addr: Addr },

    #[error("You are not a player."]
    YouAreNotAPlayer,

    #[error("The game is still waiting for players."]
    StillWaitingForPlayers,
}
