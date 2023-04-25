use sp_core::{crypto::Ss58Codec, sr25519, sr25519::Signature};
use sp_runtime::traits::Verify;

fn main() {
    let addr = "5Ebm13cUeSEFyAfC3oSwZaVuXKodbd79W8FHbXaPiG458hfJ";
    let msg = "1".as_bytes().to_vec();

    let sig = hex::decode("c46eee1875fd3a2ac7f4877080e17ecea2ab66f51bdaa1581acf92ca65323f5f415314242d5513c070ef7fbd78593c0a9116fdeb6288ff28d67a503f7e23bf84").unwrap();
    let out = self::verify(&addr, &msg, &sig);
    println!("{:?}", out);
}

fn verify(addr: &str, msg: &[u8], sig: &[u8]) -> Result<bool, &'static str> {
    let pubkey =
        <sr25519::Public as Ss58Codec>::from_ss58check(addr).map_err(|_| "Invalid address")?;
    let sig = Signature::try_from(sig).unwrap();
    Ok(sig.verify(msg, &pubkey))
}
