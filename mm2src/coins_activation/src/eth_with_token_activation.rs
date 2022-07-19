use std::collections::HashMap;

use async_trait::async_trait;
use coins::{coin_conf,
            eth::{eth_coin_from_conf_and_request_v2, valid_addr_from_str, EthActivationRequest, EthActivationV2Error,
                  EthCoin, EthCoinType},
            my_tx_history_v2::TxHistoryStorage,
            CoinBalance, CoinProtocol, MarketCoinOps};
use common::{log::info, mm_metrics::MetricsArc, Future01CompatExt};
use futures::future::AbortHandle;
use mm2_core::mm_ctx::MmArc;
use mm2_err_handle::prelude::*;
use mm2_number::BigDecimal;
use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::{platform_coin_with_tokens::{EnablePlatformCoinWithTokensError, GetPlatformBalance,
                                        PlatformWithTokensActivationOps, TokenAsMmCoinInitializer},
            prelude::*};

pub struct Erc20Initializer {
    platform_coin: EthCoin,
}

pub struct EthProtocolInfo {
    coin_type: EthCoinType,
    decimals: u8,
}

impl TryFromCoinProtocol for EthProtocolInfo {
    fn try_from_coin_protocol(proto: CoinProtocol) -> Result<Self, MmError<CoinProtocol>>
    where
        Self: Sized,
    {
        match proto {
            CoinProtocol::ETH => Ok(EthProtocolInfo {
                coin_type: EthCoinType::Eth,
                decimals: 18,
            }),
            protocol => MmError::err(protocol),
        }
    }
}

