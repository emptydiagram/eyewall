
use std::collections::{HashMap, HashSet};

#[derive(Clone, Eq, Hash, PartialEq)]
struct BskyPostId {
    did: String,
    rkey: String,
}

impl BskyPostId {
    fn from(did: &str, rkey: &str) -> BskyPostId {
        BskyPostId { did: String::from(did), rkey: String::from(rkey) }
    }
}

struct BskyPostTreeData {
    ro_root: Option<BskyPostId>,
    qo_root: Option<BskyPostId>,
    // rq_sources: Vec<BskyPostId>,
    ro_depth: u32,
    qo_depth: u32,
    // rq_depth: u32,
    is_ro_leaf: bool,
    is_qo_leaf: bool,
    // is_rq_leaf: bool,
}

impl BskyPostTreeData {
    fn new_root() -> BskyPostTreeData {
        BskyPostTreeData {
            ro_root: None, qo_root: None,
            ro_depth: 1, qo_depth: 1,
            is_ro_leaf: true, is_qo_leaf: true
        }
    }
}


struct BskyPostCounters {
    ro_size: u32,
    qo_size: u32,
    // rq_size: u32,
    ro_max_width: u32,
    qo_max_width: u32,
    // rq_max_width: u32,
    ro_max_depth: u32,
    qo_max_depth: u32,
    // rq_max_depth: u32,
}

impl BskyPostCounters {
    fn new() -> BskyPostCounters {
        BskyPostCounters {
            ro_size: 1, qo_size: 1,
            ro_max_width: 1, qo_max_width: 1,
            ro_max_depth: 1, qo_max_depth: 1
        }
    }
}


#[derive(Clone, Eq, Hash, PartialEq)]
struct BskyPostRecord {
    id: BskyPostId,
    reply_to: Option<BskyPostReplyTo>,
    quote_of: Option<BskyPostId>,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct BskyPostReplyTo {
    target: BskyPostId,
    root: BskyPostId,
}

struct BskyPostData {
    posts: HashMap<BskyPostId, BskyPostTreeData>,
    counters: HashMap<BskyPostId, BskyPostCounters>,
    pending: HashSet<BskyPostRecord>,
}

impl BskyPostData {
    fn new() -> BskyPostData {
        BskyPostData { posts: HashMap::new(), counters: HashMap::new(), pending: HashSet::new() }
    }
    fn ingest_record(&mut self, record: BskyPostRecord) {
        // invariant: if a post has been processed, all ancestors have also
        if record.reply_to.is_none() && record.quote_of.is_none() {
            let id2 = record.id.clone();
            self.posts.insert(record.id, BskyPostTreeData::new_root());
            self.counters.insert(id2, BskyPostCounters::new());
        } else if record.quote_of.is_none() {
            let reply_to = record.reply_to.as_ref().unwrap();
            if !self.posts.contains_key(&reply_to.target) {
                self.pending.insert(record.clone());
                return;
            }
            if !self.posts.contains_key(&record.id) {
                let source2 = record.id.clone();
                let parent_data = self.posts.get_mut(&reply_to.target).unwrap();
                let ro_depth = parent_data.ro_depth + 1;
                let parent_was_ro_leaf = parent_data.is_ro_leaf;
                parent_data.is_ro_leaf = false;
                let tree_data = BskyPostTreeData {
                    ro_root: Some(reply_to.root.clone()),
                    qo_root: None,
                    ro_depth: ro_depth,
                    qo_depth: 1,
                    is_ro_leaf: true,
                    is_qo_leaf: true,
                };

                self.posts.insert(source2, tree_data);

                let root_counters = self.counters.get_mut(&reply_to.root).unwrap();
                root_counters.ro_size += 1;
                root_counters.ro_max_depth = root_counters.ro_max_depth.max(ro_depth);

                // if this is a reply to a non-leaf node, then increase max width
                if !parent_was_ro_leaf {
                    root_counters.ro_max_width += 1;
                }
            }
        } else if record.reply_to.is_none() {
            let target = record.quote_of.as_ref().unwrap();
            if !self.posts.contains_key(target) {
                self.pending.insert(record.clone());
                return;
            }
            if !self.posts.contains_key(&record.id) {
                let source2 = record.id.clone();
                let parent_data = self.posts.get_mut(target).unwrap();

                // TODO
                // let qo_depth = parent_data.qo_depth + 1;
                // let parent_was_qo_leaf = parent_data.is_qo_leaf;
                // parent_data.is_qo_leaf = false;

                // let tree_data = BskyPostTreeData {
                //     ro_root: None,
                //     qo_root: Some(root.clone()),
                //     ro_depth: 1,
                //     qo_depth: qo_depth,
                //     is_ro_leaf: true,
                //     is_qo_leaf: true,
                // };

                // self.posts.insert(source2, tree_data);

                panic!("TODO");
            }
        }

    }
}

#[cfg(test)]
mod tests {
    use super::{BskyPostData, BskyPostId, BskyPostRecord, BskyPostReplyTo};

