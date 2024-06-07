use godot::{builtin::StringName, engine::AnimatedSprite2D, obj::Gd};

use crate::utils::minus;

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
    Guard,
    Attack2,
    Roll,
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
            "guard" => EAnim::Guard,
            "attack2" => EAnim::Attack2,
            "roll" => EAnim::Roll,
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
            EAnim::Guard => "guard".into(),
            EAnim::Attack2 => "attack2".into(),
            EAnim::Roll => "roll".into(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PlayAnimationData {
    pub name: EAnim,
    pub started_at: u64,
    pub looped: bool,
}

impl PlayAnimationData{
    pub fn clac_fream(&self, tick: u64, anim: Gd<AnimatedSprite2D>) -> i32 {
        let delta_tick = minus(tick, self.started_at);
        let frame_speed = 3; // 0.1초
        let frame_max = anim
            .get_sprite_frames()
            .as_mut()
            .unwrap()
            .get_frame_count(self.name.into());

        if self.looped {
            if delta_tick == 0 {
                return 0;
            } else {
                return ((delta_tick / frame_speed) % frame_max as u64) as i32;
            }
        } else {
            return (delta_tick / frame_speed).min(frame_max as u64) as i32;
        }
    }
}
