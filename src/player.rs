use godot::engine::utilities::minf;
use godot::engine::utilities::move_toward;
use godot::engine::AnimatedSprite2D;
use godot::engine::INode2D;
use godot::engine::Label;
use godot::engine::Node;
use godot::engine::Node2D;
use godot::engine::ProgressBar;
use godot::prelude::*;

use crate::effect::Effect;
use crate::game_manager::GAME_TICK;
use crate::gui_player_state::GUIPlayerState;
use crate::input_controller::InputController;
use crate::utils::minus;
use crate::utils::plus;

const MAX_SPEED: f32 = 10.0;
const ACCELERATION_SPEED: f32 = MAX_SPEED * 6.0;
const DECELERATION_SPEED: f32 = MAX_SPEED * 6.0;
const JUMP_VELOCITY: f32 = -30.0;
const TERMINAL_VELOCITY: f32 = 700.0;
const GRAVITY: f32 = 100.0;


#[derive(Debug, Clone, Copy)]
pub enum EActionMessage {
    Damaged,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EAnim {
    Idle,
    Run,
    Jump,
    Fall,
    Attack,
    Hit,
    Die,
}

impl From<&str> for EAnim {
    fn from(v: &str) -> Self {
        match v {
            "idle" => EAnim::Idle,
            "run" => EAnim::Run,
            "jump" => EAnim::Jump,
            "fall" => EAnim::Fall,
            "attack" => EAnim::Attack,
            "hit" => EAnim::Hit,
            "die" => EAnim::Die,
            _ => panic!("Unknown animation name"),
        }
    }
}

impl From<EAnim> for StringName {
    fn from(v: EAnim) -> Self {
        match v {
            EAnim::Idle => "idle".into(),
            EAnim::Run => "run".into(),
            EAnim::Jump => "jump".into(),
            EAnim::Fall => "fall".into(),
            EAnim::Attack => "attack".into(),
            EAnim::Hit => "hit".into(),
            EAnim::Die => "die".into(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PlayAnimationData {
    pub name: EAnim,
    pub started_at: u64,
    pub looped: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RollbackState {
    pub position: Vector2,
    pub velocity: Vector2,
    pub attack_cooldown: f64,
    pub stat: PlayerStat,
    pub anim_data: PlayAnimationData,
}

#[derive(Debug, Clone, Copy)]
pub struct PlayerStat {
    pub health: f64,
}

#[derive(GodotClass, Debug)]
#[class(base=Node2D)]
pub struct Player {
    pub id: Option<u8>,
    pub real_inputs: [Option<u8>; 30],
    pub predicted_inputs: [u8; 30],
    pub rollback_states: [Option<RollbackState>; 30],
    pub attack_cooldown: f64,
    pub anim_data: Option<PlayAnimationData>,
    pub stat: PlayerStat,
    vel: Vector2,
    animation_player: Option<Gd<AnimatedSprite2D>>,
    effect: Option<Gd<PackedScene>>,
    base: Base<Node2D>,
}

#[godot_api]
impl Player {
    #[func]
    pub fn set_gui(&mut self) {
        if let Ok(scene) = try_load::<PackedScene>("res://PlayerState.tscn") {
            let mut player_state = scene.instantiate_as::<GUIPlayerState>();
            player_state
                .bind_mut()
                .set_target(self.base().clone().upcast::<Node2D>());
            self.base()
                .get_node_as::<Node2D>("../")
                .add_child(player_state.upcast::<Node>());
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

    pub fn push_rollback_state(&mut self, tick: u64) {
        let index = 29 - (GAME_TICK.lock().unwrap().tick - tick) as usize;

        self.rollback_states[index] = Some(RollbackState {
            position: self.base().get_position(),
            velocity: self.vel,
            attack_cooldown: self.attack_cooldown,
            stat: self.stat,
            anim_data: self.anim_data.clone().unwrap(),
        });
    }

    pub fn load_input(&self, tick: u64) -> u8 {
        let cur_tick = GAME_TICK.lock().unwrap().tick;
        if (cur_tick - tick) > 29 {
            panic!(
                "Trying to load input from the future tick : {}, ctick : {}",
                tick, cur_tick
            );
        }

        let index = 29 - (cur_tick - tick) as usize;

        if self.real_inputs[index].is_some() {
            return self.real_inputs[index].unwrap();
        }
        return self.predicted_inputs[index];
    }

    pub fn restore_state(&mut self, tick: u64) {
        let cur_tick = GAME_TICK.lock().unwrap().tick;
        if (cur_tick - tick) > 29 {
            panic!(
                "Trying to restore state from the future tick : {}, ctick : {}",
                tick, cur_tick
            );
        }

        let index = 29 - (cur_tick - tick) as usize;

        if let Some(state) = self.rollback_states[index] {
            self.base_mut().set_position(state.position);
            self.vel = state.velocity;
            self.attack_cooldown = state.attack_cooldown;
            self.stat = state.stat;
            self.anim_data = Some(state.anim_data);
        } else {
            panic!(
                "No rollback state found for tick {}, index: {}\n{:?}",
                tick,
                index,
                self.rollback_states
                    .map(|x| if x.is_some() { "1" } else { "0" })
            );
        }
    }

    pub fn simulated_tick(&mut self, other_player: Gd<Player>, tick: u64, delta: f64) -> Vec<EActionMessage>{
        let mut velocity = self.vel;

        let cur_tick = GAME_TICK.lock().unwrap().tick;

        let current_pos = self.base().get_position();

        let can_jump = current_pos.y == 65.0 && self.attack_cooldown <= 0.5;
        let can_attack = self.attack_cooldown <= 0.0;

        let is_hit = self.anim_data.as_ref().unwrap().name == "hit".into();
        let is_die = self.anim_data.as_ref().unwrap().name == "die".into();
        
        if is_die || cur_tick == 0 {
            self.animate(tick);
            self.gui_update();
            return vec![];
        }

        let input_controller = self
            .base()
            .get_tree()
            .unwrap()
            .get_root()
            .unwrap()
            .get_node_as::<InputController>("Root/InputController");

        let mut jump = false;
        let mut attack = false;

        let mut dir = 0;
        if cur_tick > 0 {
            let input = self.load_input(tick);

            dir += (input & 0b0001 == 0b0001) as i32;
            dir -= (input & 0b0010 == 0b0010) as i32;

            jump = input & 0b0100 == 0b0100;
            attack = input & 0b1000 == 0b1000;
        } else {
            let input = input_controller.bind().local_input;
            dir += (input & 0b0001 == 0b0001) as i32;
            dir -= (input & 0b0010 == 0b0010) as i32;
            jump = input & 0b0100 == 0b0100;
            attack = input & 0b1000 == 0b1000;
        }

        if jump && can_jump {
            velocity.y = JUMP_VELOCITY;
            self.anim_data = Some(PlayAnimationData {
                name: "jump".into(),
                started_at: tick,
                looped: false,
            });
        }
        if attack && can_attack {
            self.attack_cooldown = 1.0;
            self.anim_data = Some(PlayAnimationData {
                name: "attack".into(),
                started_at: tick,
                looped: false,
            });
        }
        

        if self.attack_cooldown <= 0.5 {
            match self.anim_data.clone().unwrap().name {
                EAnim::Attack => {
                    self.anim_data = Some(PlayAnimationData {
                        name: if dir != 0 { "run" } else { "idle" }.into(),
                        started_at: tick,
                        looped: true,
                    });
                }
                EAnim::Jump => {
                    if velocity.y <= 0.0 {
                        self.anim_data = Some(PlayAnimationData {
                            name: "fall".into(),
                            started_at: tick,
                            looped: false,
                        });
                    }
                }
                EAnim::Fall => {
                    if can_jump {
                        self.anim_data = Some(PlayAnimationData {
                            name: if dir != 0 { "run" } else { "idle" }.into(),
                            started_at: tick,
                            looped: true,
                        });
                    }
                }
                EAnim::Idle => {
                    if dir != 0 {
                        self.anim_data = Some(PlayAnimationData {
                            name: "run".into(),
                            started_at: tick,
                            looped: true,
                        });
                    }
                }
                EAnim::Run => {
                    if dir == 0 {
                        self.anim_data = Some(PlayAnimationData {
                            name: "idle".into(),
                            started_at: tick,
                            looped: true,
                        });
                    }
                }
                EAnim::Die => {

                }
                EAnim::Hit => {
                    
                }
            }
        }

        let acc: f64 = if dir == 0 {
            DECELERATION_SPEED
        } else {
            ACCELERATION_SPEED
        } as f64
            * (delta.sign());

        let can_move = if !is_hit && self.attack_cooldown <= 0.5 {
            1.0
        } else {
            0.0
        };

        velocity.x = move_toward(
            velocity.x as f64,
            (dir as f32 * MAX_SPEED) as f64,
            delta.abs() * acc,
        ) as f32
            * can_move;
        velocity.y = minf(
            TERMINAL_VELOCITY as f64,
            (velocity.y + GRAVITY * delta as f32) as f64,
        ) as f32;

        let mut new_position = current_pos + velocity;
        new_position.y = new_position.y.min(65.0);
        self.base_mut().set_position(new_position);

        self.attack_cooldown = (self.attack_cooldown - delta).max(0.0);

        if velocity.x != 0.0 {
            self.animation_player
                .clone()
                .unwrap()
                .set_scale(Vector2::new(velocity.x.signum(), 1.0) * 0.2);
        }

        self.vel = velocity;
        let (changed, frame) = self.animate(tick);

        let mut rt :Vec<EActionMessage> = vec![];

        if self.anim_data.as_ref().unwrap().name == "attack".into() && frame == 4 && changed {
            let mut root_node = self.base().get_tree().unwrap().get_root().unwrap().get_node("Root".into()).unwrap();

            let other_pos = other_player.get_position();
            let dir = self.animation_player.clone().unwrap().get_scale().x.sign();
            let my_pos = new_position + dir as f32 * Vector2::new(80.0, 0.0);
            let distance = (other_pos - my_pos).length();

            if distance < 50.0 {
                rt.push(EActionMessage::Damaged);

                let effect = self.effect.as_ref().unwrap().instantiate_as::<Effect>();
                let mut effect = effect.clone();
                
                root_node.add_child(effect.clone().upcast::<Node>());
                effect.set_position(other_pos);
            }
        } else if self.anim_data.as_ref().unwrap().name == "hit".into() && frame == 6 && changed {
            self.anim_data = Some(PlayAnimationData {
                name: "idle".into(),
                started_at: tick,
                looped: true,
            });
        }

        self.gui_update();
        return rt;
    }

    pub fn show_rollback_text(&mut self) {
        let mut rollback_text = self.base().get_node_as::<Label>("RollbackText");
        let mut modulate = rollback_text.get_modulate();
        modulate.a = 1.0;
        rollback_text.set_modulate(modulate);
    }

    pub fn animate(&mut self, tick: u64) -> (bool, i32) {
        let data = self.anim_data.clone().unwrap();
        let cur_tick = tick;
        let mut anim = self.animation_player.clone().unwrap();
        anim.set_animation(data.name.clone().into());
        let frame_max = anim
            .get_sprite_frames()
            .as_mut()
            .unwrap()
            .get_frame_count(data.name.into());
        let frame: i32;

        let delta_tick = minus(cur_tick, data.started_at);
        let frame_speed = 3; // 0.1초

        if data.looped {
            if delta_tick == 0 {
                frame = 0;
            } else {
                frame = ((delta_tick / frame_speed) % frame_max as u64) as i32;
            }
        } else {
            frame = (delta_tick / frame_speed).min(frame_max as u64) as i32;
        }
        anim.set_frame(frame);
        return (
            if delta_tick >= 1 {
                (delta_tick / frame_speed) != ((delta_tick - 1) / frame_speed)
            } else {
                false
            },
            frame,
        );
    }

    pub fn gui_update(&mut self) {
        let mut player_health = self.base().get_node_as::<ProgressBar>("ProgressBar");
        player_health.set_value(self.stat.health);
    }
}

#[godot_api]
impl INode2D for Player {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            id: None,
            real_inputs: [None; 30],
            predicted_inputs: [0; 30],
            rollback_states: [None; 30],
            attack_cooldown: 0.0,
            anim_data: None,
            stat: PlayerStat { health: 100.0 },
            vel: Vector2::new(0.0, 0.0),
            animation_player: None,
            effect: None,
            base,
        }
    }

    fn ready(&mut self) {
        self.animation_player = self
            .base()
            .try_get_node_as::<AnimatedSprite2D>("AnimatedSprite2D");
        self.anim_data = Some(PlayAnimationData {
            name: "idle".into(), // "idle", "run", "attack
            started_at: GAME_TICK.lock().unwrap().tick,
            looped: true,
        });
        
        self.effect = try_load::<PackedScene>("res://Effect/Hit/hit_1.tscn").ok();

        self.base_mut().call_deferred("set_gui".into(), &[]);
    }

    fn physics_process(&mut self, delta: f64) {
        let mut rollback_text = self.base().get_node_as::<Label>("RollbackText");
        let mut modulate = rollback_text.get_modulate();
        modulate.a -= 1.0 * delta as f32;
        rollback_text.set_modulate(modulate);
    }
}
