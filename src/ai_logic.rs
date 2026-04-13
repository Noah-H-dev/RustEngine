use crate::tools::*;
use crate::game_engine::World;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;


pub struct character{
    pub position: (i32,i32),
    pub target_position: (i32,i32),
    pub path: Vec<(i32, i32)>,
    pub size: f32,
    sprite: GLObject,

}
impl character{
    pub fn new(position: (i32,i32),target_position: (i32,i32), sprite:GLObject) -> character{
        return character{position,target_position,size: 1.0,path: vec!(), sprite}
    }
    pub fn draw(&self){
        self.sprite.draw(self.position.0,self.position.1,self.size);
    }
    pub fn update(&mut self){
        let mut current_move = (0,0);
        if self.path.len() > 0{
            current_move = self.path.pop().unwrap();
            self.position.0 += current_move.0;
            self.position.1 += current_move.1;
        }
    }
    /// Runs A* from self.position to self.target_position using World tile solidity.
    /// Stores the resulting tile-coordinate path (excluding start, including goal) in self.path.
    /// self.path will be empty if already at the target or no path exists.
    pub fn update_path(&mut self, world: &World){
        let start = self.position;
        let goal  = self.target_position;

        if start == goal {
            self.path = vec![];
            return;
        }

        let is_walkable = |pos: (i32, i32)| -> bool {
            if pos.0 < 0 || pos.1 < 0 { return false; }
            let x = pos.0 as usize;
            let y = pos.1 as usize;
            if x >= world.width || y >= world.height { return false; }
            !world.tiles[y * world.width + x].physics.solid
        };

        let heuristic = |pos: (i32, i32)| -> i32 {
            (pos.0 - goal.0).abs() + (pos.1 - goal.1).abs()
        };

        // Min-heap keyed on f_score = g_score + heuristic.
        let mut open: BinaryHeap<(Reverse<i32>, (i32, i32))> = BinaryHeap::new();
        let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();

        g_score.insert(start, 0);
        open.push((Reverse(heuristic(start)), start));

        const DIRS: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];

        while let Some((_, current)) = open.pop() {
            if current == goal {
                let mut path = vec![];
                let mut cur = goal;
                while cur != start {
                    let prev = came_from[&cur];
                    // Forward delta: the direction taken to step from prev into cur.
                    // Collecting goal→start means the vec is already reversed for pop().
                    path.push((cur.0 - prev.0, cur.1 - prev.1));
                    cur = prev;
                }
                self.path = path;
                return;
            }

            let current_g = g_score[&current];

            for &(dx, dy) in &DIRS {
                let neighbor = (current.0 + dx, current.1 + dy);
                if !is_walkable(neighbor) { continue; }

                let tentative_g = current_g + 1;
                if tentative_g < *g_score.get(&neighbor).unwrap_or(&i32::MAX) {
                    came_from.insert(neighbor, current);
                    g_score.insert(neighbor, tentative_g);
                    open.push((Reverse(tentative_g + heuristic(neighbor)), neighbor));
                }
            }
        }

        self.path = vec![]; // no path found
    }
}

