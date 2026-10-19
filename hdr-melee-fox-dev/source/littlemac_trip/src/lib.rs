#![allow(unused)]
#![allow(non_snake_case)]

//! Little Mac's dash attack becomes a Villager-style trip + throw.
//!
//! Built for stability: no new game objects, no shared files, no cloned articles.
//!
//! - When Mac's dash attack starts, its animation is swapped for his own trip animation
//!   (`slip`), then his get-up animation (`slip_stand`), all still inside the dash attack status.
//! - On frame 9 (when Villager throws his flowerpot) a "thrown" hitbox leaves Mac and travels
//!   forward in an arc for 20 frames, using the flowerpot's numbers: 10% for the first 6 frames,
//!   then 6%, 70 degrees, 60 knockback growth, 65 base knockback. It's one hitbox id, so it can
//!   only hit each opponent once.
//! - Built-in impact effects (always loaded) mark where the projectile is.
//! - Caveat: because it's a hitbox on Mac rather than a separate object, it disappears if Mac
//!   gets hit out of the move, and it can't be reflected or absorbed.
//! - Real trips (bananas, etc.) are unchanged: the `game_slip`/`effect_slip` overrides only act
//!   during the dash attack status and otherwise do exactly what the vanilla scripts do.

use smash::{
    app::{self, lua_bind::*, sv_animcmd::{frame, wait}, *},
    hash40,
    lib::lua_const::*,
    lua2cpp::*,
    phx::*,
};
use smash_script::{macros::*, *};
use smashline::*;

/// Playback speed of the get-up animation after the trip (higher = less endlag).
const STAND_RATE: f32 = 1.5;

/// Frame the projectile is "thrown" (same as Villager's flowerpot).
const THROW_FRAME: f32 = 9.0;
/// How many frames the projectile travels.
const FLIGHT_FRAMES: i32 = 20;
/// Frames of the strong (10%) part before it weakens to 6%.
const STRONG_FRAMES: i32 = 6;

/// Projectile position (relative to Mac, forward = +z) on flight frame `i`.
fn projectile_pos(i: i32) -> (f32, f32) {
    let t = i as f32;
    let z = 10.0 + 2.6 * t;
    let y = (7.0 + 0.9 * t - 0.06 * t * t).max(2.0);
    (y, z)
}

// ---- visible hoodie -----------------------------------------------------------------------------
//
// Mac's own "throwsweat" article (the sweatshirt from his entrance) is spawned at the throw and
// moved along the hitbox's path each frame. It's Mac's own article, so its files are always loaded
// with him (unlike Villager's flowerpot). Its animation is frozen on frame 0 because the
// entrance flight moves it sideways in model space, which would read as toward the camera.

const HOODIE: i32 = 0x2; // FIGHTER_LITTLEMAC_GENERATE_ARTICLE_THROWSWEAT
/// Frames the hoodie stays at the end of its path before it's removed.
const HOODIE_LINGER: i32 = 4;
/// Offset baked into the frozen animation pose (model space: up, forward), subtracted so the
/// hoodie sits on the hitbox.
const HOODIE_POSE_UP: f32 = 7.8;
const HOODIE_POSE_FORWARD: f32 = 6.0;

/// Per player: -1 = not thrown this dash attack, 0.. = flight frame, -2 = finished.
static mut HOODIE_FRAME: [i32; 8] = [-1; 8];

unsafe fn update_hoodie(boma: *mut BattleObjectModuleAccessor, in_dash_attack: bool) {
    let entry = WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize;
    if entry >= 8 {
        return;
    }
    let state = &mut HOODIE_FRAME[entry];
    if !in_dash_attack {
        if *state != -1 {
            if ArticleModule::is_exist(boma, HOODIE) {
                ArticleModule::remove_exist(boma, HOODIE, ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL));
            }
            *state = -1;
        }
        return;
    }
    if *state == -1
        && MotionModule::motion_kind(boma) == hash40("slip")
        && MotionModule::frame(boma) >= THROW_FRAME
    {
        if ArticleModule::is_exist(boma, HOODIE) {
            ArticleModule::remove_exist(boma, HOODIE, ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL));
        }
        ArticleModule::generate_article(boma, HOODIE, false, -1);
        ArticleModule::change_motion(boma, HOODIE, Hash40::new("entry_r"), false, 0.0);
        ArticleModule::set_rate(boma, HOODIE, 0.0);
        *state = 0;
    }
    if *state >= 0 {
        if !ArticleModule::is_exist(boma, HOODIE) {
            *state = -2;
            return;
        }
        let (y, z) = projectile_pos((*state).min(FLIGHT_FRAMES - 1));
        let lr = PostureModule::lr(boma);
        ArticleModule::set_pos(boma, HOODIE, Vector3f {
            x: PostureModule::pos_x(boma) + (z - HOODIE_POSE_FORWARD) * lr,
            y: PostureModule::pos_y(boma) + y - HOODIE_POSE_UP,
            z: PostureModule::pos_z(boma),
        });
        // stay in sync with the hitbox, which pauses during hitlag
        if !StopModule::is_stop(boma) {
            *state += 1;
        }
        if *state > FLIGHT_FRAMES + HOODIE_LINGER {
            ArticleModule::remove_exist(boma, HOODIE, ArticleOperationTarget(*ARTICLE_OPE_TARGET_ALL));
            *state = -2;
        }
    }
}

