use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fmt};

// ulid and a sha256 hash for lexicographic ordering
// When merging two IDs, use the earliest timestamp
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ID {
    timestamp: i64,
    hash: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Event {
    id: ID, // what should this be?
    precursors: BTreeSet<ID>,
}

#[derive(Debug)]
struct Node {
    basis: BTreeSet<Event>,
}

impl ID {
    fn new(precursors: &BTreeSet<ID>) -> Self {
        let timestamp = chrono::Utc::now().timestamp();
        Self::with_ts(timestamp, precursors)
    }
    fn with_ts(timestamp: i64, precursors: &BTreeSet<ID>) -> Self {
        let mut hasher = Sha256::new();
        // later this will include event payload as well - but timestamp is serving double duty here for the PoC
        hasher.update(timestamp.to_be_bytes());
        for precursor in precursors {
            hasher.update(precursor.hash);
        }
        let hash = hasher.finalize().into();
        Self { timestamp, hash }
    }
    // include the last 2 digits of the timestamp (decimal) and the last 2 digits of the hash (hex)
    fn human_readable(&self) -> String {
        let ts = self.timestamp.to_string();
        let len = ts.len();
        let ts_last_2 = &ts[len - 3.min(len)..];
        let hash = hex::encode(self.hash);
        let hash_last_2 = &hash[hash.len() - 3.min(hash.len())..];
        format!("{}.{}", ts_last_2, hash_last_2)
    }
}

impl fmt::Display for ID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.human_readable())
    }
}

impl Event {
    fn new(precursors: BTreeSet<ID>) -> Self {
        let id = ID::new(&precursors);
        Self { id, precursors }
    }
    fn with_ts(timestamp: i64, precursors: BTreeSet<ID>) -> Self {
        let id = ID::with_ts(timestamp, &precursors);
        Self { id, precursors }
    }
    fn merge(&self, other: Event) -> Event {
        let timestamp = self.id.timestamp.max(other.id.timestamp);
        let mut precursors = self.precursors.clone();
        precursors.extend(other.precursors);
        Event {
            id: ID::with_ts(timestamp, &precursors),
            precursors,
        }
    }
}
impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}[{}]",
            self.id.human_readable(),
            self.precursors
                .iter()
                .map(|id| id.human_readable())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

impl Node {
    fn new() -> Self {
        Self {
            basis: BTreeSet::new(),
        }
    }
    fn with_seed(seed: &Event) -> Self {
        let mut node = Self::new();
        node.basis.insert(seed.clone());
        node
    }
    fn new_event(&mut self, ts: i64, precursors: BTreeSet<ID>) -> Event {
        let event = Event::with_ts(ts, precursors);
        self.merge_or_insert(event.clone());
        event
    }
    fn receive_events<'a, I>(&mut self, events: I)
    where
        I: IntoIterator<Item = &'a Event>,
    {
        for event in events {
            self.merge_or_insert(event.clone());
        }
    }
    // If the event can be merged with an existing event, merge them and replace the existing event with the merged event
    fn merge_or_insert(&mut self, event: Event) {
        if let Some(overlap) = self
            .basis
            .iter()
            .find(|e| e.precursors.contains(&event.id))
            .cloned()
        {
            let merged_event = overlap.merge(event);
            self.basis.remove(&overlap);
            self.basis.insert(merged_event);
        } else {
            self.basis.insert(event);
        }
    }
    fn readable_basis(&self) -> String {
        self.basis
            .iter()
            .map(|e| e.id.human_readable())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl fmt::Display for Node {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Node({})", self.readable_basis())
    }
}

fn main() {
    println!("Hello, merkle-dag world!");

    let seed = Event::with_ts(0, BTreeSet::new());

    // Imagine three events, A, B, and C, each independently generated by different nodes. These events are linked in a DAG structure, where each event points to its precursors.
    let mut a = Node::with_seed(&seed);
    let mut b = Node::with_seed(&seed);
    let mut c = Node::with_seed(&seed);

    // current state:
    // 0 (seed event)

    println!("a: {a}");
    println!("b: {b}");
    println!("c: {c}");
    let e1 = a.new_event(1, BTreeSet::new());
    let _e2 = b.new_event(2, BTreeSet::new());
    let _e3 = c.new_event(3, BTreeSet::new());

    println!("e1: {}", e1);

    println!("a: {a}");
    println!("b: {b}");
    println!("c: {c}");

    // a->b, b->c, compare a <> c
    // Pretend we are sending events from A to B over the network
    b.receive_events(&a.basis);
    c.receive_events(&b.basis);
    a.receive_events(&c.basis);
    b.receive_events(&a.basis);

    println!("a: {a}");
    println!("b: {b}");
    println!("c: {c}");
    assert_eq!(a.basis, b.basis);
    assert_eq!(a.basis, c.basis);

    // Current state:
    //   0
    // / | \
    // 1 2 3
    //
    // a: Node(0.dfc, 1.a50, 2.f70, 3.975)
    // b: Node(0.dfc, 1.a50, 2.f70, 3.975)
    // c: Node(0.dfc, 1.a50, 2.f70, 3.975)

    // TODO: cause 0 to be subsumed by 1, 2, and 3 individually
    // then cause 1,2,3 to be merged into 4, eliding each.
    // TODO: determine what happens if someone references 1, 2, 3 after they are elided.
    // How to we construct either: Strictures that prevent them from knowing about the elided events
    // Or some sort of apology layer
}
