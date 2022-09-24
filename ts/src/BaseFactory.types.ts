/**
* This file was automatically generated by @cosmwasm/ts-codegen@0.16.5.
* DO NOT MODIFY IT BY HAND. Instead, modify the source JSONSchema file,
* and run the @cosmwasm/ts-codegen generate command to regenerate this file.
*/

export type Uint128 = string;
export interface InstantiateMsg {
  params: MinterParamsForNullable_Empty;
  [k: string]: unknown;
}
export interface MinterParamsForNullable_Empty {
  code_id: number;
  creation_fee: Coin;
  extension?: Empty | null;
  max_trading_offset_secs: number;
  min_mint_price: Coin;
  mint_fee_bps: number;
  [k: string]: unknown;
}
export interface Coin {
  amount: Uint128;
  denom: string;
  [k: string]: unknown;
}
export interface Empty {
  [k: string]: unknown;
}
export type Sg2QueryMsg = {
  params: {
    [k: string]: unknown;
  };
};