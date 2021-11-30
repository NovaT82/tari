// Copyright 2021. The Tari Project
//
// Redistribution and use in source and binary forms, with or without modification, are permitted provided that the
// following conditions are met:
//
// 1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following
// disclaimer.
//
// 2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the
// following disclaimer in the documentation and/or other materials provided with the distribution.
//
// 3. Neither the name of the copyright holder nor the names of its contributors may be used to endorse or promote
// products derived from this software without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES,
// INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY,
// WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE
// USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use crate::{
    digital_assets_error::DigitalAssetError,
    models::{Payload, StateRoot, TariDanPayload},
    services::AssetProcessor,
    storage::state::StateDbUnitOfWork,
};
use async_trait::async_trait;

use crate::{
    models::{AssetDefinition, TreeNodeHash},
    services::MempoolService,
};
use tari_core::transactions::transaction::TemplateParameter;

#[async_trait]
pub trait PayloadProcessor<TPayload: Payload> {
    fn init_template<TUnitOfWork: StateDbUnitOfWork>(
        &self,
        template_parameter: &TemplateParameter,
        asset_definition: &AssetDefinition,
        state_db: &mut TUnitOfWork,
    ) -> Result<(), DigitalAssetError>;
    async fn process_payload<TUnitOfWork: StateDbUnitOfWork>(
        &mut self,
        node_hash: TreeNodeHash,
        payload: &TPayload,
        unit_of_work: TUnitOfWork,
    ) -> Result<StateRoot, DigitalAssetError>;
    async fn remove_payload_for_node(&mut self, node_hash: &TreeNodeHash) -> Result<(), DigitalAssetError>;
}

pub struct TariDanPayloadProcessor<TAssetProcessor, TMempoolService>
where
    TAssetProcessor: AssetProcessor,
    TMempoolService: MempoolService,
{
    mempool: TMempoolService,
    asset_processor: TAssetProcessor,
}

impl<TAssetProcessor: AssetProcessor, TMempoolService: MempoolService>
    TariDanPayloadProcessor<TAssetProcessor, TMempoolService>
{
    pub fn new(asset_processor: TAssetProcessor, mempool: TMempoolService) -> Self {
        Self {
            asset_processor,
            mempool,
        }
    }
}

#[async_trait]
impl<TAssetProcessor: AssetProcessor + Send + Sync, TMempool: MempoolService + Send + Sync>
    PayloadProcessor<TariDanPayload> for TariDanPayloadProcessor<TAssetProcessor, TMempool>
{
    fn init_template<TUnitOfWork: StateDbUnitOfWork>(
        &self,
        template_parameter: &TemplateParameter,
        asset_definition: &AssetDefinition,
        state_db: &mut TUnitOfWork,
    ) -> Result<(), DigitalAssetError> {
        self.asset_processor
            .init_template(template_parameter, asset_definition, state_db)
    }

    async fn process_payload<TUnitOfWork: StateDbUnitOfWork + Clone + Send>(
        &mut self,
        node_hash: TreeNodeHash,
        payload: &TariDanPayload,
        state_tx: TUnitOfWork,
    ) -> Result<StateRoot, DigitalAssetError> {
        let mut state_tx = state_tx;
        for instruction in payload.instructions() {
            dbg!("Executing instruction");
            dbg!(&instruction);
            // TODO: Should we swallow + log the error instead of propagating it?
            self.asset_processor.execute_instruction(instruction, &mut state_tx)?;
        }
        // Reserve all instructions if they succeeded
        for instruction in payload.instructions() {
            self.mempool
                .reserve_instruction_in_block(instruction.hash(), node_hash.0.clone())
                .await?;
        }

        Ok(state_tx.calculate_root()?)
    }

    async fn remove_payload_for_node(&mut self, node_hash: &TreeNodeHash) -> Result<(), DigitalAssetError> {
        self.mempool.remove_all_in_block(node_hash.as_bytes()).await
    }
}