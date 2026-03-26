//! Concurrency analysis for dataflow graphs.
//!
//! Identifies independent subgraphs (connected components) that can execute
//! in parallel without data races.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::dataflow::block::BlockId;
use crate::dataflow::channel::Channel;

/// A group of blocks that form a connected component in the dataflow graph.
/// All blocks in a group must execute sequentially (they share data),
/// but different groups can execute in parallel.
#[derive(Debug, Clone)]
pub struct ParallelGroup {
    /// Block IDs in topological order within this group.
    pub blocks: Vec<BlockId>,
    /// Channels internal to this group.
    pub channels: Vec<Channel>,
}

/// Analyze a dataflow graph for parallelism.
///
/// Returns a list of independent groups. If the graph is fully connected,
/// returns a single group. If there are N disconnected subgraphs, returns
/// N groups that can safely execute in parallel.
pub fn find_parallel_groups(
    block_ids: &[BlockId],
    channels: &[Channel],
) -> Result<Vec<ParallelGroup>, String> {
    if block_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Build undirected adjacency list.
    let mut adj: HashMap<BlockId, HashSet<BlockId>> =
        block_ids.iter().map(|&id| (id, HashSet::new())).collect();

    for ch in channels {
        if adj.contains_key(&ch.from_block) && adj.contains_key(&ch.to_block) {
            adj.entry(ch.from_block).or_default().insert(ch.to_block);
            adj.entry(ch.to_block).or_default().insert(ch.from_block);
        }
    }

    // BFS to find connected components.
    let mut visited: HashSet<BlockId> = HashSet::new();
    let mut components: Vec<HashSet<BlockId>> = Vec::new();

    // Process blocks in sorted order for determinism.
    let mut sorted_ids: Vec<BlockId> = block_ids.to_vec();
    sorted_ids.sort_by_key(|id| id.0);

    for &start in &sorted_ids {
        if visited.contains(&start) {
            continue;
        }

        let mut component = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(start);
        visited.insert(start);

        while let Some(node) = queue.pop_front() {
            component.insert(node);
            let mut neighbors: Vec<BlockId> = adj
                .get(&node)
                .map(|s| s.iter().copied().collect())
                .unwrap_or_default();
            neighbors.sort_by_key(|id| id.0);
            for neighbor in neighbors {
                if visited.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }

        components.push(component);
    }

    // Build ParallelGroups from components.
    let mut groups: Vec<ParallelGroup> = Vec::with_capacity(components.len());

    for component in &components {
        let component_block_ids: Vec<BlockId> = {
            let mut ids: Vec<BlockId> = component.iter().copied().collect();
            ids.sort_by_key(|id| id.0);
            ids
        };

        let component_channels: Vec<Channel> = channels
            .iter()
            .filter(|ch| component.contains(&ch.from_block) && component.contains(&ch.to_block))
            .cloned()
            .collect();

        let sorted_blocks =
            super::topo::topological_sort(&component_block_ids, &component_channels)?;

        groups.push(ParallelGroup {
            blocks: sorted_blocks,
            channels: component_channels,
        });
    }

    // Sort groups by their smallest block ID for determinism.
    groups.sort_by_key(|g| g.blocks.first().map(|id| id.0).unwrap_or(u32::MAX));

    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dataflow::channel::ChannelId;

    fn ch(id: u32, from: u32, from_port: usize, to: u32, to_port: usize) -> Channel {
        Channel {
            id: ChannelId(id),
            from_block: BlockId(from),
            from_port,
            to_block: BlockId(to),
            to_port,
        }
    }

    #[test]
    fn single_chain_one_group() {
        // A -> B -> C should produce 1 group.
        let ids = vec![BlockId(1), BlockId(2), BlockId(3)];
        let channels = vec![ch(1, 1, 0, 2, 0), ch(2, 2, 0, 3, 0)];
        let groups = find_parallel_groups(&ids, &channels).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].blocks.len(), 3);
        assert_eq!(groups[0].channels.len(), 2);
    }

    #[test]
    fn two_disconnected_chains() {
        // (A -> B) and (C -> D) produce 2 groups.
        let ids = vec![BlockId(1), BlockId(2), BlockId(3), BlockId(4)];
        let channels = vec![ch(1, 1, 0, 2, 0), ch(2, 3, 0, 4, 0)];
        let groups = find_parallel_groups(&ids, &channels).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].blocks, vec![BlockId(1), BlockId(2)]);
        assert_eq!(groups[0].channels.len(), 1);
        assert_eq!(groups[1].blocks, vec![BlockId(3), BlockId(4)]);
        assert_eq!(groups[1].channels.len(), 1);
    }

    #[test]
    fn mixed_connected_disconnected() {
        // (A -> B, A -> C) and (D) produce 2 groups.
        let ids = vec![BlockId(1), BlockId(2), BlockId(3), BlockId(4)];
        let channels = vec![ch(1, 1, 0, 2, 0), ch(2, 1, 0, 3, 0)];
        let groups = find_parallel_groups(&ids, &channels).unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].blocks.len(), 3);
        assert!(groups[0].blocks.contains(&BlockId(1)));
        assert!(groups[0].blocks.contains(&BlockId(2)));
        assert!(groups[0].blocks.contains(&BlockId(3)));
        assert_eq!(groups[1].blocks, vec![BlockId(4)]);
    }

    #[test]
    fn fully_disconnected() {
        // 3 isolated blocks produce 3 groups.
        let ids = vec![BlockId(5), BlockId(2), BlockId(8)];
        let channels: Vec<Channel> = vec![];
        let groups = find_parallel_groups(&ids, &channels).unwrap();
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].blocks, vec![BlockId(2)]);
        assert_eq!(groups[1].blocks, vec![BlockId(5)]);
        assert_eq!(groups[2].blocks, vec![BlockId(8)]);
    }

    #[test]
    fn empty_graph() {
        let groups = find_parallel_groups(&[], &[]).unwrap();
        assert!(groups.is_empty());
    }

    #[test]
    fn groups_are_topo_sorted() {
        // Two chains: (1 -> 3 -> 5) and (2 -> 4).
        // Within each group, blocks must be in topological order.
        let ids = vec![BlockId(1), BlockId(2), BlockId(3), BlockId(4), BlockId(5)];
        let channels = vec![ch(1, 1, 0, 3, 0), ch(2, 3, 0, 5, 0), ch(3, 2, 0, 4, 0)];
        let groups = find_parallel_groups(&ids, &channels).unwrap();
        assert_eq!(groups.len(), 2);

        // Group 0: blocks 1, 3, 5 in topo order.
        assert_eq!(groups[0].blocks, vec![BlockId(1), BlockId(3), BlockId(5)]);
        // Group 1: blocks 2, 4 in topo order.
        assert_eq!(groups[1].blocks, vec![BlockId(2), BlockId(4)]);
    }
}
