#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, BankMsg, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo, Order,
    Reply, ReplyOn, Response, StdError, StdResult, SubMsg, WasmMsg,
};
use cw2::set_contract_version;
use cw721::TokensResponse as Cw721TokensResponse;
use cw721_base::{msg::ExecuteMsg as Cw721ExecuteMsg, MintMsg};
use cw_storage_plus::Bound;
use cw_utils::{must_pay, parse_reply_instantiate_data, Expiration};
use sg721::msg::{InstantiateMsg as Sg721InstantiateMsg, QueryMsg as Sg721QueryMsg};
use url::Url;

use crate::error::ContractError;
use crate::msg::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, MintableNumTokensResponse, OnWhitelistResponse,
    QueryMsg, StartTimeResponse, UpdateWhitelistMsg, WhitelistAddressesResponse,
    WhitelistExpirationResponse,
};
use crate::state::{
    Config, CONFIG, MINTABLE_TOKEN_IDS, NUM_WHITELIST_ADDRS, SG721_ADDRESS, WHITELIST_ADDRS,
};

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:sg-minter";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const INSTANTIATE_SG721_REPLY_ID: u64 = 1;
const MAX_TOKEN_LIMIT: u32 = 10000;
const MAX_WHITELIST_ADDRS_LENGTH: u32 = 15000;
const MAX_PER_ADDRESS_LIMIT: u64 = 30;
const MAX_BATCH_MINT_LIMIT: u64 = 30;
const STARTING_BATCH_MINT_LIMIT: u64 = 5;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    if msg.num_tokens > MAX_TOKEN_LIMIT.into() {
        return Err(ContractError::MaxTokenLimitExceeded {
            max: MAX_TOKEN_LIMIT,
        });
    }

    if let Some(per_address_limit) = msg.per_address_limit {
        // Check per address limit is valid
        if per_address_limit > MAX_PER_ADDRESS_LIMIT {
            return Err(ContractError::InvalidPerAddressLimit {
                max: MAX_PER_ADDRESS_LIMIT.to_string(),
                got: per_address_limit.to_string(),
            });
        }
    }

    if let Some(batch_mint_limit) = msg.batch_mint_limit {
        // Check batch mint limit is valid
        if batch_mint_limit > MAX_BATCH_MINT_LIMIT {
            return Err(ContractError::InvalidBatchMintLimit {
                max: MAX_BATCH_MINT_LIMIT.to_string(),
                got: batch_mint_limit.to_string(),
            });
        }
    }

    // Check that base_token_uri is a valid IPFS uri
    let parsed_token_uri = Url::parse(&msg.base_token_uri)?;
    if parsed_token_uri.scheme() != "ipfs" {
        return Err(ContractError::InvalidBaseTokenURI {});
    }

    // Initially set batch_mint_limit if no msg
    let batch_mint_limit: Option<u64> = msg.batch_mint_limit.or(Some(STARTING_BATCH_MINT_LIMIT));

    let config = Config {
        admin: info.sender,
        base_token_uri: msg.base_token_uri,
        num_tokens: msg.num_tokens,
        sg721_code_id: msg.sg721_code_id,
        unit_price: msg.unit_price,
        whitelist_expiration: msg.whitelist_expiration,
        start_time: msg.start_time,
        per_address_limit: msg.per_address_limit,
        batch_mint_limit,
    };
    CONFIG.save(deps.storage, &config)?;

    // Set whitelist addresses and num_whitelist_addresses
    if let Some(whitelist_addresses) = msg.whitelist_addresses {
        // Check length of whitelist addresses is not greater than max allowed
        if MAX_WHITELIST_ADDRS_LENGTH <= (whitelist_addresses.len() as u32) {
            return Err(ContractError::MaxWhitelistAddressLengthExceeded {});
        }

        for whitelist_address in whitelist_addresses.clone().into_iter() {
            WHITELIST_ADDRS.save(deps.storage, whitelist_address, &Empty {})?;
        }
        NUM_WHITELIST_ADDRS.save(deps.storage, &(whitelist_addresses.len() as u32))?;
    }

    // save mintable token ids map
    for token_id in 0..msg.num_tokens {
        MINTABLE_TOKEN_IDS.save(deps.storage, token_id, &Empty {})?;
    }

    let sub_msgs: Vec<SubMsg> = vec![SubMsg {
        msg: WasmMsg::Instantiate {
            code_id: msg.sg721_code_id,
            msg: to_binary(&Sg721InstantiateMsg {
                name: msg.sg721_instantiate_msg.name,
                symbol: msg.sg721_instantiate_msg.symbol,
                minter: env.contract.address.to_string(),
                config: msg.sg721_instantiate_msg.config,
            })?,
            funds: info.funds,
            admin: None,
            label: String::from("Fixed price minter"),
        }
        .into(),
        id: INSTANTIATE_SG721_REPLY_ID,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_submessages(sub_msgs))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Mint {} => execute_mint(deps, env, info),
        ExecuteMsg::UpdateWhitelist(update_whitelist_msg) => {
            execute_update_whitelist(deps, env, info, update_whitelist_msg)
        }
        ExecuteMsg::UpdateWhitelistExpiration(expiration) => {
            execute_update_whitelist_expiration(deps, env, info, expiration)
        }
        ExecuteMsg::UpdateStartTime(expiration) => {
            execute_update_start_time(deps, env, info, expiration)
        }
        ExecuteMsg::UpdatePerAddressLimit { per_address_limit } => {
            execute_update_per_address_limit(deps, env, info, per_address_limit)
        }
        ExecuteMsg::UpdateBatchMintLimit { batch_mint_limit } => {
            execute_update_batch_mint_limit(deps, env, info, batch_mint_limit)
        }
        ExecuteMsg::MintTo { recipient } => execute_mint_to(deps, env, info, recipient),
        ExecuteMsg::MintFor {
            token_id,
            recipient,
        } => execute_mint_for(deps, env, info, token_id, recipient),
        ExecuteMsg::BatchMint { num_mints } => execute_batch_mint(deps, env, info, num_mints),
    }
}

