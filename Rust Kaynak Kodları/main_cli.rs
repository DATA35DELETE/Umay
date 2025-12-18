mod behaviour;
mod chat;
mod contacts;
mod transport;

use anyhow::Result;
use behaviour::AppBehaviour;
use clap::Parser;
use futures::StreamExt;
use libp2p::{
    Multiaddr, PeerId, dcutr, identify, identity, mdns, ping, relay, request_response,
    swarm::SwarmEvent,
};
use tokio::{io, io::AsyncBufReadExt, select};

#[derive(Parser, Debug)]
#[command(name = "theCommunication")]
struct Opt {
    #[arg(long)]
    secret_key_seed: Option<u8>,

    #[arg(long)]
    port: Option<u16>,

    #[arg(long)]
    bootstrap_node: Option<Multiaddr>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging to file and terminal
    use simplelog::*;
    use std::fs::File;

    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Info,
            Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Info,
            Config::default(),
            File::create("log.txt").unwrap(),
        ),
    ])
    .unwrap();

    let opt = Opt::parse();

    // 1. Create or load identity
    const IDENTITY_FILE: &str = "identity.key";

    let id_keys = if std::path::Path::new(IDENTITY_FILE).exists() {
        // Load existing identity
        println!("📂 Loading existing identity from {}", IDENTITY_FILE);
        let bytes = std::fs::read(IDENTITY_FILE)?;
        identity::Keypair::from_protobuf_encoding(&bytes)
            .map_err(|e| anyhow::anyhow!("Failed to decode keypair: {}", e))?
    } else {
        // Generate new identity
        println!("🔑 Generating new identity...");
        let keypair = match opt.secret_key_seed {
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
        std::fs::write(IDENTITY_FILE, bytes)?;
        println!("💾 Identity saved to {}", IDENTITY_FILE);

        keypair
    };

    let local_peer_id = PeerId::from(id_keys.public());
    println!("Local peer id: {local_peer_id}");

    // 2. Create Relay Client (libp2p handles renewal automatically)
    let (relay_transport, relay_client) = relay::client::new(local_peer_id);

    // 3. Build Transport
    let transport = transport::build_transport(id_keys.clone(), relay_transport)?;

    // 4. Create Behaviours (Minimal for Relay)
    let mdns = mdns::tokio::Behaviour::new(mdns::Config::default(), local_peer_id)?;

    let identify = identify::Behaviour::new(identify::Config::new(
        "/ipfs/0.1.0".into(),
        id_keys.public(),
    ));

    let ping =
        ping::Behaviour::new(ping::Config::new().with_interval(std::time::Duration::from_secs(5)));

    let dcutr = dcutr::Behaviour::new(local_peer_id);

    // Chat (Request-Response for direct messaging)
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
    let port = opt.port.unwrap_or(0);
    swarm.listen_on(format!("/ip4/0.0.0.0/udp/{}/quic-v1", port).parse()?)?;
    swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{}", port).parse()?)?;

    // 7. Connect to relay bootnode
    let bootnodes = vec![
        "/ip4/104.131.131.82/udp/4001/quic-v1/p2p/QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ",
    ];

    for peer in &bootnodes {
        let addr: Multiaddr = match peer.parse() {
            Ok(a) => a,
            Err(e) => {
                println!("Failed to parse bootnode {}: {:?}", peer, e);
                continue;
            }
        };
        println!("Dialing bootnode: {}", peer);
        if let Err(e) = swarm.dial(addr.clone()) {
            println!("Failed to dial {}: {:?}", peer, e);
        }
        // Note: Relay reservation will be done in ConnectionEstablished event
    }

    // Dial provided bootstrap node if any
    if let Some(addr) = opt.bootstrap_node {
        println!("Dialing custom bootstrap node: {}", addr);
        swarm.dial(addr)?;
    }

    println!("-------------------------------------------------");
    println!("Welcome to theCommunication P2P Chat!");
    println!("Type '/help' for commands.");
    println!("Your Peer ID: {}", local_peer_id);
    println!("-------------------------------------------------");

    // Relay renewal - save relay addr and re-dial every 90s
    let mut renewal_timer = tokio::time::interval(std::time::Duration::from_secs(90));
    let mut relay_addr: Option<Multiaddr> = None;

    // Load contact book
    let mut contacts = contacts::ContactBook::load().unwrap_or_else(|e| {
        println!("⚠️  Failed to load contacts: {}", e);
        contacts::ContactBook::default()
    });
    println!("📇 Loaded {} contact(s)", contacts.list().len());

    // Event loop
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    loop {
        select! {
            line = stdin.next_line() => {
                if let Ok(Some(line)) = line {
                    handle_input_line(&mut swarm, &mut contacts, &line).await;
                }
            }
            _ = renewal_timer.tick() => {
                // Renew relay reservation every 90s
                if let Some(addr) = &relay_addr {
                    println!("⏰ Renewing relay reservation...");
                    // To renew, we must call listen_on again with p2p-circuit
                    let circuit_addr = addr.clone().with(libp2p::multiaddr::Protocol::P2pCircuit);
                    if let Err(e) = swarm.listen_on(circuit_addr) {
                        println!("⚠️  Renewal failed: {:?}", e);
                    }
                }
            }
            event = swarm.select_next_some() => {
                match event {
                    /*SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Listening on {}", address);
                    }*/
                    SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
                        //println!("Connection established: {} via {:?}", peer_id, endpoint);

                        // Request relay reservation if this is a bootnode
                        if endpoint.is_dialer() {
                            let addr = endpoint.get_remote_address();
                            // Check if this looks like our relay bootnode
                            if addr.to_string().contains("QmaCpDMGvV2BGHeYERUEnRQAwe3N8SzbUtfsmvsqQLuvuJ") {
                                // Save relay addr for renewal
                                relay_addr = Some(addr.clone());

                                let relay_reserve_addr = addr.clone().with(libp2p::multiaddr::Protocol::P2pCircuit);
                                println!("🔗 Requesting relay reservation on: {}", relay_reserve_addr);
                                if let Err(e) = swarm.listen_on(relay_reserve_addr) {
                                    println!("Failed to listen on relay: {:?}", e);
                                }
                            }
                        }
                    }
                    SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
                        println!("Connection closed: {} (cause: {:?})", peer_id, cause);
                    }
                    SwarmEvent::Behaviour(behaviour::AppBehaviourEvent::RelayClient(event)) => {
                        println!("Relay Client Event: {:?}", event);
                    }
                    SwarmEvent::Behaviour(behaviour::AppBehaviourEvent::Identify(event)) => {
                        println!("Identify Event: {:?}", event);
                    }
                    SwarmEvent::Behaviour(behaviour::AppBehaviourEvent::Dcutr(event)) => {
                        println!("DCUtR Event: {:?}", event);
                    }
                    SwarmEvent::Behaviour(behaviour::AppBehaviourEvent::Chat(
                        request_response::Event::Message { peer, message },
                    )) => match message {
                        request_response::Message::Request {
                            request, channel, ..
                        } => {
                            // Received a message
                            println!("\n📨 [{}]: {}", peer, request.content);

                            // Send acknowledgment
                            let response = chat::ChatMessage {
                                from: swarm.local_peer_id().to_string(),
                                content: "✓".to_string(),
                                timestamp: 0,
                            };
                            if let Err(e) = swarm.behaviour_mut().chat.send_response(channel, response) {
                                println!("Failed to send response: {:?}", e);
                            }
                        }
                        request_response::Message::Response { response, .. } => {
                            // Message was delivered (ack received)
                            if response.content == "✓" {
                                println!("✓ Message delivered to {}", peer);
                            }
                        }
                    },
                    SwarmEvent::Behaviour(behaviour::AppBehaviourEvent::Chat(
                        request_response::Event::OutboundFailure {
                            peer,
                            error,
                            ..
                        },
                    )) => {
                        println!("❌ Failed to send message to {}: {:?}", peer, error);
                    }
                    SwarmEvent::Behaviour(behaviour::AppBehaviourEvent::Chat(
                        request_response::Event::InboundFailure { peer, error, .. },
                    )) => {
                        println!("❌ Inbound message failure from {}: {:?}", peer, error);
                    }
                    SwarmEvent::Behaviour(behaviour::AppBehaviourEvent::Chat(_)) => {
                        // Other chat events (ResponseSent, etc.)
                    }
                    SwarmEvent::IncomingConnection { .. } => {}
                    SwarmEvent::IncomingConnectionError { error, .. } => {
                        println!("Incoming connection error: {:?}", error);
                    }
                    SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
                        println!("Outgoing connection error to {:?}: {:?}", peer_id, error);
                    }
                    SwarmEvent::ListenerClosed { reason, .. } => {
                        println!("Listener Closed: {:?}", reason);
                    }
                    SwarmEvent::ListenerError { error, .. } => {
                        println!("Listener Error: {:?}", error);
                    }
                    _ => {}
                }
            }
        }
    }
}

