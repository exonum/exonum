extern crate exonum;
extern crate sandbox;

use exonum::messages::{Message, Propose, Prevote, Precommit};

use sandbox::timestamping_sandbox;


// =======================

// HANDLE REQUEST

// - ignore if to incorrect
// - ignore if incorrect time
// - ignore if time < 0
// - ignore if time > REQUEST_ALIVE
// - ignore if incorrect signature

// REQUEST PROPOSE:
// - ignore if wrong height
// - ignore if hasn’t propose
// - send propose

// REQUEST TXS:
// - ignore if hasn’t
// - send from pool
// - send from blockchain

// REQUEST PREVOTES:
// - ignore if height != our height
// - send prevotes we have (> +2/3, <+2/3, 0)

// REQUEST PRECOMITS:
// - ignore if height > our height
// - send precommits we have (> +2/3, <+2/3, 0) for out height
// - send precommits from blockchain for prev height if we have (or not send if haven’t)

// REQUEST COMMIT:
// - ignore if height = our height
// - ignore if height > our height
// - send +2/3 precommits if we have (not send if haven’t)

// BYZANTINE:
// - get precommits with different block_hash
// - send different proposes
// - not send proposes
// - update lock
