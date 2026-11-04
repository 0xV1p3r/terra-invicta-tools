//! Layered left → right layout of a prerequisite graph.
//!
//! * Column = longest path from a root using only non-alternative edges
//!   (alternative edges are drawn but do not push nodes right), starting from a
//!   minimum column given by the log research cost (like the game's
//!   PlaceTechsBehindLowerCosts, so the tree reads cheap → expensive and columns stay
//!   short). Empty columns are dropped. Cycles are broken deterministically (lowest
//!   unplaced index first).
//! * Rows: barycenter heuristic over alternating left→right / right→left sweeps;
//!   the ordering with the fewest edge crossings (straight lines) is kept.
//! * Connected components are grouped (largest first) inside every column.
//! * Coordinates: the column order is kept but y is continuous, like the game's
//!   align-to-prereqs / align-to-unlocks / push-apart passes. Alternating passes move
//!   each node toward the weighted mean of its prereqs (forward) or dependents
//!   (backward), then resolve overlaps per column with an order-preserving
//!   least-squares pack (pool adjacent violators) at min distance `NODE_HEIGHT + GAP_Y`.
//!   A final forward pass aligns nodes to their prereqs.
//! * Long edges: an edge spanning several columns gets one dummy node per column it
//!   passes (standard Sugiyama step). Dummies take part in ordering and coordinates
//!   with a smaller height (`DUMMY_SIZE` rows), so edges run between cards instead of
//!   through them; segments touching dummies weigh more in alignment so long edges
//!   stay straight. Each dummy becomes a pair of waypoints (left/right column edge).
//! * Optional groups (e.g. research categories): every group becomes a horizontal band
//!   of its own (components inside), stacked in group order with `GROUP_GAP`. Dummies
//!   of an edge between groups belong to the source's group for the first half of the
//!   edge and to the target's for the rest.
//! * Components are stacked vertically with `COMPONENT_GAP` (isolated nodes as one
//!   plain column); min y is 0. Positions are top-left corners.
//!
//! Output depends only on the input order of nodes and edges (callers pre-sort).

/// Card size used by the frontend.
pub const NODE_WIDTH: f64 = 300.0;
pub const NODE_HEIGHT: f64 = 100.0;
pub const GAP_X: f64 = 180.0;
pub const GAP_Y: f64 = 24.0;
pub const COMPONENT_GAP: f64 = 2.0 * NODE_HEIGHT;
/// Space between group bands.
pub const GROUP_GAP: f64 = 4.0 * NODE_HEIGHT;
const SWEEPS: usize = 12;
/// Columns spanned by the research-cost range (see [`layout_by_cost`]).
const COST_COLUMNS: usize = 30;
const ROUNDS: usize = 12;
const ALT_WEIGHT: f64 = 0.5;
/// Weight of a node's current position when it has no neighbors on the aligned side.
const STAY_WEIGHT: f64 = 0.01;
/// Height of an edge dummy in rows (`NODE_HEIGHT + GAP_Y`); cards are 1.
const DUMMY_SIZE: f64 = 0.12;
/// Alignment weight factors for segments with one / two dummy ends (Gansner et al.).
const DUMMY_WEIGHT: f64 = 2.0;
const DUMMY_DUMMY_WEIGHT: f64 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutEdge {
    /// Index of the prerequisite node.
    pub from: usize,
    /// Index of the node requiring it.
    pub to: usize,
    pub alternative: bool,
}

/// Result of [`route`].
#[derive(Debug, Clone, PartialEq)]
pub struct Layout {
    /// Indexed like the nodes.
    pub nodes: Vec<Placement>,
    /// Indexed like the edges: points the edge passes between its endpoints, left to
    /// right (pairs at the left/right side of each column it crosses). Empty = direct.
    pub waypoints: Vec<Vec<[f64; 2]>>,
    /// Edge crossings of the routed drawing (straight segments).
    pub crossings: usize,
    /// Indexed like the groups passed to [`route_grouped`]: `(top, bottom)` of the band in
    /// px, or `None` if the group is empty. Empty without groups.
    pub groups: Vec<Option<(f64, f64)>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub column: usize,
    pub row: usize,
    pub x: f64,
    pub y: f64,
}

/// Lay out `n` nodes (indices `0..n`) connected by `edges`; result is indexed like the nodes.
#[cfg(test)]
pub fn layout(n: usize, edges: &[LayoutEdge]) -> Vec<Placement> {
    layout_by_cost(n, edges, &[])
}

