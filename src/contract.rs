use cosmwasm_std::{
    entry_point, to_binary, Deps, DepsMut, Env,
    MessageInfo, QueryResponse, Response, StdError, StdResult
};

use rand::{RngCore, SeedableRng};
use rand_chacha::ChaChaRng;
use sha2::{Digest, Sha256};

use crate::error::{ContractError};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, DiceRollResponse, WinnerResponse};
use crate::state::{config, config_read, ContractState, DiceRoller, State};


//////////////////////////////////////////////////////////////////////
//////////////////////////////// Init ////////////////////////////////
//////////////////////////////////////////////////////////////////////

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> StdResult<Response> {

    let state = State::default();
    config(deps.storage).save(&state)?;

    Ok(Response::default())
}

//////////////////////////////////////////////////////////////////////
//////////////////////////// Execute /////////////////////////////////
//////////////////////////////////////////////////////////////////////

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Join { secret } => try_join(deps, info, secret),
        ExecuteMsg::RollDice { } => try_roll_dice(deps, info),
        ExecuteMsg::Leave { } => try_leave(deps, info),
    }
}

pub fn try_join(
    deps: DepsMut,
    info: MessageInfo,
    secret: UInt128,
) -> Result<Response, ContractError> {
    let mut state = config(deps.storage).load()?;

    // Check the state of the game
    match state.state {
        ContractState::Init => {
            state.player_1 = DiceRoller::new(info.sender, secret);
            state.state = ContractState::Got1;
        }
        ContractState::Got1 => {
            state.player_2 = DiceRoller::new(info.sender, secret);
            state.state = ContractState::Got2;
        }
        ContractState::Got2 => {
            // We already have both players
            return Err(ContractError::GameIsFull);
        }
    }

    config(deps.storage).save(&state)?;

    Ok(Response::new()
        .add_attribute("action", "join"))
}

pub fn try_roll_dice(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = config(deps.storage).load()?;

    if state.winner.is_some() {
        return Err(ContractError::GameIsAlreadyOver { state.winner });
    }

    config(deps.storage).save(&state)?;

    Ok(Response::new()
        .add_attribute("action", "roll dice"))
}

pub fn try_leave(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = config(deps.storage).load()?;

    // if player 2 isn't in yet, player 1 can leave and get their money back
    if state.player_1.addr != Some(&info.sender) {
        return Err(ContractError::YouAreNotAPlayer));
    }

    if state.winner.is_some() {
        return Err(
            ContractError::GameIsAlreadyOver, 
            state.winner.unwrap()
        );
    }

    state.state = ContractState::Init;
    state.player_1.addr = None;
    state.player_1.secret = 0;
    state.player_2.addr = None;
    state.player_2.secret = 0;

    config(deps.storage).save(&state)?;

    // TODO: send funds back to player 1
    /*
    Ok(HandleResponse {
        messages: vec![CosmosMsg::Bank(BankMsg::Send {
            from_address: env.contract.address,
            to_address: env.message.sender,
            amount: vec![Coin::new(1_000_000, "uscrt")], // 1mn uscrt = 1 SCRT
        })],
        log: vec![],
        data: None,
    })
    */

    Ok(Response::new()
        .add_attribute("action", "leave"))
}

///////////////////////////////////////////////////////////////////////
//////////////////////////////// Query ////////////////////////////////
///////////////////////////////////////////////////////////////////////

#[entry_point]
pub fn query(
    deps: Deps,
    _env: Env,
    msg: QueryMsg
) -> StdResult<QueryResponse> {
    match msg {
        QueryMsg::WhoWon {} => to_binary(&query_who_won(deps)?),
    }
}

fn query_who_won(
    deps: Deps
) -> Result<WinnerResponse, ContractError> {
    let state = config_read(deps.storage).load()?;


    if state.winner.is_none() {
        return Err(ContractError::StillWaitingForPlayers));
    }

    Ok(&Result {
        winner: state.winner.unwrap(),
        dice_roll: state.dice_roll,
    })?)
}

/*
#[cfg(test)]
mod tests {
    use super::*;

    use cosmwasm_std::testing::{mock_env, mock_info, mock_dependencies};
    use cosmwasm_std::coins;

    #[test]
    fn proper_instantialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // it worked, let's query the state
        let _ = query_who_is_richer(deps.as_ref()).unwrap_err();
    }

    #[test]
    fn solve_millionaire() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let msg_player1 = ExecuteMsg::SubmitNetWorth {worth: 1, name: "alice".to_string()};
        let msg_player2 = ExecuteMsg::SubmitNetWorth {worth: 2, name: "bob".to_string()};

        let info = mock_info("creator", &[]);

        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg_player1).unwrap();
        let _res = execute(deps.as_mut(), mock_env(), info, msg_player2).unwrap();

        // it worked, let's query the state
        let value = query_who_is_richer(deps.as_ref()).unwrap();

        assert_eq!(&value.richer, "bob")

    }

    #[test]
    fn test_reset_state() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(2, "token"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        let msg_player1 = ExecuteMsg::SubmitNetWorth {worth: 1, name: "alice".to_string()};

        let info = mock_info("creator", &[]);
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg_player1).unwrap();

        let reset_msg = ExecuteMsg::Reset {};
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), reset_msg).unwrap();

        let msg_player2 = ExecuteMsg::SubmitNetWorth {worth: 2, name: "bob".to_string()};
        let msg_player3 = ExecuteMsg::SubmitNetWorth {worth: 3, name: "carol".to_string()};

        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg_player2).unwrap();
        let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg_player3).unwrap();

        // it worked, let's query the state
        let value = query_who_is_richer(deps.as_ref()).unwrap();

        assert_eq!(&value.richer, "carol")    }
}
*/
