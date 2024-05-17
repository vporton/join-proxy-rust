use k256::ecdsa::{Signature, VerifyingKey};
use serde_derive::{Deserialize, Serialize};
use candid::{Encode, Decode, CandidType};
use ic_agent::{agent::{PollResult, UpdateCall}, export::Principal, Agent, AgentError, RequestId};
use k256::ecdsa::signature::hazmat::PrehashVerifier;
use ic_cdk_macros::*;

pub static CANISTER_SIGN_KEY: &[&[u8; 4]; 4] = &[
    // Random 128-bit key follows. Generated using `uuidgen -r` on Linux.
    &[0x7e, 0x0c, 0xc8, 0x36],
    &[0x1a, 0xce, 0x49, 0x2f],
    &[0xab, 0xc1, 0x1a, 0x86],
    &[0x46, 0xfe, 0xaf, 0xd1],
];

/// TODO: Do we need to convert it to Vec<Vec<_>>?
pub fn get_canister_sign_key() -> Vec<Vec<u8>> {
    CANISTER_SIGN_KEY.into_iter().map(|&t| t.to_vec()).collect::<Vec<Vec<_>>>()
}

#[derive(CandidType, Serialize)]
pub enum EcdsaCurve {
    #[serde(rename = "secp256k1")]
    Secp256k1,
}

#[derive(CandidType, Serialize)]
struct EcdsaKeyId {
    pub curve: EcdsaCurve,
    pub name: String,
}

#[derive(CandidType, Serialize)]
pub struct EcdsaPublicKeyArgs {
    canister_id : Option<Principal>,
    derivation_path : Vec<Vec<u8>>, // FIXME: What should be the type?
    key_id : EcdsaKeyId,
}

#[derive(CandidType, Deserialize)]
pub struct ECDSAPublicKeyReply {
    pub public_key: Vec<u8>,
    pub chain_code: Vec<u8>,
}

pub struct CanisterPublicKeyStatus {
    request_id: RequestId,
}

pub enum CanisterPublicKeyPollResult {
    Submitted,
    Accepted,
    Completed(VerifyingKey),
}

impl CanisterPublicKeyStatus {
    pub async fn poll(
        agent: &Agent,
        status: &CanisterPublicKeyStatus,
    ) -> Result<CanisterPublicKeyPollResult, anyhow::Error> {
        let base = agent.poll(&status.request_id, Principal::management_canister()).await?;
        Ok(match base {
            PollResult::Submitted => CanisterPublicKeyPollResult::Submitted,
            PollResult::Accepted => CanisterPublicKeyPollResult::Accepted,
            PollResult::Completed(v) => {
                let res = Decode!(v.as_slice(), ECDSAPublicKeyReply)?;
                CanisterPublicKeyPollResult::Completed(VerifyingKey::from_sec1_bytes(&res.public_key)?)
            },
        })
    }
}

pub async fn get_canister_pubkey(agent: &Agent, canister_id: Principal) -> Result<CanisterPublicKeyStatus, AgentError> {
    let arg = EcdsaPublicKeyArgs {
        canister_id: Some(canister_id),
        derivation_path: get_canister_sign_key(),
        key_id: EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: "allowed_canister".to_string(),
        },
    };
    Ok(CanisterPublicKeyStatus {
        request_id: agent.update(&Principal::management_canister(), "sign_with_ecdsa")
            .with_arg(Encode!(&arg)?).with_effective_canister_id(canister_id).call().await?,
    })
}

pub fn verify_signature(signature: Signature, hash: &[u8; 32], pubkey: VerifyingKey) -> Result<(), k256::ecdsa::Error> {
    pubkey.verify_prehash(hash, &signature)
}