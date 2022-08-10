use cosmwasm_std::{
    entry_point, to_binary, Deps, DepsMut, Env, Uint128,
    MessageInfo, QueryResponse, Response, StdError, StdResult
};

use rand::{RngCore, SeedableRng};
use rand_chacha::ChaChaRng;
use sha2::{Digest, Sha256};

use crate::error::{ContractError};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, WinnerResponse};
use crate::state::{config, config_read, ContractState, DiceRoller, Winner, State};


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
        ExecuteMsg::Join { name, secret } => try_join(deps, info, name, secret),
        ExecuteMsg::RollDice {} => try_roll_dice(deps, info),
        ExecuteMsg::Leave {} => try_leave(deps, info),
    }
}

pub fn try_join(
    deps: DepsMut,
    info: MessageInfo,
    name: String,
    secret: Uint128,
) -> Result<Response, ContractError> {
    let mut state = config(deps.storage).load()?;

    // player 1 joins, sends a secret and deposits 1 SCRT to the contract
    // player 1's secret is stored privately
    //
    // player 2 joins, sends a secret and deposits 1 SCRT to the contract
    // player 2's secret is stored privately

    // Check the state of the game
    match state.state {
        ContractState::Init => {
            deposit_funds(&info);
            state.player_1 = DiceRoller::new(name, info.sender, secret);
            state.state = ContractState::Got1;
        },
        ContractState::Got1 => {
            deposit_funds(&info);
            state.player_2 = DiceRoller::new(name, info.sender, secret);
            state.state = ContractState::Got2;
        },
        ContractState::Got2 => {
            // We already have both players
            return Err(ContractError::GameIsFull);
        },
        ContractState::Done => {
            // Game is already over
            return Err(ContractError::GameIsAlreadyOver);
        },
    }

    config(deps.storage).save(&state)?;

    Ok(Response::new()
        .add_attribute("action", "join"))
}

fn deposit_funds(
    info: &MessageInfo,
) -> Result<Response, ContractError> {

    let amount = Uint128::new(1_000_000 /* 1mn uscrt = 1 SCRT */);
    if info.funds.len() != 1
        || info.funds[0].amount != amount
        || info.funds[0].denom != String::from("uscrt")
    {
        return Err(ContractError::MustDepositScrtToPlay);
    }

    Ok(Response::default())
}

pub fn try_roll_dice(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = config(deps.storage).load()?;

    // once player 2 joins, we can derive a shared secret that no one knows
    // then we can roll the dice and choose a winner
    // dice roll 1-3: player 1 wins / dice roll 4-6: player 2 wins
    //
    // the winner then gets 2 SCRT

    // Check the state of the game
    match state.state {
        ContractState::Init => {
            return Err(ContractError::StillWaitingForPlayers);
        },
        ContractState::Got1 => {
            return Err(ContractError::StillWaitingForPlayers);
        },
        ContractState::Got2 => {
            let mut combined_secret: Vec<u8> = state.player_1.secret().to_be_bytes().to_vec();
            combined_secret.extend(&state.player_2.secret().to_be_bytes());

            let random_seed: [u8;32] = Sha256::digest(&combined_secret).into();
            let mut rng = ChaChaRng::from_seed(random_seed);

            state.dice_roll = ((rng.next_u32() % 6) + 1) as u8;   // a number between 1 and 6

            if state.dice_roll >= 1 && state.dice_roll <=3 {
                state.winner = Winner::new(
                    state.player_1.name().to_string(),
                    state.player_1.addr().clone()
                );
            } else {
                state.winner = Winner::new(
                    state.player_2.name().to_string(),
                    state.player_2.addr().clone()
                );
            }

            state.state = ContractState::Done;

            // TODO: send all funds to winner
        },
        // Has a player already won the game?
        ContractState::Done => {
            return Err(ContractError::GameIsAlreadyOver);
        },
    }

    config(deps.storage).save(&state)?;

    Ok(Response::new()
        .add_attribute("action", "roll dice")
        .add_attribute("result", state.dice_roll.to_string()))
}

pub fn try_leave(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = config(deps.storage).load()?;

    // if player 2 isn't in yet, player 1 can leave and get their money back
    if state.player_1.addr().as_ref() != info.sender {
        return Err(ContractError::YouAreNotAPlayer);
    }

    if state.state == ContractState::Done {
        return Err(ContractError::GameIsAlreadyOver);
    }

    state.state = ContractState::Init;

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
) -> StdResult<WinnerResponse> {

    let state = config_read(deps.storage).load()?;

    if state.state != ContractState::Done{
        return Err(StdError::generic_err("No winner yet."));
    }

    let resp = WinnerResponse {
        name: state.winner.name().to_string(),
        addr: state.winner.addr().clone(),
        dice_roll: state.dice_roll,
    };
        
    Ok(resp)
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
