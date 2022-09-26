#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChannelOrder {
    /// Output the latest videos first
    #[default]
    Latest,
    /// Output the oldest videos first
    Oldest,
    /// Output the most viewed videos first
    Popular,
}
