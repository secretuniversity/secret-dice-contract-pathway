use cosmwasm_std::{
    entry_point, to_binary, Deps, DepsMut, Env, Uint128, Coin,
    MessageInfo, QueryResponse, Response,
    CosmosMsg, BankMsg, BalanceResponse,
    StdError, StdResult
};

use rand::{RngCore, SeedableRng};
use rand_chacha::ChaChaRng;
use sha2::{Digest, Sha256};

use crate::error::{ContractError};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg, WinnerResponse};
use crate::state::{
    config, config_read, block_height, block_height_read,
    ContractState, DiceRoller, Winner, State,
};


//////////////////////////////////////////////////////////////////////
//////////////////////////////// Init ////////////////////////////////
//////////////////////////////////////////////////////////////////////

#[cfg_attr(not(feature = "library"), entry_point)]
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Join { name, secret } => try_join(deps, info, name, secret),
        ExecuteMsg::RollDice {} => try_roll_dice(deps, env, info),
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
            deposit_funds(&info)?;
            state.player_1 = Some(DiceRoller::new(name, info.sender, secret));
            state.state = ContractState::Got1;
        },
        ContractState::Got1 => {
            deposit_funds(&info)?;
            state.player_2 = Some(DiceRoller::new(name, info.sender, secret));
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
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = config(deps.storage).load()?;

    // once player 2 joins, we can derive a shared secret that no one knows
    // then we can roll the dice and choose a winner
    // dice roll 1-3: player 1 wins / dice roll 4-6: player 2 wins
    //
    // the winner then gets 2 SCRT

    let dice_roll: u8;
    let mut messages: Vec<CosmosMsg> = vec![];

    // Check the state of the game
    match state.state {
        ContractState::Init => {
            return Err(ContractError::StillWaitingForPlayers);
        },
        ContractState::Got1 => {
            return Err(ContractError::StillWaitingForPlayers);
        },
        ContractState::Got2 => {
            // get players
            let player_1 = if let Some(player_1) = &state.player_1 {
              player_1
            } else { return Err(ContractError::StillWaitingForPlayers) };

            let player_2 = if let Some(player_2) = &state.player_2 {
              player_2
            } else { return Err(ContractError::StillWaitingForPlayers) };

            // validate players
            if player_1.addr() != &info.sender && player_2.addr() != &info.sender {
                return Err(ContractError::YouAreNotAPlayer);
            }

            // saving the block height so that the winner cannpt be queried in the same block
            block_height(deps.storage).save(&env.block.height)?;

            let mut combined_secret: Vec<u8> = player_1.secret().to_be_bytes().to_vec();
            combined_secret.extend(&player_2.secret().to_be_bytes());

            let random_seed: [u8;32] = Sha256::digest(&combined_secret).into();
            let mut rng = ChaChaRng::from_seed(random_seed);

            dice_roll = ((rng.next_u32() % 6) + 1) as u8;   // a number between 1 and 6
            state.dice_roll = Some(dice_roll);

            let winner: Winner;
            if dice_roll >= 1 && dice_roll <= 3 {
                winner = Winner::new(
                    player_1.name().to_string(),
                    player_1.addr().clone()
                );
            } else {
                winner = Winner::new(
                    player_2.name().to_string(),
                    player_2.addr().clone()
                );
            }
            println!("dice roll = {}", dice_roll);
            println!("winner is {}", winner.name());

            state.winner = Some(winner.clone());
            state.state = ContractState::Done;

            // Winner takes all!
            messages.push(CosmosMsg::Bank(BankMsg::Send {
                to_address: winner.addr().to_string(),
                amount: vec![Coin::new(2_000_000, "uscrt")],
            }));

        },
        // Has a player already won the game?
        ContractState::Done => {
            return Err(ContractError::GameIsAlreadyOver);
        },
    }

    config(deps.storage).save(&state)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "roll dice")
        .add_attribute("result", dice_roll.to_string()))
}

