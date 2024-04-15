use std::collections::HashMap;

use godot::engine::utilities::minf;
use godot::engine::utilities::move_toward;
use godot::engine::ProjectSettings;
use godot::prelude::*;
use godot::engine::Node;
use godot::engine::Node2D;
use godot::engine::INode2D;
use godot::engine::AnimationPlayer;

use crate::game_manager::GameTick;
use crate::input_controller::InputController;
use crate::gui_player_state::GUIPlayerState;
use crate::game_manager::GAME_TICK;

const MAX_SPEED:f32 = 10.0;
const ACCELERATION_SPEED:f32 = MAX_SPEED * 6.0;
const DECELERATION_SPEED:f32 = MAX_SPEED * 6.0;
const JUMP_VELOCITY:f32 = -30.0; 
const TERMINAL_VELOCITY:f32 = 700.0;
const GRAVITY:f32 = 100.0;

const ROLLBACK_TICKS:u64 = 30;

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct Player {
    pub id: Option<u8>,
    pub real_inputs: [Option<u8>; 30],
    pub predicted_inputs: [u8; 30],
    vel: Vector2,
    animation_player: Option<Gd<AnimationPlayer>>,
    base: Base<Node2D>
}

#[godot_api]
impl Player {
    #[func]
    pub fn set_gui(&mut self) {
        if let Ok(scene) = try_load::<PackedScene>("res://PlayerState.tscn") {
            let mut player_state = scene.instantiate_as::<GUIPlayerState>();
            player_state.bind_mut().set_target(self.base().clone().upcast::<Node2D>());
            self.base().get_node_as::<Node2D>("../").add_child(player_state.upcast::<Node>());
        }
    }

    pub fn push_input(&mut self, input: u8) {
        let mut inputs = self.real_inputs;
        for i in 0..inputs.len() - 1 {
            inputs[i] = inputs[i + 1];
        }
        inputs[inputs.len() - 1] = Some(input);
        self.real_inputs = inputs;
    }

    pub fn load_input(&self, tick: u64) -> u8 {

        let cur_tick = GAME_TICK.lock().unwrap().tick;
        if (cur_tick - tick) > 29 {
            panic!("Trying to load input from the future tick : {}, ctick : {}", tick, cur_tick);
        }

        let index = 29 - (cur_tick - tick) as usize;

        if self.real_inputs[index].is_some() {
            return self.real_inputs[index].unwrap();
        } 
        return self.predicted_inputs[index];
    }

    pub fn simulated_tick(&mut self, tick: u64, delta: f64) {
        let mut velocity = self.vel;

        let cur_tick = GAME_TICK.lock().unwrap().tick;
        
        let current_pos = self.to_gd().get_position();
        let can_jump = current_pos.y == -5.0;
        let input_controller = self.base().get_tree().unwrap().get_root().unwrap().get_node_as::<InputController>("Root/InputController");

        let mut jump = false;

        let mut dir = 0;
        if cur_tick > 0 {
            let input = self.load_input(tick);

            dir += (input & 0b0001 == 0b0001) as i32;
            dir -= (input & 0b0010 == 0b0010) as i32;

            jump = input & 0b0100 == 0b0100;
        } else {
            let input = input_controller.bind().local_input;
            dir += (input & 0b0001 == 0b0001) as i32;
            dir -= (input & 0b0010 == 0b0010) as i32;
            jump = input & 0b0100 == 0b0100;
        }

	    if jump && can_jump {
	    	velocity.y = JUMP_VELOCITY;
        }

        let mut anim = self.animation_player.clone().unwrap();
        anim.set_current_animation(if dir != 0 {"anim/run"} else {"anim/idle"}.into());
        anim.play();
        
        let acc:f64 = if dir == 0 {DECELERATION_SPEED} else {ACCELERATION_SPEED} as f64;

        velocity.x = move_toward(velocity.x as f64, (dir as f32 * MAX_SPEED) as f64, delta * acc) as f32;
        velocity.y = minf(TERMINAL_VELOCITY as f64, (velocity.y + GRAVITY * delta as f32) as f64) as f32;

        let mut new_position = current_pos + velocity;
        new_position.y = new_position.y.min(-5.0);
        self.to_gd().set_position(new_position);
        
        if velocity.x != 0.0 {
            self.to_gd().set_scale(Vector2::new(velocity.x.signum(), 1.0));
        }

        self.vel = velocity;
    }
}

#[godot_api]
impl INode2D for Player {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            id: None,
            real_inputs: [None; 30],
            predicted_inputs: [0; 30],
            vel: Vector2::new(0.0, 0.0),
            animation_player: None,
            base,
        }
    }

    fn ready(&mut self) {
        self.animation_player = self.base().try_get_node_as::<AnimationPlayer>("AnimationPlayer");
        let mut anim = self.animation_player.clone().unwrap();
        anim.set_current_animation("anim/idle".into());
        anim.play();
        
        self.base_mut().call_deferred("set_gui".into(), &[]);
    }
    
    fn physics_process(&mut self, delta: f64) {}
}