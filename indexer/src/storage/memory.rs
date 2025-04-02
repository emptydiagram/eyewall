
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

struct BskyPostGraphData {
    ro_root: Option<BskyPostId>,
    qo_root: Option<BskyPostId>,
    ro_depth: u32,
    qo_depth: u32,
    // with the reply-quote graph, there isn't a well-defined depth, so we
    // only speak of max-depth
    rq_max_depth: u32,
    is_ro_leaf: bool,
    is_qo_leaf: bool,
    is_rq_leaf: bool,
}

impl BskyPostGraphData {
    fn new_root() -> BskyPostGraphData {
        BskyPostGraphData {
            ro_root: None,
            qo_root: None,
            ro_depth: 1,
            qo_depth: 1,
            rq_max_depth: 1,
            is_ro_leaf: true,
            is_qo_leaf: true,
            is_rq_leaf: true
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

struct SubgraphStats {
    size: u32,
    max_width: u32,
    max_depth: u32,
}

impl SubgraphStats {
    fn new() -> SubgraphStats {
        SubgraphStats {
            size: 1,
            max_width: 1,
            max_depth: 1
        }
    }
}

type SubgraphId = u32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SubgraphType {
    Reply,
    Quote,
    ReplyQuote,
}

#[derive(Clone, Copy)]
struct SubgraphInfo {
    id: SubgraphId,
    ty: SubgraphType,
}

struct BskyGraphData {
    posts: HashMap<BskyPostId, BskyPostGraphData>,
    pending: HashSet<BskyPostRecord>,
    subgraph_stats: HashMap<SubgraphId, SubgraphStats>,
    source_subgraphs: HashMap<BskyPostId, Vec<SubgraphInfo>>,
    next_subgraph_id: u32,
}

impl BskyGraphData {
    fn new() -> BskyGraphData {
        BskyGraphData {
            posts: HashMap::new(),
            pending: HashSet::new(),
            subgraph_stats: HashMap::new(),
            source_subgraphs: HashMap::new(),
            next_subgraph_id: 1,
        }
    }

    fn make_subgraph(&mut self, post_id: BskyPostId, subgraph_type: SubgraphType) -> SubgraphId {
        let subgraph_id = self.next_subgraph_id + 1;
        self.next_subgraph_id += 1;

        self.subgraph_stats.insert(subgraph_id, SubgraphStats::new());
        let sources = self.source_subgraphs.entry(post_id).or_insert(Vec::new());
        sources.push(SubgraphInfo { id: subgraph_id, ty: subgraph_type });
        subgraph_id
    }

    fn ingest_record(&mut self, record: BskyPostRecord) {
        // invariant: if a post has been processed, all ancestors have also
        if record.reply_to.is_none() && record.quote_of.is_none() {
            let record_id = record.id.clone();
            self.posts.insert(record.id, BskyPostGraphData::new_root());
            // defer making subgraph until we have a parent/child relationship
        } else if record.quote_of.is_none() {
            let reply_to = record.reply_to.as_ref().unwrap();
            if !self.posts.contains_key(&reply_to.target) {
                self.pending.insert(record.clone());
                return;
            }
            if self.posts.contains_key(&record.id) {
                return;
            }

            // not pending, didn't process yet

            // calculate depths for the new node, update parent leaf flags
            let parent_data = self.posts.get_mut(&reply_to.target).unwrap();
            let ro_depth = parent_data.ro_depth + 1;
            let rq_max_depth = parent_data.rq_max_depth + 1;
            let parent_was_ro_leaf = parent_data.is_ro_leaf;
            let parent_was_rq_leaf = parent_data.is_rq_leaf;
            parent_data.is_ro_leaf = false;
            parent_data.is_rq_leaf = false;
            let tree_data = BskyPostGraphData {
                ro_root: Some(reply_to.root.clone()),
                qo_root: None,
                ro_depth: ro_depth,
                qo_depth: 1,
                rq_max_depth: rq_max_depth,
                is_ro_leaf: true,
                is_qo_leaf: true,
                is_rq_leaf: true
            };

            // parent is a reply root iff ro_root is None. we defer creating a subgraph until we have a parent/child
            // relationship, so we have to create it here
            let mut ro_sid_maybe = None;
            if parent_data.ro_root.is_none() && !self.source_subgraphs.contains_key(&reply_to.target) {
                let sid = self.make_subgraph(reply_to.target.clone(), SubgraphType::Reply);
                ro_sid_maybe = Some(sid);
                // don't insert RQ subgraph yet. only do it if it's actually different from the R subgraph,
                // which happens if some post in the R subgraph gets quoted
            }

            self.posts.insert(record.id.clone(), tree_data);

            // the new post we just inserted is part of at most two subgraphs:
            //  - reply subgraph
            //  - reply-quote subgraph
            // find each of these, if they exist, and update the appropriate stats

            if ro_sid_maybe.is_none() {
                // if we're here, it's a reply to a non-reply-root post, so the subgraph already exists
                let root_subgraphs = self.source_subgraphs
                    .get(&reply_to.root)
                    .expect("Expected reply root to have subgraph");
                for sg in root_subgraphs {
                    if sg.ty == SubgraphType::Reply {
                        ro_sid_maybe = Some(sg.id);
                        break;
                    }
                }
            }

            let ro_sid = ro_sid_maybe.expect("Expected reply subgraph to exist for reply");

            let ro_sg_stats = self.subgraph_stats
                .get_mut(&ro_sid)
                .expect("Expected reply subgraph stats to exist for reply");

            ro_sg_stats.size += 1;
            ro_sg_stats.max_depth = ro_sg_stats.max_depth.max(ro_depth);
            if !parent_was_ro_leaf {
                ro_sg_stats.max_width += 1;
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
        } else {
            panic!("TODO");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{BskyGraphData, BskyPostId, BskyPostRecord, BskyPostReplyTo, SubgraphType};

    #[test]
    fn test_two_roots() {
        let mut data = BskyGraphData::new();
        let id = BskyPostId::from("a", "1");
        let id1 = id.clone();
        data.ingest_record(BskyPostRecord { id, reply_to: None, quote_of: None });

        let id = BskyPostId::from("b", "2");
        let id2 = id.clone();
        data.ingest_record(BskyPostRecord { id, reply_to: None, quote_of: None });

        assert_eq!(data.posts.len(), 2);
        assert_eq!(data.pending.len(), 0);
        assert_eq!(data.subgraph_stats.len(), 0);
        assert_eq!(data.source_subgraphs.len(), 0);

        let post1 = data.posts.get(&id1);
        assert!(post1.is_some());
        let post = post1.unwrap();
        assert!(post.ro_root.is_none());
        assert!(post.qo_root.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 1);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_root.is_none());
        assert!(post.qo_root.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 1);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);
    }

    #[test]
    fn test_simple_thread() {
        let mut data = BskyGraphData::new();

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "2");
        let id3 = BskyPostId::from("a", "2");
        data.ingest_record(BskyPostRecord { id: id1.clone(), reply_to: None, quote_of: None });
        data.ingest_record(BskyPostRecord { id: id2.clone(), reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }), quote_of: None });
        data.ingest_record(BskyPostRecord { id: id3.clone(), reply_to: Some(BskyPostReplyTo { target: id2.clone(), root: id1.clone() }), quote_of: None });

        assert_eq!(data.posts.len(), 3);
        assert_eq!(data.pending.len(), 0);
        assert_eq!(data.subgraph_stats.len(), 1);
        assert_eq!(data.source_subgraphs.len(), 1);

        let post1 = data.posts.get(&id1);
        assert!(post1.is_some());
        let post = post1.unwrap();
        assert!(post.ro_root.is_none());
        assert!(post.qo_root.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 1);
        assert!(!post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_root.is_some());
        let post_ro_root = post.ro_root.as_ref().unwrap();
        assert_eq!(&post_ro_root.did, &id1.did);
        assert_eq!(&post_ro_root.rkey, &id1.rkey);
        assert!(post.qo_root.is_none());
        assert_eq!(post.ro_depth, 2);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 2);
        assert!(!post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post3 = data.posts.get(&id3);
        assert!(post3.is_some());
        let post = post3.unwrap();
        assert!(post.ro_root.is_some());
        let post_ro_root = post.ro_root.as_ref().unwrap();
        assert_eq!(&post_ro_root.did, &id1.did);
        assert_eq!(&post_ro_root.rkey, &id1.rkey);
        assert!(post.qo_root.is_none());
        assert_eq!(post.ro_depth, 3);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 3);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let sub_infos_maybe = data.source_subgraphs.get(&id1);
        assert!(sub_infos_maybe.is_some());
        let sub_infos = sub_infos_maybe.unwrap();
        assert_eq!(sub_infos.len(), 1);
        let sub_info = sub_infos[0];
        assert_eq!(sub_info.ty, SubgraphType::Reply);
        let sid = sub_info.id;
        let stats_maybe = data.subgraph_stats.get(&sid);
        assert!(stats_maybe.is_some());
        let stats = stats_maybe.unwrap();
        assert_eq!(stats.size, 3);
        assert_eq!(stats.max_width, 1);
        assert_eq!(stats.max_depth, 3);
    }