    #[test]
    fn test_two_roots() {
        let mut data = BskyPostData::new();
        let id = BskyPostId::from("a", "1");
        let id1 = id.clone();
        data.ingest_record(BskyPostRecord { id, reply_to: None, quote_of: None });

        let id = BskyPostId::from("b", "2");
        let id2 = id.clone();
        data.ingest_record(BskyPostRecord { id, reply_to: None, quote_of: None });

        assert_eq!(data.posts.len(), 2);
        assert_eq!(data.counters.len(), 2);
        assert_eq!(data.pending.len(), 0);

        let counters = data.counters.get(&id1);
        assert!(counters.is_some());
        let counters = counters.unwrap();
        assert_eq!(counters.ro_size, 1);
        assert_eq!(counters.qo_size, 1);
        assert_eq!(counters.ro_max_width, 1);
        assert_eq!(counters.qo_max_width, 1);
        assert_eq!(counters.ro_max_depth, 1);
        assert_eq!(counters.qo_max_depth, 1);

        let counters = data.counters.get(&id2);
        assert!(counters.is_some());
        let counters = counters.unwrap();
        assert_eq!(counters.ro_size, 1);
        assert_eq!(counters.qo_size, 1);
        assert_eq!(counters.ro_max_width, 1);
        assert_eq!(counters.qo_max_width, 1);
        assert_eq!(counters.ro_max_depth, 1);
        assert_eq!(counters.qo_max_depth, 1);
    }

