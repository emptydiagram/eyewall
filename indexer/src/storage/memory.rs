
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
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
    ro_sid: Option<SubgraphId>,
    qo_sid: Option<SubgraphId>,
    rq_sid: Option<SubgraphId>,
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
            ro_sid: None,
            qo_sid: None,
            rq_sid: None,
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


type SubgraphId = u32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SubgraphType {
    Reply,
    Quote,
    ReplyQuote,
}

#[derive(Clone)]
struct SubgraphInfo {
    ty: SubgraphType,
    sources: Vec<BskyPostId>,
    size: u32,
    max_width: u32,
    max_depth: u32,
}

impl SubgraphInfo {
    fn new(ty: SubgraphType, sources: Vec<BskyPostId>, size: u32, max_width: u32, max_depth: u32) -> SubgraphInfo {
        SubgraphInfo {
            ty,
            sources,
            size,
            max_width,
            max_depth
        }
    }
}


struct BskyGraphData {
    posts: HashMap<BskyPostId, BskyPostGraphData>,
    pending: HashSet<BskyPostRecord>,
    subgraphs: HashMap<SubgraphId, SubgraphInfo>,
    subgraph_nodes: HashMap<SubgraphId, Vec<BskyPostId>>,
    next_subgraph_id: u32,
}

impl BskyGraphData {
    fn new() -> BskyGraphData {
        BskyGraphData {
            posts: HashMap::new(),
            pending: HashSet::new(),
            subgraphs: HashMap::new(),
            subgraph_nodes: HashMap::new(),
            next_subgraph_id: 1,
        }
    }

    fn make_subgraph(&mut self, post_ids: Vec<BskyPostId>, subgraph_type: SubgraphType, size: u32, max_width: u32, max_depth: u32) -> SubgraphId {
        let subgraph_id = self.next_subgraph_id;
        self.next_subgraph_id += 1;

        self.subgraphs.insert(subgraph_id, SubgraphInfo::new(subgraph_type, post_ids.clone(), size, max_width, max_depth));

        self.subgraph_nodes.insert(subgraph_id, post_ids);
        subgraph_id
    }

