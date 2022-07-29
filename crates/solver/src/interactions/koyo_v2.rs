use super::balancer_v2::{SwapKind, NEVER};
use crate::{encoding::EncodedInteraction, settlement::Interaction};
use contracts::{GPv2Settlement, KoyoV2Vault};
use ethcontract::{Bytes, H160, H256};
use primitive_types::U256;

#[derive(Clone, Debug)]
pub struct KoyoSwapGivenOutInteraction {
    pub settlement: GPv2Settlement,
    pub vault: KoyoV2Vault,
    pub pool_id: H256,
    pub asset_in: H160,
    pub asset_out: H160,
    pub amount_out: U256,
    pub amount_in_max: U256,
    pub user_data: Bytes<Vec<u8>>,
}

impl Interaction for KoyoSwapGivenOutInteraction {
    fn encode(&self) -> Vec<EncodedInteraction> {
        let method = self.vault.swap(
            (
                Bytes(self.pool_id.0),
                SwapKind::GivenOut as _,
                self.asset_in,
                self.asset_out,
                self.amount_out,
                self.user_data.clone(),
            ),
            (
                self.settlement.address(), // sender
                false,                     // fromInternalBalance
                self.settlement.address(), // recipient
                false,                     // toInternalBalance
            ),
            self.amount_in_max,
            *NEVER,
        );
        let calldata = method.tx.data.expect("no calldata").0;
        vec![(self.vault.address(), 0.into(), Bytes(calldata))]
    }
}
