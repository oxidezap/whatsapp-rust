use std::sync::Arc;

use crate::Client;
use wacore::libsignal::protocol::{
    IdentityKeyPair, KeyPair, PreKeyBundle, SignalProtocolError, UsePQRatchet,
    process_prekey_bundle,
};
use wacore::types::jid::JidExt;
use wacore_binary::Jid;

pub(crate) async fn seed_peer_session(client: &Arc<Client>, peer: &Jid) -> anyhow::Result<()> {
    let bundle = tokio::task::spawn_blocking(|| -> Result<PreKeyBundle, SignalProtocolError> {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let receiver = IdentityKeyPair::generate(&mut rng);
        let spk = KeyPair::generate(&mut rng);
        let opk = KeyPair::generate(&mut rng);
        let signature = receiver
            .private_key()
            .calculate_signature(&spk.public_key.serialize(), &mut rng)?;
        PreKeyBundle::new(
            1,
            1u32.into(),
            Some((1u32.into(), opk.public_key)),
            1u32.into(),
            spk.public_key,
            signature,
            *receiver.identity_key(),
        )
    })
    .await??;
    let mut adapter = client.signal_adapter();
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    process_prekey_bundle(
        &peer.to_protocol_address(),
        &mut adapter.session_store,
        &mut adapter.identity_store,
        &bundle,
        &mut rng,
        UsePQRatchet::No,
    )
    .await?;
    Ok(())
}