/// [`layout`] where research `costs` (indexed like the nodes; empty = ignore) also push
/// nodes right, so columns read cheap → expensive like the game's tech tree.
#[cfg(test)]
pub fn layout_by_cost(n: usize, edges: &[LayoutEdge], costs: &[f64]) -> Vec<Placement> {
    route(n, edges, costs).nodes
}

/// [`layout_by_cost`] with waypoints for edges spanning more than one column.
#[cfg(test)]
pub fn route(n: usize, edges: &[LayoutEdge], costs: &[f64]) -> Layout {
    route_grouped(n, edges, costs, &[])
}

/// [`route`] with every node in a band of its `group` (indexed like the nodes; empty = no bands).
pub fn route_grouped(n: usize, edges: &[LayoutEdge], costs: &[f64], groups: &[usize]) -> Layout {
    let real_column = compact(columns_from(n, edges, &cost_floor(costs, n)));
    let (column, segments, chains) = expand(&real_column, edges);
    let total = column.len();
    let ncols = column.iter().max().map_or(0, |m| m + 1);
    let mut cols: Vec<Vec<usize>> = vec![Vec::new(); ncols];
    for (v, &c) in column.iter().enumerate() {
        cols[c].push(v);
    }
    order(&mut cols, &column, &segments, total);
    let comp = components(total, &segments);
    let grouped = groups.len() == n && n > 0;
    // Blocks packed and stacked separately: components, or (group, component) pairs.
    let (block, block_group) = if grouped {
        let mut group = vec![0; total];
        group[..n].copy_from_slice(groups);
        for (e, chain) in edges.iter().zip(&chains) {
            for (i, &d) in chain.iter().enumerate() {
                group[d] = if 2 * i < chain.len() { groups[e.from] } else { groups[e.to] };
            }
        }
        let mut keys: Vec<(usize, usize)> = (0..total).map(|v| (group[v], comp[v])).collect();
        keys.sort_unstable();
        keys.dedup();
        let block = (0..total).map(|v| keys.binary_search(&(group[v], comp[v])).unwrap()).collect();
        (block, keys.iter().map(|k| k.0).collect())
    } else {
        let ncomp = comp.iter().max().map_or(0, |m| m + 1);
        (comp, vec![0; ncomp])
    };
    for col in &mut cols {
        col.sort_by_key(|&v| block[v]);
    }
    let size: Vec<f64> = (0..total).map(|v| if v < n { 1.0 } else { DUMMY_SIZE }).collect();
    let pos = coordinates(&cols, &column, &segments, &block, &size, n);
    let crossings = crossings_of(&pos, &column, &segments);
    let (all, bands) = place(&cols, &pos, &block, &block_group, &size, n);
    let groups = if grouped {
        let ngroups = groups.iter().max().map_or(0, |m| m + 1);
        (0..ngroups).map(|g| bands.get(&g).copied()).collect()
    } else {
        Vec::new()
    };
    let waypoints = chains
        .iter()
        .map(|chain| {
            chain
                .iter()
                .flat_map(|&d| {
                    let p = all[d];
                    let y = p.y + NODE_HEIGHT / 2.0;
                    [[p.x, y], [p.x + NODE_WIDTH, y]]
                })
                .collect()
        })
        .collect();
    let mut nodes = all;
    nodes.truncate(n);
    Layout { nodes, waypoints, crossings, groups }
}

/// Split edges spanning several columns into unit segments through dummy nodes
/// (indices `n..`). Returns the column of every node, the segments (plus edges that
/// are not split: same column or right to left) and each edge's dummies in order.
fn expand(column: &[usize], edges: &[LayoutEdge]) -> (Vec<usize>, Vec<LayoutEdge>, Vec<Vec<usize>>) {
    let mut column = column.to_vec();
    let mut segments = Vec::with_capacity(edges.len());
    let mut chains = Vec::with_capacity(edges.len());
    for e in edges {
        let (cf, ct) = (column[e.from], column[e.to]);
        let mut chain = Vec::new();
        if ct > cf + 1 {
            let mut prev = e.from;
            for c in cf + 1..ct {
                let d = column.len();
                column.push(c);
                segments.push(LayoutEdge { from: prev, to: d, alternative: e.alternative });
                chain.push(d);
                prev = d;
            }
            segments.push(LayoutEdge { from: prev, to: e.to, alternative: e.alternative });
        } else {
            segments.push(*e);
        }
        chains.push(chain);
    }
    (column, segments, chains)
}

