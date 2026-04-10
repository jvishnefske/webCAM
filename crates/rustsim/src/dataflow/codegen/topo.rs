//! Topological sort for dataflow graphs using Kahn's algorithm.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::dataflow::block::BlockId;
use crate::dataflow::channel::Channel;

/// Topologically sort block IDs based on channel dependencies.
///
/// Blocks with no incoming edges (sources) appear first. Returns `Err` if a
/// cycle is detected.
///
/// `delay_blocks` contains IDs of blocks that act as z⁻¹ delay elements
/// (e.g. Register blocks). Edges feeding INTO these blocks are excluded from
/// dependency analysis, allowing feedback loops through delay elements.
pub fn topological_sort(
    block_ids: &[BlockId],
    channels: &[Channel],
    delay_blocks: &HashSet<BlockId>,
) -> Result<Vec<BlockId>, String> {
    // Build in-degree map and adjacency list.
    let mut in_degree: HashMap<BlockId, usize> = block_ids.iter().map(|&id| (id, 0)).collect();
    let mut adj: HashMap<BlockId, Vec<BlockId>> =
        block_ids.iter().map(|&id| (id, Vec::new())).collect();

    for ch in channels {
        // Only count edges between blocks that are in the input set.
        if in_degree.contains_key(&ch.from_block) && in_degree.contains_key(&ch.to_block) {
            // Skip edges INTO delay blocks — these are back-edges that break
            // feedback cycles. The delay block outputs the previous tick's value.
            if delay_blocks.contains(&ch.to_block) {
                continue;
            }
            // Avoid counting duplicate edges between the same pair multiple times
            // for in-degree (each channel is a separate dependency).
            *in_degree.entry(ch.to_block).or_insert(0) += 1;
            adj.entry(ch.from_block).or_default().push(ch.to_block);
        }
    }

    // Seed the queue with zero in-degree blocks, sorted by ID for determinism.
    let mut queue: VecDeque<BlockId> = {
        let mut sources: Vec<BlockId> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(&id, _)| id)
            .collect();
        sources.sort_by_key(|id| id.0);
        sources.into_iter().collect()
    };

    let mut result = Vec::with_capacity(block_ids.len());

    while let Some(node) = queue.pop_front() {
        result.push(node);

        // Collect and sort neighbors for deterministic ordering.
        let mut neighbors: Vec<BlockId> = adj.get(&node).cloned().unwrap_or_default();
        neighbors.sort_by_key(|id| id.0);
        neighbors.dedup();

        for &neighbor in &neighbors {
            // Decrement by the actual number of channels from node to neighbor.
            let edge_count = adj
                .get(&node)
                .map(|v| v.iter().filter(|&&n| n == neighbor).count())
                .unwrap_or(0);
            let deg = in_degree.get_mut(&neighbor).unwrap();
            *deg = deg.saturating_sub(edge_count);
            if *deg == 0 {
                queue.push_back(neighbor);
            }
        }
    }

    if result.len() != block_ids.len() {
        Err("cycle detected in dataflow graph".to_string())
    } else {
        Ok(result)
    }
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
    fn simple_chain() {
        // A -> B -> C
        let ids = vec![BlockId(1), BlockId(2), BlockId(3)];
        let channels = vec![ch(1, 1, 0, 2, 0), ch(2, 2, 0, 3, 0)];
        let sorted = topological_sort(&ids, &channels, &HashSet::new()).unwrap();
        assert_eq!(sorted, vec![BlockId(1), BlockId(2), BlockId(3)]);
    }

    #[test]
    fn parallel_branches() {
        // A -> B, A -> C
        let ids = vec![BlockId(1), BlockId(2), BlockId(3)];
        let channels = vec![ch(1, 1, 0, 2, 0), ch(2, 1, 0, 3, 0)];
        let sorted = topological_sort(&ids, &channels, &HashSet::new()).unwrap();
        assert_eq!(sorted[0], BlockId(1));
        // B and C can be in either order, but deterministic by ID sort
        assert_eq!(sorted[1], BlockId(2));
        assert_eq!(sorted[2], BlockId(3));
    }

    #[test]
    fn cycle_detection() {
        // A -> B -> A (cycle)
        let ids = vec![BlockId(1), BlockId(2)];
        let channels = vec![ch(1, 1, 0, 2, 0), ch(2, 2, 0, 1, 0)];
        let result = topological_sort(&ids, &channels, &HashSet::new());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cycle"));
    }

    #[test]
    fn disconnected_blocks() {
        // A, B, C with no channels
        let ids = vec![BlockId(3), BlockId(1), BlockId(2)];
        let channels = vec![];
        let sorted = topological_sort(&ids, &channels, &HashSet::new()).unwrap();
        // All are sources, sorted by ID
        assert_eq!(sorted, vec![BlockId(1), BlockId(2), BlockId(3)]);
    }

    #[test]
    fn empty_graph() {
        let sorted = topological_sort(&[], &[], &HashSet::new()).unwrap();
        assert!(sorted.is_empty());
    }

    #[test]
    fn diamond_dag() {
        // A -> B, A -> C, B -> D, C -> D
        let ids = vec![BlockId(1), BlockId(2), BlockId(3), BlockId(4)];
        let channels = vec![
            ch(1, 1, 0, 2, 0),
            ch(2, 1, 0, 3, 0),
            ch(3, 2, 0, 4, 0),
            ch(4, 3, 0, 4, 1),
        ];
        let sorted = topological_sort(&ids, &channels, &HashSet::new()).unwrap();
        assert_eq!(sorted[0], BlockId(1));
        assert_eq!(sorted[3], BlockId(4));
        // B and C in the middle
        assert!(sorted[1] == BlockId(2) || sorted[1] == BlockId(3));
    }

    #[test]
    fn delay_block_breaks_cycle() {
        // Register(3) -> SM(1) -> Gain(2) -> Register(3) — cycle through delay block
        let ids = vec![BlockId(1), BlockId(2), BlockId(3)];
        let channels = vec![
            ch(1, 3, 0, 1, 0), // Register -> SM
            ch(2, 1, 0, 2, 0), // SM -> Gain
            ch(3, 2, 0, 3, 0), // Gain -> Register (back-edge, excluded)
        ];
        let delay_blocks = HashSet::from([BlockId(3)]);
        let sorted = topological_sort(&ids, &channels, &delay_blocks).unwrap();
        assert_eq!(sorted.len(), 3);
        // Register(3) should come first (no incoming dependencies after back-edge exclusion)
        assert_eq!(sorted[0], BlockId(3));
    }

    #[test]
    fn multiple_channels_same_pair() {
        // A has two output ports both connected to B (different input ports)
        let ids = vec![BlockId(1), BlockId(2)];
        let channels = vec![ch(1, 1, 0, 2, 0), ch(2, 1, 1, 2, 1)];
        let sorted = topological_sort(&ids, &channels, &HashSet::new()).unwrap();
        assert_eq!(sorted, vec![BlockId(1), BlockId(2)]);
    }
}
