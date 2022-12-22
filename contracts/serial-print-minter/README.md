# Stargaze Serial Print Minter Contract

A minter that best works for generated art collections. It's designed for collections stored on IPFS that have a base URI root.

- Pay the creation fee 

  at creating minter, 

      if num_tokens > 10k, creation_fee = num_tokens * creation_fee_per_token(stars)

      if num_tokens <=10k, creation_fee = 1k(stars)

  at appending a new uri

      if num_tokens > 10k, creation_fee = num_tokens * creation_fee_per_token(stars)

      if num_tokens <=10k, creation_fee = 0(stars)

- Appending a new uri 

    base_token_id, minted_num_tokens are the states of the previous round.

    base_token_id:  
    
      To avoid the conflicts of token id, token id should be calculated
      based on the sum of the previous round’s num_tokens.
    
    minted_num_tokens:  
    
      number of the tokens that are minted on previous rounds.
      


    Ex.
    
    -round1(after initializing)
    
    base_token_id = 0
    
    minted_num_tokens = 0

    base_token_uri = “ipfs://round1”
    
    num_tokens = 100
    
    Minted 5th, 100th nfts for token_id 5, 100


    -round2(After set-token-uri(uri: “ipfs://round2”, num_tokens: 100))
    
    base_token_id = 100 = (num_tokens of round1)
    
    minted_num_tokens = 2 (number of tokens that was minted in round1)

    base_token_uri=”ipfs://round2”
    
    num_tokens = 102 = 100 + (minted_num_tokens)
    
    Mint 5th, 17th, 100th nft for token_id 105(base_token_id + 5), 117(base_token_id + 17), 200(base_token_id+ 100)


    -round3(After set-token-uri(uri: “ipfs://round3”, num_tokens: 10))
    
    base_token_id = 200 = (sum of num_tokens of round1, round2)
    
    minted_num_tokens = 5(number of tokens that was minted in round1, round2)


    base_token_uri=”ipfs://round3”
    
    num_tokens = 15 = 10 + (minted_num_tokens)
    
    Mint 5th nft for token_id 205(base_token_id + 5)
    
 - set minting_pause by owner
    
      mintable on/off by owner.
      
# Test
cargo test -- --nocapture