pub fn execute_mint(deps: DepsMut, env: Env, info: MessageInfo) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let sg721_address = SG721_ADDRESS.load(deps.storage)?;
    let action = "mint";

    let allowlist = WHITELIST_ADDRS.has(deps.storage, info.sender.to_string());
    if let Some(whitelist_expiration) = config.whitelist_expiration {
        // Check if whitelist not expired and sender is not whitelisted
        if !whitelist_expiration.is_expired(&env.block) && !allowlist {
            return Err(ContractError::NotWhitelisted {
                addr: info.sender.to_string(),
            });
        }
    }

    let payment = must_pay(&info, &config.unit_price.denom)?;
    if payment != config.unit_price.amount {
        return Err(ContractError::IncorrectPaymentAmount {});
    }

    if let Some(start_time) = config.start_time {
        // Check if after start_time
        if !start_time.is_expired(&env.block) {
            return Err(ContractError::BeforeMintStartTime {});
        }
    }

    // Check if already minted max per address limit
    if let Some(per_address_limit) = config.per_address_limit {
        let tokens: Cw721TokensResponse = deps.querier.query_wasm_smart(
            sg721_address.to_string(),
            &Sg721QueryMsg::Tokens {
                owner: info.sender.to_string(),
                start_after: None,
                limit: Some(MAX_PER_ADDRESS_LIMIT as u32),
            },
        )?;
        if tokens.tokens.len() >= per_address_limit as usize {
            return Err(ContractError::MaxPerAddressLimitExceeded {});
        }
    }

    _execute_mint(deps, env, info, action, None, None)
}

pub fn execute_mint_to(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: Addr,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let action = "mint_to";

    // Check only admin
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    _execute_mint(deps, env, info, action, Some(recipient), None)
}

pub fn execute_mint_for(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    token_id: u64,
    recipient: Addr,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let action = "mint_for";

    // Check only admin
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    _execute_mint(deps, env, info, action, Some(recipient), Some(token_id))
}

pub fn execute_batch_mint(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    num_mints: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    let mint_limit = config
        .batch_mint_limit
        .ok_or(ContractError::MaxBatchMintLimitExceeded {})?;

    if num_mints > mint_limit {
        return Err(ContractError::MaxBatchMintLimitExceeded {});
    }

    for _ in 0..num_mints {
        execute_mint(deps.branch(), env.clone(), info.clone())?;
    }

    Ok(Response::default()
        .add_attribute("action", "batch_mint")
        .add_attribute("num_mints", num_mints.to_string()))
}

