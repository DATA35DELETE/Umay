use crate::behaviour::{self, AppBehaviour};
use crate::{chat, contacts, transport};
use anyhow::Result;
use futures::StreamExt;
use libp2p::{
    dcutr, identify, identity, mdns, ping, relay, request_response, swarm::SwarmEvent, Multiaddr,
    PeerId,
};
use std::path::Path;
use std::sync::Mutex;
use tokio::{select, sync::mpsc};

pub enum Command {
    Start,
    Dial(Multiaddr),
    SaveContact(String, String),
    ConnectContact(String),
    SendMessage(PeerId, String),
    GetInfo,
}

pub async fn run_p2p_node(
    mut command_rx: mpsc::Receiver<Command>,
    secret_key_seed: Option<u8>,
    identity_file_path: String,
) -> Result<()> {
    // Load or create identity from the provided path
    let id_keys = if Path::new(&identity_file_path).exists() {
        log::info!("📂 Loading existing identity from {}", identity_file_path);
        let bytes = std::fs::read(&identity_file_path)?;
        identity::Keypair::from_protobuf_encoding(&bytes)
            .map_err(|e| anyhow::anyhow!("Failed to decode keypair: {}", e))?
    } else {
        log::info!("🔑 Generating new identity...");
        let keypair = match secret_key_seed {
            Some(seed) => {
                let mut bytes = [0u8; 32];
                bytes[0] = seed;
                identity::Keypair::ed25519_from_bytes(bytes).unwrap()
            }
            None => identity::Keypair::generate_ed25519(),
        };

        // Save to file
        let bytes = keypair
            .to_protobuf_encoding()
            .map_err(|e| anyhow::anyhow!("Failed to encode keypair: {}", e))?;
        std::fs::write(&identity_file_path, bytes)?;
        log::info!("💾 Identity saved to {}", identity_file_path);

        keypair
    };

    let local_peer_id = PeerId::from(id_keys.public());
    log::info!("Local peer id: {local_peer_id}");

    // Store peer ID globally for JNI access
    crate::LOCAL_PEER_ID
        .set(Mutex::new(local_peer_id.to_string()))
        .ok();

    // 2. Create Relay Client
    let (relay_transport, relay_client) = relay::client::new(local_peer_id);

    // 3. Build Transport
    let transport = transport::build_transport(id_keys.clone(), relay_transport)?;

    // 4. Create Behaviours
    let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;
    let identify = identify::Behaviour::new(identify::Config::new(
        "/ipfs/0.1.0".into(),
        id_keys.public(),
    ));
    let ping =
        ping::Behaviour::new(ping::Config::new().with_interval(std::time::Duration::from_secs(5)));
    let dcutr = dcutr::Behaviour::new(local_peer_id);
    let chat = request_response::json::Behaviour::<chat::ChatMessage, chat::ChatMessage>::new(
        [(
            libp2p::StreamProtocol::new("/chat/1.0.0"),
            request_response::ProtocolSupport::Full,
        )],
        request_response::Config::default()
            .with_request_timeout(std::time::Duration::from_secs(60)),
    );

    let behaviour = AppBehaviour {
        mdns,
        identify,
        ping,
        relay_client,
        dcutr,
        chat,
    };

    // 5. Build Swarm
    let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
        .with_tokio()
        .with_other_transport(|_key| transport)?
        .with_behaviour(|_key| behaviour)?
        .with_swarm_config(|c| c.with_idle_connection_timeout(std::time::Duration::from_secs(180)))
        .build();

    // 6. Listen on all interfaces
    swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // 7. Connect to relay bootnode
    let bootnodes = vec![
        "/ip4/104.131.131.82/udp/4001/quic-v1/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ",
    ];

    for peer in &bootnodes {
        if let Ok(addr) = peer.parse::<Multiaddr>() {
            log::info!("Dialing bootnode: {}", peer);
            swarm.dial(addr)?;
        }
    }

    // Relay renewal
    let mut renewal_timer = tokio::time::interval(std::time::Duration::from_secs(90));
    let mut relay_addr: Option<Multiaddr> = None;

    // Contact book
    let mut contacts = contacts::ContactBook::default();

    loop {
        select! {
            command = command_rx.recv() => {
                match command {
                    Some(Command::Start) => {
                        log::info!("P2P Node started via command");
                    }
                    Some(Command::Dial(addr)) => {
                        log::info!("Command: Dialing {}", addr);
                        if let Err(e) = swarm.dial(addr) {
                            log::error!("Failed to dial: {:?}", e);
                        }
                    }
                    Some(Command::SaveContact(name, addr_str)) => {
                        contacts.add(name.clone(), addr_str.clone());
                        log::info!("Command: Saved contact {} -> {}", name, addr_str);
                    }
                    Some(Command::ConnectContact(name)) => {
                        if let Some(addr_str) = contacts.get(&name) {
                            if let Ok(addr) = addr_str.parse::<Multiaddr>() {
                                log::info!("Command: Connecting to contact {}", name);
                                swarm.dial(addr).ok();
                            }
                        } else {
                            log::error!("Contact not found: {}", name);
                        }
                    }
                    Some(Command::SendMessage(peer_id, msg_content)) => {
                        let msg = chat::ChatMessage::new(
                            swarm.local_peer_id().to_string(),
                            msg_content.clone(),
                        );
                        swarm.behaviour_mut().chat.send_request(&peer_id, msg);
                        log::info!("Command: Sending message to {}", peer_id);
                    }
                    Some(Command::GetInfo) => {
                        log::info!("My Peer ID: {}", swarm.local_peer_id());
                        log::info!("Connected Peers: {:?}", swarm.connected_peers().collect::<Vec<_>>());
                    }
                    None => {
                        log::info!("Command channel closed, shutting down P2P node");
                        break;
                    }
                }
            }
            _ = renewal_timer.tick() => {
                if let Some(addr) = &relay_addr {
                    log::info!("⏰ Renewing relay reservation...");
                    let circuit_addr = addr.clone().with(libp2p::multiaddr::Protocol::P2pCircuit);
                    if let Err(e) = swarm.listen_on(circuit_addr) {
                        log::error!("⚠️  Renewal failed: {:?}", e);
                    }
                }
            }
            event = swarm.select_next_some() => {
                match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        log::info!("Listening on {}", address);
                        // Store listen addresses globally
                        if let Some(addrs_mutex) = crate::LISTEN_ADDRESSES.get() {
                            if let Ok(mut addrs) = addrs_mutex.lock() {
                                let addr_str = address.to_string();
                                if !addrs.contains(&addr_str) {
                                    addrs.push(addr_str);
                                }
                            }
                        } else {
                            let mut addrs = vec![address.to_string()];
                            crate::LISTEN_ADDRESSES.set(Mutex::new(addrs)).ok();
                        }
                    }
                    SwarmEvent::ConnectionEstablished { endpoint, .. } => {
                        if endpoint.is_dialer() {
                            let addr = endpoint.get_remote_address();
                            if addr.to_string().contains("QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ") {
                                relay_addr = Some(addr.clone());
                                let relay_reserve_addr = addr.clone().with(libp2p::multiaddr::Protocol::P2pCircuit);
                                log::info!("🔗 Requesting relay reservation on: {}", relay_reserve_addr);
                                swarm.listen_on(relay_reserve_addr).ok();
                            }
                        }
                    }
                    SwarmEvent::Behaviour(behaviour::AppBehaviourEvent::Chat(
                        request_response::Event::Message { peer, message },
                    )) => match message {
                        request_response::Message::Request { request, channel, .. } => {
                            log::info!("\n📨 [{}]: {}", peer, request.content);

                            // Call Java callback
                            crate::notify_message_received(peer.to_string(), request.content.clone());

                            let response = chat::ChatMessage {
                                from: swarm.local_peer_id().to_string(),
                                content: "✓".to_string(),
                                timestamp: 0,
                            };
                            swarm.behaviour_mut().chat.send_response(channel, response).ok();
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
