use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    // let thiserror implement From<StdError> for you
    Std(#[from] StdError),

    #[error("The game is full.")]
    GameIsFull,

    #[error("The game is already over.")]
    GameIsAlreadyOver,

    #[error("You are not a player.")]
    YouAreNotAPlayer,

    #[error("The game is still waiting for players.")]
    StillWaitingForPlayers,

    #[error("The game is still waiting for player 2.")]
    StillWaitingForPlayer2,

    #[error("Need to roll the dice to get a winner.")]
    NeedToDiceRollDiceForWinner,

    #[error("Must deposit 1 SCRT to play.")]
    MustDepositScrtToPlay,

    #[error("Player 1 not found.")]
    PlayerOneNotFound,

    #[error("The game is in progress.")]
    GameIsInProgress,

    #[error("No winner yet.")]
    NoWinnerYet,
}
