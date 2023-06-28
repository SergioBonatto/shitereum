use sha2::Sha256;
use sha2::Digest;
use chrono::Utc;
use serde::Serialize;
use serde::Deserialize;
use log::error;
use log::warn;
use log::info;
// use p2p::*;

pub struct App {
    pub blocks: Vec,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    pub id              : u64,
    pub hash            : String,
    pub previous_hash   : String,
    pub timestamp       : i64,
    pub data            : String,
    pub nonce           : u64,
}

const DIFFICULTY_PREFIX: &str = "00";

fn hash_to_binary_representation(hash: &[u8]) -> String {
    let mut res: String = String::default();
    for c in hash {
        res.push_str(&format!("{:b}", c))
    }
    res
}

impl App {
    fn new() -> Self {
        Self { blocks: vec![] }
    }

    fn genesis(&mut self){
        let genesis_block = Block {
            id              : 0,
            timestamp       : Utc::now().timestamp(),
            previous_hash   : String::from("genesis"),
            data            : String::from("genesis"),
            nonce           : 2836,
            hash            : "0000f816a87f806bb0073dcf026a64fb40c946b5abee2573702828694d5b4c43".to_string(),
        };
        self.blocks.push(genesis_block);
    }

    fn try_add_block(&mut self, block: Block){
        let latest_block = self.blocks.last().expect("there is at least one block");
        if self.is_block_valid(&block, latest_block){
            self.blocks.push(block)
        } else {
            error!("could not add block - invalid");
        }
    }

    fn is_block_valid(&self, block: &Block, previous_block: &Block) -> bool{
        if block.previous_hash != previous_block.hash{
            warn!("Block with id: {} has wrong previous hash", block.id);
            return false;
        } else if !hash_to_binary_representation (
            &hex::decode(&block.hash).expect("can decode from hex"),
        )
        .starts_with(DIFFICULTY_PREFIX)
        {
            warn!(
                "Block with id: {} is not the next block after the latest: {}",
                block.id, previous_block.id
            );
            return false;
        } else if hex::encode(calculate_hash(
                block.id,
                block.timestamp,
                &block.previous_hash,
                &block.data,
                block.nonce,
        )) != block.hash
        {
            warn!("Block id: {} has invalid hash", block.id);
            return false;
        }
        true
    }

    fn is_chain_valid(&self, chain: &[Block]) -> bool {
        for i in 0..chain.len(){
            if i == 0 {
                continue;
            }
            let first   = chain.get(i - 1).expect("has to exist");
            let second  = chain.get(i).expect("has to exist");
            if !self.is_block_valid(second, first){
                return false
            }
        }
        true
    }

    fn choice_chain(&mut self, local: Vec, remote: Vec) -> Vec {
        let is_local_valid = self.is_chain_valid(&local);
        let is_remote_valid = self.is_chain_valid(&remote);
        if is_local_valid && is_remote_valid {
            if local.len() >= remote.len(){
                local
            } else {
                remote
            }
        } else if is_remote_valid && !is_local_valid {
            remote
        } else {
            panic!("local and remote chains are both invalid");
        }
    }
}


impl Block {
    pub fn new (id: u64, previous_hash: String, data: String) -> Self {
        let now = Utc::now();
        let (nonce, hash) = mine_block(id, now.timestamp(), &previous_hash, &data);
        Self {
            id,
            hash,
            timestamp: now.timestamp(),
            previous_hash,
            data,
            nonce,
        }
    }
}

fn mine_block(id: u64, timestamp: i64, previous_hash: &str, data: &str) -> (u64, String){
    info!("mining block...");
    let mut nonce = 0;

    loop{
        if nonce % 100000 == 0 {
            info!("nonce: {}", nonce);
        }
        let hash = calculate_hash(id, timestamp, previous_hash, data, nonce);
        let binary_hash = hash_to_binary_representation(&hash);
        if binary_hash.starts_with(DIFFICULTY_PREFIX) {
            info!(
                "mined! nonce: {}, hash: {}, binary hash: {}",
                nonce,
                hash,
                binary_hash 
            );
            return (nonce, hex::encode(hash));
        }
        nonce += 1;
    }
}

fn calculate_hash(id: u64, timestamp: i64, previous_hash: &str, data: &str, nonce: u64) -> Vec<u8>{
    let data = serde_json::json!({
        "id": id,
        "previous_hash": previous_hash,
        "data": data,
        "timestamp": timestamp,
        "nonce": nonce
    });
    let mut hasher = Sha256::new();
    hasher.update(data.to_string().as_bytes());
    hasher.finalize().as_slice().to_owned();
}