impl From<EthActivationV2Error> for EnablePlatformCoinWithTokensError {
    fn from(err: EthActivationV2Error) -> Self {
        // match err {
        //     BchWithTokensActivationError::PlatformCoinCreationError { ticker, error } => {
        //         EnablePlatformCoinWithTokensError::PlatformCoinCreationError { ticker, error }
        //     },
        //     BchWithTokensActivationError::InvalidSlpPrefix { ticker, prefix, error } => {
        //         EnablePlatformCoinWithTokensError::Internal(format!(
        //             "Invalid slp prefix {} configured for {}. Error: {}",
        //             prefix, ticker, error
        //         ))
        //     },
        //     BchWithTokensActivationError::PrivKeyNotAllowed(e) => {
        //         EnablePlatformCoinWithTokensError::PrivKeyNotAllowed(e)
        //     },
        //     BchWithTokensActivationError::UnexpectedDerivationMethod(e) => {
        //         EnablePlatformCoinWithTokensError::UnexpectedDerivationMethod(e)
        //     },
        //     BchWithTokensActivationError::Transport(e) => EnablePlatformCoinWithTokensError::Transport(e),
        //     BchWithTokensActivationError::Internal(e) => EnablePlatformCoinWithTokensError::Internal(e),
        // }
        match err {
            EthActivationV2Error::InvalidPayload(e) => EnablePlatformCoinWithTokensError::InvalidPayload(e),
            EthActivationV2Error::ActivationFailed { ticker, error } => {
                EnablePlatformCoinWithTokensError::PlatformCoinCreationError { ticker, error }
            },
            EthActivationV2Error::CouldNotFetchBalance(e)
            | EthActivationV2Error::UnreachableNodes(e)
            | EthActivationV2Error::AtLeastOneNodeRequired(e) => EnablePlatformCoinWithTokensError::Transport(e),
            EthActivationV2Error::InternalError(e) => EnablePlatformCoinWithTokensError::Internal(e),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct EthWithTokensActivationRequest {
    #[serde(flatten)]
    platform_request: EthActivationRequest,
    erc20_tokens_requests: Vec<
        crate::platform_coin_with_tokens::TokenActivationRequest<crate::slp_token_activation::SlpActivationRequest>,
    >,
}

impl TxHistory for EthWithTokensActivationRequest {
    fn tx_history(&self) -> bool { self.platform_request.tx_history.unwrap_or(false) }
}

#[derive(Debug, Serialize)]
pub struct EthWithTokensActivationResult {
    current_block: u64,
    eth_addresses_infos: HashMap<String, CoinAddressInfo<CoinBalance>>,
    erc20_addresses_infos: HashMap<String, CoinAddressInfo<TokenBalances>>,
}

impl GetPlatformBalance for EthWithTokensActivationResult {
    fn get_platform_balance(&self) -> BigDecimal {
        self.eth_addresses_infos
            .iter()
            .fold(BigDecimal::from(0), |total, (_, addr_info)| {
                &total + &addr_info.balances.get_total()
            })
    }
}

impl CurrentBlock for EthWithTokensActivationResult {
    fn current_block(&self) -> u64 { self.current_block }
}

#[async_trait]
impl PlatformWithTokensActivationOps for EthCoin {
    type ActivationRequest = EthWithTokensActivationRequest;
    type PlatformProtocolInfo = EthProtocolInfo;
    type ActivationResult = EthWithTokensActivationResult;
    type ActivationError = EthActivationV2Error;

    async fn enable_platform_coin(
        ctx: MmArc,
        ticker: String,
        platform_conf: Json,
        activation_request: Self::ActivationRequest,
        _protocol_conf: Self::PlatformProtocolInfo,
        priv_key: &[u8],
    ) -> Result<Self, MmError<Self::ActivationError>> {
        let coins_en = coin_conf(&ctx, &ticker);

        let protocol: CoinProtocol = serde_json::from_value(coins_en["protocol"].clone()).map_err(|e| {
            Self::ActivationError::ActivationFailed {
                ticker: ticker.clone(),
                error: e.to_string(),
            }
        })?;

        let platform_coin = eth_coin_from_conf_and_request_v2(
            &ctx,
            &ticker,
            &platform_conf,
            activation_request.platform_request,
            priv_key,
            protocol,
        )
        .await?;

        Ok(platform_coin)
    }

    fn token_initializers(
        &self,
    ) -> Vec<Box<dyn TokenAsMmCoinInitializer<PlatformCoin = Self, ActivationRequest = Self::ActivationRequest>>> {
        vec![]

        // vec![Box::new(Erc20Initializer {
        //     platform_coin: self.clone(),
        // })]
    }

    async fn get_activation_result(&self) -> Result<EthWithTokensActivationResult, MmError<EthActivationV2Error>> {
        let my_address = self.my_address().map_err(EthActivationV2Error::InternalError)?;

        let current_block = self
            .current_block()
            .compat()
            .await
            .map_err(EthActivationV2Error::InternalError)?;

        // let bch_unspents = self.bch_unspents_for_display(my_address).await?;
        // let bch_balance = bch_unspents.platform_balance(self.decimals());

        // let mut token_balances = HashMap::new();
        // for (token_ticker, info) in self.get_slp_tokens_infos().iter() {
        //     let token_balance = bch_unspents.slp_token_balance(&info.token_id, info.decimals);
        //     token_balances.insert(token_ticker.clone(), token_balance);
        // }

        let mut result = EthWithTokensActivationResult {
            current_block,
            eth_addresses_infos: HashMap::new(),
            erc20_addresses_infos: HashMap::new(),
        };

        // result
        //     .bch_addresses_infos
        //     .insert(my_address.to_string(), CoinAddressInfo {
        //         derivation_method: DerivationMethod::Iguana,
        //         pubkey: self.my_public_key()?.to_string(),
        //         balances: bch_balance,
        //     });

        // result.slp_addresses_infos.insert(my_slp_address, CoinAddressInfo {
        //     derivation_method: DerivationMethod::Iguana,
        //     pubkey: self.my_public_key()?.to_string(),
        //     balances: token_balances,
        // });
        Ok(result)
    }

    fn start_history_background_fetching(
        &self,
        _metrics: MetricsArc,
        _storage: impl TxHistoryStorage + Send + 'static,
        _initial_balance: BigDecimal,
    ) -> AbortHandle {
        todo!()
    }
}