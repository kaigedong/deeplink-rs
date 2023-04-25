use anyhow::{anyhow, Error, Result};
use sp_core::{crypto::Ss58Codec, sr25519, sr25519::Signature};
use sp_runtime::traits::Verify;
use std::time::Instant;

pub fn now() -> String {
    let now = Instant::now();
    let elapsed = now.elapsed();
    let elapsed_secs = elapsed.as_secs();
    let elapsed_nanos = elapsed.subsec_nanos();
    format!("{}.{:09}", elapsed_secs, elapsed_nanos)
}

pub fn verify_signature(addr: &str, msg: &str, sig: &str) -> Result<bool, Error> {
    let msg: &[u8] = &msg.as_bytes().to_vec();
    let sig: &[u8] = &hex::decode(sig.trim_start_matches("0x"))?;

    let pubkey = <sr25519::Public as Ss58Codec>::from_ss58check(addr)?;
    let sig = Signature::try_from(sig).map_err(|e| anyhow!("{:?}", e))?;
    Ok(sig.verify(msg, &pubkey))
}
