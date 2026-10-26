/// Stores the relevant information for the "state" of a visualisation at a given timestamp
/// This does not have all the information to "save the state" of the system, as it depends on previous packets,
/// but it is enough to reconstruct the visualisation at a given timestamp
/// Total size so far:
/// 2w x 2h x 8 bytes
pub struct Checkpoint {
    /// Vector size w x h which stores the timestamp of the last polarity off event
    /// Used in voxel grids
    /// Size = width x height x 4 bytes
    last_polarity_off: Vec<u64>,
    /// Vector size w x h which stores the timestamp of the last polarity on event
    /// Used in time surfaces as well as voxel grids
    /// Size = width x height x 4 bytes
    last_polarity_on: Vec<u64>,
}
