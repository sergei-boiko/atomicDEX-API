use crate::context::CoinsActivationContext;
use crate::prelude::TryFromCoinProtocol;
use crate::standalone_coin::{InitStandaloneCoinActivationOps, InitStandaloneCoinTaskHandle,
                             InitStandaloneCoinTaskManagerShared};
use crate::utxo_activation::common_impl::{get_activation_result, priv_key_build_policy};
use crate::utxo_activation::init_utxo_standard_activation_error::InitUtxoStandardError;
use crate::utxo_activation::init_utxo_standard_statuses::{UtxoStandardAwaitingStatus, UtxoStandardInProgressStatus,
                                                          UtxoStandardUserAction};
use crate::utxo_activation::utxo_standard_activation_result::UtxoStandardActivationResult;
use async_trait::async_trait;
use coins::utxo::qtum::{QtumCoin, QtumCoinBuilder};
use coins::utxo::utxo_builder::UtxoCoinBuilder;
use coins::utxo::UtxoActivationParams;
use coins::CoinProtocol;
use crypto::CryptoCtx;
use mm2_core::mm_ctx::MmArc;
use mm2_err_handle::prelude::*;
use serde_json::Value as Json;

pub type QtumTaskManagerShared = InitStandaloneCoinTaskManagerShared<QtumCoin>;
pub type QtumRpcTaskHandle = InitStandaloneCoinTaskHandle<QtumCoin>;

pub struct QtumProtocolInfo;

impl TryFromCoinProtocol for QtumProtocolInfo {
    fn try_from_coin_protocol(proto: CoinProtocol) -> Result<Self, MmError<CoinProtocol>>
    where
        Self: Sized,
    {
        match proto {
            CoinProtocol::QTUM => Ok(QtumProtocolInfo),
            protocol => MmError::err(protocol),
        }
    }
}

#[async_trait]
impl InitStandaloneCoinActivationOps for QtumCoin {
    type ActivationRequest = UtxoActivationParams;
    type StandaloneProtocol = QtumProtocolInfo;
    type ActivationResult = UtxoStandardActivationResult;
    type ActivationError = InitUtxoStandardError;
    type InProgressStatus = UtxoStandardInProgressStatus;
    type AwaitingStatus = UtxoStandardAwaitingStatus;
    type UserAction = UtxoStandardUserAction;

    fn rpc_task_manager(activation_ctx: &CoinsActivationContext) -> &QtumTaskManagerShared {
        &activation_ctx.init_qtum_task_manager
    }

    async fn init_standalone_coin(
        ctx: MmArc,
        ticker: String,
        coin_conf: Json,
        activation_request: &Self::ActivationRequest,
        _protocol_info: Self::StandaloneProtocol,
        _task_handle: &QtumRpcTaskHandle,
    ) -> Result<Self, MmError<Self::ActivationError>> {
        let crypto_ctx = CryptoCtx::from_ctx(&ctx)?;
        let priv_key_policy = priv_key_build_policy(&crypto_ctx, activation_request.priv_key_policy);

        let coin = QtumCoinBuilder::new(&ctx, &ticker, &coin_conf, activation_request, priv_key_policy)
            .build()
            .await
            .mm_err(|e| InitUtxoStandardError::from_build_err(e, ticker.clone()))?;
        Ok(coin)
    }

    async fn get_activation_result(
        &self,
        ctx: MmArc,
        task_handle: &QtumRpcTaskHandle,
        activation_request: &Self::ActivationRequest,
    ) -> MmResult<Self::ActivationResult, InitUtxoStandardError> {
        get_activation_result(&ctx, self, task_handle, activation_request).await
    }
}
