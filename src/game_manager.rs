use std::sync::Mutex;

use godot::engine::Node2D;
use godot::prelude::*;

use crate::time;

use lazy_static::lazy_static;

pub struct NetworkStat {
    pub tick: u64,
    pub latency: u64,
}

lazy_static! {
    pub static ref GAME_TICK: Mutex<NetworkStat> = {
        Mutex::new(NetworkStat {
            tick: 0,
            latency: 0,
        })
    };
}

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct GameTick {
    base: Base<Node2D>,
    pub game_start_time: u64,
}

#[godot_api]
impl INode2D for GameTick {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            game_start_time: 0,
        }
    }

    fn physics_process(&mut self, _delta: f64) {
      if (self.game_start_time == 0) || self.game_start_time > time::get_ms_timestamp() {
          return;
      }
      (*GAME_TICK.lock().unwrap()).tick += 1;
  }
}