pub fn try_leave(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let mut state = config(deps.storage).load()?;

    let player_1 = if let Some(player_1) = &state.player_1 {
        player_1
    } else {
        return Err(ContractError::PlayerOneNotFound);
    };

    // if player 2 isn't in yet, player 1 can leave and get their money back
    if player_1.addr().as_ref() != info.sender {
        return Err(ContractError::YouAreNotAPlayer);
    }

    // if we have both player 1 and player 2, game is in progress
    if state.player_2.is_some() && state.state != ContractState::Done {
        return Err(ContractError::GameIsInProgress);
    }

    if state.state == ContractState::Done {
        return Err(ContractError::GameIsAlreadyOver);
    }

    state.state = ContractState::Init;

    config(deps.storage).save(&state)?;

    // Player 1 leaves the game before another player can join, and gets a refund on their deposit
    let mut messages: Vec<CosmosMsg> = vec![];
    messages.push(CosmosMsg::Bank(BankMsg::Send {
        to_address: player_1.addr().to_string(),
        amount: vec![Coin::new(1_000_000, "uscrt")], // 1mn uscrt = 1 SCRT
    }));

    Ok(Response::new()
        .add_messages(messages)
        .add_attributes(vec![
            ("action", "leave"),
            ("receiver", player_1.name()),
    ]))
}

///////////////////////////////////////////////////////////////////////
//////////////////////////////// Query ////////////////////////////////
///////////////////////////////////////////////////////////////////////

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(
    deps: Deps,
    env: Env,
    msg: QueryMsg
) -> StdResult<QueryResponse> {
    match msg {
        QueryMsg::WhoWon {} => to_binary(&query_who_won(deps, env)?),
    }
}

