use libp2p::{
    PeerId, Transport,
    core::{muxing::StreamMuxerBox, transport::Boxed},
    identity, noise, quic, tcp, yamux,
};
use std::time::Duration;

pub fn build_transport(
    keypair: identity::Keypair,
    relay_transport: libp2p::relay::client::Transport,
) -> std::io::Result<Boxed<(PeerId, StreamMuxerBox)>> {
    let tcp_transport = tcp::tokio::Transport::new(tcp::Config::default().nodelay(true))
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&keypair).unwrap())
        .multiplex(yamux::Config::default())
        .timeout(Duration::from_secs(20));

    // DNS removed due to dependency issues
    // let tcp_transport = libp2p::dns::TokioDnsConfig::system(tcp_transport)?;

    let quic_transport = quic::tokio::Transport::new(quic::Config::new(&keypair));
    // let quic_transport = libp2p::dns::TokioDnsConfig::system(quic_transport)?;

    let base_transport = libp2p::core::transport::OrTransport::new(quic_transport, tcp_transport)
        .map(|either_output, _| match either_output {
            futures::future::Either::Left((peer_id, muxer)) => {
                (peer_id, StreamMuxerBox::new(muxer))
            }
            futures::future::Either::Right((peer_id, muxer)) => {
                (peer_id, StreamMuxerBox::new(muxer))
            }
        });

    let relay_transport = relay_transport
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&keypair).unwrap())
        .multiplex(yamux::Config::default());

    let transport = libp2p::core::transport::OrTransport::new(relay_transport, base_transport)
        .map(|either_output, _| match either_output {
            futures::future::Either::Left((peer_id, muxer)) => {
                (peer_id, StreamMuxerBox::new(muxer))
            }
            futures::future::Either::Right((peer_id, muxer)) => {
                (peer_id, StreamMuxerBox::new(muxer))
            }
        })
        .boxed();

    Ok(transport)
}
