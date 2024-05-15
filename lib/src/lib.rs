use serde_derive::Serialize;
use candid::{Encode, Decode, CandidType, Nat};
use ic_agent::{agent::UpdateCall, export::Principal, Agent, AgentError};
use ic_cdk_macros::*;

pub static canister_sign_key: &[&[u8; 4]; 4] = &[
    // Random 128-bit key follows. Generated using `uuidgen -r` on Linux.
    &[0x7e, 0x0c, 0xc8, 0x36],
    &[0x1a, 0xce, 0x49, 0x2f],
    &[0xab, 0xc1, 0x1a, 0x86],
    &[0x46, 0xfe, 0xaf, 0xd1],
];

/// TODO: Do we need to convert it to Vec<Vec<_>>?
pub fn get_canister_sign_key() -> Vec<Vec<u8>> {
    canister_sign_key.into_iter().map(|&t| t.to_vec()).collect::<Vec<Vec<_>>>()
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

pub struct CanisterPublicKeyStatus<'a>(UpdateCall<'a>);

pub fn get_canister_pubkey<'a>(agent: &'a Agent, canister_id: Principal) -> Result<CanisterPublicKeyStatus<'a>, AgentError> {
    let arg = EcdsaPublicKeyArgs {
        canister_id: Some(canister_id),
        derivation_path: get_canister_sign_key(),
        key_id: EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: "allowed_canister".to_string(),
        },
    };
    Ok(CanisterPublicKeyStatus(agent.update(&Principal::management_canister(), "sign_with_ecdsa")
        .with_arg(Encode!(&arg)?).with_effective_canister_id(canister_id).call()))
}