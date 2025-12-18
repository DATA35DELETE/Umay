use libp2p::{dcutr, identify, mdns, ping, relay, request_response, swarm::NetworkBehaviour};

#[derive(NetworkBehaviour)]
pub struct AppBehaviour {
    pub mdns: mdns::tokio::Behaviour,
    pub identify: identify::Behaviour,
    pub ping: ping::Behaviour,
    pub relay_client: relay::client::Behaviour,
    pub dcutr: dcutr::Behaviour,
    pub chat: request_response::json::Behaviour<crate::chat::ChatMessage, crate::chat::ChatMessage>,
}
