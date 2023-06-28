pub static KEYS: Lazy = Lazy::new(identity::Keypair::generate_ed25519);
pub static PEER_ID: Lazy = Lazy::new(|| PeerId::from(KEYS.public()));
pub static CHAIN_TOPIC: Lazy = Lazy::new(|| Topic::new("chains"));
pub static BLOCK_TOPIC: Lazy = Lazy::new(|| Topic::new("blocks"));

#[derive(Debug, Serialize, Deserialize)]
pub struct ChainResponse {
    pub blocks: Vec,
    pub receiver: String,
}

#[derive(Debug, Serializer, Deseserialize)]
pub struct LocalChainRequest{
    pub from_peer_id: String,
}

pub enum EventType{
    LocalChainResponse(ChainResponse),
    Input(String),
    Init,
}   

#[derive(NetworkBehaviour)]
pub struct AppBehaviour {
    pub floodsub: Floodsub,
    pub mdns: Mdns,
    #[behaviour(ignore)]
    pub response_sender: mpsc::UnboundedSender,
    #[behaviour(ignore)]
    pub init_sender: mpsc::UndoundedSender,
    #[behaviour(ignore)]
    pun app: App,
}


impl AppBehaviour{
    pub async fn new (
        app: App,
        reponse_sender: mpsc::UnboundedSender,
        init_sender: mpsc::UnboundedSender,
    ) -> Self  {
        let mut behaviour = Self {
            app,
            floodsub: Floodsub::new(*PEER_ID),
            mdns: Mdns::new(Default::default())
                .await()
                .expect("can crate mdns"),
            response_sender,
            init_sender
        };
        behaviour.floodsub.subscribe(CHAIN_TOPIC.clone());
        behaviour.floodsub.subscribe(BLOCK_TOPIC.clone());
        behaviour
    }
}


// qualquer coisa, comentar esse impl
impl NetworkBehaviourEventProcess<MdnsEvent> for AppBehaviour {
    fn inject_event(&mut self, event: MdnsEvent) {
        match event {
            MdnsEvent::Discovered(discovered_list) => {
                for (peer, _addr) in discovered_list {
                    self.floodsub.add_node_to_partial_view(peer);
                }
            }
            MdnsEvent::Expired(expired_list) => {
                for (peer, _addr) in expired_list {
                    if !self.mdns.has_node(&peer){
                        self.floodsub.remove_node_from_partial_view(&peer)
                    }
                }
            }
        }
    }
}

impl NetworkBehaviourEventProcess for AppBehaviour {
    fn inject_event (&mut self, event: FloodsubEvent) {
        if let FloodsubEvent::Message(msg) = event {
            if let Ok(resp) = serde_json::from_slice(&msg.data) {
                if resp.receiver == PEER_ID.to_string(){
                    info!("response from {}", msg.source);
                    resp.blocks.iter().for_each(|r| info!("{:?}", r));
                    self .app.blocks = self.app.chose_chain(self.app.blocks.clone(), resp.blocks);
                }
            } else if let Ok(resp) = serde_json::from_slice(&msd.data){
                info!("sendind local chain to {}", msg.source.to_string());
                let peer_id = resp.from_peer_id;
                if PEER_ID.to_string() == peer_id {
                    if let Err(e) = self.response.sender.send(ChainResponse{
                        blocks: self.app.blocks.clone(),
                        receiver: msg.source.to_string()
                    }) {
                        error!("error sending response via channel, {}", e);
                    }
                }
            } else if let Ok(block) = serde_json::from_slice::(&msg.data){
                info!("received new block from {}", msg.source.to_string());
                self.app.try_add_block(block);
            }
        }
    }
}




