use std::cmp::*;
use std::iter;

pub struct AStarTiles {
    row_size: usize,
    row_number: usize,
    inner: Vec<u8>,
}

impl AStarTiles {
    pub fn do_pathfinding(&self,
                          start: Point,
                          end: Point) -> Option<Path> {
        pathfinding_heap(self, start, end)
    }

    pub fn new(pos: Vec<u8>, row_size: usize) -> AStarTiles {
        assert!(row_size >= 2);
        assert!(pos.len() % row_size == 0);
        assert!(pos.len() / row_size >= 2);
        AStarTiles {
            row_size: row_size,
            row_number: pos.len() / row_size,
            inner: pos,
        }
    }

    fn get(&self, index: usize) -> u8 {
        self.inner[index]
    }

    fn get_point(&self, point: Point) -> u8 {
        assert!(point.y < self.row_size);
        let pos = point.x * self.row_size + point.y;
        self.inner[pos]
    }

    fn valid_points_around(&self, point: Point) -> Vec<Point> {
        let mut res = vec![];
        if point.x != 0 {
            let p = Point::new(point.x - 1, point.y);
            if self.get_point(p) != 0 {
                res.push(p);
            }
        }
        if point.x != self.row_number - 1 {
            let p = Point::new(point.x + 1, point.y);
            if self.get_point(p) != 0 {
                res.push(p);
            }
        }
        if point.y != 0 {
            let p = Point::new(point.x, point.y - 1);
            if self.get_point(p) != 0 {
                res.push(p);
            }
        }
        if point.y != self.row_size - 1 {
            let p = Point::new(point.x, point.y + 1);
            if self.get_point(p) != 0 {
                res.push(p);
            }
        }
        res
    }
}

#[derive(Debug,Clone,Copy,PartialEq,Eq)]
pub struct Point {
    pub x: usize,
    pub y: usize,
}

impl Point {
    pub fn new(x: usize, y: usize) -> Point {
        Point { x: x, y: y }
    }

    pub fn get_distance(&self, other: Point) -> usize {
        let hor = if self.x > other.x { self.x - other.x } else { other.x - self.x };
        let ver = if self.y > other.y { self.y - other.y } else { other.y - self.y };
        hor + ver
    }

    fn to_index(&self, row_size: usize) -> usize {
        self.x * row_size + self.y
    }
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Path(Vec<Point>);

#[derive(Debug,Clone)]
struct AStarNode {
    point: Point,
    f: usize,
    g: usize,
    path: Path,
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &AStarNode) -> Option<Ordering> {
        self.f.partial_cmp(&other.f)
    }
}

impl PartialEq for AStarNode {
    fn eq(&self, other: &AStarNode) -> bool {
        self.f.eq(&other.f)
    }
}

impl Eq for AStarNode {}

impl Ord for AStarNode {
    fn cmp(&self, other: &AStarNode) -> Ordering {
        self.f.cmp(&other.f)
    }
}

impl AStarNode {
    fn new(point: Point, f: usize, g: usize, path: Path) -> AStarNode {
        AStarNode { point: point, f: f, g: g, path: path }
    }
}

/*
fn do_pathfinding(map: &AStarTiles, start: Point, end: Point) -> Option<Path> {
    assert!(map.get_point(start) == 1);
    assert!(map.get_point(end) == 1);
    let mut active_nodes = Vec::new();
    let mut closed_list = vec![];
    active_nodes.push(AStarNode::new(start, 0, start.get_distance(end), Path(vec![])));

    loop {
        let pos = match active_nodes.iter().enumerate().min_by_key(|a| a.1) {
            Some((pos, _)) => pos,
            None => return None,
        };
        let min = active_nodes.remove(pos);
        for point in map.valid_points_around(min.point) {
            if point == end {
                let mut path = min.path;
                path.0.push(point);
                return Some(path);
            }
            if !closed_list.contains(&point) {
                update_active_list(&mut active_nodes, &min, point, end);
            }
        }
        closed_list.push(min.point);
    }
}

fn update_active_list(list: &mut Vec<AStarNode>, node: &AStarNode, point: Point, end: Point) {
    match list.iter_mut().find(|p| p.point == point) {
        Some(to_update) => {
            if to_update.g > node.g + 1 {
                let diff = to_update.g - (node.g + 1);
                to_update.g = node.g + 1;
                to_update.f -= diff;
                to_update.path.0.clear();
                to_update.path.0.extend(node.path.0.iter());
                to_update.path.0.push(point);
            }
            return;
        }
        None => {}
    }
    let mut path = Vec::with_capacity(node.path.0.capacity() + 1);
    path.extend(node.path.0.iter());
    path.push(point);
    let new_node = AStarNode::new(
        point,
        node.g + 1,
        (node.g + 1) + point.get_distance(end),
        Path(path),
        );
    list.push(new_node);
}
*/

