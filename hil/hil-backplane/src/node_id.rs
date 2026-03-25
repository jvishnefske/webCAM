//! Node identifier for backplane participants.

/// A unique identifier for a node on the backplane.
///
/// Wraps a `u32` value. Node IDs are assigned by the user and must be
/// unique within a backplane network.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(u32);

impl NodeId {
    /// Creates a new node identifier.
    pub const fn new(id: u32) -> Self {
        Self(id)
    }

    /// Returns the raw `u32` value.
    pub const fn raw(self) -> u32 {
        self.0
    }
}

impl<C> minicbor::Encode<C> for NodeId {
    fn encode<W: minicbor::encode::Write>(
        &self,
        e: &mut minicbor::Encoder<W>,
        _ctx: &mut C,
    ) -> Result<(), minicbor::encode::Error<W::Error>> {
        e.u32(self.0)?;
        Ok(())
    }
}

impl<'b, C> minicbor::Decode<'b, C> for NodeId {
    fn decode(
        d: &mut minicbor::Decoder<'b>,
        _ctx: &mut C,
    ) -> Result<Self, minicbor::decode::Error> {
        Ok(Self(d.u32()?))
    }
}
