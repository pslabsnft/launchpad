use anyhow::{bail, Result as AnyResult};
use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
use cosmwasm_std::{BankMsg, OwnedDeps};
use std::fmt::Debug;
use std::marker::PhantomData;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use std::ops::{Deref, DerefMut};

use cosmwasm_std::{
    Addr, Api, Binary, BlockInfo, CustomQuery, Empty, Querier, QuerierResult, Storage,
};
use cw_multi_test::{
    App, AppResponse, BankKeeper, BasicAppBuilder, CosmosRouter, Module, WasmKeeper,
};

use sg_std::{StargazeMsgWrapper, StargazeQuery};

pub struct StargazeModule {}

pub type StargazeDeps = OwnedDeps<MockStorage, MockApi, MockQuerier, StargazeQuery>;

pub fn mock_deps() -> StargazeDeps {
    OwnedDeps {
        storage: MockStorage::default(),
        api: MockApi::default(),
        querier: MockQuerier::default(),
        custom_query_type: PhantomData,
    }
}

impl StargazeModule {}

impl Module for StargazeModule {
    type ExecT = StargazeMsgWrapper;
    type QueryT = Empty;
    type SudoT = Empty;

    fn execute<ExecC, QueryC>(
        &self,
        api: &dyn Api,
        storage: &mut dyn Storage,
        router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        block: &BlockInfo,
        sender: Addr,
        msg: StargazeMsgWrapper,
    ) -> AnyResult<AppResponse>
    where
        ExecC: Debug + Clone + PartialEq + JsonSchema + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        match msg {
            StargazeMsgWrapper {
                route: _,
                msg_data,
                version: _,
            } => match msg_data {
                sg_std::StargazeMsg::FundCommunityPool { amount } => {
                    let msg = BankMsg::Send {
                        to_address: "an_address".to_owned(),
                        amount: amount,
                    }
                    .into();
                    router.execute(api, storage, block, sender, msg)?;
                    Ok(AppResponse::default())
                }
                _ => {
                    bail!("not implemented")
                }
            },
        }
    }
    fn sudo<ExecC, QueryC>(
        &self,
        _api: &dyn Api,
        _storage: &mut dyn Storage,
        _router: &dyn CosmosRouter<ExecC = ExecC, QueryC = QueryC>,
        _block: &BlockInfo,
        _msg: Self::SudoT,
    ) -> AnyResult<AppResponse>
    where
        ExecC: Debug + Clone + PartialEq + JsonSchema + DeserializeOwned + 'static,
        QueryC: CustomQuery + DeserializeOwned + 'static,
    {
        bail!("sudo not implemented")
    }

    fn query(
        &self,
        _api: &dyn Api,
        storage: &dyn Storage,
        _querier: &dyn Querier,
        _block: &BlockInfo,
        request: Empty,
    ) -> anyhow::Result<Binary> {
        bail!("Unexpected custom query {:?}", request)
    }
}

pub type StargazeBasicApp =
    App<BankKeeper, MockApi, MockStorage, StargazeModule, WasmKeeper<StargazeMsgWrapper, Empty>>;

pub struct StargazeApp(StargazeBasicApp);

impl Deref for StargazeApp {
    type Target = StargazeBasicApp;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for StargazeApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Querier for StargazeApp {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        self.0.raw_query(bin_request)
    }
}

impl StargazeApp {
    pub fn new() -> Self {
        Self(
            BasicAppBuilder::<StargazeMsgWrapper, Empty>::new_custom()
                .with_custom(StargazeModule {})
                .build(|_, _, _| {}),
        )
    }
}