fn query_who_won(
    deps: Deps,
    env: Env,
) -> StdResult<WinnerResponse> {

    let state = config_read(deps.storage).load()?;

    if state.state != ContractState::Done {
        return Err(StdError::generic_err("No winner yet."));
    }

    // check that the query is happening after the block where the winner is decided
    let winner_height = block_height_read(deps.storage).load()?;
    let current_height = env.block.height;

    if current_height <= winner_height {
        return Err(
            StdError::generic_err(
                "Querying who won is not allowed until after the winner has been finalized."
        ));
    }

    let dice_roll = if let Some(dice_roll) = state.dice_roll {
        dice_roll
    } else {
        return Err(StdError::generic_err("Dice roll not found."));
    };

    let winner = if let Some(winner) = &state.winner {
        winner
    } else {
        return Err(StdError::generic_err("Winner not found."));
    };

    let resp = WinnerResponse {
        name: winner.name().to_string(),
        addr: winner.addr().clone(),
        dice_roll: dice_roll,
    };
        
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies, mock_env, mock_info};
    use cosmwasm_std::{coins, Coin, from_binary, QueryRequest, BankQuery::Balance};

    #[test]
    fn proper_instantialization() {
        let mut deps = mock_dependencies();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));

        // we can just call .unwrap() to assert this was a success
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());
    }

    #[test]
    fn still_waiting_for_players() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        // Player 1 joins the game
        let secret = Uint128::new(1234u128);
        let msg = ExecuteMsg::Join {name: "alice".to_string(), secret: secret};
        let info = mock_info("alice", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // Player 1 tries to roll the dice -- should produce an error
        let err = execute(deps.as_mut(), env, info, ExecuteMsg::RollDice {}).unwrap_err();
        match err {
            ContractError::StillWaitingForPlayers {} => assert!(true),
            _e => { assert!(false) }
        }
    }

    #[test]
    fn player_1_leaves_and_gets_refund() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        // Player 1 joins the game
        let secret = Uint128::new(1234u128);
        let msg = ExecuteMsg::Join {name: "alice".to_string(), secret: secret};
        let info = mock_info("alice", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // Player 1 leaves the games and gets their 1 SCRT back
        let _res = execute(deps.as_mut(), env, info, ExecuteMsg::Leave {}).unwrap();
    }

    #[test]
    fn leave_when_game_in_progress() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        // Player 1 joins the game
        let secret = Uint128::new(1234u128);
        let msg = ExecuteMsg::Join {name: "alice".to_string(), secret: secret};
        let info_player_1 = mock_info("alice", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), info_player_1.clone(), msg).unwrap();

        // Player 2 joins the game
        let secret = Uint128::new(5678u128);
        let msg = ExecuteMsg::Join {name: "bob".to_string(), secret: secret};
        let info_player_2 = mock_info("bob", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), info_player_2, msg).unwrap();

        // Player 1 tries to leave the game, but it's in progress, awaiting a dice roll
        let err = execute(deps.as_mut(), env, info_player_1, ExecuteMsg::Leave {}).unwrap_err();
        match err {
            ContractError::GameIsInProgress {} => assert!(true),
            e => {
                assert!(false);
                panic!("error: {}", e);
            },
        }
    }

    #[test]
    fn not_a_player() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Player 1 joins the game
        let secret = Uint128::new(1234u128);
        let msg = ExecuteMsg::Join {name: "alice".to_string(), secret: secret};
        let info = mock_info("alice", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg).unwrap();

        // Player 2 joins the game
        let secret = Uint128::new(5678u128);
        let msg = ExecuteMsg::Join {name: "bob".to_string(), secret: secret};
        let info = mock_info("bob", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        // Non-player tries to interact with the game
        let info = mock_info("ted", &coins(1_000_000, "uscrt"));
        let err = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::Leave {}).unwrap_err();
        match err {
            ContractError::YouAreNotAPlayer {} => assert!(true),
            e => {
                assert!(false);
                panic!("error: {}", e);
            },
        }

        let info = mock_info("ted", &coins(1_000_000, "uscrt"));
        let err = execute(deps.as_mut(), env, info, ExecuteMsg::RollDice {}).unwrap_err();
        match err {
            ContractError::YouAreNotAPlayer {} => assert!(true),
            e => {
                assert!(false);
                panic!("error: {}", e);
            },
        }
    }

    #[test]
    fn game_is_full() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Player 1 joins the game
        let secret = Uint128::new(1234u128);
        let msg = ExecuteMsg::Join {name: "alice".to_string(), secret: secret};
        let info = mock_info("alice", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        // Player 2 joins the game
        let secret = Uint128::new(5678u128);
        let msg = ExecuteMsg::Join {name: "bob".to_string(), secret: secret};
        let info = mock_info("bob", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        // A 3rd player tries to join
        let secret = Uint128::new(9810u128);
        let info = mock_info("ted", &coins(1_000_000, "uscrt"));
        let msg = ExecuteMsg::Join {name: "ted".to_string(), secret: secret};
        let err = execute(deps.as_mut(), env, info, msg).unwrap_err();
        match err {
            ContractError::GameIsFull {} => assert!(true),
            e => {
                assert!(false);
                panic!("error: {}", e);
            },
        }
    }

    #[test]
    fn no_winner_yet() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));
        let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Player 1 joins the game
        let secret = Uint128::new(1234u128);
        let msg = ExecuteMsg::Join {name: "alice".to_string(), secret: secret};
        let info = mock_info("alice", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        // Player 2 joins the game
        let secret = Uint128::new(5678u128);
        let msg = ExecuteMsg::Join {name: "bob".to_string(), secret: secret};
        let info = mock_info("bob", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), info, msg).unwrap();

        // there should be no winner yet since we didn't do a dice roll!
        let err = query(deps.as_ref(), env, QueryMsg::WhoWon {}).unwrap_err();
        match err {
            _ => { assert!(true) }
        }
    }

    #[test]
    fn query_not_allowed_in_same_block() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        // Player 1 joins the game
        let secret_1 = Uint128::new(1234u128);
        let msg_player_1 = ExecuteMsg::Join {name: "alice".to_string(), secret: secret_1};
        let info = mock_info("alice", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), info, msg_player_1).unwrap();

        // Player 2 joins the game
        let secret_2 = Uint128::new(5678u128);
        let msg_player_2 = ExecuteMsg::Join {name: "bob".to_string(), secret: secret_2};
        let info = mock_info("bob", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg_player_2).unwrap();

        // Player 2 rolls the dice
        let _res = execute(deps.as_mut(), env.clone(), info, ExecuteMsg::RollDice {}).unwrap();

        // should result in an error because execute and query on winner cannot be done in the same block height
        let err = query(deps.as_ref(), env, QueryMsg::WhoWon {}).unwrap_err();
        match err {
            _ => {
                assert!(true);
            }
        }
    }

    #[test]
    fn roll_the_dice() {
        let mut deps = mock_dependencies();
        let mut env = mock_env();
        let uscrt_denom = "uscrt".to_string();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        let secret_1 = Uint128::new(1234u128);
        let msg_player_1 = ExecuteMsg::Join {name: "alice".to_string(), secret: secret_1};
        let info = mock_info("alice", &coins(1_000_000, uscrt_denom.clone()));
        let _res = execute(deps.as_mut(), env.clone(), info, msg_player_1).unwrap();

        let secret_2 = Uint128::new(5678u128);
        let msg_player_2 = ExecuteMsg::Join {name: "bob".to_string(), secret: secret_2};
        let info = mock_info("bob", &coins(1_000_000, uscrt_denom.clone()));
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), msg_player_2).unwrap();

        // player 2 rolls the dice
        let _res = execute(deps.as_mut(), env.clone(), info.clone(), ExecuteMsg::RollDice {}).unwrap();

        // advance block height by 1 to be able to query for winner
        env.block.height += 1;
        let res = query(deps.as_ref(), env, QueryMsg::WhoWon {}).unwrap();
        let value: WinnerResponse = from_binary(&res).unwrap();
        assert_eq!(value.name.is_empty(), false);

        let msg = QueryRequest::Bank(
            Balance {
                address: value.addr.to_string(),
                denom: uscrt_denom.clone(),
            });
        let _value: BalanceResponse = from_binary(&deps.querier.handle_query(&msg).unwrap().unwrap()).unwrap();
        let _expected_amount = Coin { amount: Uint128::new(2_000_000), denom: uscrt_denom };

        // TODO: verify winner's balance has increased by 2 SCRT
        // assert_eq!(value.amount, Coin { amount: Uint128::new(2_000_000), denom: uscrt_denom });
        // TODO: winner is always "alice" with a dice roll of 1 -- need to troubleshoot that
    }

    #[test]
    fn game_is_already_over() {
        let mut deps = mock_dependencies();
        let env = mock_env();

        let msg = InstantiateMsg {};
        let info = mock_info("creator", &coins(1000, "earth"));
        let _res = instantiate(deps.as_mut(), env.clone(), info, msg).unwrap();

        // Player 1 joins the game
        let secret = Uint128::new(1234u128);
        let msg = ExecuteMsg::Join {name: "alice".to_string(), secret: secret};
        let player_1_info = mock_info("alice", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), player_1_info.clone(), msg).unwrap();

        // Player 2 joins the game
        let secret = Uint128::new(5678u128);
        let msg = ExecuteMsg::Join {name: "bob".to_string(), secret: secret};
        let player_2_info = mock_info("bob", &coins(1_000_000, "uscrt"));
        let _res = execute(deps.as_mut(), env.clone(), player_2_info.clone(), msg).unwrap();

        // player 1 rolls the dice
        let msg = ExecuteMsg::RollDice {};
        let _res = execute(deps.as_mut(), env.clone(), player_1_info, msg.clone()).unwrap();

        // player 2 tries to roll the dice when game is over
        let err = execute(deps.as_mut(), env, player_2_info, msg).unwrap_err();
        match err {
            ContractError::GameIsAlreadyOver {} => assert!(true),
            e => {
                assert!(false);
                panic!("error: {}", e);
            },
        }
    }
}