    #[test]
    fn test_simple_thread() {
        let mut data = BskyPostData::new();

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "2");
        let id3 = BskyPostId::from("a", "2");
        data.ingest_record(BskyPostRecord { id: id1.clone(), reply_to: None, quote_of: None });
        data.ingest_record(BskyPostRecord { id: id2.clone(), reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }), quote_of: None });
        data.ingest_record(BskyPostRecord { id: id3.clone(), reply_to: Some(BskyPostReplyTo { target: id2.clone(), root: id1.clone() }), quote_of: None });

        assert_eq!(data.posts.len(), 3);
        assert_eq!(data.counters.len(), 1);
        assert_eq!(data.pending.len(), 0);

        let post1_counters = data.counters.get(&id1);
        assert!(post1_counters.is_some());
        let counters = post1_counters.unwrap();
        assert_eq!(counters.ro_size, 3);
        assert_eq!(counters.qo_size, 1);
        assert_eq!(counters.ro_max_width, 1);
        assert_eq!(counters.qo_max_width, 1);
        assert_eq!(counters.ro_max_depth, 3);
        assert_eq!(counters.qo_max_depth, 1);

        let post2_data = data.posts.get(&id2);
        assert!(post2_data.is_some());
        let post2_data = post2_data.unwrap();
        assert_eq!(post2_data.ro_depth, 2);
        assert_eq!(post2_data.qo_depth, 1);
        assert!(post2_data.ro_root.is_some());
        let post2_ro_root = post2_data.ro_root.as_ref().unwrap();
        assert_eq!(&post2_ro_root.did, &id1.did);
        assert_eq!(&post2_ro_root.rkey, &id1.rkey);

        let post3_data = data.posts.get(&id3);
        assert!(post3_data.is_some());
        let post3_data = post3_data.unwrap();
        assert_eq!(post3_data.ro_depth, 3);
        assert_eq!(post3_data.qo_depth, 1);
        assert!(post3_data.ro_root.is_some());
        let post3_ro_root = post3_data.ro_root.as_ref().unwrap();
        assert_eq!(&post3_ro_root.did, &id1.did);
        assert_eq!(&post3_ro_root.rkey, &id1.rkey);
    }

    #[test]
    fn test_3_replies() {
        let mut data = BskyPostData::new();

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "2");
        let id3 = BskyPostId::from("a", "2");
        let id4 = BskyPostId::from("c", "1");
        data.ingest_record(BskyPostRecord { id: id1.clone(), reply_to: None, quote_of: None });
        data.ingest_record(BskyPostRecord { id: id2.clone(), reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }), quote_of: None });
        data.ingest_record(BskyPostRecord { id: id3.clone(), reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }), quote_of: None });
        data.ingest_record(BskyPostRecord { id: id4.clone(), reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }), quote_of: None });

        assert_eq!(data.posts.len(), 4);
        assert_eq!(data.counters.len(), 1);
        assert_eq!(data.pending.len(), 0);

        let post1_counters = data.counters.get(&id1);
        assert!(post1_counters.is_some());
        let counters = post1_counters.unwrap();
        assert_eq!(counters.ro_size, 4);
        assert_eq!(counters.qo_size, 1);
        assert_eq!(counters.ro_max_width, 3);
        assert_eq!(counters.qo_max_width, 1);
        assert_eq!(counters.ro_max_depth, 2);
        assert_eq!(counters.qo_max_depth, 1);

        let post2_data = data.posts.get(&id2);
        assert!(post2_data.is_some());
        let post2_data = post2_data.unwrap();
        assert_eq!(post2_data.ro_depth, 2);
        assert_eq!(post2_data.qo_depth, 1);
        assert!(post2_data.ro_root.is_some());
        let post2_ro_root = post2_data.ro_root.as_ref().unwrap();
        assert_eq!(&post2_ro_root.did, &id1.did);
        assert_eq!(&post2_ro_root.rkey, &id1.rkey);

        let post3_data = data.posts.get(&id3);
        assert!(post3_data.is_some());
        let post3_data = post3_data.unwrap();
        assert_eq!(post3_data.ro_depth, 2);
        assert_eq!(post3_data.qo_depth, 1);
        assert!(post3_data.ro_root.is_some());
        let post3_ro_root = post3_data.ro_root.as_ref().unwrap();
        assert_eq!(&post3_ro_root.did, &id1.did);
        assert_eq!(&post3_ro_root.rkey, &id1.rkey);
    }

    #[test]
    fn test_3_non_root_replies() {
        let mut data = BskyPostData::new();

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "1");
        let id3 = BskyPostId::from("a", "2");
        let id4 = BskyPostId::from("b", "2");
        let id5 = BskyPostId::from("c", "1");
        data.ingest_record(BskyPostRecord { id: id1.clone(), reply_to: None, quote_of: None });
        data.ingest_record(BskyPostRecord { id: id2.clone(), reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }), quote_of: None });
        data.ingest_record(BskyPostRecord { id: id3.clone(), reply_to: Some(BskyPostReplyTo { target: id2.clone(), root: id1.clone() }), quote_of: None });
        data.ingest_record(BskyPostRecord { id: id4.clone(), reply_to: Some(BskyPostReplyTo { target: id2.clone(), root: id1.clone() }), quote_of: None });
        data.ingest_record(BskyPostRecord { id: id5.clone(), reply_to: Some(BskyPostReplyTo { target: id2.clone(), root: id1.clone() }), quote_of: None });

        assert_eq!(data.posts.len(), 5);
        assert_eq!(data.counters.len(), 1);
        assert_eq!(data.pending.len(), 0);

        let post1_counters = data.counters.get(&id1);
        assert!(post1_counters.is_some());
        let counters = post1_counters.unwrap();
        assert_eq!(counters.ro_size, 5);
        assert_eq!(counters.qo_size, 1);
        assert_eq!(counters.ro_max_width, 3);
        assert_eq!(counters.qo_max_width, 1);
        assert_eq!(counters.ro_max_depth, 3);
        assert_eq!(counters.qo_max_depth, 1);

        let post3_data = data.posts.get(&id3);
        assert!(post3_data.is_some());
        let post3_data = post3_data.unwrap();
        assert_eq!(post3_data.ro_depth, 3);
        assert_eq!(post3_data.qo_depth, 1);
        assert!(post3_data.ro_root.is_some());
        let post3_ro_root = post3_data.ro_root.as_ref().unwrap();
        assert_eq!(&post3_ro_root.did, &id1.did);
        assert_eq!(&post3_ro_root.rkey, &id1.rkey);
    }
}