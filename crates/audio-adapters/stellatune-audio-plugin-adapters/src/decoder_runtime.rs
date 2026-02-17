use stellatune_plugins::runtime::introspection::CapabilityKind as RuntimeCapabilityKind;

const DEFAULT_DECODER_PLUGIN_ID: &str = "host.audio.decoder";
const DEFAULT_DECODER_TYPE_ID: &str = "refresh";

pub struct DecoderRuntimeIdentity {
    pub plugin_id: String,
    pub type_id: String,
    pub generation: u64,
}

pub fn resolve_decoder_runtime_identity(
    decoder_plugin_id: Option<&str>,
    decoder_type_id: Option<&str>,
) -> DecoderRuntimeIdentity {
    let plugin_id = decoder_plugin_id
        .unwrap_or(DEFAULT_DECODER_PLUGIN_ID)
        .to_string();
    let type_id = decoder_type_id
        .unwrap_or(DEFAULT_DECODER_TYPE_ID)
        .to_string();
    let generation = match (decoder_plugin_id, decoder_type_id) {
        (Some(plugin_id), Some(type_id)) => stellatune_runtime::block_on(
            stellatune_plugins::runtime::handle::shared_runtime_service().find_capability(
                plugin_id,
                RuntimeCapabilityKind::Decoder,
                type_id,
            ),
        )
        .map(|cap| cap.lease_id)
        .unwrap_or(0),
        _ => 0,
    };

    DecoderRuntimeIdentity {
        plugin_id,
        type_id,
        generation,
    }
}