    fn ingest_record(&mut self, record: BskyPostRecord) {
        // invariant: if a post has been processed, all ancestors have also
        if self.posts.contains_key(&record.id) {
            return;
        }
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

            // not pending, didn't process yet

            // calculate depths for the new node, update parent leaf flags
            let parent_data = self.posts.get_mut(&reply_to.target).unwrap();
            let ro_depth = parent_data.ro_depth + 1;
            let rq_max_depth = parent_data.rq_max_depth + 1;
            let parent_was_ro_leaf = parent_data.is_ro_leaf;
            let parent_was_rq_leaf = parent_data.is_rq_leaf;
            let parent_qo_sid_maybe = parent_data.qo_sid;
            let parent_rq_sid_maybe = parent_data.rq_sid;
            parent_data.is_ro_leaf = false;
            parent_data.is_rq_leaf = false;

            let ro_sid: SubgraphId;

            // parent is a reply root iff ro_sid is None. we defer creating a subgraph until we have a parent/child
            // relationship, so we have to create it here
            // if parent_data.ro_sid.is_none() && !self.source_subgraphs.contains_key(&reply_to.target) {
            if parent_data.ro_sid.is_none() {
                let post_ids = vec![reply_to.target.clone()];
                ro_sid = self.make_subgraph( post_ids, SubgraphType::Reply, 1, 1, 1);
                let parent_post = self.posts.get_mut(&reply_to.target).unwrap();
                parent_post.ro_sid = Some(ro_sid);
                // don't insert RQ subgraph yet. only do it if it's actually different from the R subgraph,
                // which happens if some post in the R subgraph gets quoted
            } else {
                ro_sid = parent_data.ro_sid.unwrap();
            }

            // if we replied to a post in quote subgraph and the corresponding RQ graph doesnt exist
            let mut rq_sid_maybe: Option<SubgraphId> = None;
            if parent_qo_sid_maybe.is_some() && parent_rq_sid_maybe.is_none() {
                let qo_sid = parent_qo_sid_maybe.unwrap();
                let qo_subgraph = self.subgraphs.get(&qo_sid).expect("Expected quote subgraph");
                let post_ids = qo_subgraph.sources.clone();
                let sid = self.make_subgraph(post_ids, SubgraphType::ReplyQuote, qo_subgraph.size, qo_subgraph.max_width, qo_subgraph.max_depth);
                rq_sid_maybe = Some(sid);

                for node in self.subgraph_nodes.get(&qo_sid).expect("Expected subgraph nodes") {
                    let post = self.posts.get_mut(node).unwrap();
                    post.rq_sid = rq_sid_maybe;
                }
            } else if parent_rq_sid_maybe.is_some() {
                rq_sid_maybe = parent_rq_sid_maybe;
            }

            let tree_data = BskyPostGraphData {
                ro_sid: Some(ro_sid),
                qo_sid: None,
                rq_sid: rq_sid_maybe,
                ro_depth: ro_depth,
                qo_depth: 1,
                rq_max_depth: rq_max_depth,
                is_ro_leaf: true,
                is_qo_leaf: true,
                is_rq_leaf: true
            };

            self.posts.insert(record.id.clone(), tree_data);

            self.subgraph_nodes.entry(ro_sid).and_modify(|e| e.push(record.id.clone()));

            // the new post we just inserted is part of at most two subgraphs:
            //  - reply subgraph
            //  - reply-quote subgraph
            // find each of these, if they exist, and update the appropriate stats

            let ro_subgraph = self.subgraphs
                .get_mut(&ro_sid)
                .expect("Expected reply subgraph to exist for reply");

            ro_subgraph.size += 1;
            ro_subgraph.max_depth = ro_subgraph.max_depth.max(ro_depth);
            if !parent_was_ro_leaf {
                ro_subgraph.max_width += 1;
            }

            if let Some(rq_sid) = rq_sid_maybe {
                let rq_subgraph = self.subgraphs
                    .get_mut(&rq_sid)
                    .expect("Expected reply-quote subgraph to exist for reply");

                rq_subgraph.size += 1;
                rq_subgraph.max_depth = rq_subgraph.max_depth.max(rq_max_depth);
                if !parent_was_rq_leaf {
                    rq_subgraph.max_width += 1;
                }
            }

        } else if record.reply_to.is_none() {
            let target = record.quote_of.as_ref().unwrap();
            if !self.posts.contains_key(target) {
                self.pending.insert(record.clone());
                return;
            }

            // not pending, didn't process yet

            // calculate depths for the new node, update parent leaf flags
            let parent_data = self.posts.get_mut(target).unwrap();
            let qo_depth = parent_data.qo_depth + 1;
            let rq_max_depth = parent_data.rq_max_depth + 1;
            let parent_was_qo_leaf = parent_data.is_qo_leaf;
            let parent_was_rq_leaf = parent_data.is_rq_leaf;
            let parent_ro_sid_maybe = parent_data.ro_sid;
            let parent_rq_sid_maybe = parent_data.rq_sid;
            parent_data.is_qo_leaf = false;
            parent_data.is_rq_leaf = false;

            let qo_sid: SubgraphId;

            // parent is a quote root iff qo_sid is None. we defer creating a subgraph until we have a parent/child
            // relationship, so we have to create it here
            // if parent_data.ro_sid.is_none() && !self.source_subgraphs.contains_key(&reply_to.target) {
            if parent_data.qo_sid.is_none() {
                let post_ids = vec![target.clone()];
                qo_sid = self.make_subgraph( post_ids, SubgraphType::Quote, 1, 1, 1);
                let parent_post = self.posts.get_mut(target).unwrap();
                parent_post.qo_sid = Some(qo_sid);
                // don't insert RQ subgraph yet. only do it if it's actually different from the R subgraph,
                // which happens if some post in the R subgraph gets quoted
            } else {
                qo_sid = parent_data.qo_sid.unwrap();
            }


            // if we quoted a post in reply subgraph and the corresponding RQ graph doesnt exist
            let mut rq_sid_maybe: Option<SubgraphId> = None;
            if parent_ro_sid_maybe.is_some() && parent_rq_sid_maybe.is_none() {
                let ro_sid = parent_ro_sid_maybe.unwrap();
                let ro_subgraph = self.subgraphs.get(&ro_sid).expect("Expected reply subgraph");
                let post_ids = ro_subgraph.sources.clone();
                let sid = self.make_subgraph(post_ids, SubgraphType::ReplyQuote, ro_subgraph.size, ro_subgraph.max_width, ro_subgraph.max_depth);
                rq_sid_maybe = Some(sid);

                for node in self.subgraph_nodes.get(&ro_sid).expect("Expected subgraph nodes") {
                    let post = self.posts.get_mut(node).unwrap();
                    post.rq_sid = rq_sid_maybe;
                }
            } else if parent_rq_sid_maybe.is_some() {
                rq_sid_maybe = parent_rq_sid_maybe;
            }


            let tree_data = BskyPostGraphData {
                ro_sid: None,
                qo_sid: Some(qo_sid),
                rq_sid: rq_sid_maybe,
                ro_depth: 1,
                qo_depth: qo_depth,
                rq_max_depth: rq_max_depth,
                is_ro_leaf: true,
                is_qo_leaf: true,
                is_rq_leaf: true
            };

            self.posts.insert(record.id.clone(), tree_data);

            self.subgraph_nodes.entry(qo_sid).and_modify(|e| e.push(record.id.clone()));

            // the new post we just inserted is part of at most two subgraphs:
            //  - reply subgraph
            //  - reply-quote subgraph
            // find each of these, if they exist, and update the appropriate stats

            let qo_subgraph = self.subgraphs
                .get_mut(&qo_sid)
                .expect("Expected reply subgraph to exist for reply");

            qo_subgraph.size += 1;
            qo_subgraph.max_depth = qo_subgraph.max_depth.max(qo_depth);
            if !parent_was_qo_leaf {
                qo_subgraph.max_width += 1;
            }

            if let Some(rq_sid) = rq_sid_maybe {
                let rq_subgraph = self.subgraphs
                    .get_mut(&rq_sid)
                    .expect("Expected reply-quote subgraph to exist for reply");

                rq_subgraph.size += 1;
                rq_subgraph.max_depth = rq_subgraph.max_depth.max(rq_max_depth);
                if !parent_was_rq_leaf {
                    rq_subgraph.max_width += 1;
                }
            }

        } else {
            let reply_to = record.reply_to.as_ref().unwrap();
            let quote_target = record.quote_of.as_ref().unwrap();

            if !self.posts.contains_key(&reply_to.target) || !self.posts.contains_key(quote_target) {
                self.pending.insert(record.clone());
                return;
            }

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
        assert_eq!(data.subgraphs.len(), 0);

        let post1 = data.posts.get(&id1);
        assert!(post1.is_some());
        let post = post1.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 1);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 1);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);
    }

    #[test]
    fn test_simple_reply_thread() {
        let mut data = BskyGraphData::new();

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "2");
        let id3 = BskyPostId::from("a", "2");
        data.ingest_record(BskyPostRecord { id: id1.clone(), reply_to: None, quote_of: None });
        data.ingest_record(BskyPostRecord { id: id2.clone(), reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }), quote_of: None });
        data.ingest_record(BskyPostRecord { id: id3.clone(), reply_to: Some(BskyPostReplyTo { target: id2.clone(), root: id1.clone() }), quote_of: None });

        assert_eq!(data.posts.len(), 3);
        assert_eq!(data.pending.len(), 0);
        assert_eq!(data.subgraphs.len(), 1);

        let post1 = data.posts.get(&id1);
        assert!(post1.is_some());
        let post = post1.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 1);
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 1);
        assert!(!post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 1);
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 2);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 2);
        assert!(!post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post3 = data.posts.get(&id3);
        assert!(post3.is_some());
        let post = post3.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 1);
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 3);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 3);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let sid = 1;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Reply);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 3);
        assert_eq!(sg.max_width, 1);
        assert_eq!(sg.max_depth, 3);
    }

    #[test]
    fn test_simple_quote_thread() {
        let mut data = BskyGraphData::new();

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "2");
        let id3 = BskyPostId::from("a", "2");
        data.ingest_record(BskyPostRecord { id: id1.clone(), reply_to: None, quote_of: None });
        data.ingest_record(BskyPostRecord { id: id2.clone(), reply_to: None, quote_of: Some(id1.clone()) });
        data.ingest_record(BskyPostRecord { id: id3.clone(), reply_to: None, quote_of: Some(id2.clone()) });

        assert_eq!(data.posts.len(), 3);
        assert_eq!(data.pending.len(), 0);
        assert_eq!(data.subgraphs.len(), 1);

        let post1 = data.posts.get(&id1);
        assert!(post1.is_some());
        let post = post1.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 1);
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 1);
        assert!(post.is_ro_leaf);
        assert!(!post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 1);
        assert!(post.rq_sid.is_none());

        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 2);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(!post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post3 = data.posts.get(&id3);
        assert!(post3.is_some());
        let post = post3.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 1);
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 3);
        assert_eq!(post.rq_max_depth, 3);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let sid = 1;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Quote);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 3);
        assert_eq!(sg.max_width, 1);
        assert_eq!(sg.max_depth, 3);
    }

    #[test]
    fn test_simple_quote_reply_graph_1() {
        let mut data = BskyGraphData::new();

        //  1
        //  \Q\
        //    2
        //   |R|
        //    3

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "2");
        let id3 = BskyPostId::from("a", "2");
        data.ingest_record(BskyPostRecord { id: id1.clone(), reply_to: None, quote_of: None });
        data.ingest_record(BskyPostRecord { id: id2.clone(), reply_to: None, quote_of: Some(id1.clone()) });
        data.ingest_record(BskyPostRecord { id: id3.clone(), reply_to: Some(BskyPostReplyTo { target: id2.clone(), root: id2.clone() }), quote_of: None });

        assert_eq!(data.posts.len(), 3);
        assert_eq!(data.pending.len(), 0);
        assert_eq!(data.subgraphs.len(), 3);

        let post1 = data.posts.get(&id1);
        assert!(post1.is_some());
        let post = post1.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 1);
        assert!(post.rq_sid.is_some());
        assert_eq!(post.rq_sid.unwrap(), 3);
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 1);
        assert!(post.is_ro_leaf);
        assert!(!post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 2);
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 1);
        assert!(post.rq_sid.is_some());
        assert_eq!(post.rq_sid.unwrap(), 3);
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 2);
        assert_eq!(post.rq_max_depth, 2);
        assert!(!post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post3 = data.posts.get(&id3);
        assert!(post3.is_some());
        let post = post3.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 2);
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_some());
        assert_eq!(post.rq_sid.unwrap(), 3);
        assert_eq!(post.ro_depth, 2);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 3);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let sid = 1;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Quote);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 2);
        assert_eq!(sg.max_width, 1);
        assert_eq!(sg.max_depth, 2);

        let sid = 2;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Reply);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id2);
        assert_eq!(sg.size, 2);
        assert_eq!(sg.max_width, 1);
        assert_eq!(sg.max_depth, 2);

        let sid = 3;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::ReplyQuote);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 3);
        assert_eq!(sg.max_width, 1);
        assert_eq!(sg.max_depth, 3);

    }

    #[test]
    fn test_simple_quote_reply_graph_2() {
        let mut data = BskyGraphData::new();

        //  1
        // |R|
        //  2
        //  \Q\
        //    3

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "2");
        let id3 = BskyPostId::from("a", "2");
        data.ingest_record(BskyPostRecord { id: id1.clone(), reply_to: None, quote_of: None });
        data.ingest_record(BskyPostRecord { id: id2.clone(), reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }), quote_of: None });
        data.ingest_record(BskyPostRecord { id: id3.clone(), reply_to: None, quote_of: Some(id2.clone()) });

        assert_eq!(data.posts.len(), 3);
        assert_eq!(data.pending.len(), 0);
        assert_eq!(data.subgraphs.len(), 3);

        let post1 = data.posts.get(&id1);
        assert!(post1.is_some());
        let post = post1.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 1);
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_some());
        assert_eq!(post.rq_sid.unwrap(), 3);
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 1);
        assert!(!post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 1);
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 2);
        assert!(post.rq_sid.is_some());
        assert_eq!(post.rq_sid.unwrap(), 3);
        assert_eq!(post.ro_depth, 2);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(!post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post3 = data.posts.get(&id3);
        assert!(post3.is_some());
        let post = post3.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 2);
        assert!(post.rq_sid.is_some());
        assert_eq!(post.rq_sid.unwrap(), 3);
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 2);
        assert_eq!(post.rq_max_depth, 3);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let sid = 1;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Reply);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 2);
        assert_eq!(sg.max_width, 1);
        assert_eq!(sg.max_depth, 2);

        let sid = 2;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Quote);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id2);
        assert_eq!(sg.size, 2);
        assert_eq!(sg.max_width, 1);
        assert_eq!(sg.max_depth, 2);

        let sid = 3;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::ReplyQuote);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 3);
        assert_eq!(sg.max_width, 1);
        assert_eq!(sg.max_depth, 3);
    }

    #[test]
    fn test_simple_quote_reply_graph_3() {
        let mut data = BskyGraphData::new();

        //  1
        //  \Q\
        // |R| 2
        //  3

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "2");
        let id3 = BskyPostId::from("a", "2");
        data.ingest_record(BskyPostRecord { id: id1.clone(), reply_to: None, quote_of: None });
        data.ingest_record(BskyPostRecord { id: id2.clone(), reply_to: None, quote_of: Some(id1.clone()) });
        data.ingest_record(BskyPostRecord { id: id3.clone(), reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }), quote_of: None });

        assert_eq!(data.posts.len(), 3);
        assert_eq!(data.pending.len(), 0);
        assert_eq!(data.subgraphs.len(), 3);

        let post1 = data.posts.get(&id1);
        assert!(post1.is_some());
        let post = post1.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 2);
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 1);
        assert!(post.rq_sid.is_some());
        assert_eq!(post.rq_sid.unwrap(), 3);
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 1);
        assert!(!post.is_ro_leaf);
        assert!(!post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 1);
        assert!(post.rq_sid.is_some());
        assert_eq!(post.rq_sid.unwrap(), 3);
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 2);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let post3 = data.posts.get(&id3);
        assert!(post3.is_some());
        let post = post3.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 2);
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_some());
        assert_eq!(post.rq_sid.unwrap(), 3);
        assert_eq!(post.ro_depth, 2);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let sid = 1;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Quote);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 2);
        assert_eq!(sg.max_width, 1);
        assert_eq!(sg.max_depth, 2);

        let sid = 2;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Reply);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 2);
        assert_eq!(sg.max_width, 1);
        assert_eq!(sg.max_depth, 2);

        let sid = 3;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::ReplyQuote);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 3);
        assert_eq!(sg.max_width, 2);
        assert_eq!(sg.max_depth, 2);
    }

    #[test]
    fn test_simple_quote_reply_graph_4() {
        let mut data = BskyGraphData::new();

        //  1
        //  \Q\
        // |R| 3
        //  2

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "2");
        let id3 = BskyPostId::from("a", "2");
        data.ingest_record(BskyPostRecord { id: id1.clone(), reply_to: None, quote_of: None });
        data.ingest_record(BskyPostRecord { id: id2.clone(), reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }), quote_of: None });
        data.ingest_record(BskyPostRecord { id: id3.clone(), reply_to: None, quote_of: Some(id1.clone()) });

        assert_eq!(data.posts.len(), 3);
        assert_eq!(data.pending.len(), 0);
        assert_eq!(data.subgraphs.len(), 3);

        let post1 = data.posts.get(&id1);
        assert!(post1.is_some());
        let post = post1.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 1);
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 2);
        assert!(post.rq_sid.is_some());
        assert_eq!(post.rq_sid.unwrap(), 3);
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 1);
        assert!(!post.is_ro_leaf);
        assert!(!post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 1);
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_some());
        assert_eq!(post.rq_sid.unwrap(), 3);
        assert_eq!(post.ro_depth, 2);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let post3 = data.posts.get(&id3);
        assert!(post3.is_some());
        let post = post3.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 2);
        assert!(post.rq_sid.is_some());
        assert_eq!(post.rq_sid.unwrap(), 3);
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 2);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let sid = 1;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Reply);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 2);
        assert_eq!(sg.max_width, 1);
        assert_eq!(sg.max_depth, 2);

        let sid = 2;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Quote);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 2);
        assert_eq!(sg.max_width, 1);
        assert_eq!(sg.max_depth, 2);

        let sid = 3;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::ReplyQuote);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 3);
        assert_eq!(sg.max_width, 2);
        assert_eq!(sg.max_depth, 2);
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
        assert_eq!(data.subgraphs.len(), 1);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 1);
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 2);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let post3 = data.posts.get(&id3);
        assert!(post3.is_some());
        let post = post3.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 1);
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 2);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let post4 = data.posts.get(&id4);
        assert!(post4.is_some());
        let post = post4.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 1);
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 2);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let sid = 1;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Reply);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 4);
        assert_eq!(sg.max_width, 3);
        assert_eq!(sg.max_depth, 2);
    }

    #[test]
    fn test_3_root_quotes() {
        let mut data = BskyGraphData::new();

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "2");
        let id3 = BskyPostId::from("a", "2");
        let id4 = BskyPostId::from("c", "1");
        data.ingest_record(BskyPostRecord { id: id1.clone(), reply_to: None, quote_of: None });
        data.ingest_record(BskyPostRecord { id: id2.clone(), reply_to: None, quote_of: Some(id1.clone()) });
        data.ingest_record(BskyPostRecord { id: id3.clone(), reply_to: None, quote_of: Some(id1.clone()) });
        data.ingest_record(BskyPostRecord { id: id4.clone(), reply_to: None, quote_of: Some(id1.clone()) });

        assert_eq!(data.posts.len(), 4);
        assert_eq!(data.pending.len(), 0);
        assert_eq!(data.subgraphs.len(), 1);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 1);
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 2);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let post3 = data.posts.get(&id3);
        assert!(post3.is_some());
        let post = post3.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 1);
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 2);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let post4 = data.posts.get(&id4);
        assert!(post4.is_some());
        let post = post4.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 1);
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 2);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let sid = 1;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Quote);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 4);
        assert_eq!(sg.max_width, 3);
        assert_eq!(sg.max_depth, 2);
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
        assert_eq!(data.subgraphs.len(), 1);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 1);
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 2);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 2);
        assert!(!post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post3 = data.posts.get(&id3);
        assert!(post3.is_some());
        let post = post3.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 1);
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 3);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 3);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let post4 = data.posts.get(&id4);
        assert!(post4.is_some());
        let post = post4.unwrap();
        assert!(post.ro_sid.is_some());
        assert_eq!(post.ro_sid.unwrap(), 1);
        assert!(post.qo_sid.is_none());
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 3);
        assert_eq!(post.qo_depth, 1);
        assert_eq!(post.rq_max_depth, 3);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let sid = 1;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Reply);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 5);
        assert_eq!(sg.max_width, 3);
        assert_eq!(sg.max_depth, 3);
    }

    #[test]
    fn test_3_non_root_quotes() {
        let mut data = BskyGraphData::new();

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "1");
        let id3 = BskyPostId::from("a", "2");
        let id4 = BskyPostId::from("b", "2");
        let id5 = BskyPostId::from("c", "1");
        data.ingest_record(BskyPostRecord { id: id1.clone(), reply_to: None, quote_of: None });
        data.ingest_record(BskyPostRecord { id: id2.clone(), reply_to: None, quote_of: Some(id1.clone()) });
        data.ingest_record(BskyPostRecord { id: id3.clone(), reply_to: None, quote_of: Some(id2.clone()) });
        data.ingest_record(BskyPostRecord { id: id4.clone(), reply_to: None, quote_of: Some(id2.clone()) });
        data.ingest_record(BskyPostRecord { id: id5.clone(), reply_to: None, quote_of: Some(id2.clone()) });

        assert_eq!(data.posts.len(), 5);
        assert_eq!(data.pending.len(), 0);
        assert_eq!(data.subgraphs.len(), 1);

        let post2 = data.posts.get(&id2);
        assert!(post2.is_some());
        let post = post2.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 1);
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 2);
        assert_eq!(post.rq_max_depth, 2);
        assert!(post.is_ro_leaf);
        assert!(!post.is_qo_leaf);
        assert!(!post.is_rq_leaf);

        let post3 = data.posts.get(&id3);
        assert!(post3.is_some());
        let post = post3.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 1);
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 3);
        assert_eq!(post.rq_max_depth, 3);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let post4 = data.posts.get(&id4);
        assert!(post4.is_some());
        let post = post4.unwrap();
        assert!(post.ro_sid.is_none());
        assert!(post.qo_sid.is_some());
        assert_eq!(post.qo_sid.unwrap(), 1);
        assert!(post.rq_sid.is_none());
        assert_eq!(post.ro_depth, 1);
        assert_eq!(post.qo_depth, 3);
        assert_eq!(post.rq_max_depth, 3);
        assert!(post.is_ro_leaf);
        assert!(post.is_qo_leaf);
        assert!(post.is_rq_leaf);

        let sid = 1;
        let sg_maybe = data.subgraphs.get(&sid);
        assert!(sg_maybe.is_some());
        let sg = sg_maybe.unwrap();
        assert_eq!(sg.ty, SubgraphType::Quote);
        assert_eq!(sg.sources.len(), 1);
        assert_eq!(&sg.sources[0], &id1);
        assert_eq!(sg.size, 5);
        assert_eq!(sg.max_width, 3);
        assert_eq!(sg.max_depth, 3);
    }
}