fn pathfinding_heap(map: &AStarTiles, start: Point, end: Point) -> Option<Path> {
    let mut open_list: Vec<usize> = vec![];
    let mut closed_list: Vec<Point> = vec![];
    let mut index_to_checked: Vec<Option<usize>> = iter::repeat(None).take(map.inner.len()).collect();
    let mut checked: Vec<AStarNode> = vec![];
    checked.push(AStarNode::new(start, 0, start.get_distance(end), Path(vec![])));
    index_to_checked[start.to_index(map.row_size)] = Some(0);
    open_list.push(0);

    while let Some(node) = binary_heap_pop(&mut open_list, &mut checked) {
        println!("Poped node {} from heap", node);
        println!("Checked list: {:#?}", checked);
        for point in map.valid_points_around(checked[node].point) {
            if point == end {
                let mut path = checked[node].path.clone();
                path.0.push(point);
                return Some(path);
            }
            if !closed_list.contains(&point) {
                add_to_open(&mut open_list,
                            &mut index_to_checked,
                            &mut checked,
                            node,
                            point,
                            end,
                            map);
            }
        }
        closed_list.push(checked[node].point);
        println!("closed list {:?}", closed_list);
    }
    None
}

fn add_to_open(open_list: &mut Vec<usize>,
               index_to_checked: &mut Vec<Option<usize>>,
               checked: &mut Vec<AStarNode>,
               parent_idx: usize,
               point: Point,
               end: Point,
               map: &AStarTiles) {
    let point_index = point.to_index(map.row_size);
    let new_node;
    // Scope for borrow checker
    {
        let parent: &AStarNode = unsafe {
            &*(&checked[parent_idx] as *const _)
        };
        let g = parent.g + 1;
        let f = g + point.get_distance(end);
        if let Some(pos) = index_to_checked[point_index] {
            // We already came across that node, check if we found a better route to reach it

            // This should never happen
            // But safety of the previous unsafe block depends on it
            assert!(pos != parent_idx);

            // Scope because of the borrow checker
            {
                let to_update = &mut checked[pos];
                if f < to_update.f {
                    // We found a better route
                    to_update.g = g;
                    to_update.f = f;
                    to_update.path.0.clear();
                    to_update.path.0.extend(parent.path.0.iter());
                    to_update.path.0.push(point);

                    // Update binary heap (done after the scope because of borrow checker)
                } else {
                    // Nothing to do
                    return;
                }
            }

            // Now we can update the binary heap
            // As we only increase the priority, the new node can only go up
            binary_heap_up(open_list, checked, pos);

            return;
        }

        let mut path = Vec::with_capacity(parent.path.0.capacity() + 1);
        path.extend(parent.path.0.iter());
        path.push(point);
        new_node = AStarNode::new(
            point,
            g,
            f,
            Path(path),
            );
    }
    checked.push(new_node);
    let new_index = checked.len() - 1;
    index_to_checked[point_index] = Some(new_index);
    open_list.push(new_index);
    let heap_index = open_list.len() - 1;
    binary_heap_up(open_list, checked, heap_index);
}

fn binary_heap_up(open_list: &mut Vec<usize>, checked: &[AStarNode], index: usize) {
    println!("Binary heap up enter");
    let mut current_position = index;
    let node = &checked[open_list[current_position]];
    let f = node.f;
    loop {
        println!("Binary heap up loop iter");
        if current_position == 0 {
            // We are at the top
            return;
        }
        let parent = ((current_position + 1) / 2) - 1;
        if checked[parent].f > f {
            // swap with the parent
            open_list.swap(current_position, parent);
            current_position = parent;
        } else {
            // we are done
            return;
        }
    }
}

fn binary_heap_pop(open_list: &mut Vec<usize>, checked: &[AStarNode]) -> Option<usize>{
    println!("Binary heap pop enter");
    if open_list.len() == 0 {
        return None;
    }
    let result = Some(open_list.swap_remove(0));
    if open_list.is_empty() {
        return result;
    }
    let mut current_position = 0;
    let node = &checked[open_list[current_position]];
    let f = node.f;
    loop {
        println!("Binary heap pop iter");
        let child1_idx = ((current_position + 1) * 2) - 1;
        let child2_idx = child1_idx + 1;
        match (open_list.get(child1_idx).cloned(), open_list.get(child2_idx).cloned()) {
            (Some(child1), Some(child2)) => {
                let child1_f = checked[child1].f;
                let child2_f = checked[child2].f;
                if (f < child1_f) && (f < child2_f) {
                    // We have higher priority than both our children, we are done
                    return result;
                }
                if child1_f <= child2_f {
                    open_list.swap(current_position, child1_idx);
                    current_position = child1_idx;
                } else {
                    open_list.swap(current_position, child2_idx);
                    current_position = child2_idx;
                }
            }
            (Some(child), None) => {
                let child_f = checked[child].f;
                if child_f < f {
                    open_list.swap(current_position, child1_idx);
                    // Never used: current_position = child;
                }
                return result;
            }
            (None, None) => {
                // No more children, we are done
                return result;
            }
            (None, Some(_)) => { unreachable!(); },
        }
    }
}