    #[test]
    fn test_3_root_replies() {
        let mut data = BskyGraphData::new();

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "2");
        let id3 = BskyPostId::from("a", "2");
        let id4 = BskyPostId::from("c", "1");
        data.ingest_record(BskyPostRecord { id: id1.clone(), reply_to: None, quote_of: None });
        data.ingest_record(BskyPostRecord { id: id2.clone(), reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }), quote_of: None });
        data.ingest_record(BskyPostRecord { id: id3.clone(), reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }), quote_of: None });
        data.ingest_record(BskyPostRecord { id: id4.clone(), reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }), quote_of: None });

        assert_eq!(data.posts.len(), 4);
        assert_eq!(data.pending.len(), 0);
        assert_eq!(data.subgraph_stats.len(), 1);
        assert_eq!(data.source_subgraphs.len(), 1);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_root.is_some());
        let post_ro_root = post.ro_root.as_ref().unwrap();
        assert_eq!(&post_ro_root.did, &id1.did);
        assert_eq!(&post_ro_root.rkey, &id1.rkey);
        assert!(post.qo_root.is_none());
        assert_eq!(post.ro_depth, 2);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let post3 = data.posts.get(&id3);
        assert!(post3.is_some());
        let post = post3.unwrap();
        assert!(post.ro_root.is_some());
        let post_ro_root = post.ro_root.as_ref().unwrap();
        assert_eq!(&post_ro_root.did, &id1.did);
        assert_eq!(&post_ro_root.rkey, &id1.rkey);
        assert!(post.qo_root.is_none());
        assert_eq!(post.ro_depth, 2);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let post4 = data.posts.get(&id4);
        assert!(post4.is_some());
        let post = post4.unwrap();
        assert!(post.ro_root.is_some());
        let post_ro_root = post.ro_root.as_ref().unwrap();
        assert_eq!(&post_ro_root.did, &id1.did);
        assert_eq!(&post_ro_root.rkey, &id1.rkey);
        assert!(post.qo_root.is_none());
        assert_eq!(post.ro_depth, 2);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let sub_infos_maybe = data.source_subgraphs.get(&id1);
        assert!(sub_infos_maybe.is_some());
        let sub_infos = sub_infos_maybe.unwrap();
        assert_eq!(sub_infos.len(), 1);
        let sub_info = sub_infos[0];
        assert_eq!(sub_info.ty, SubgraphType::Reply);
        let sid = sub_info.id;
        let stats_maybe = data.subgraph_stats.get(&sid);
        assert!(stats_maybe.is_some());
        let stats = stats_maybe.unwrap();
        assert_eq!(stats.size, 4);
        assert_eq!(stats.max_width, 3);
        assert_eq!(stats.max_depth, 2);
    }

    #[test]
    fn test_3_non_root_replies() {
        let mut data = BskyGraphData::new();

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
        assert_eq!(data.pending.len(), 0);
        assert_eq!(data.subgraph_stats.len(), 1);
        assert_eq!(data.source_subgraphs.len(), 1);

        let sub_infos_maybe = data.source_subgraphs.get(&id1);
        assert!(sub_infos_maybe.is_some());
        let sub_infos = sub_infos_maybe.unwrap();
        assert_eq!(sub_infos.len(), 1);
        let sub_info = sub_infos[0];
        assert_eq!(sub_info.ty, SubgraphType::Reply);
        let sid = sub_info.id;
        let stats_maybe = data.subgraph_stats.get(&sid);
        assert!(stats_maybe.is_some());
        let stats = stats_maybe.unwrap();
        assert_eq!(stats.size, 5);
        assert_eq!(stats.max_width, 3);
        assert_eq!(stats.max_depth, 3);

        let post4 = data.posts.get(&id4);
        assert!(post4.is_some());
        let post = post4.unwrap();
        assert!(post.ro_root.is_some());
        let post_ro_root = post.ro_root.as_ref().unwrap();
        assert_eq!(&post_ro_root.did, &id1.did);
        assert_eq!(&post_ro_root.rkey, &id1.rkey);
        assert!(post.qo_root.is_none());
        assert_eq!(post.ro_depth, 3);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 3);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);
    }
}