unsafe extern "C" fn littlemac_trip_frame(fighter: &mut L2CFighterCommon) {
    let boma = fighter.module_accessor;
    let in_dash_attack = StatusModule::status_kind(boma) == *FIGHTER_STATUS_KIND_ATTACK_DASH;
    update_hoodie(boma, in_dash_attack);
    if !in_dash_attack {
        return;
    }
    let motion = MotionModule::motion_kind(boma);
    if motion == hash40("attack_dash") {
        HOODIE_FRAME[(WorkModule::get_int(boma, *FIGHTER_INSTANCE_WORK_ID_INT_ENTRY_ID) as usize).min(7)] = -1;
        MotionModule::change_motion(boma, Hash40::new("slip"), 0.0, 1.0, false, 0.0, false, false);
    } else if motion == hash40("slip") {
        // switch to the get-up just before the trip ends, so the dash attack status
        // doesn't see the animation finish and snap Mac back to standing
        if MotionModule::frame(boma) >= MotionModule::end_frame(boma) - 2.0 {
            MotionModule::change_motion(boma, Hash40::new("slip_stand"), 0.0, STAND_RATE, false, 0.0, false, false);
        }
    }
}

unsafe extern "C" fn game_slip(agent: &mut L2CAgentBase) {
    let lua_state = agent.lua_state_agent;
    let boma = agent.module_accessor;
    if StatusModule::status_kind(boma) == *FIGHTER_STATUS_KIND_ATTACK_DASH {
        frame(lua_state, THROW_FRAME);
        for i in 0..FLIGHT_FRAMES {
            if is_excute(agent) {
                let (y, z) = projectile_pos(i);
                let damage = if i < STRONG_FRAMES { 10.0 } else { 6.0 };
                let size = if i < STRONG_FRAMES { 4.5 } else { 4.0 };
                ATTACK(agent, 0, 0, Hash40::new("top"), damage, 70, 60, 0, 65, size, 0.0, y, z, None, None, None, 1.0, 1.0, *ATTACK_SETOFF_KIND_ON, *ATTACK_LR_CHECK_F, false, 0, 0.0, 0, false, false, false, false, true, *COLLISION_SITUATION_MASK_GA, *COLLISION_CATEGORY_MASK_ALL, *COLLISION_PART_MASK_ALL, false, Hash40::new("collision_attr_normal"), *ATTACK_SOUND_LEVEL_L, *COLLISION_SOUND_ATTR_KICK, *ATTACK_REGION_OBJECT);
            }
            wait(lua_state, 1.0);
        }
        if is_excute(agent) {
            AttackModule::clear_all(boma);
        }
    } else {
        // vanilla game_slip
        frame(lua_state, 30.0);
        if is_excute(agent) {
            WorkModule::on_flag(boma, *FIGHTER_STATUS_WORK_ID_FLAG_RESERVE_CHANGE_STATUS_DLAY_MOTION);
        }
    }
}

unsafe extern "C" fn effect_slip(agent: &mut L2CAgentBase) {
    let lua_state = agent.lua_state_agent;
    let boma = agent.module_accessor;
    if StatusModule::status_kind(boma) == *FIGHTER_STATUS_KIND_ATTACK_DASH {
        frame(lua_state, 8.0);
        if is_excute(agent) {
            FOOT_EFFECT(agent, Hash40::new("sys_turn_smoke"), Hash40::new("top"), 0, 0, 0, 0, 180, 0, 1, 0, 0, 0, 0, 0, 0, false);
        }
        // mark the projectile every other frame along its path
        frame(lua_state, THROW_FRAME);
        for i in 0..FLIGHT_FRAMES {
            if i % 2 == 0 && is_excute(agent) {
                // Placed in world space (Mac's position + the hitbox offset toward where he faces).
                // EFFECT's bone-space axes differ from ATTACK's, which sent the sparks toward the camera.
                let (y, z) = projectile_pos(i);
                let pos = Vector3f {
                    x: PostureModule::pos_x(boma) + z * PostureModule::lr(boma),
                    y: PostureModule::pos_y(boma) + y,
                    z: PostureModule::pos_z(boma),
                };
                let rot = Vector3f { x: 0.0, y: 0.0, z: 0.0 };
                EffectModule::req(boma, Hash40::new("sys_attack_impact"), &pos, &rot, 0.7, 0, 0, false, 0);
            }
            wait(lua_state, 1.0);
        }
    } else {
        // vanilla effect_slip
        frame(lua_state, 8.0);
        if is_excute(agent) {
            FOOT_EFFECT(agent, Hash40::new("sys_turn_smoke"), Hash40::new("top"), 0, 0, 0, 0, 180, 0, 1, 0, 0, 0, 0, 0, 0, false);
        }
        frame(lua_state, 22.0);
        if is_excute(agent) {
            LANDING_EFFECT(agent, Hash40::new("sys_down_smoke"), Hash40::new("top"), 0, 0, 0, 0, 0, 0, 0.6, 0, 0, 0, 0, 0, 0, false);
        }
    }
}

#[skyline::main(name = "littlemac_trip")]
pub fn main() {
    Agent::new("littlemac")
        .acmd("game_slip", game_slip, Priority::High)
        .acmd("effect_slip", effect_slip, Priority::High)
        .on_line(Main, littlemac_trip_frame)
        .install();
}