fn _execute_mint(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    action: &str,
    recipient: Option<Addr>,
    token_id: Option<u64>,
) -> Result<Response, ContractError> {
    // generalize checks and mint message creation
    // mint -> _execute_mint(recipient: None, token_id: None)
    // mint_to(recipient: "friend") -> _execute_mint(Some(recipient), token_id: None)
    // mint_for(recipient: "friend2", token_id: 420) -> _execute_mint(recipient, token_id)
    let config = CONFIG.load(deps.storage)?;
    let sg721_address = SG721_ADDRESS.load(deps.storage)?;
    let recipient_addr = if recipient.is_none() {
        info.sender
    } else if let Some(some_recipient) = recipient {
        some_recipient
    } else {
        return Err(ContractError::InvalidAddress {});
    };

    // if token_id None, find and assign one. else check token_id exists on mintable map.
    let mintable_token_id: u64 = if token_id.is_none() {
        let mintable_tokens_result: StdResult<Vec<u64>> = MINTABLE_TOKEN_IDS
            .keys(deps.storage, None, None, Order::Ascending)
            .take(1)
            .collect();
        let mintable_tokens = mintable_tokens_result?;
        if mintable_tokens.is_empty() {
            return Err(ContractError::SoldOut {});
        }
        mintable_tokens[0]
    } else if let Some(some_token_id) = token_id {
        let mintable_tokens_result: StdResult<Vec<u64>> = MINTABLE_TOKEN_IDS
            .keys(
                deps.storage,
                None,
                Some(Bound::inclusive(vec![some_token_id as u8])),
                Order::Ascending,
            )
            .take(1)
            .collect();
        // If token_id not mintable, throw err
        let mintable_tokens = mintable_tokens_result?;
        if mintable_tokens.is_empty() {
            return Err(ContractError::TokenIdAlreadySold {
                token_id: some_token_id,
            });
        }
        mintable_tokens[0]
    } else {
        return Err(ContractError::InvalidTokenId {});
    };

    let mut msgs: Vec<CosmosMsg> = vec![];

    let mint_msg = Cw721ExecuteMsg::Mint(MintMsg::<Empty> {
        token_id: mintable_token_id.to_string(),
        owner: recipient_addr.to_string(),
        token_uri: Some(format!("{}/{}", config.base_token_uri, mintable_token_id)),
        extension: Empty {},
    });

    let msg = CosmosMsg::Wasm(WasmMsg::Execute {
        contract_addr: sg721_address.to_string(),
        msg: to_binary(&mint_msg)?,
        funds: vec![],
    });
    msgs.append(&mut vec![msg]);

    // remove mintable token id from map
    MINTABLE_TOKEN_IDS.remove(deps.storage, mintable_token_id);

    let seller_msg = BankMsg::Send {
        to_address: config.admin.to_string(),
        amount: vec![config.unit_price],
    };
    msgs.append(&mut vec![seller_msg.into()]);

    Ok(Response::default()
        .add_attribute("action", action)
        .add_messages(msgs))
}

pub fn execute_update_whitelist(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    update_whitelist_msg: UpdateWhitelistMsg,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut num_whitelist_addresses = NUM_WHITELIST_ADDRS.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    // Add whitelist addresses
    if let Some(add_whitelist_addrs) = update_whitelist_msg.add_addresses {
        if MAX_WHITELIST_ADDRS_LENGTH
            <= (add_whitelist_addrs.len() as u32 + num_whitelist_addresses)
        {
            return Err(ContractError::MaxWhitelistAddressLengthExceeded {});
        }
        for whitelist_address in add_whitelist_addrs.clone().into_iter() {
            WHITELIST_ADDRS.save(deps.storage, whitelist_address, &Empty {})?;
        }
        num_whitelist_addresses += add_whitelist_addrs.len() as u32;
    }

    // Remove whitelist addresses
    if let Some(remove_whitelist_addrs) = update_whitelist_msg.remove_addresses {
        for whitelist_address in remove_whitelist_addrs.clone().into_iter() {
            WHITELIST_ADDRS.remove(deps.storage, whitelist_address);
        }
        num_whitelist_addresses -= remove_whitelist_addrs.len() as u32;
    }

    NUM_WHITELIST_ADDRS.save(deps.storage, &num_whitelist_addresses)?;

    Ok(Response::new().add_attribute("action", "update_whitelist"))
}

pub fn execute_update_whitelist_expiration(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    whitelist_expiration: Expiration,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }

    config.whitelist_expiration = Some(whitelist_expiration);
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_whitelist_expiration"))
}

pub fn execute_update_start_time(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    start_time: Expiration,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    config.start_time = Some(start_time);
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_start_time"))
}

pub fn execute_update_per_address_limit(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    per_address_limit: u64,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    if per_address_limit > MAX_PER_ADDRESS_LIMIT {
        return Err(ContractError::InvalidPerAddressLimit {
            max: MAX_PER_ADDRESS_LIMIT.to_string(),
            got: per_address_limit.to_string(),
        });
    }
    config.per_address_limit = Some(per_address_limit);
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_per_address_limit"))
}