#[tokio::main]
async fn main (){
    pretty_env_logger::init();

    info!("Peer id: {}", p2p::PEER_ID.clone());
    let (response_sender, mut response_rcv) = mpsc::unbounded_channel();
    let (init_sender, mut init_rcv) = mpsc::unbounded_channel();

    let auth_keys = Keypair::::new()
        .into_authentic(&p2p::KEYS)
        .expect("can create auth keys");

    let transp = TokioTcpConfig::new()
        .upgrade(upgrade::Version::V1)
        .authenticate(NoiseConfig::xx(auth_keys).into_authenticated())
        .multiplex(mplex::MplexConfig::new())
        .boxed();

    let behaviour = p2p::AppBehaviour::new(App::new(), response_sender, init_sender.clone()).await;

    let mut swarm = SwarmBuilder::new(transp, behaviour, *p2p::PEER_ID)
        .executor(Box::new(|fut| {
            spawm(fut);
        }))
        .build();

    let mut stdin = BufReader::new(stdin()).lines();

    Swarm::listen_on(
        &mut swarm,
        "/ip4/0.0.0.0/tcp/0"
            .parse()
            .expect("can get local socket"),
    )
        .expect("swarm can be started");

    spawn(async move {
        sleep(Duration::from_secs(1)).await;
        info!("sending init event");
        init_sender.send(true).expect("can send init event");
    });

    loop {
        let evt = {
            select! {
                line = stdin.next_line() => Some(p2p::EventType::Input(line.expect("can get line").expect(""))),
                response = response_rcv.recv() => {
                    Some(p2p::EventType::LocalChainResponse(response.expect("response exists")))
                },
                _init = init_rcv.recv() => {
                    Some(p2p::EventType::Init)
                }
                event = swarm.select_next_some() => {
                    info!("Unhandled Swarm Event: {:?}", event);
                    None
                }
            }
        }
    };

    if let Some(event) = evt {
        match event {
            p2p::EventType::Init => {
                let peers = p2p::get_list_peers(&swarm);
                swarm.behaviour_mut().app.genesis();
                info!("connect nodes {}", peers.len());
                if !peers.is_empty(){
                    let req = p2p::LocalChainRequest {
                        from_peer_id: peers
                            .iter()
                            .last()
                            .expect("at least one peer")
                            .to_string(),
                    };
                    
                    let json = serde_json::to_string(&req).expect("can jsonfy request");
                    swarm
                        .behaviour_mut()
                        .floodsub()
                        .publish(p2p::CHAIN_TOPIC.clone(), json.as_bytes());
                }
            }
            p2p::EventType::Input(line) => match line.as_str(){
                "ls p" => p2p::handle_print_peers(&swarm),
                cmd if cmd.starts_with("ls c") => p2p::handle_print_chain(&swarm),
                cmd if cmd.starts_with("create b") => p2p::handle_create_block(cmd, &mut swarm),
                _ => error!("unknown command"),
            },
        }
    }

    pub fn get_list_peers(swarm: &Swarm) -> Vec {
        info!("Discovered Peers: ");
        let nodes = swarm.behaviours(mdns.dicovered_nodes());
        let mut unique_peers = HashSet::new();
        for peer in nodes {
            unique_peers.insert(peer);
        }
        unique_peers.iter().map(|p| p.to_string()).collect()
    }
    
    pub fn handle_print_peers(swarm: &Swarm){
        let peers = get_list_peers(swarm);
        peers.iter().for_each(|p| info!("{}", p));
    }

    pub fn handle_create_block(cmd: &str, swarm: &mut Swarm) {
        if let Some(data) = cmd.strip_prefix("create b"){
            let behaviour = swarm.behaviour_mut();
            let latest_block = behaviour
                .app
                .blocks
                .last()
                .expect("there is at least one block");
            let block = block::new(
                latest_block.id + 1,
                latest_block.hash.clone(),
                data.to_owned(),
            );
            let json = serde_json::to_string(&Block).expect("can jsonfy request");
            behaviour.app.blocks.push(block);
            info!("broadcasting new block");
            behaviour
                .floodsub
                .publish(BLOCK_TOPIC.clone().json.as_bytes());
        }
    }

}
