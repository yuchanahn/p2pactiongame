use std::collections::HashMap;

use godot::engine::utilities::minf;
use godot::engine::ProjectSettings;
use godot::prelude::*;
use godot::engine::Node;
use godot::engine::Node2D;
use godot::engine::INode2D;
use godot::engine::AnimationPlayer;

use crate::input_controller::InputController;
use crate::gui_player_state::GUIPlayerState;
use crate::game_manager::GAME_TICK;

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct Player {
    pub id: Option<u8>,
    input_of_tick: HashMap<u64, u8>,
    input_ok: HashMap<u64, bool>,
    speed: f64,
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

    #[func]
    pub fn push_input(&mut self, input: u8, tick: u64) {
        self.input_of_tick.insert(tick, input);
        self.input_ok.insert(tick, false);
    }

    #[func]
    pub fn push_input_ok(&mut self, tick: u64) {
        self.input_ok.get_mut(&tick).map(|x| *x = true);
    }

    pub fn get_input_5(&mut self, tick: u64) -> [(u64, u8); 5] {
        let mut index = 0;
        let mut inputs = [(0u64, 0u8); 5];
        let mut index_counter = 0;
        //저장된 인풋들 중 최근 5개의 인풋을 반환
        while tick - index > 0 {
            if let Some(x) = self.input_of_tick.get(&(tick - index)) {
                inputs[index_counter] = (tick - index, *x);
                index_counter += 1;

                if index_counter == 5 {
                    break;
                }
            }
            index += 1;
        }
        inputs
    }
}

#[godot_api]
impl INode2D for Player {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            id: None,
            input_of_tick: HashMap::new(),
            input_ok: HashMap::new(),
            speed: 400.0,
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
    
    fn physics_process(&mut self, delta: f64) {
        let mut velocity = self.vel;

        let net_stat = GAME_TICK.lock().unwrap();
        let current_pos = self.to_gd().get_position();
        let can_jump = current_pos.y == -5.0;
        let mut jump = false;

        if (*net_stat).tick > 0 {
            let tick = (*net_stat).tick;

            let mut input = 0u8;
            if self.input_of_tick.contains_key(&tick) && *self.input_ok.get(&tick).unwrap() {
                input = *self.input_of_tick.get(&tick).unwrap();
                self.input_of_tick.remove(&tick);
            }
            velocity.x += if input & 0b0001 == 0b0001 { 1.0 } else { 0.0 };
            velocity.x -= if input & 0b0010 == 0b0010 { 1.0 } else { 0.0 };
            jump = input & 0b0100 == 0b0100;
        } else {
            let input_controller = self.base().get_tree().unwrap().get_root().unwrap().get_node_as::<InputController>("Root/InputController");
            let input = input_controller.bind().local_input;
            
            velocity.x += if input & 0b0001 == 0b0001 { 1.0 } else { 0.0 };
            velocity.x -= if input & 0b0010 == 0b0010 { 1.0 } else { 0.0 };
            jump = input & 0b0100 == 0b0100;
        }

        let JUMP_VELOCITY = -30f32; //-725f32;
        //Maximum speed at which the player can fall.
        let TERMINAL_VELOCITY = 700f32;

	    if jump && can_jump {
	    	velocity.y = JUMP_VELOCITY;
        }
        
        let gravity = 100f64;
	    //Fall.
	    velocity.y = minf(TERMINAL_VELOCITY as f64, velocity.y as f64 + gravity as f64 * delta) as f32;

        if velocity.x.abs() > 0.0 {
            velocity.x = velocity.x * delta as f32 * self.speed as f32;
            let mut anim = self.animation_player.clone().unwrap();
            anim.set_current_animation("anim/run".into());
            anim.play();
        } else {
            let mut anim = self.animation_player.clone().unwrap();
            anim.set_current_animation("anim/idle".into());
            anim.play();
        }

        let mut new_position = current_pos + velocity;
        new_position.y = new_position.y.min(-5.0);
        self.to_gd().set_position(new_position);
        
        if velocity.x != 0.0 {
            self.to_gd().set_scale(Vector2::new(velocity.x.signum(), 1.0));
        }

        self.vel = velocity;
        self.vel.x = 0.0;
    }
}