pub fn execute_update_batch_mint_limit(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    batch_mint_limit: u64,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;
    if info.sender != config.admin {
        return Err(ContractError::Unauthorized {});
    }
    if batch_mint_limit > MAX_BATCH_MINT_LIMIT {
        return Err(ContractError::InvalidBatchMintLimit {
            max: MAX_BATCH_MINT_LIMIT.to_string(),
            got: batch_mint_limit.to_string(),
        });
    }
    config.batch_mint_limit = Some(batch_mint_limit);
    CONFIG.save(deps.storage, &config)?;
    Ok(Response::new().add_attribute("action", "update_batch_mint_limit"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::WhitelistAddresses {} => to_binary(&query_whitelist_addresses(deps)?),
        QueryMsg::WhitelistExpiration {} => to_binary(&query_whitelist_expiration(deps)?),
        QueryMsg::StartTime {} => to_binary(&query_start_time(deps)?),
        QueryMsg::OnWhitelist { address } => to_binary(&query_on_whitelist(deps, address)?),
        QueryMsg::MintableNumTokens {} => to_binary(&query_mintable_num_tokens(deps)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let sg721_address = SG721_ADDRESS.load(deps.storage)?;

    Ok(ConfigResponse {
        admin: config.admin,
        base_token_uri: config.base_token_uri,
        sg721_address,
        sg721_code_id: config.sg721_code_id,
        num_tokens: config.num_tokens,
        unit_price: config.unit_price,
        per_address_limit: config.per_address_limit,
        batch_mint_limit: config.batch_mint_limit,
    })
}

fn query_whitelist_addresses(deps: Deps) -> StdResult<WhitelistAddressesResponse> {
    let addrs: StdResult<Vec<String>> = WHITELIST_ADDRS
        .keys(deps.storage, None, None, Order::Ascending)
        .take_while(|x| x.is_ok())
        .collect::<StdResult<Vec<String>>>();
    Ok(WhitelistAddressesResponse { addresses: addrs? })
}

fn query_whitelist_expiration(deps: Deps) -> StdResult<WhitelistExpirationResponse> {
    let config = CONFIG.load(deps.storage)?;
    if let Some(expiration) = config.whitelist_expiration {
        Ok(WhitelistExpirationResponse {
            expiration_time: expiration.to_string(),
        })
    } else {
        Err(StdError::GenericErr {
            msg: "whitelist expiration not found".to_string(),
        })
    }
}

fn query_start_time(deps: Deps) -> StdResult<StartTimeResponse> {
    let config = CONFIG.load(deps.storage)?;
    if let Some(expiration) = config.start_time {
        Ok(StartTimeResponse {
            start_time: expiration.to_string(),
        })
    } else {
        Err(StdError::GenericErr {
            msg: "start time not found".to_string(),
        })
    }
}

fn query_on_whitelist(deps: Deps, address: String) -> StdResult<OnWhitelistResponse> {
    let allowlist = WHITELIST_ADDRS.has(deps.storage, address);
    Ok(OnWhitelistResponse {
        on_whitelist: allowlist,
    })
}

fn query_mintable_num_tokens(deps: Deps) -> StdResult<MintableNumTokensResponse> {
    let count = MINTABLE_TOKEN_IDS
        .keys(deps.storage, None, None, Order::Ascending)
        .count();
    Ok(MintableNumTokensResponse {
        count: count as u64,
    })
}
// Reply callback triggered from cw721 contract instantiation
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> Result<Response, ContractError> {
    if msg.id != INSTANTIATE_SG721_REPLY_ID {
        return Err(ContractError::InvalidReplyID {});
    }

    let reply = parse_reply_instantiate_data(msg);
    match reply {
        Ok(res) => {
            SG721_ADDRESS.save(deps.storage, &Addr::unchecked(res.contract_address))?;
            Ok(Response::default().add_attribute("action", "instantiated sg721"))
        }
        Err(_) => Err(ContractError::InstantiateSg721Error {}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{mock_dependencies_with_balance, mock_env, mock_info};
    use cosmwasm_std::{coin, coins, Decimal, Timestamp};
    use cw721::{Cw721QueryMsg, OwnerOfResponse};
    use cw_multi_test::{App, BankSudo, Contract, ContractWrapper, Executor, SudoMsg};
    use sg721::state::{Config, RoyaltyInfo};

    const DENOM: &str = "ustars";
    const CREATION_FEE: u128 = 1_000_000_000;
    const INITIAL_BALANCE: u128 = 2000;
    const PRICE: u128 = 10;

    fn mock_app() -> App {
        App::default()
    }

    pub fn contract_minter() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            crate::contract::execute,
            crate::contract::instantiate,
            crate::contract::query,
        )
        .with_reply(crate::contract::reply);
        Box::new(contract)
    }

    pub fn contract_sg721() -> Box<dyn Contract<Empty>> {
        let contract = ContractWrapper::new(
            sg721::contract::execute,
            sg721::contract::instantiate,
            sg721::contract::query,
        );
        Box::new(contract)
    }

    // Upload contract code and instantiate sale contract
    fn setup_minter_contract(
        router: &mut App,
        creator: &Addr,
        num_tokens: u64,
    ) -> Result<(Addr, ConfigResponse), ContractError> {
        // Upload contract code
        let sg721_code_id = router.store_code(contract_sg721());
        let minter_code_id = router.store_code(contract_minter());
        let creation_fee = coins(CREATION_FEE, DENOM);

        // Instantiate sale contract
        let msg = InstantiateMsg {
            unit_price: coin(PRICE, DENOM),
            num_tokens,
            whitelist_expiration: None,
            whitelist_addresses: Some(vec![String::from("VIPcollector")]),
            start_time: None,
            per_address_limit: None,
            batch_mint_limit: None,
            base_token_uri: "ipfs://QmYxw1rURvnbQbBRTfmVaZtxSrkrfsbodNzibgBrVrUrtN".to_string(),
            sg721_code_id,
            sg721_instantiate_msg: Sg721InstantiateMsg {
                name: String::from("TEST"),
                symbol: String::from("TEST"),
                minter: creator.to_string(),
                config: Some(Config {
                    contract_uri: Some(String::from("test")),
                    creator: Some(creator.clone()),
                    royalties: Some(RoyaltyInfo {
                        payment_address: creator.clone(),
                        share: Decimal::percent(10),
                    }),
                }),
            },
        };
        let minter_addr = router
            .instantiate_contract(
                minter_code_id,
                creator.clone(),
                &msg,
                &creation_fee,
                "Minter",
                None,
            )
            .unwrap();

        let config: ConfigResponse = router
            .wrap()
            .query_wasm_smart(minter_addr.clone(), &QueryMsg::Config {})
            .unwrap();

        Ok((minter_addr, config))
    }

    // Add a creator account with initial balances
    fn setup_accounts(router: &mut App) -> Result<(Addr, Addr), ContractError> {
        let buyer = Addr::unchecked("buyer");
        let creator = Addr::unchecked("creator");
        let creator_funds = coins(INITIAL_BALANCE + CREATION_FEE, DENOM);
        let buyer_funds = coins(INITIAL_BALANCE, DENOM);
        router
            .sudo(SudoMsg::Bank({
                BankSudo::Mint {
                    to_address: creator.to_string(),
                    amount: creator_funds.clone(),
                }
            }))
            .map_err(|err| println!("{:?}", err))
            .ok();

        router
            .sudo(SudoMsg::Bank({
                BankSudo::Mint {
                    to_address: buyer.to_string(),
                    amount: buyer_funds.clone(),
                }
            }))
            .map_err(|err| println!("{:?}", err))
            .ok();

        // Check native balances
        let creator_native_balances = router.wrap().query_all_balances(creator.clone()).unwrap();
        assert_eq!(creator_native_balances, creator_funds);

        // Check native balances
        let buyer_native_balances = router.wrap().query_all_balances(buyer.clone()).unwrap();
        assert_eq!(buyer_native_balances, buyer_funds);

        Ok((creator, buyer))
    }

    #[test]
    fn initialization() {
        let mut deps = mock_dependencies_with_balance(&coins(2, "token"));

        // Invalid uri returns error
        let info = mock_info("creator", &coins(INITIAL_BALANCE, DENOM));
        let msg = InstantiateMsg {
            unit_price: coin(PRICE, DENOM),
            num_tokens: 100,
            whitelist_expiration: None,
            whitelist_addresses: Some(vec![String::from("VIPcollector")]),
            start_time: None,
            per_address_limit: None,
            batch_mint_limit: None,
            base_token_uri: "https://QmYxw1rURvnbQbBRTfmVaZtxSrkrfsbodNzibgBrVrUrtN".to_string(),
            sg721_code_id: 1,
            sg721_instantiate_msg: Sg721InstantiateMsg {
                name: String::from("TEST"),
                symbol: String::from("TEST"),
                minter: info.sender.to_string(),
                config: Some(Config {
                    contract_uri: Some(String::from("test")),
                    creator: Some(info.sender.clone()),
                    royalties: Some(RoyaltyInfo {
                        payment_address: info.sender.clone(),
                        share: Decimal::percent(10),
                    }),
                }),
            },
        };
        let res = instantiate(deps.as_mut(), mock_env(), info, msg);
        assert!(res.is_err());
    }

    #[test]
    fn happy_path() {
        let mut router = mock_app();
        let (creator, buyer) = setup_accounts(&mut router).unwrap();
        let num_tokens: u64 = 2;
        let (minter_addr, config) =
            setup_minter_contract(&mut router, &creator, num_tokens).unwrap();

        // Succeeds if funds are sent
        let mint_msg = ExecuteMsg::Mint {};
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());

        // Balances are correct
        let creator_native_balances = router.wrap().query_all_balances(creator.clone()).unwrap();
        assert_eq!(
            creator_native_balances,
            coins(INITIAL_BALANCE + PRICE, DENOM)
        );
        let buyer_native_balances = router.wrap().query_all_balances(buyer.clone()).unwrap();
        assert_eq!(buyer_native_balances, coins(INITIAL_BALANCE - PRICE, DENOM));

        // Check NFT is transferred
        let query_owner_msg = Cw721QueryMsg::OwnerOf {
            token_id: String::from("0"),
            include_expired: None,
        };
        let res: OwnerOfResponse = router
            .wrap()
            .query_wasm_smart(config.sg721_address.clone(), &query_owner_msg)
            .unwrap();
        assert_eq!(res.owner, buyer.to_string());

        // Buyer can't call MintTo
        let mint_to_msg = ExecuteMsg::MintTo {
            recipient: buyer.clone(),
        };
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_to_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());

        // Creator mints an extra NFT for the buyer (who is a friend)
        let res = router.execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &mint_to_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());

        // Check that NFT is transferred
        let query_owner_msg = Cw721QueryMsg::OwnerOf {
            token_id: String::from("1"),
            include_expired: None,
        };
        let res: OwnerOfResponse = router
            .wrap()
            .query_wasm_smart(config.sg721_address, &query_owner_msg)
            .unwrap();
        assert_eq!(res.owner, buyer.to_string());

        // Errors if sold out
        let mint_msg = ExecuteMsg::Mint {};
        let res =
            router.execute_contract(buyer, minter_addr.clone(), &mint_msg, &coins(PRICE, DENOM));
        assert!(res.is_err());

        // Creator can't use MintFor if sold out
        let res = router.execute_contract(creator, minter_addr, &mint_to_msg, &coins(PRICE, DENOM));
        assert!(res.is_err());
    }

    #[test]
    fn whitelist_access_len_add_remove_expiration() {
        let mut router = mock_app();
        let (creator, buyer) = setup_accounts(&mut router).unwrap();
        let num_tokens: u64 = 1;
        let (minter_addr, _config) =
            setup_minter_contract(&mut router, &creator, num_tokens).unwrap();
        const EXPIRATION_TIME: Timestamp = Timestamp::from_seconds(100000 + 10);

        // set block info
        let mut block = router.block_info();
        block.time = Timestamp::from_seconds(100000);
        router.set_block(block);

        // update whitelist_expiration fails if not admin
        let whitelist_msg = ExecuteMsg::UpdateWhitelistExpiration(Expiration::Never {});
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &whitelist_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());

        // enable whitelist
        let whitelist_msg =
            ExecuteMsg::UpdateWhitelistExpiration(Expiration::AtTime(EXPIRATION_TIME));
        let res = router.execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &whitelist_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());

        // mint fails, buyer is not on whitelist
        let mint_msg = ExecuteMsg::Mint {};
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());

        // fails, add too many whitelist addresses
        let over_max_limit_whitelist_addrs =
            vec!["addr".to_string(); MAX_WHITELIST_ADDRS_LENGTH as usize + 10];
        let whitelist: Option<Vec<String>> = Some(over_max_limit_whitelist_addrs);
        let add_whitelist_msg = UpdateWhitelistMsg {
            add_addresses: whitelist,
            remove_addresses: None,
        };
        let update_whitelist_msg = ExecuteMsg::UpdateWhitelist(add_whitelist_msg);
        let res = router.execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &update_whitelist_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());

        // add buyer to whitelist
        let whitelist: Option<Vec<String>> = Some(vec![buyer.clone().into_string()]);
        let add_whitelist_msg = UpdateWhitelistMsg {
            add_addresses: whitelist,
            remove_addresses: None,
        };
        let update_whitelist_msg = ExecuteMsg::UpdateWhitelist(add_whitelist_msg);
        let res = router.execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &update_whitelist_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());

        // query whitelist, confirm buyer on allowlist
        let allowlist: OnWhitelistResponse = router
            .wrap()
            .query_wasm_smart(
                minter_addr.clone(),
                &QueryMsg::OnWhitelist {
                    address: "buyer".to_string(),
                },
            )
            .unwrap();
        assert!(allowlist.on_whitelist);

        // query whitelist_expiration, confirm not expired
        let expiration: WhitelistExpirationResponse = router
            .wrap()
            .query_wasm_smart(minter_addr.clone(), &QueryMsg::WhitelistExpiration {})
            .unwrap();
        assert_eq!(
            "expiration time: ".to_owned() + &EXPIRATION_TIME.to_string(),
            expiration.expiration_time
        );

        // mint succeeds
        let mint_msg = ExecuteMsg::Mint {};
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());

        // remove buyer from whitelist
        let remove_whitelist: Option<Vec<String>> = Some(vec![buyer.clone().into_string()]);
        let remove_whitelist_msg = UpdateWhitelistMsg {
            add_addresses: None,
            remove_addresses: remove_whitelist,
        };
        let update_whitelist_msg = ExecuteMsg::UpdateWhitelist(remove_whitelist_msg);
        let res = router.execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &update_whitelist_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());

        // mint fails
        let mint_msg = ExecuteMsg::Mint {};
        let res = router.execute_contract(buyer, minter_addr, &mint_msg, &coins(PRICE, DENOM));
        assert!(res.is_err());
    }

    #[test]
    fn before_start_time() {
        let mut router = mock_app();
        let (creator, buyer) = setup_accounts(&mut router).unwrap();
        let num_tokens: u64 = 1;
        let (minter_addr, _config) =
            setup_minter_contract(&mut router, &creator, num_tokens).unwrap();
        const START_TIME: Timestamp = Timestamp::from_seconds(100000 + 10);

        // set block info
        let mut block = router.block_info();
        block.time = Timestamp::from_seconds(100000);
        router.set_block(block);

        // set start_time fails if not admin
        let start_time_msg = ExecuteMsg::UpdateStartTime(Expiration::Never {});
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &start_time_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());

        // if block before start_time, throw error
        let start_time_msg = ExecuteMsg::UpdateStartTime(Expiration::AtTime(START_TIME));
        let res = router.execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &start_time_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());

        let mint_msg = ExecuteMsg::Mint {};
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());

        // query start_time, confirm expired
        let start_time_response: StartTimeResponse = router
            .wrap()
            .query_wasm_smart(minter_addr.clone(), &QueryMsg::StartTime {})
            .unwrap();
        assert_eq!(
            "expiration time: ".to_owned() + &START_TIME.to_string(),
            start_time_response.start_time
        );

        // set block forward, after start time. mint succeeds
        let mut block = router.block_info();
        block.time = START_TIME.plus_seconds(10);
        router.set_block(block);

        // mint succeeds
        let mint_msg = ExecuteMsg::Mint {};
        let res = router.execute_contract(buyer, minter_addr, &mint_msg, &coins(PRICE, DENOM));
        assert!(res.is_ok());
    }

    #[test]
    fn check_per_address_limit() {
        let mut router = mock_app();
        let (creator, buyer) = setup_accounts(&mut router).unwrap();
        let num_tokens = 2;
        let (minter_addr, _config) =
            setup_minter_contract(&mut router, &creator, num_tokens).unwrap();

        // set limit, check unauthorized
        let per_address_limit_msg = ExecuteMsg::UpdatePerAddressLimit {
            per_address_limit: 30,
        };
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &per_address_limit_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());

        // set limit, invalid limit over max
        let per_address_limit_msg = ExecuteMsg::UpdatePerAddressLimit {
            per_address_limit: 100,
        };
        let res = router.execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &per_address_limit_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());

        // set limit, mint fails, over max
        let per_address_limit_msg = ExecuteMsg::UpdatePerAddressLimit {
            per_address_limit: 1,
        };
        let res = router.execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &per_address_limit_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());

        // first mint succeeds
        let mint_msg = ExecuteMsg::Mint {};
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());

        // second mint fails from exceeding per address limit
        let mint_msg = ExecuteMsg::Mint {};
        let res = router.execute_contract(buyer, minter_addr, &mint_msg, &coins(PRICE, DENOM));
        assert!(res.is_err());
    }

    #[test]
    fn batch_mint_limit_access_max_sold_out() {
        let mut router = mock_app();
        let (creator, buyer) = setup_accounts(&mut router).unwrap();
        let num_tokens = 4;
        let (minter_addr, _config) =
            setup_minter_contract(&mut router, &creator, num_tokens).unwrap();

        // batch mint limit set to STARTING_BATCH_MINT_LIMIT if no mint provided
        let batch_mint_msg = ExecuteMsg::BatchMint { num_mints: 1 };
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &batch_mint_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());

        // update batch mint limit, test unauthorized
        let update_batch_mint_limit_msg = ExecuteMsg::UpdateBatchMintLimit {
            batch_mint_limit: 1,
        };
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &update_batch_mint_limit_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(ContractError::Unauthorized {}.to_string(), err.to_string());

        // update limit, invalid limit over max
        let update_batch_mint_limit_msg = ExecuteMsg::UpdateBatchMintLimit {
            batch_mint_limit: 100,
        };
        let res = router.execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &update_batch_mint_limit_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(
            ContractError::InvalidBatchMintLimit {
                max: 30.to_string(),
                got: 100.to_string()
            }
            .to_string(),
            err.to_string()
        );

        // update limit successfully as admin
        let update_batch_mint_limit_msg = ExecuteMsg::UpdateBatchMintLimit {
            batch_mint_limit: 2,
        };
        let res = router.execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &update_batch_mint_limit_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());

        // test over max batch mint limit
        let batch_mint_msg = ExecuteMsg::BatchMint { num_mints: 50 };
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &batch_mint_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(
            ContractError::MaxBatchMintLimitExceeded {}.to_string(),
            err.to_string()
        );

        // success
        let batch_mint_msg = ExecuteMsg::BatchMint { num_mints: 2 };
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &batch_mint_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());

        // test sold out and fails
        let batch_mint_msg = ExecuteMsg::BatchMint { num_mints: 2 };
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &batch_mint_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(ContractError::SoldOut {}.to_string(), err.to_string());

        // batch mint smaller amount
        let batch_mint_msg = ExecuteMsg::BatchMint { num_mints: 1 };
        let res =
            router.execute_contract(buyer, minter_addr, &batch_mint_msg, &coins(PRICE, DENOM));
        assert!(res.is_ok());
    }

    #[test]
    fn mint_for_token_id_addr() {
        let mut router = mock_app();
        let (creator, buyer) = setup_accounts(&mut router).unwrap();
        let num_tokens: u64 = 4;
        let (minter_addr, _config) =
            setup_minter_contract(&mut router, &creator, num_tokens).unwrap();

        // try mint_for, test unauthorized
        let mint_for_msg = ExecuteMsg::MintFor {
            token_id: 1,
            recipient: buyer.clone(),
        };
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_for_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(ContractError::Unauthorized {}.to_string(), err.to_string());

        // test token id already sold
        // 1. mint token_id 0
        // 2. mint_for token_id 0
        let mint_msg = ExecuteMsg::Mint {};
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());

        let token_id = 0;
        let mint_for_msg = ExecuteMsg::MintFor {
            token_id,
            recipient: buyer.clone(),
        };
        let res = router.execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &mint_for_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_err());
        let err = res.unwrap_err();
        assert_eq!(
            ContractError::TokenIdAlreadySold { token_id }.to_string(),
            err.to_string()
        );
        let mintable_num_tokens_response: MintableNumTokensResponse = router
            .wrap()
            .query_wasm_smart(minter_addr.clone(), &QueryMsg::MintableNumTokens {})
            .unwrap();
        assert_eq!(mintable_num_tokens_response.count, 3);

        // test mint_for token_id 2 then normal mint
        let token_id = 2;
        let mint_for_msg = ExecuteMsg::MintFor {
            token_id,
            recipient: buyer,
        };
        let res = router.execute_contract(
            creator.clone(),
            minter_addr.clone(),
            &mint_for_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());
        let batch_mint_msg = ExecuteMsg::BatchMint { num_mints: 2 };
        let res = router.execute_contract(
            creator,
            minter_addr.clone(),
            &batch_mint_msg,
            &coins(PRICE, DENOM),
        );
        assert!(res.is_ok());
        let mintable_num_tokens_response: MintableNumTokensResponse = router
            .wrap()
            .query_wasm_smart(minter_addr, &QueryMsg::MintableNumTokens {})
            .unwrap();
        assert_eq!(mintable_num_tokens_response.count, 0);
    }

    #[test]
    fn check_max_num_tokens() {
        let mut router = mock_app();
        let (creator, _) = setup_accounts(&mut router).unwrap();

        let over_max_num_tokens = MAX_TOKEN_LIMIT + 1;

        let sg721_code_id = router.store_code(contract_sg721());
        let minter_code_id = router.store_code(contract_minter());

        // Instantiate sale contract
        let msg = InstantiateMsg {
            unit_price: coin(PRICE, DENOM),
            num_tokens: over_max_num_tokens.into(),
            whitelist_expiration: None,
            whitelist_addresses: Some(vec![String::from("VIPcollector")]),
            start_time: None,
            per_address_limit: None,
            batch_mint_limit: None,
            base_token_uri: "ipfs://QmYxw1rURvnbQbBRTfmVaZtxSrkrfsbodNzibgBrVrUrtN".to_string(),
            sg721_code_id,
            sg721_instantiate_msg: Sg721InstantiateMsg {
                name: String::from("TEST"),
                symbol: String::from("TEST"),
                minter: creator.to_string(),
                config: Some(Config {
                    contract_uri: Some(String::from("test")),
                    creator: Some(creator.clone()),
                    royalties: Some(RoyaltyInfo {
                        payment_address: creator.clone(),
                        share: Decimal::percent(10),
                    }),
                }),
            },
        };
        let res = router.instantiate_contract(minter_code_id, creator, &msg, &[], "Minter", None);

        // setup_minter_contract(&mut router.branch(), &creator, over_max_num_tokens.into());
        assert!(res.is_err());
        assert_eq!(
            ContractError::MaxTokenLimitExceeded {
                max: MAX_TOKEN_LIMIT
            }
            .to_string(),
            res.unwrap_err().to_string()
        );
    }

    #[test]
    fn unhappy_path() {
        let mut router = mock_app();
        let (creator, buyer) = setup_accounts(&mut router).unwrap();
        let num_tokens: u64 = 1;
        let (minter_addr, _config) =
            setup_minter_contract(&mut router, &creator, num_tokens).unwrap();

        // Fails if too little funds are sent
        let mint_msg = ExecuteMsg::Mint {};
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_msg,
            &coins(1, DENOM),
        );
        assert!(res.is_err());

        // Fails if too many funds are sent
        let mint_msg = ExecuteMsg::Mint {};
        let res = router.execute_contract(
            buyer.clone(),
            minter_addr.clone(),
            &mint_msg,
            &coins(11111, DENOM),
        );
        assert!(res.is_err());

        // Fails wrong denom is sent
        let mint_msg = ExecuteMsg::Mint {};
        let res = router.execute_contract(buyer, minter_addr, &mint_msg, &coins(PRICE, "uatom"));
        assert!(res.is_err());
    }
}