async fn handle_input_line(
    swarm: &mut libp2p::Swarm<AppBehaviour>,
    contacts: &mut contacts::ContactBook,
    line: &str,
) {
    let mut args = line.split_whitespace();
    match args.next() {
        Some("/help") => {
            println!("Commands:");
            println!("  /help - Show this help");
            println!("  /myid - Show your peer ID");
            println!("  /list - List connected peers");
            println!("  /relay - Show relay addresses");
            println!("  /dial <multiaddr> - Connect to a peer");
            println!("  /save <name> <multiaddr> - Save a contact");
            println!("  /connect <name> - Connect to saved contact");
            println!("  /contacts - List all contacts");
            println!("  /remove <name> - Remove a contact");
        }
        Some("/myid") => {
            println!("Your Peer ID: {}", swarm.local_peer_id());
        }
        Some("/list") => {
            println!("Connected Peers:");
            let mut count = 0;
            for peer in swarm.connected_peers() {
                println!("  - {}", peer);
                count += 1;
            }
            if count == 0 {
                println!("  (none)");
            }
        }
        Some("/relay") => {
            println!("My Listen Addresses:");
            for addr in swarm.listeners() {
                println!("  - {}", addr);
            }
        }
        Some("/dial") => {
            if let Some(addr_str) = args.next() {
                match addr_str.parse::<Multiaddr>() {
                    Ok(addr) => {
                        println!("Dialing {}...", addr);
                        if let Err(e) = swarm.dial(addr) {
                            println!("Failed to dial: {:?}", e);
                        }
                    }
                    Err(e) => println!("Invalid address: {:?}", e),
                }
            } else {
                println!("Usage: /dial <multiaddr>");
            }
        }
        Some("/save") => {
            if let (Some(name), Some(address)) = (args.next(), args.next()) {
                contacts.add(name.to_string(), address.to_string());
                if let Err(e) = contacts.save() {
                    println!("❌ Failed to save contacts: {}", e);
                } else {
                    println!("✅ Saved contact: {}", name);
                }
            } else {
                println!("Usage: /save <name> <multiaddr>");
            }
        }
        Some("/connect") => {
            if let Some(name) = args.next() {
                if let Some(address) = contacts.get(name) {
                    match address.parse::<Multiaddr>() {
                        Ok(addr) => {
                            println!("🔗 Connecting to {} ({})...", name, addr);
                            if let Err(e) = swarm.dial(addr) {
                                println!("Failed to dial: {:?}", e);
                            }
                        }
                        Err(e) => println!("Invalid address in contacts: {:?}", e),
                    }
                } else {
                    println!("❌ Contact '{}' not found", name);
                }
            } else {
                println!("Usage: /connect <name>");
            }
        }
        Some("/contacts") => {
            let contacts_list = contacts.list();
            if contacts_list.is_empty() {
                println!("📇 No contacts saved");
            } else {
                println!("📇 Saved Contacts:");
                for (name, address) in contacts_list {
                    println!("  {} → {}", name, address);
                }
            }
        }
        Some("/remove") => {
            if let Some(name) = args.next() {
                if contacts.remove(name) {
                    if let Err(e) = contacts.save() {
                        println!("❌ Failed to save contacts: {}", e);
                    } else {
                        println!("✅ Removed contact: {}", name);
                    }
                } else {
                    println!("❌ Contact '{}' not found", name);
                }
            } else {
                println!("Usage: /remove <name>");
            }
        }
        Some(cmd) if cmd.starts_with('/') => {
            println!("Unknown command. Type /help for list.");
        }
        Some(first_word) => {
            // Check if it looks like a peer ID (starts with "12D3" or "Qm")
            if first_word.starts_with("12D3") || first_word.starts_with("Qm") {
                // Format: <peer_id> <message>
                if let Ok(peer_id) = first_word.parse::<PeerId>() {
                    let message_content = args.collect::<Vec<&str>>().join(" ");
                    if message_content.is_empty() {
                        println!("Usage: <peer_id> <message>");
                    } else {
                        // Send message
                        let msg = chat::ChatMessage::new(
                            swarm.local_peer_id().to_string(),
                            message_content.clone(),
                        );
                        swarm.behaviour_mut().chat.send_request(&peer_id, msg);
                        println!("💬 Sending: {}", message_content);
                    }
                } else {
                    println!("Invalid peer ID");
                }
            } else {
                // Assume it's a message to the most recent connected peer
                let peer_id = swarm.connected_peers().last().copied();

                if let Some(peer_id) = peer_id {
                    let full_message =
                        format!("{} {}", first_word, args.collect::<Vec<&str>>().join(" "));
                    let msg = chat::ChatMessage::new(
                        swarm.local_peer_id().to_string(),
                        full_message.clone(),
                    );
                    swarm.behaviour_mut().chat.send_request(&peer_id, msg);
                    println!("💬 Sending to {}: {}", peer_id, full_message);
                } else {
                    println!("No peers connected. Use /dial to connect first.");
                }
            }
        }
        None => {}
    }
}
