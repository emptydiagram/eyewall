use std::{collections::HashMap, hash::Hash};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct BskyPostId {
    did: String,
    rkey: String,
}

impl BskyPostId {
    pub fn new(did: String, rkey: String) -> BskyPostId {
        BskyPostId { did: did, rkey: rkey }
    }
}

impl BskyPostId {
    fn from(did: &str, rkey: &str) -> BskyPostId {
        BskyPostId { did: String::from(did), rkey: String::from(rkey) }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SubgraphType {
    Reply,
    Quote,
    ReplyQuote,
}


#[derive(Clone, Eq, Hash, PartialEq)]
struct BskyPostReplyTo<Id> {
    target: Id,
    root: Id,
}

#[derive(Clone, Eq, Hash, PartialEq)]
struct BskyPostRecord<Id> {
    id: Id,
    reply_to: Option<BskyPostReplyTo<Id>>,
    quote_of: Option<Id>,
}

type UFIndex = u32;
type UFSize = u32;

#[derive(Debug)]
struct BskyPostUnionFind<Id> {
    id_to_index: HashMap<Id, UFIndex>,
    parents_ro: HashMap<UFIndex, UFIndex>,
    parents_qo: HashMap<UFIndex, UFIndex>,
    parents_rq: HashMap<UFIndex, UFIndex>,
    sizes_ro: HashMap<UFIndex, UFSize>,
    sizes_qo: HashMap<UFIndex, UFSize>,
    sizes_rq: HashMap<UFIndex, UFSize>,
    next_index: UFIndex,
}

impl<Id: Clone + Eq + Hash> BskyPostUnionFind<Id> {
    pub fn new() -> BskyPostUnionFind<Id> {
        BskyPostUnionFind {
            id_to_index: HashMap::new(),
            parents_ro: HashMap::new(),
            parents_qo: HashMap::new(),
            parents_rq: HashMap::new(),
            sizes_ro: HashMap::new(),
            sizes_qo: HashMap::new(),
            sizes_rq: HashMap::new(),
            next_index: 0,
        }
    }

    fn get_index(&mut self, id: &Id) -> UFIndex {
        match self.id_to_index.get(id) {
            None => {
                let idx = self.next_index;
                self.id_to_index.insert(id.clone(), idx);
                self.next_index += 1;
                idx
            },
            Some(idx) => *idx,
        }
    }

    pub fn ingest_post(&mut self, record: BskyPostRecord<Id>) -> bool {
        if record.reply_to.is_none() && record.quote_of.is_none() {
            return false;
        }
        let post_idx = self.get_index(&record.id);

        if let Some(ref reply_to) = record.reply_to {
            let root_idx = self.get_index(&reply_to.root);
            let parent_idx = self.get_index(&reply_to.target);

            // assuming that root has a parents_ro entry iff it has a sizes_ro entry.
            // this is *only true* iff the root of the reply tree is always the UF root.
            // but by construction it is, since we never have to call union() or find()
            // for a reply tree.

            let root_ro_size = match self.parents_ro.insert(root_idx, root_idx) {
                None => 1,
                Some(_) => *self.sizes_ro.get(&root_idx).unwrap(),
            };
            let mut new_root_ro_size = root_ro_size;
            match self.parents_ro.insert(parent_idx, root_idx) {
                None => {
                    new_root_ro_size += 1;
                },
                Some(_) => (),
            }
            match self.parents_ro.insert(post_idx, root_idx) {
                None => {
                    new_root_ro_size += 1;
                },
                Some(_) => (),
            }

            if new_root_ro_size > root_ro_size {
                self.sizes_ro.insert(root_idx, new_root_ro_size);
            }

            let mut rq_to_add = Vec::new();
            if !self.parents_rq.contains_key(&post_idx) {
                rq_to_add.push(post_idx);
            }
            if !self.parents_rq.contains_key(&root_idx) {
                rq_to_add.push(root_idx);
            }
            if parent_idx != root_idx && !self.parents_rq.contains_key(&parent_idx) {
                rq_to_add.push(parent_idx);
            }

            for idx in rq_to_add {
                self.parents_rq.insert(idx, idx);
                self.sizes_rq.insert(idx, 1);
            }
            if parent_idx != root_idx {
                self.union_rq(root_idx, parent_idx);
            }
            self.union_rq(parent_idx, post_idx);
        }

        if let Some(ref quote_of) = record.quote_of {
            let parent_idx = self.get_index(&quote_of);
            // assuming: if it's not in parents_qo, it's also not in sizes_qo

            match (self.parents_qo.get(&parent_idx), self.parents_qo.get(&post_idx)) {
                (None, None) => {
                    self.parents_qo.insert(parent_idx, parent_idx);
                    self.parents_qo.insert(post_idx, parent_idx);
                    self.sizes_qo.insert(parent_idx, 2);
                },
                (None, Some(post_parent)) => {
                    let post_parent_idx = *post_parent;
                    self.parents_qo.insert(parent_idx, parent_idx);
                    self.sizes_qo.insert(parent_idx, 1);
                    self.union_qo(parent_idx, post_parent_idx);
                },
                (Some(parent_parent), None) => {
                    let parent_root_idx = self.find_qo(*parent_parent).unwrap();
                    self.parents_qo.insert(post_idx, parent_root_idx);
                    self.sizes_qo.entry(parent_root_idx).and_modify(|e| *e += 1);
                },
                (Some(parent_parent), Some(post_parent)) => {
                    self.union_qo(*parent_parent, *post_parent);
                },
            }

            let mut rq_to_add = Vec::new();
            if !self.parents_rq.contains_key(&post_idx) {
                rq_to_add.push(post_idx);
            }
            if !self.parents_rq.contains_key(&parent_idx) {
                rq_to_add.push(parent_idx);
            }
            for idx in rq_to_add {
                self.parents_rq.insert(idx, idx);
                self.sizes_rq.insert(idx, 1);
            }
            self.union_rq(parent_idx, post_idx);

        }

        true
    }

    fn find(parents: &mut HashMap<UFIndex, UFIndex>, idx: UFIndex) -> Option<UFIndex> {
        let mut curr_idx = idx;
        let mut pa_idx = *parents.get(&curr_idx)?;
        while curr_idx != pa_idx {
            curr_idx = pa_idx;
            pa_idx = *parents.get(&curr_idx).unwrap();
        }
        let rep = curr_idx;

        curr_idx = idx;
        pa_idx = *parents.get(&curr_idx).unwrap();
        while curr_idx != pa_idx {
            parents.insert(curr_idx, rep);
            curr_idx = pa_idx;
            pa_idx = *parents.get(&curr_idx).unwrap();
        }
        Some(curr_idx)
    }

    // find the representative node for a given index
    fn find_qo(&mut self, idx: UFIndex) -> Option<UFIndex> {
        Self::find(&mut self.parents_qo, idx)
    }

    fn find_rq(&mut self, idx: UFIndex) -> Option<UFIndex> {
        Self::find(&mut self.parents_rq, idx)
    }


    fn union_qo(&mut self, idx1: UFIndex, idx2: UFIndex) {
        let pa1 = self.find_qo(idx1).unwrap();
        let pa2 = self.find_qo(idx2).unwrap();

        if pa1 == pa2 {
            return;
        }

        let pa1_size = self.sizes_qo.get(&pa1).unwrap();
        let pa2_size = self.sizes_qo.get(&pa2).unwrap();

        if pa1_size >= pa2_size {
            self.parents_qo.insert(pa2, pa1);
            self.sizes_qo.insert(pa1, pa1_size + pa2_size);
            self.sizes_qo.remove(&pa2);
        } else {
            self.parents_qo.insert(pa1, pa2);
            self.sizes_qo.insert(pa2, pa1_size + pa2_size);
            self.sizes_qo.remove(&pa1);
        }
    }

    fn union_rq(&mut self, idx1: UFIndex, idx2: UFIndex) {
        let pa1 = self.find_rq(idx1).unwrap();
        let pa2 = self.find_rq(idx2).unwrap();

        if pa1 == pa2 {
            return;
        }

        let pa1_size = self.sizes_rq.get(&pa1).unwrap();
        let pa2_size = self.sizes_rq.get(&pa2).unwrap();

        if pa1_size >= pa2_size {
            self.parents_rq.insert(pa2, pa1);
            self.sizes_rq.insert(pa1, pa1_size + pa2_size);
            self.sizes_rq.remove(&pa2);
        } else {
            self.parents_rq.insert(pa1, pa2);
            self.sizes_rq.insert(pa2, pa1_size + pa2_size);
            self.sizes_rq.remove(&pa1);
        }
    }

    pub fn component_size(&mut self, subgraph_type: SubgraphType, id: &Id) -> Option<UFSize> {
        let idx = self.id_to_index.get(id).map(|idx| *idx)?;
        match subgraph_type {
            SubgraphType::Reply => {
                // special structure of replies means all non-root nodes are direct children of root
                let pa = *self.parents_ro.get(&idx)?;
                self.sizes_ro.get(&pa).map(|idx| *idx)
            },
            SubgraphType::Quote => {
                let pa = self.find_qo(idx)?;
                self.sizes_qo.get(&pa).map(|idx| *idx)
            },
            SubgraphType::ReplyQuote => {
                let pa = self.find_rq(idx)?;
                self.sizes_rq.get(&pa).map(|idx| *idx)
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use itertools::Itertools;

    use super::{BskyPostId, BskyPostRecord, BskyPostReplyTo, BskyPostUnionFind, SubgraphType};

    #[test]
    fn test_simple_reply_thread_1() {
        let mut uf = BskyPostUnionFind::new();

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "1");
        let id3 = BskyPostId::from("a", "2");
        let id4 = BskyPostId::from("a", "3");
        uf.ingest_post(BskyPostRecord {
            id: id2.clone(),
            reply_to: Some(super::BskyPostReplyTo { target: id1.clone(), root: id1.clone() }),
            quote_of: None
        });
        uf.ingest_post(BskyPostRecord {
            id: id3.clone(),
            reply_to: Some(super::BskyPostReplyTo { target: id2.clone(), root: id1.clone() }),
            quote_of: None
        });
        uf.ingest_post(BskyPostRecord {
            id: id4.clone(),
            reply_to: Some(super::BskyPostReplyTo { target: id3.clone(), root: id1.clone() }),
            quote_of: None
        });

        assert_eq!(uf.component_size(SubgraphType::Reply, &id1), Some(4));
        assert_eq!(uf.component_size(SubgraphType::Reply, &id2), Some(4));
        assert_eq!(uf.component_size(SubgraphType::Reply, &id3), Some(4));
        assert_eq!(uf.component_size(SubgraphType::Reply, &id4), Some(4));
        assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id1), Some(4));
        assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id2), Some(4));
        assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id3), Some(4));
        assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id4), Some(4));

    }

    #[test]
    fn test_simple_reply_thread_2() {
        let mut uf = BskyPostUnionFind::new();

        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "1");
        let id3 = BskyPostId::from("a", "2");
        let id4 = BskyPostId::from("a", "3");
        uf.ingest_post(BskyPostRecord {
            id: id4.clone(),
            reply_to: Some(super::BskyPostReplyTo { target: id3.clone(), root: id1.clone() }),
            quote_of: None
        });
        uf.ingest_post(BskyPostRecord {
            id: id3.clone(),
            reply_to: Some(super::BskyPostReplyTo { target: id2.clone(), root: id1.clone() }),
            quote_of: None
        });
        uf.ingest_post(BskyPostRecord {
            id: id2.clone(),
            reply_to: Some(super::BskyPostReplyTo { target: id1.clone(), root: id1.clone() }),
            quote_of: None
        });

        assert_eq!(uf.component_size(SubgraphType::Reply, &id1), Some(4));
        assert_eq!(uf.component_size(SubgraphType::Reply, &id2), Some(4));
        assert_eq!(uf.component_size(SubgraphType::Reply, &id3), Some(4));
        assert_eq!(uf.component_size(SubgraphType::Reply, &id4), Some(4));
        assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id1), Some(4));
        assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id2), Some(4));
        assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id3), Some(4));
        assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id4), Some(4));

    }

    #[test]
    fn test_simple_quote_chain() {
        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "1");
        let id3 = BskyPostId::from("a", "2");
        let id4 = BskyPostId::from("a", "3");
        let mut records: Vec<BskyPostRecord<BskyPostId>> = vec![
            BskyPostRecord {
                id: id2.clone(),
                reply_to: None,
                quote_of: Some(id1.clone())
            },
            BskyPostRecord {
                id: id3.clone(),
                reply_to: None,
                quote_of: Some(id2.clone())
            },
            BskyPostRecord {
                id: id4.clone(),
                reply_to: None,
                quote_of: Some(id3.clone())
            }
        ];

        for (i, perm) in records.iter().permutations(records.len()).enumerate() {
            let mut uf = BskyPostUnionFind::new();
            uf.ingest_post(perm[0].clone());
            uf.ingest_post(perm[1].clone());
            uf.ingest_post(perm[2].clone());
            assert_eq!(uf.component_size(SubgraphType::Quote, &id1), Some(4));
            assert_eq!(uf.component_size(SubgraphType::Quote, &id2), Some(4));
            assert_eq!(uf.component_size(SubgraphType::Quote, &id3), Some(4));
            assert_eq!(uf.component_size(SubgraphType::Quote, &id4), Some(4));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id1), Some(4));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id2), Some(4));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id3), Some(4));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id4), Some(4));
        }



    }

    #[test]
    fn test_simple_reply_quote_graph_1() {
        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "1");
        let id3 = BskyPostId::from("a", "2");
        let id4 = BskyPostId::from("a", "3");
        let mut records: Vec<BskyPostRecord<BskyPostId>> = vec![
            BskyPostRecord {
                id: id2.clone(),
                reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }),
                quote_of: None
            },
            BskyPostRecord {
                id: id3.clone(),
                reply_to: Some(BskyPostReplyTo { target: id2.clone(), root: id1.clone() }),
                quote_of: None
            },
            BskyPostRecord {
                id: id4.clone(),
                reply_to: None,
                quote_of: Some(id3.clone())
            }
        ];

        for perm in records.iter().permutations(records.len()) {
            let mut uf = BskyPostUnionFind::new();
            uf.ingest_post(perm[0].clone());
            uf.ingest_post(perm[1].clone());
            uf.ingest_post(perm[2].clone());
            assert_eq!(uf.component_size(SubgraphType::Reply, &id1), Some(3));
            assert_eq!(uf.component_size(SubgraphType::Reply, &id2), Some(3));
            assert_eq!(uf.component_size(SubgraphType::Reply, &id3), Some(3));
            assert_eq!(uf.component_size(SubgraphType::Reply, &id4), None);
            assert_eq!(uf.component_size(SubgraphType::Quote, &id3), Some(2));
            assert_eq!(uf.component_size(SubgraphType::Quote, &id4), Some(2));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id1), Some(4));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id2), Some(4));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id3), Some(4));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id4), Some(4));
        }
    }

    #[test]
    fn test_simple_reply_quote_graph_2() {
        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "1");
        let id3 = BskyPostId::from("a", "2");
        let id4 = BskyPostId::from("a", "3");
        let mut records: Vec<BskyPostRecord<BskyPostId>> = vec![
            BskyPostRecord {
                id: id2.clone(),
                reply_to: Some(BskyPostReplyTo { target: id1.clone(), root: id1.clone() }),
                quote_of: None
            },
            BskyPostRecord {
                id: id4.clone(),
                reply_to: Some(BskyPostReplyTo { target: id3.clone(), root: id3.clone() }),
                quote_of: Some(id2.clone())
            },
        ];

        for (i, perm) in records.iter().permutations(records.len()).enumerate() {
            let mut uf = BskyPostUnionFind::new();
            uf.ingest_post(perm[0].clone());
            uf.ingest_post(perm[1].clone());
            assert_eq!(uf.component_size(SubgraphType::Reply, &id1), Some(2));
            assert_eq!(uf.component_size(SubgraphType::Reply, &id2), Some(2));
            assert_eq!(uf.component_size(SubgraphType::Reply, &id3), Some(2));
            assert_eq!(uf.component_size(SubgraphType::Reply, &id4), Some(2));
            assert_eq!(uf.component_size(SubgraphType::Quote, &id1), None);
            assert_eq!(uf.component_size(SubgraphType::Quote, &id2), Some(2));
            assert_eq!(uf.component_size(SubgraphType::Quote, &id3), None);
            assert_eq!(uf.component_size(SubgraphType::Quote, &id4), Some(2));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id1), Some(4));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id2), Some(4));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id3), Some(4));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id4), Some(4));
        }

    }

    #[test]
    fn test_reply_quote_graph() {
        let id1 = BskyPostId::from("a", "1");
        let id2 = BskyPostId::from("b", "1");
        let id3 = BskyPostId::from("c", "1");
        let id4 = BskyPostId::from("a", "2");
        let id5 = BskyPostId::from("a", "3");
        let id6 = BskyPostId::from("a", "4");
        let mut records: Vec<BskyPostRecord<BskyPostId>> = vec![
            BskyPostRecord {
                id: id3.clone(),
                reply_to: Some(BskyPostReplyTo { target: id2.clone(), root: id1.clone() }),
                quote_of: None
            },
            BskyPostRecord {
                id: id6.clone(),
                reply_to: Some(BskyPostReplyTo { target: id5.clone(), root: id4.clone() }),
                quote_of: Some(id3.clone())
            },
        ];

        for (i, perm) in records.iter().permutations(records.len()).enumerate() {
            let mut uf = BskyPostUnionFind::new();
            uf.ingest_post(perm[0].clone());
            uf.ingest_post(perm[1].clone());
            assert_eq!(uf.component_size(SubgraphType::Reply, &id1), Some(3));
            assert_eq!(uf.component_size(SubgraphType::Reply, &id2), Some(3));
            assert_eq!(uf.component_size(SubgraphType::Reply, &id3), Some(3));
            assert_eq!(uf.component_size(SubgraphType::Reply, &id4), Some(3));
            assert_eq!(uf.component_size(SubgraphType::Reply, &id5), Some(3));
            assert_eq!(uf.component_size(SubgraphType::Reply, &id6), Some(3));
            assert_eq!(uf.component_size(SubgraphType::Quote, &id1), None);
            assert_eq!(uf.component_size(SubgraphType::Quote, &id2), None);
            assert_eq!(uf.component_size(SubgraphType::Quote, &id3), Some(2));
            assert_eq!(uf.component_size(SubgraphType::Quote, &id4), None);
            assert_eq!(uf.component_size(SubgraphType::Quote, &id5), None);
            assert_eq!(uf.component_size(SubgraphType::Quote, &id6), Some(2));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id1), Some(6));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id2), Some(6));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id3), Some(6));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id4), Some(6));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id5), Some(6));
            assert_eq!(uf.component_size(SubgraphType::ReplyQuote, &id6), Some(6));
        }

    }
}