#[cfg(test)]
mod tests {
    use crate::msg::InstantiateMsg;
    use crate::state::ParamsExtension;
    use crate::{helpers::FactoryContract, state::VendingMinterParams};
    use cosmwasm_std::{coin, Addr};
    use cw_multi_test::{Contract, ContractWrapper, Executor};
    use sg_multi_test::StargazeApp;
    use sg_std::StargazeMsgWrapper;

    pub fn factory_contract() -> Box<dyn Contract<StargazeMsgWrapper>> {
        let contract = ContractWrapper::new(
            crate::contract::execute,
            crate::contract::instantiate,
            crate::contract::query,
        )
        .with_sudo(crate::contract::sudo);
        Box::new(contract)
    }

    const GOVERNANCE: &str = "governance";
    const NATIVE_DENOM: &str = "ustars";

    pub const CREATION_FEE: u128 = 0;
    pub const DYNAMIC_CREATION_FEE_THRESHOLD: u32 = 10_000;
    pub const CREATION_FEE_PER_TOKEN: u128 = 10_000;
    pub const MIN_MINT_PRICE: u128 = 50_000_000;
    pub const AIRDROP_MINT_PRICE: u128 = 15_000_000;
    pub const MINT_FEE_BPS: u64 = 1_000; // 10%
    pub const AIRDROP_MINT_FEE_BPS: u64 = 10_000; // 100%
    pub const MAX_PER_ADDRESS_LIMIT: u32 = 50;

    fn custom_mock_app() -> StargazeApp {
        StargazeApp::default()
    }

    pub fn mock_params() -> VendingMinterParams {
        VendingMinterParams {
            code_id: 1,
            creation_fee: coin(CREATION_FEE, NATIVE_DENOM),
            min_mint_price: coin(MIN_MINT_PRICE, NATIVE_DENOM),
            mint_fee_bps: MINT_FEE_BPS,
            max_trading_offset_secs: 60 * 60 * 24 * 7,
            extension: ParamsExtension {
                dynamic_creation_fee_threshold: DYNAMIC_CREATION_FEE_THRESHOLD,
                creation_fee_per_token: CREATION_FEE_PER_TOKEN,
                max_per_address_limit: MAX_PER_ADDRESS_LIMIT,
                airdrop_mint_price: coin(AIRDROP_MINT_PRICE, NATIVE_DENOM),
                airdrop_mint_fee_bps: AIRDROP_MINT_FEE_BPS,
            },
        }
    }

    fn proper_instantiate() -> (StargazeApp, FactoryContract) {
        let mut app = custom_mock_app();
        let factory_id = app.store_code(factory_contract());
        let minter_id = 2;

        let mut params = mock_params();
        params.code_id = minter_id;

        let factory_contract_addr = app
            .instantiate_contract(
                factory_id,
                Addr::unchecked(GOVERNANCE),
                &InstantiateMsg { params },
                &[],
                "factory",
                None,
            )
            .unwrap();

        (app, FactoryContract(factory_contract_addr))
    }

    mod init {
        use super::*;

        #[test]
        fn can_init() {
            let (_, factory_contract) = proper_instantiate();
            assert_eq!(factory_contract.addr().to_string(), "contract0");
        }
    }
}