/// Placement with columns but no reordering (baseline for crossing tests).
#[cfg(test)]
pub fn unordered(n: usize, edges: &[LayoutEdge]) -> Vec<Placement> {
    let column = columns(n, edges);
    let mut cols = vec![Vec::new(); column.iter().max().map_or(0, |m| m + 1)];
    for (v, &c) in column.iter().enumerate() {
        cols[c].push(v);
    }
    let pos = centered_positions(&cols, n);
    place(&cols, &pos, &vec![0; n], &[0], &vec![1.0; n], n).0
}

/// Previous placement: barycenter order, each column packed and centered (comparison baseline).
#[cfg(test)]
pub fn packed(n: usize, edges: &[LayoutEdge]) -> Vec<Placement> {
    let column = columns(n, edges);
    let mut cols = vec![Vec::new(); column.iter().max().map_or(0, |m| m + 1)];
    for (v, &c) in column.iter().enumerate() {
        cols[c].push(v);
    }
    order(&mut cols, &column, edges, n);
    let pos = centered_positions(&cols, n);
    place(&cols, &pos, &vec![0; n], &[0], &vec![1.0; n], n).0
}

/// Minimum column per node from its log research cost: the 2nd–98th percentile range of
/// the given costs spans `COST_COLUMNS` columns. All zeros without costs.
fn cost_floor(costs: &[f64], n: usize) -> Vec<usize> {
    if costs.len() != n || n == 0 {
        return vec![0; n];
    }
    let logs: Vec<f64> = costs.iter().map(|c| c.max(1.0).ln()).collect();
    let mut sorted = logs.clone();
    sorted.sort_by(f64::total_cmp);
    let (lo, hi) = (sorted[n * 2 / 100], sorted[(n * 98 / 100).min(n - 1)]);
    if hi <= lo {
        return vec![0; n];
    }
    logs.iter().map(|l| (((l - lo) / (hi - lo)).clamp(0.0, 1.0) * (COST_COLUMNS - 1) as f64) as usize).collect()
}

/// Renumber columns so that none is empty (keeps their order).
fn compact(mut column: Vec<usize>) -> Vec<usize> {
    let mut used: Vec<usize> = column.clone();
    used.sort_unstable();
    used.dedup();
    for c in &mut column {
        *c = used.binary_search(c).unwrap();
    }
    column
}

/// Longest-path layering over non-alternative edges.
#[cfg(test)]
pub fn columns(n: usize, edges: &[LayoutEdge]) -> Vec<usize> {
    columns_from(n, edges, &vec![0; n])
}

/// Longest-path layering where node `v` starts at column `floor[v]`.
fn columns_from(n: usize, edges: &[LayoutEdge], floor: &[usize]) -> Vec<usize> {
    let mut succ: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut indeg = vec![0usize; n];
    for e in edges.iter().filter(|e| !e.alternative && e.from != e.to) {
        succ[e.from].push(e.to);
        indeg[e.to] += 1;
    }
    let mut column = floor.to_vec();
    let mut done = vec![false; n];
    let mut stack: Vec<usize> = (0..n).rev().filter(|&v| indeg[v] == 0).collect();
    let mut next_unplaced = 0;
    let mut placed = 0;
    while placed < n {
        let v = match stack.pop() {
            Some(v) => v,
            None => {
                // Only cycles left: break at the lowest unplaced index.
                while done[next_unplaced] {
                    next_unplaced += 1;
                }
                next_unplaced
            }
        };
        if done[v] {
            continue;
        }
        done[v] = true;
        placed += 1;
        for &w in &succ[v] {
            if done[w] {
                continue;
            }
            column[w] = column[w].max(column[v] + 1);
            indeg[w] -= 1;
            if indeg[w] == 0 {
                stack.push(w);
            }
        }
    }
    column
}

/// Vertical position of each node relative to its column's center, in rows.
fn centered_positions(cols: &[Vec<usize>], n: usize) -> Vec<f64> {
    let mut pos = vec![0.0; n];
    for col in cols {
        let mid = (col.len() as f64 - 1.0) / 2.0;
        for (r, &v) in col.iter().enumerate() {
            pos[v] = r as f64 - mid;
        }
    }
    pos
}

