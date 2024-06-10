use std::collections::HashMap;

use godot::{builtin::{math::{self, FloatExt}, StringName, Vector2}, engine::{input, utilities::{minf, move_toward, rand_from_seed}, AnimatedSprite2D}, log::godot_print, obj::Gd};

use crate::{animation_controller::{EAnim, PlayAnimationData}, col2d::{circle_cast, Box2D, Circle2D, Collision2D}, game_manager::GameTick, player::*, random, utils::minus};

#[derive(Clone)]
pub struct WorldData {
    pub players: HashMap<u64, PlayerData>,
    pub collision: Vec<Collision2D>,
}

#[derive(Clone)]
pub struct PlayerData {
    pub id: u64,
    pub pos: Vector2,
    pub vel: Vector2,
    pub attack_cooldown: f64,
    pub health: f64,
    pub anim_data: Option<PlayAnimationData>,
    pub object: Option<Gd<Player>>
}

#[derive(Clone, Debug)]
pub enum ActionMessage {
    Attack(AmAttack),

}

#[derive(Clone, Debug)]
pub struct AmAttack {
    pub form: u64,
    pub to: u64,
    pub damage: f64,
}

fn simulate_player(mut data: PlayerData, col_world: &Vec<Collision2D>, input: u8, tick: u64, delta: f64) -> (PlayerData, Option<ActionMessage>, Vec<Collision2D>) {
    let current_anim = data.clone().anim_data.as_ref().unwrap().name;

    let is_hit = current_anim == "hit".into();
    let is_die = current_anim == "die".into();
    let is_roll = current_anim == "roll".into();
    let is_guard = current_anim == "guard".into();

    let can_jump = data.pos.y == 65.0 && data.attack_cooldown <= 0.5;
    let can_roll = data.pos.y == 65.0 && data.attack_cooldown <= 0.5 && !is_hit && !is_roll;
    let can_guard = data.pos.y == 65.0 && data.attack_cooldown <= 0.5;
    let can_attack = data.attack_cooldown <= 0.0 && !is_hit;

    let mut player_object = data.object.clone().unwrap();

    let anim_player = player_object.bind_mut().animation_player.clone().unwrap();

    if is_die || tick == 0 {
        animate(data.anim_data.unwrap(), anim_player.clone(), tick);
        player_object.bind_mut().gui_update();
        return (data.clone(), None, col_world.clone());
    }

    let mut dir = 0;

    dir += (input & 0b0001 == 0b0001) as i32;
    dir -= (input & 0b0010 == 0b0010) as i32;
    let jump = input & 0b0100 == 0b0100;
    let attack = input & 0b1000 == 0b1000;
    let roll = input & 0b10000 == 0b10000;
    let guard = input & 0b100000 == 0b100000;

    if jump && can_jump {
        data.vel.y = JUMP_VELOCITY;
        data.anim_data = Some(PlayAnimationData {
            name: "jump".into(),
            started_at: tick,
            looped: false,
        });
    }
    if attack && can_attack {
        data.attack_cooldown = 1.0;
        data.anim_data = Some(PlayAnimationData {
            name: "attack".into(),
            started_at: tick,
            looped: false,
        });
    }
    if roll && can_roll {
        data.anim_data = Some(PlayAnimationData {
            name: "roll".into(),
            started_at: tick,
            looped: false,
        });
    }
    if guard && can_guard {
        data.anim_data = Some(PlayAnimationData {
            name: "guard".into(),
            started_at: tick,
            looped: false,
        });
    }
    
    let mut roll_delta = 1.0f32; 

    if data.attack_cooldown <= 0.5 {
        match data.anim_data.clone().unwrap().name {
            EAnim::Attack => {
                data.anim_data = Some(PlayAnimationData {
                    name: if dir != 0 { "run" } else { "idle" }.into(),
                    started_at: tick,
                    looped: true,
                });
            }
            EAnim::Jump => {
                if data.vel.y <= 0.0 {
                    data.anim_data = Some(PlayAnimationData {
                        name: "fall".into(),
                        started_at: tick,
                        looped: false,
                    });
                }
            }
            EAnim::Fall => {
                if can_jump {
                    data.anim_data = Some(PlayAnimationData {
                        name: if dir != 0 { "run" } else { "idle" }.into(),
                        started_at: tick,
                        looped: true,
                    });
                }
            }
            EAnim::Idle => {
                if dir != 0 {
                    data.anim_data = Some(PlayAnimationData {
                        name: "run".into(),
                        started_at: tick,
                        looped: true,
                    });
                }
            }
            EAnim::Run => {
                if dir == 0 {
                    data.anim_data = Some(PlayAnimationData {
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
            EAnim::Attack2 => {
                
            }
            EAnim::Guard => {
                
            }
            EAnim::Roll => {
                let anim = data.anim_data.clone().unwrap();
                let frame = anim.clac_fream(tick, anim_player.clone());

                dir = anim_player.clone().get_scale().x.signum() as i32;
                match frame {
                    0..=1 => {
                        roll_delta = 0.0;
                    }
                    2..=3 => {
                        roll_delta = 5.0;
                    }
                    4..=6 => {
                        roll_delta = 1.5;
                    }
                    7..=9 => {
                        roll_delta = 0.3;
                    }
                    _ => {
                        // 프레임이 0에서 2 사이가 아닌 경우에 대한 처리
                    }
                }

                godot_print!("frame: {}", frame);
            }
        }
    }

    let acc: f64 = if dir == 0 {
        DECELERATION_SPEED
    } else {
        ACCELERATION_SPEED
    } as f64
        * (delta.sign());

    let can_move = if !is_hit && data.attack_cooldown <= 0.5 && !is_guard {
        1.0
    } else {
        0.0
    };

    data.vel.x = move_toward(
        data.vel.x as f64,
        (dir as f32 * MAX_SPEED * roll_delta) as f64,
        delta.abs() * acc,
    ) as f32
        * can_move;
    data.vel.y = minf(
        TERMINAL_VELOCITY as f64,
        (data.vel.y + GRAVITY * delta as f32) as f64,
    ) as f32;

    let mut new_position = data.pos + data.vel;
    new_position.y = new_position.y.min(65.0);
    new_position.x = new_position.x.max(-450.0).min(450.0);

    data.pos = new_position;

    data.attack_cooldown = (data.attack_cooldown - delta).max(0.0);
    
    let mut new_col_world = vec![];

    for col_ori in col_world {
        match col_ori {
            Collision2D::Box(col) => {
                if col.owner_id == data.id {
                    new_col_world.push(Collision2D::Box(Box2D {
                        owner_id: data.id,
                        pos: data.pos,
                        size: Vector2::new(50.0, 50.0),
                    }));
                }
                else {
                    new_col_world.push(col_ori.clone());
                }
            }
            _ => {}
        }
    }

    if data.vel.x != 0.0 {
        anim_player
            .clone()
            .set_scale(Vector2::new(data.vel.x.signum(), 1.0) * 0.2);
    }
    
    let anim_dir = anim_player.clone().get_scale().x.signum() as i32;
    let (changed, frame) = animate(data.anim_data.unwrap(), anim_player, tick);

    let mut action = None;

    if current_anim == "attack".into() && frame == 4 && changed {
        let attack_range = Circle2D {
            owner_id: data.id,
            pos: data.pos + Vector2::new(75.0 * anim_dir as f32, 0.0),
            radius: 55.0,
        };

        godot_print!("attack range : {:?}", attack_range);

        circle_cast(col_world, &attack_range).iter().for_each(|col| {
            match col {
                Collision2D::Box(col) => {
                    godot_print!("attack collision : {:?}", col);
                    if data.id != col.owner_id {
                        let seed = (tick % 100000) as i64; 
                        let damage = random::Rand::new(seed as u32).rand_range(10, 20);
                        action = Some(ActionMessage::Attack(AmAttack {
                            form: data.id,
                            to: col.owner_id,
                            damage: damage as f64,
                        }));
                    }
                }
                _ => {}
            }
        });
    } else if current_anim == "hit".into() && frame == 6 && changed {
        data.anim_data = Some(PlayAnimationData {
            name: "idle".into(),
            started_at: tick,
            looped: true,
        });
    } else if current_anim == "roll".into() && frame == 9 && changed {
        data.anim_data = Some(PlayAnimationData {
            name: "idle".into(),
            started_at: tick,
            looped: true,
        });
    } else if current_anim == "guard".into() && frame == 6 && changed {
        data.anim_data = Some(PlayAnimationData {
            name: "idle".into(),
            started_at: tick,
            looped: true,
        });
    }

    (data, action, new_col_world)
}

pub fn animate(anim_data: PlayAnimationData, anim_player: Gd<AnimatedSprite2D>, tick: u64) -> (bool, i32) {
    let data = anim_data;
    let cur_tick = tick;
    let mut anim = anim_player;
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

pub fn action_process(actions: Vec<ActionMessage>, world_data: &mut WorldData, tick: u64) {
    for action in actions {
        match action {
            ActionMessage::Attack(attack) => {
                let target = world_data.players.get_mut(&attack.to).unwrap();
                target.health -= attack.damage;
                if target.health <= 0.0 {
                    target.anim_data = Some(PlayAnimationData {
                        name: "die".into(),
                        started_at: tick,
                        looped: false,
                    });
                } else {
                    target.anim_data = Some(PlayAnimationData {
                        name: "hit".into(),
                        started_at: tick,
                        looped: false,
                    });
                }
            }
        }
    }
}

pub fn simulate_world(mut world_data: WorldData, input_data: HashMap<u64, u8>, tick: u64) -> WorldData {
    let mut actions = Vec::new();

    for (id, player) in world_data.players.iter_mut() {
        let (new_data, action, cols) = simulate_player(player.clone(), &world_data.collision, input_data[id], tick, 1.0 / 60.0);
        *player = new_data;
        world_data.collision = cols;
        if let Some(action) = action {
            actions.push(action);
        }
    }

    action_process(actions, &mut world_data, tick);

    world_data
}

pub fn simulate_world_range(mut world_data: WorldData, input_data: HashMap<u64, HashMap<u64, u8>>, start_tick: u64, end_tick: u64, real_input_tick: u64)
 -> (WorldData, HashMap<u64, WorldData>) {
    let mut snapshot = HashMap::new();
    for tick in start_tick..end_tick {
        if tick > real_input_tick {
            snapshot.insert(tick, world_data.clone());
        }
        for (id, input) in input_data.iter() {
            let player = world_data.players.get_mut(id).unwrap();
            let (new_data, action, cols) = simulate_player(player.clone(), &world_data.collision, input[&tick], tick, 1.0 / 60.0);
            
            world_data.collision = cols;
            world_data.players.insert(*id, new_data);

            if let Some(action) = action {
                action_process(vec![action], &mut world_data, tick);
            }
        }
    }
    (world_data, snapshot)
}

pub fn update_all_player_gd(world_data: &mut WorldData) {
    for (_, player) in world_data.players.iter_mut() {
        player.object.clone().unwrap().set_position(player.pos);
    }
}