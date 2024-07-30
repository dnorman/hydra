pub trait Record: serde::de::DeserializeOwned {
    type ID: Clone;
    fn id(&self) -> &Self::ID;
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum Direction {
    Ascending,
    Descending,
}
impl Direction {
    pub fn inverse(&self) -> Self {
        match self {
            Direction::Ascending => Direction::Descending,
            Direction::Descending => Direction::Ascending,
        }
    }
}

pub enum PaginatedCursor {
    Before(Vec<u8>),
    After(Vec<u8>),
    // TODO so we can switch the display_order without having to know the preceeding/anteceding keys
    // StartingWith(Vec<u8>),
    // EndingWith(Vec<u8>),
}
