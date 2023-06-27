pub struct App {
    bub blocks: Vec,
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
}



