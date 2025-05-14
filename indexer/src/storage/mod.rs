pub mod db;
pub mod uf;

use std::hash::Hash;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct BskyPostId<S: AsRef<str>> {
    did: S,
    rkey: S,
}

impl<S: AsRef<str>> BskyPostId<S> {
    pub fn new(did: S, rkey: S) -> BskyPostId<S> {
        BskyPostId { did: did, rkey: rkey }
    }
}

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct BskyPostReplyTo<Id> {
    target: Id,
    root: Id,
}

impl<Id> BskyPostReplyTo<Id> {
    pub fn new(target: Id, root: Id) -> BskyPostReplyTo<Id> {
        BskyPostReplyTo { target, root}
    }
}


#[derive(Clone, Eq, Hash, PartialEq)]
pub struct BskyPostRecord<Id> {
    id: Id,
    reply_to: Option<BskyPostReplyTo<Id>>,
    quote_of: Option<Id>,
    created_at: Option<u64>,
}

impl<Id> BskyPostRecord<Id> {
    pub fn new(id: Id, reply_to: Option<BskyPostReplyTo<Id>>, quote_of: Option<Id>, created_at: Option<u64>) -> BskyPostRecord<Id> {
        BskyPostRecord { id, reply_to, quote_of, created_at }
    }
}