/// Barycenter ordering within columns; keeps the best ordering seen.
fn order(cols: &mut [Vec<usize>], column: &[usize], edges: &[LayoutEdge], n: usize) {
    let mut neighbors: Vec<Vec<usize>> = vec![Vec::new(); n];
    for e in edges.iter().filter(|e| column[e.from] != column[e.to]) {
        neighbors[e.from].push(e.to);
        neighbors[e.to].push(e.from);
    }
    let mut pos = centered_positions(cols, n);
    let mut best = cols.to_vec();
    let mut best_crossings = crossings_of(&pos, column, edges);
    let ncols = cols.len();
    for sweep in 0..SWEEPS {
        let forward = sweep % 2 == 0;
        let order: Vec<usize> = if forward { (1..ncols).collect() } else { (0..ncols.saturating_sub(1)).rev().collect() };
        for c in order {
            let mut keyed: Vec<(f64, f64, usize)> = cols[c]
                .iter()
                .map(|&v| {
                    let (sum, count) = neighbors[v]
                        .iter()
                        .filter(|&&w| if forward { column[w] < c } else { column[w] > c })
                        .fold((0.0, 0usize), |(s, k), &w| (s + pos[w], k + 1));
                    let bary = if count > 0 { sum / count as f64 } else { pos[v] };
                    (bary, pos[v], v)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
            cols[c] = keyed.into_iter().map(|k| k.2).collect();
            let mid = (cols[c].len() as f64 - 1.0) / 2.0;
            for (r, &v) in cols[c].iter().enumerate() {
                pos[v] = r as f64 - mid;
            }
        }
        let k = crossings_of(&pos, column, edges);
        if k < best_crossings {
            best_crossings = k;
            best.clone_from_slice(cols);
        }
    }
    cols.clone_from_slice(&best);
}

/// Connected component rank of each node: largest first, ties by lowest node index.
fn components(n: usize, edges: &[LayoutEdge]) -> Vec<usize> {
    fn find(parent: &mut [usize], mut v: usize) -> usize {
        while parent[v] != v {
            parent[v] = parent[parent[v]];
            v = parent[v];
        }
        v
    }
    let mut parent: Vec<usize> = (0..n).collect();
    for e in edges {
        let (a, b) = (find(&mut parent, e.from), find(&mut parent, e.to));
        // Keep the lower index as root so roots are each component's first node.
        parent[a.max(b)] = a.min(b);
    }
    let root: Vec<usize> = (0..n).map(|v| find(&mut parent, v)).collect();
    let mut size = vec![0usize; n];
    for &r in &root {
        size[r] += 1;
    }
    let mut roots: Vec<usize> = (0..n).filter(|&v| root[v] == v).collect();
    roots.sort_by_key(|&r| (std::cmp::Reverse(size[r]), r));
    let mut rank = vec![0usize; n];
    for (i, &r) in roots.iter().enumerate() {
        rank[r] = i;
    }
    root.iter().map(|&r| rank[r]).collect()
}

#[derive(Clone, Copy, PartialEq)]
enum Side {
    Prereqs,
    Dependents,
}

/// Continuous vertical centers in rows (`NODE_HEIGHT + GAP_Y`), keeping each column's order.
/// `size` is each node's height in rows; nodes from `real` on are edge dummies.
fn coordinates(
    cols: &[Vec<usize>],
    column: &[usize],
    edges: &[LayoutEdge],
    comp: &[usize],
    size: &[f64],
    real: usize,
) -> Vec<f64> {
    let n = column.len();
    let mut neighbors: Vec<Vec<(usize, f64)>> = vec![Vec::new(); n];
    for e in edges.iter().filter(|e| column[e.from] != column[e.to]) {
        let dummies = (e.from >= real) as usize + (e.to >= real) as usize;
        let w = if e.alternative { ALT_WEIGHT } else { 1.0 }
            * [1.0, DUMMY_WEIGHT, DUMMY_DUMMY_WEIGHT][dummies];
        neighbors[e.from].push((e.to, w));
        neighbors[e.to].push((e.from, w));
    }
    let mut pos = centered_positions(cols, n);
    let mut targets: Vec<(f64, f64)> = Vec::new();
    let mut sizes: Vec<f64> = Vec::new();
    let mut pass = |pos: &mut Vec<f64>, c: usize, side: Side| {
        targets.clear();
        targets.extend(cols[c].iter().map(|&v| {
            let (sum, weight) = neighbors[v]
                .iter()
                .filter(|&&(w, _)| (column[w] < c) == (side == Side::Prereqs))
                .fold((0.0, 0.0), |(s, k), &(w, wt)| (s + wt * pos[w], k + wt));
            if weight > 0.0 { (sum / weight, weight) } else { (pos[v], STAY_WEIGHT) }
        }));
        let col = &cols[c];
        sizes.clear();
        sizes.extend(col.iter().map(|&v| size[v]));
        let mut start = 0;
        while start < col.len() {
            let end = start + col[start..].iter().take_while(|&&v| comp[v] == comp[col[start]]).count();
            for (&v, y) in col[start..end].iter().zip(pack(&targets[start..end], &sizes[start..end])) {
                pos[v] = y;
            }
            start = end;
        }
    };
    let ncols = cols.len();
    for _ in 0..ROUNDS {
        for c in 1..ncols {
            pass(&mut pos, c, Side::Prereqs);
        }
        for c in (0..ncols.saturating_sub(1)).rev() {
            pass(&mut pos, c, Side::Dependents);
        }
    }
    for c in 1..ncols {
        pass(&mut pos, c, Side::Prereqs);
    }
    pos
}

/// Order-preserving centers, neighbors at least half their summed `sizes` apart, minimizing
/// the weighted squared distance to `(target, weight)` (isotonic regression on
/// `target - offset` by pool adjacent violators, `offset` = packed position from 0).
fn pack(targets: &[(f64, f64)], sizes: &[f64]) -> impl Iterator<Item = f64> + use<> {
    let mut offsets = Vec::with_capacity(targets.len());
    let mut o = 0.0;
    for (i, s) in sizes.iter().enumerate() {
        if i > 0 {
            o += (sizes[i - 1] + s) / 2.0;
        }
        offsets.push(o);
    }
    // Blocks of (weighted sum, weight, length).
    let mut blocks: Vec<(f64, f64, usize)> = Vec::with_capacity(targets.len());
    for (&(t, w), &o) in targets.iter().zip(&offsets) {
        let mut b = (w * (t - o), w, 1);
        while let Some(&last) = blocks.last() {
            if last.0 / last.1 < b.0 / b.1 {
                break;
            }
            blocks.pop();
            b = (b.0 + last.0, b.1 + last.1, b.2 + last.2);
        }
        blocks.push(b);
    }
    blocks.into_iter().flat_map(|(s, w, len)| std::iter::repeat_n(s / w, len)).zip(offsets).map(|(y, o)| y + o)
}

/// Grid positions from row centers (`y` = top of a card centered there); blocks
/// (components) are stacked top to bottom, `GROUP_GAP` apart where `block_group` changes.
/// Covers all nodes of `cols`; `row` counts the first `real` only. Also returns each
/// group's `(top, bottom)` in px.
fn place(
    cols: &[Vec<usize>],
    pos: &[f64],
    comp: &[usize],
    block_group: &[usize],
    size: &[f64],
    real: usize,
) -> (Vec<Placement>, std::collections::BTreeMap<usize, (f64, f64)>) {
    let total = pos.len();
    let ncomp = comp.iter().max().map_or(0, |m| m + 1);
    // Per component: (top, bottom) in rows (cards span 1 including the gap), node count.
    let mut span = vec![(f64::INFINITY, f64::NEG_INFINITY, 0usize); ncomp];
    for v in 0..total {
        let s = &mut span[comp[v]];
        *s = (s.0.min(pos[v] - size[v] / 2.0), s.1.max(pos[v] + size[v] / 2.0), s.2 + 1);
    }
    let row = NODE_HEIGHT + GAP_Y;
    let mut offset = vec![0.0; ncomp];
    let mut bands = std::collections::BTreeMap::new();
    let mut top = 0.0;
    for (i, &(lo, hi, count)) in span.iter().enumerate() {
        if count == 0 {
            continue;
        }
        let group = block_group[i];
        let new_group = i > 0 && block_group[i - 1] != group;
        if new_group {
            top += GROUP_GAP - GAP_Y;
        } else if i > 0 && !(count == 1 && span[i - 1].2 == 1) {
            // Isolated nodes (sorted last) form a plain column.
            top += COMPONENT_GAP - GAP_Y;
        }
        offset[i] = top - (lo + 0.5) * row;
        // Card extents: half a gap inside the row extent on each side.
        let band = bands.entry(group).or_insert((top, top));
        band.1 = top + (hi - lo) * row - GAP_Y;
        top += (hi - lo) * row;
    }
    let mut out = vec![Placement { column: 0, row: 0, x: 0.0, y: 0.0 }; total];
    for (c, col) in cols.iter().enumerate() {
        for (r, &v) in col.iter().filter(|&&v| v < real).chain(col.iter().filter(|&&v| v >= real)).enumerate() {
            out[v] = Placement {
                column: c,
                row: r,
                x: c as f64 * (NODE_WIDTH + GAP_X),
                y: pos[v] * row + offset[comp[v]],
            };
        }
    }
    (out, bands)
}

/// Number of crossings of the edges drawn as straight lines between node centers.
#[cfg(test)]
pub fn count_crossings(placements: &[Placement], edges: &[LayoutEdge]) -> usize {
    let pos: Vec<f64> = placements.iter().map(|p| p.y).collect();
    let column: Vec<usize> = placements.iter().map(|p| p.column).collect();
    crossings_of(&pos, &column, edges)
}

fn crossings_of(pos: &[f64], column: &[usize], edges: &[LayoutEdge]) -> usize {
    let ncols = column.iter().max().map_or(0, |m| m + 1);
    // Segments of every edge per gap between adjacent columns: (y at left, y at right).
    let mut gaps: Vec<Vec<(f64, f64)>> = vec![Vec::new(); ncols.saturating_sub(1)];
    for e in edges {
        let (a, b) = if column[e.from] <= column[e.to] { (e.from, e.to) } else { (e.to, e.from) };
        let (ca, cb) = (column[a], column[b]);
        if ca == cb {
            continue;
        }
        let span = (cb - ca) as f64;
        let y = |c: usize| pos[a] + (pos[b] - pos[a]) * (c - ca) as f64 / span;
        for (g, segs) in gaps.iter_mut().enumerate().take(cb).skip(ca) {
            segs.push((y(g), y(g + 1)));
        }
    }
    gaps.iter_mut()
        .map(|segs| {
            segs.sort_by(|p, q| p.0.total_cmp(&q.0).then(p.1.total_cmp(&q.1)));
            let mut right: Vec<f64> = segs.iter().map(|s| s.1).collect();
            inversions(&mut right)
        })
        .sum()
}

/// Pairs `i < j` with `v[i] > v[j]` (merge sort; sorts `v`).
fn inversions(v: &mut [f64]) -> usize {
    if v.len() < 2 {
        return 0;
    }
    let mid = v.len() / 2;
    let mut count = inversions(&mut v[..mid]) + inversions(&mut v[mid..]);
    let mut merged = Vec::with_capacity(v.len());
    let (mut i, mut j) = (0, mid);
    while i < mid && j < v.len() {
        if v[j] < v[i] {
            count += mid - i;
            merged.push(v[j]);
            j += 1;
        } else {
            merged.push(v[i]);
            i += 1;
        }
    }
    merged.extend_from_slice(&v[i..mid]);
    merged.extend_from_slice(&v[j..]);
    v.copy_from_slice(&merged);
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(from: usize, to: usize) -> LayoutEdge {
        LayoutEdge { from, to, alternative: false }
    }

    fn alt(from: usize, to: usize) -> LayoutEdge {
        LayoutEdge { from, to, alternative: true }
    }

    /// Deterministic pseudo-random DAG resembling the tech tree.
    fn random_dag(n: usize, m: usize, seed: u64) -> Vec<LayoutEdge> {
        let mut s = seed;
        let mut rnd = move || {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (s >> 33) as usize
        };
        let mut edges = Vec::new();
        while edges.len() < m {
            let to = 1 + rnd() % (n - 1);
            let from = to.saturating_sub(1 + rnd() % 60.min(to));
            edges.push(LayoutEdge { from, to, alternative: rnd() % 10 == 0 });
        }
        edges
    }

    #[test]
    fn columns_follow_longest_non_alternative_path() {
        // 0 → 1 → 2 → 3, 0 → 3, 2 ⇢ 4 (alternative), 5 isolated
        let edges = [e(0, 1), e(1, 2), e(2, 3), e(0, 3), alt(2, 4)];
        assert_eq!(columns(6, &edges), vec![0, 1, 2, 3, 0, 0]);
    }

    #[test]
    fn cycles_do_not_hang() {
        let edges = [e(0, 1), e(1, 2), e(2, 1), e(2, 3)];
        let p = layout(4, &edges);
        assert_eq!(p.len(), 4);
        assert!(p[3].column > p[2].column);
    }

    #[test]
    fn positions_use_card_grid_and_center_columns() {
        // column 0: three roots; column 1: one child
        let p = layout(4, &[e(0, 3), e(1, 3), e(2, 3)]);
        assert_eq!(p[3].x, NODE_WIDTH + GAP_X);
        assert_eq!(p[3].y, NODE_HEIGHT + GAP_Y); // centered next to the middle root
        let mut ys: Vec<f64> = p[..3].iter().map(|q| q.y).collect();
        ys.sort_by(f64::total_cmp);
        assert_eq!(ys, vec![0.0, 124.0, 248.0]);
    }

    #[test]
    fn removes_simple_crossing() {
        // initial order crosses: 0 → 3, 1 → 2
        let edges = [e(0, 3), e(1, 2)];
        let p = layout(4, &edges);
        assert_eq!(count_crossings(&p, &edges), 0);
    }

    #[test]
    fn deterministic() {
        let edges = random_dag(400, 800, 7);
        assert_eq!(layout(400, &edges), layout(400, &edges));
    }

    #[test]
    fn reduces_crossings_vs_input_order() {
        let n = 500;
        let edges = random_dag(n, 1000, 42);
        let before = count_crossings(&unordered(n, &edges), &edges);
        let after = count_crossings(&layout(n, &edges), &edges);
        eprintln!("crossings: input order {before}, laid out {after}");
        assert!(after < before);
    }

    fn assert_no_overlap(p: &[Placement]) {
        for a in p {
            for b in p.iter().filter(|b| b.column == a.column && b.row > a.row) {
                assert!(b.y - a.y >= NODE_HEIGHT + GAP_Y - 1e-6, "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn columns_never_overlap() {
        let edges = random_dag(400, 800, 3);
        let r = route(400, &edges, &[]);
        assert_no_overlap(&r.nodes);
        // Row extents (cards: card plus half a gap each side) start at -GAP_Y / 2, so a
        // topmost card is at 0; a dummy may be topmost instead.
        let half_dummy = DUMMY_SIZE * (NODE_HEIGHT + GAP_Y) / 2.0;
        let tops = r
            .nodes
            .iter()
            .map(|q| q.y - GAP_Y / 2.0)
            .chain(r.waypoints.iter().flatten().map(|w| w[1] - half_dummy));
        let top = tops.fold(f64::INFINITY, f64::min);
        assert!((top + GAP_Y / 2.0).abs() < 1e-6, "{top}");
        assert!(r.nodes.iter().all(|q| q.y >= 0.0));
    }

    #[test]
    fn long_edges_get_waypoints_between_cards() {
        let edges = random_dag(400, 800, 5);
        let r = route(400, &edges, &[]);
        let row = NODE_HEIGHT + GAP_Y;
        for (e, points) in edges.iter().zip(&r.waypoints) {
            let (a, b) = (r.nodes[e.from], r.nodes[e.to]);
            let span = b.column.saturating_sub(a.column);
            assert_eq!(points.len(), if span > 1 { 2 * (span - 1) } else { 0 }, "{e:?}");
            assert!(points.windows(2).all(|w| w[0][0] < w[1][0]), "{points:?}");
            for w in points {
                // A waypoint never runs through a card of its column.
                let clear = (row * (1.0 + DUMMY_SIZE) / 2.0) - 1e-6;
                for q in r.nodes.iter().filter(|q| q.x <= w[0] && w[0] < q.x + NODE_WIDTH) {
                    assert!((q.y + NODE_HEIGHT / 2.0 - w[1]).abs() >= clear, "{w:?} through {q:?}");
                }
            }
        }
    }

    #[test]
    fn groups_form_disjoint_bands_in_order() {
        let n = 300;
        let edges = random_dag(n, 600, 9);
        let groups: Vec<usize> = (0..n).map(|v| [2, 0, 3][v % 3]).collect();
        let r = route_grouped(n, &edges, &[], &groups);
        assert_no_overlap(&r.nodes);
        assert_eq!(r.groups.len(), 4);
        assert!(r.groups[1].is_none());
        let band = |g: usize| r.groups[g].unwrap();
        for (v, q) in r.nodes.iter().enumerate() {
            let (top, bottom) = band(groups[v]);
            assert!(q.y >= top - 1e-6 && q.y + NODE_HEIGHT <= bottom + 1e-6, "{v} {q:?} not in {top}..{bottom}");
        }
        assert!(band(0).1 + GROUP_GAP - GAP_Y <= band(2).0 + 1e-6);
        assert!(band(2).1 + GROUP_GAP - GAP_Y <= band(3).0 + 1e-6);
        assert_eq!(route_grouped(n, &edges, &[], &[]), route(n, &edges, &[]));
    }

    #[test]
    fn routing_keeps_crossings_low() {
        let n = 500;
        let edges = random_dag(n, 1000, 42);
        let r = route(n, &edges, &[]);
        let straight = count_crossings(&unordered(n, &edges), &edges);
        eprintln!("routed crossings {} (input order, straight: {straight})", r.crossings);
        assert!(r.crossings < straight);
    }

    #[test]
    fn chain_is_horizontal() {
        let p = layout(5, &[e(3, 4), e(0, 1), e(2, 3), e(1, 2)]);
        assert!(p.iter().all(|q| q.y == 0.0), "{p:?}");
        assert_eq!(p.iter().map(|q| q.column).collect::<Vec<_>>(), vec![0, 1, 2, 3, 4]);
    }

    #[test]
    fn single_parent_child_aligns() {
        // 0 → 2; 1 → 3, 1 → 4, 1 → 5: 2 sits next to 0, 1 centered on its fan.
        let p = layout(6, &[e(0, 2), e(1, 3), e(1, 4), e(1, 5)]);
        assert_no_overlap(&p);
        assert!((p[2].y - p[0].y).abs() < 1e-6, "{p:?}");
        assert!((p[1].y - p[4].y).abs() < 1e-6, "{p:?}");
    }

    #[test]
    fn components_are_stacked() {
        // Two chains and an isolated pair: the larger chain on top, no vertical interleaving.
        let p = layout(7, &[e(0, 1), e(2, 3), e(3, 4)]);
        assert!(p[2].y == 0.0 && p[3].y == 0.0 && p[4].y == 0.0, "{p:?}");
        assert!(p[0].y >= NODE_HEIGHT + COMPONENT_GAP && p[0].y == p[1].y, "{p:?}");
        assert_eq!(p[6].y - p[5].y, NODE_HEIGHT + GAP_Y);
        assert!(p[5].y > p[0].y);
    }

    #[test]
    fn pack_preserves_order_and_targets() {
        let ones = [1.0, 1.0];
        assert_eq!(pack(&[(0.0, 1.0), (5.0, 1.0)], &ones).collect::<Vec<_>>(), vec![0.0, 5.0]);
        assert_eq!(pack(&[(2.0, 1.0), (2.0, 1.0)], &ones).collect::<Vec<_>>(), vec![1.5, 2.5]);
        assert_eq!(pack(&[(3.0, 3.0), (0.0, 1.0)], &ones).collect::<Vec<_>>(), vec![2.0, 3.0]);
        // A dummy (0.2) next to a card (1) needs only 0.6 rows.
        assert_eq!(pack(&[(2.0, 1.0), (2.0, 1.0)], &[1.0, 0.2]).collect::<Vec<_>>(), vec![1.7, 2.3]);
    }

    #[test]
    fn inversion_count() {
        assert_eq!(inversions(&mut [3.0, 1.0, 2.0]), 2);
        assert_eq!(inversions(&mut [1.0, 1.0, 2.0]), 0);
    }

    #[test]
    fn tree_sized_layout_is_fast() {
        let edges = random_dag(917, 1700, 1);
        let t = std::time::Instant::now();
        layout(917, &edges);
        let elapsed = t.elapsed();
        eprintln!("layout of 917 nodes / 1700 edges: {elapsed:?}");
        if !cfg!(debug_assertions) {
            assert!(elapsed.as_millis() < 50, "{elapsed:?}");
        }
    }

    #[test]
    fn costs_push_expensive_nodes_right_without_empty_columns() {
        // 0 → 1; 2 and 3 are roots, 3 is far more expensive than everything else.
        let edges = [e(0, 1)];
        let p = layout_by_cost(4, &edges, &[10.0, 20.0, 10.0, 1e6]);
        assert!(p[1].column > p[0].column);
        assert!(p[3].column > p[1].column);
        assert_eq!(p[2].column, p[0].column);
        let mut used: Vec<usize> = p.iter().map(|q| q.column).collect();
        used.sort_unstable();
        used.dedup();
        assert_eq!(used, (0..used.len()).collect::<Vec<_>>());
        assert_eq!(layout_by_cost(4, &edges, &[]), layout(4, &edges));
    }
}
