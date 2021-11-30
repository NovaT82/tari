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

use crate::{digital_assets_error::DigitalAssetError, models::Instruction};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::Mutex;

#[async_trait]
pub trait MempoolService: Sync + Send + 'static {
    async fn submit_instruction(&mut self, instruction: Instruction) -> Result<(), DigitalAssetError>;
    async fn read_block(&self, limit: usize) -> Result<Vec<Instruction>, DigitalAssetError>;
    async fn reserve_instruction_in_block(
        &mut self,
        instruction_hash: &[u8],
        block_hash: Vec<u8>,
    ) -> Result<(), DigitalAssetError>;
    async fn remove_all_in_block(&mut self, block_hash: &[u8]) -> Result<(), DigitalAssetError>;
    async fn release_reservations(&mut self, block_hash: &[u8]) -> Result<(), DigitalAssetError>;
    async fn size(&self) -> usize;
}

#[derive(Default)]
pub struct ConcreteMempoolService {
    instructions: Vec<(Instruction, Option<Vec<u8>>)>,
}

#[async_trait]
impl MempoolService for ConcreteMempoolService {
    async fn submit_instruction(&mut self, instruction: Instruction) -> Result<(), DigitalAssetError> {
        self.instructions.push((instruction, None));
        Ok(())
    }

    async fn read_block(&self, limit: usize) -> Result<Vec<Instruction>, DigitalAssetError> {
        let mut result = vec![];
        for (i, instruction) in self.instructions.iter().enumerate() {
            if i > limit {
                break;
            }
            if instruction.1.is_none() {
                result.push(instruction.0.clone());
            }
        }
        Ok(result)
    }

    async fn reserve_instruction_in_block(
        &mut self,
        instruction_hash: &[u8],
        block_hash: Vec<u8>,
    ) -> Result<(), DigitalAssetError> {
        for mut instruction in self.instructions.iter_mut() {
            if instruction.0.hash() == instruction_hash {
                instruction.1 = Some(block_hash);
                break;
            }
        }

        Ok(())
    }

    async fn remove_all_in_block(&mut self, block_hash: &[u8]) -> Result<(), DigitalAssetError> {
        let mut new_instructions = Vec::with_capacity(self.instructions.len());
        for instruction in self.instructions.drain(..) {
            if instruction.1.as_ref().map(|i| i.as_slice()) != Some(block_hash) {
                new_instructions.push(instruction)
            }
        }
        self.instructions = new_instructions;
        Ok(())
    }

    async fn release_reservations(&mut self, block_hash: &[u8]) -> Result<(), DigitalAssetError> {
        for mut instruction in self.instructions.iter_mut() {
            if instruction.1.as_ref().map(|i| i.as_slice()) == Some(block_hash) {
                instruction.1 = None;
            }
        }
        Ok(())
    }

    // async fn remove_instructions(&mut self, instructions: &[Instruction]) -> Result<(), DigitalAssetError> {
    //     let mut result = self.instructions.clone();
    //     for i in instructions {
    //         if let Some(position) = result.iter().position(|r| r == i) {
    //             result.remove(position);
    //         }
    //     }
    //     self.instructions = result;
    //     Ok(())
    // }

    async fn size(&self) -> usize {
        self.instructions
            .iter()
            .fold(0, |a, b| if b.1.is_none() { a + 1 } else { a })
    }
}

#[derive(Clone)]
pub struct MempoolServiceHandle {
    mempool: Arc<Mutex<ConcreteMempoolService>>,
}

impl Default for MempoolServiceHandle {
    fn default() -> Self {
        Self {
            mempool: Arc::new(Mutex::new(ConcreteMempoolService::default())),
        }
    }
}

#[async_trait]
impl MempoolService for MempoolServiceHandle {
    async fn submit_instruction(&mut self, instruction: Instruction) -> Result<(), DigitalAssetError> {
        self.mempool.lock().await.submit_instruction(instruction).await
    }

    async fn read_block(&self, limit: usize) -> Result<Vec<Instruction>, DigitalAssetError> {
        self.mempool.lock().await.read_block(limit).await
    }

    async fn reserve_instruction_in_block(
        &mut self,
        instruction_hash: &[u8],
        block_hash: Vec<u8>,
    ) -> Result<(), DigitalAssetError> {
        self.mempool
            .lock()
            .await
            .reserve_instruction_in_block(instruction_hash, block_hash)
            .await
    }

    async fn remove_all_in_block(&mut self, block_hash: &[u8]) -> Result<(), DigitalAssetError> {
        self.mempool.lock().await.remove_all_in_block(block_hash).await
    }

    async fn release_reservations(&mut self, block_hash: &[u8]) -> Result<(), DigitalAssetError> {
        self.mempool.lock().await.release_reservations(block_hash).await
    }

    async fn size(&self) -> usize {
        self.mempool.lock().await.size().await
    }
}